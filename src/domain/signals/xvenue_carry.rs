use crate::domain::error::DomainError;
use crate::domain::signals::carry_uta::payback_apr_hurdle_pct;
use crate::domain::types::{Candle, SymbolId};
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet, VecDeque};

pub const BARS_4H_PER_YEAR: f64 = 2_190.0;
type BarMap = BTreeMap<SymbolId, Candle>;

#[repr(i8)]
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum XVenueDirection {
    LongBybitShortHl = 1,
    LongHlShortBybit = -1,
}

impl XVenueDirection {
    pub const fn sign_f64(self) -> f64 {
        self as i8 as f64
    }

    pub fn from_signed_apr(signed_apr: f64) -> Self {
        if signed_apr >= 0.0 {
            Self::LongBybitShortHl
        } else {
            Self::LongHlShortBybit
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct XVenueStrategySpec {
    pub name: &'static str,
    pub total_notional_l: f64,
    pub recenter_trigger_pct: f64,
    pub window_bars: usize,
    pub payback_days: usize,
    pub check_cadence_bars: usize,
    pub min_hold_bars: usize,
    pub max_pairs: usize,
    pub min_diversification: usize,
    pub entry_apr_pct: f64,
    pub exit_apr_pct: f64,
}

impl XVenueStrategySpec {
    pub fn validate(&self) -> Result<(), DomainError> {
        let (l, r) = (self.total_notional_l, self.recenter_trigger_pct);
        let (ent, ext) = (self.entry_apr_pct, self.exit_apr_pct);
        let ok_sz = self.window_bars > 0 && self.payback_days > 0 && self.check_cadence_bars > 0;
        let ok_k = self.max_pairs > 0 && self.min_diversification > 0;
        let ok_f = l.is_finite() && l > 0.0 && r.is_finite() && r > 0.0;
        let ok_a = ent.is_finite() && ext.is_finite() && ent > ext;
        if self.name.is_empty() || !ok_sz || !ok_k || !ok_f || !ok_a {
            return Err(DomainError::InvalidStatParam("xvenue_strategy_spec"));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Default)]
struct SymbolState {
    prev_bybit_ns: Option<i64>,
    prev_hl_ns: Option<i64>,
    diff_window: VecDeque<f64>,
}
#[derive(Copy, Clone, Debug)]
struct HeldPos {
    dir: XVenueDirection,
    held: usize,
}
#[derive(Copy, Clone, Debug)]
struct Cand {
    sym: SymbolId,
    dir: XVenueDirection,
    apr: f64,
}

#[derive(Clone, Debug)]
pub struct XVenueCarryEngine {
    spec: XVenueStrategySpec,
    payback_hurdle_pct: f64,
    bars_since_check: Option<usize>,
    symbols: BTreeMap<SymbolId, SymbolState>,
    held: BTreeMap<SymbolId, HeldPos>,
    held_weights: BTreeMap<SymbolId, f64>,
}

impl XVenueCarryEngine {
    pub fn new(spec: XVenueStrategySpec, xvenue_cost_vip0_bps: f64) -> Result<Self, DomainError> {
        spec.validate()?;
        let payback_hurdle_pct = payback_apr_hurdle_pct(xvenue_cost_vip0_bps, spec.payback_days)?;
        Ok(Self {
            spec,
            payback_hurdle_pct,
            bars_since_check: None,
            symbols: BTreeMap::new(),
            held: BTreeMap::new(),
            held_weights: BTreeMap::new(),
        })
    }

    pub const fn spec(&self) -> &XVenueStrategySpec {
        &self.spec
    }
    pub const fn is_ready(&self) -> bool {
        self.bars_since_check.is_some()
    }
    pub const fn payback_hurdle_pct(&self) -> f64 {
        self.payback_hurdle_pct
    }
    pub fn current_signed_apr_pct(&self, symbol_id: SymbolId) -> Option<f64> {
        let state = self.symbols.get(&symbol_id)?;
        window_signed_apr_pct(&state.diff_window, self.spec.window_bars)
    }

    pub fn on_slice(
        &mut self,
        bybit: &BarMap,
        hl: &BarMap,
    ) -> Result<Option<BTreeMap<SymbolId, f64>>, DomainError> {
        check_venue(&self.symbols, bybit, |s| s.prev_bybit_ns)?;
        check_venue(&self.symbols, hl, |s| s.prev_hl_ns)?;
        let win = self.spec.window_bars;
        let mut dual_apr = BTreeMap::new();
        for (&sym, byb) in bybit {
            let st = self.symbols.entry(sym).or_default();
            st.prev_bybit_ns = Some(byb.open_ts_ns.raw());
            let Some(hl_bar) = hl.get(&sym) else { continue };
            let diff = hl_bar.funding_rate_bps.raw() - byb.funding_rate_bps.raw();
            if !diff.is_finite() {
                return Err(DomainError::NonFiniteBps);
            }
            if st.diff_window.len() == win {
                st.diff_window.pop_front();
            }
            st.diff_window.push_back(diff);
            if let Some(apr) = window_signed_apr_pct(&st.diff_window, win) {
                dual_apr.insert(sym, apr);
            }
        }
        for (&sym, bar) in hl {
            self.symbols.entry(sym).or_default().prev_hl_ns = Some(bar.open_ts_ns.raw());
        }
        if self.bars_since_check.is_none() && dual_apr.is_empty() {
            return Ok(None);
        }
        self.held.values_mut().for_each(|pos| pos.held += 1);
        let cad = self.spec.check_cadence_bars;
        let next = self.bars_since_check.map_or(0, |b| (b + 1) % cad);
        self.bars_since_check = Some(next);
        if next == 0 {
            self.rebalance(&dual_apr);
        } else {
            let both = |s: &SymbolId| bybit.contains_key(s) && hl.contains_key(s);
            self.held.retain(|s, _| both(s));
            self.held_weights.retain(|s, _| both(s));
        }
        Ok(Some(self.held_weights.clone()))
    }

    fn rebalance(&mut self, dual: &BTreeMap<SymbolId, f64>) {
        let sp = self.spec;
        let flip_hurdle = sp.entry_apr_pct.max(self.payback_hurdle_pct);
        let mut blocked_flip = BTreeSet::new();
        self.held.retain(|&sym, pos| {
            let Some(&apr) = dual.get(&sym) else {
                return false;
            };
            let held_apr = pos.dir.sign_f64() * apr;
            let can_flip = pos.held >= sp.min_hold_bars && (-held_apr) >= flip_hurdle;
            if held_apr <= sp.exit_apr_pct && held_apr < 0.0 && !can_flip {
                blocked_flip.insert(sym);
            }
            held_apr > sp.exit_apr_pct && !(held_apr < 0.0 && can_flip)
        });
        let mut cands = Vec::new();
        for (&sym, &apr) in dual {
            let open = !self.held.contains_key(&sym) && !blocked_flip.contains(&sym);
            if open && apr.abs() >= sp.entry_apr_pct {
                cands.push(cand(sym, XVenueDirection::from_signed_apr(apr), apr.abs()));
            }
        }
        cands.sort_by(best_cmp);
        let free = sp.max_pairs.saturating_sub(self.held.len());
        for &c in cands.iter().take(free) {
            put_held(&mut self.held, c);
        }
        let mut evicted = BTreeSet::new();
        while self.held.len() == sp.max_pairs {
            let inc = self
                .held
                .iter()
                .filter(|(_, p)| p.held >= sp.min_hold_bars)
                .filter_map(|(&s, p)| dual.get(&s).map(|&a| cand(s, p.dir, p.dir.sign_f64() * a)))
                .max_by(best_cmp);
            let chal = cands
                .iter()
                .copied()
                .find(|c| !self.held.contains_key(&c.sym) && !evicted.contains(&c.sym));
            match inc.zip(chal) {
                Some((i, c)) if c.apr - i.apr >= self.payback_hurdle_pct => {
                    self.held.remove(&i.sym);
                    evicted.insert(i.sym);
                    put_held(&mut self.held, c);
                }
                _ => break,
            }
        }
        let min_div = sp.min_diversification.min(sp.max_pairs);
        let leg_l = sp.total_notional_l / (self.held.len().max(min_div) as f64);
        let weights_iter = self
            .held
            .iter()
            .map(|(&s, p)| (s, p.dir.sign_f64() * leg_l));
        self.held_weights = weights_iter.collect();
    }
}
fn cand(sym: SymbolId, dir: XVenueDirection, apr: f64) -> Cand {
    Cand { sym, dir, apr }
}
fn put_held(held: &mut BTreeMap<SymbolId, HeldPos>, item: Cand) {
    let dir = item.dir;
    held.insert(item.sym, HeldPos { dir, held: 0 });
}
fn check_venue(
    syms: &BTreeMap<SymbolId, SymbolState>,
    map: &BarMap,
    prev_fn: fn(&SymbolState) -> Option<i64>,
) -> Result<(), DomainError> {
    for (&sym, bar) in map {
        if bar.symbol_id != sym || !bar.volume.is_finite() || bar.volume < 0.0 {
            return Err(DomainError::InvalidCandle(sym.0.to_string()));
        }
        if !bar.funding_rate_bps.raw().is_finite() {
            return Err(DomainError::NonFiniteBps);
        }
        let curr_ns = bar.open_ts_ns.raw();
        if let Some(prev_ns) = syms.get(&sym).and_then(prev_fn).filter(|&p| curr_ns <= p) {
            return Err(DomainError::NonMonotonicTimestamp {
                symbol_id: sym.0,
                prev_ns,
                curr_ns,
            });
        }
    }
    Ok(())
}
fn best_cmp(l: &Cand, r: &Cand) -> Ordering {
    r.apr.total_cmp(&l.apr).then_with(|| l.sym.cmp(&r.sym))
}
fn window_signed_apr_pct(window: &VecDeque<f64>, window_bars: usize) -> Option<f64> {
    if window_bars == 0 || window.len() != window_bars {
        return None;
    }
    let apr = (window.iter().sum::<f64>() / window_bars as f64) * (BARS_4H_PER_YEAR / 100.0);
    apr.is_finite().then_some(apr)
}
