use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet, VecDeque};

use serde::{Deserialize, Serialize};

use crate::domain::error::DomainError;
use crate::domain::types::{Candle, SymbolId};

pub const DAYS_PER_YEAR: f64 = 365.0;
pub const BPS_TO_PCT_DIVISOR: f64 = 100.0;

#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CarrySelectionMode {
    BlindPeriodic,
    CostAmortized,
}

#[derive(Copy, Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CarryUtaStrategySpec {
    pub name: &'static str,
    pub selection_mode: CarrySelectionMode,
    pub coverage_ratio: f64,
    pub recenter_trigger_pct: f64,
    pub window_days: usize,
    pub entry_apr_pct: f64,
    pub exit_apr_pct: f64,
    pub max_pairs: usize,
    pub min_diversification: usize,
    pub check_cadence_days: usize,
    pub payback_days: usize,
    pub min_hold_days: usize,
}

impl CarryUtaStrategySpec {
    pub fn validate(&self) -> Result<(), DomainError> {
        let (cov, rec) = (self.coverage_ratio, self.recenter_trigger_pct);
        let (ent, ext) = (self.entry_apr_pct, self.exit_apr_pct);
        let ok_sz = self.window_days > 0 && self.max_pairs > 0 && self.min_diversification > 0;
        let ok_cad = self.check_cadence_days > 0 && self.payback_days > 0;
        let ok_flt = cov > 0.0 && cov <= 1.0 && rec.is_finite() && rec > 0.0;
        let ok_apr = ent.is_finite() && ext.is_finite() && ent > ext;
        if self.name.is_empty() || !ok_sz || !ok_cad || !ok_flt || !ok_apr {
            return Err(DomainError::InvalidStatParam("carry_uta_strategy_spec"));
        }
        Ok(())
    }
}

pub fn payback_apr_hurdle_pct(
    carry_cost_vip0_bps: f64,
    payback_days: usize,
) -> Result<f64, DomainError> {
    if !carry_cost_vip0_bps.is_finite() || carry_cost_vip0_bps < 0.0 {
        return Err(DomainError::InvalidFeeBps);
    }
    if payback_days == 0 {
        return Err(DomainError::InvalidStatParam("payback_days"));
    }
    let hurdle =
        2.0 * carry_cost_vip0_bps * DAYS_PER_YEAR / (BPS_TO_PCT_DIVISOR * (payback_days as f64));
    if !hurdle.is_finite() {
        return Err(DomainError::NonFiniteValue("payback_apr_hurdle_pct"));
    }
    Ok(hurdle)
}

#[derive(Clone, Debug, Default)]
struct SymbolUtaState {
    prev_linear_ts_ns: Option<i64>,
    funding_window: VecDeque<f64>,
    active: bool,
}

#[derive(Copy, Clone, Debug)]
struct Cand {
    sym: SymbolId,
    apr: f64,
}

#[derive(Clone, Debug)]
pub struct CarryUtaEngine {
    spec: CarryUtaStrategySpec,
    payback_hurdle_pct: f64,
    is_ready: bool,
    days_since_check: Option<usize>,
    symbols: BTreeMap<SymbolId, SymbolUtaState>,
    held_days: BTreeMap<SymbolId, usize>,
    held_covered_notionals: BTreeMap<SymbolId, f64>,
}

impl CarryUtaEngine {
    pub fn new(spec: CarryUtaStrategySpec, carry_cost_vip0_bps: f64) -> Result<Self, DomainError> {
        spec.validate()?;
        let payback_hurdle_pct = payback_apr_hurdle_pct(carry_cost_vip0_bps, spec.payback_days)?;
        Ok(Self {
            spec,
            payback_hurdle_pct,
            is_ready: false,
            days_since_check: None,
            symbols: BTreeMap::new(),
            held_days: BTreeMap::new(),
            held_covered_notionals: BTreeMap::new(),
        })
    }

    pub const fn spec(&self) -> &CarryUtaStrategySpec {
        &self.spec
    }
    pub const fn is_ready(&self) -> bool {
        self.is_ready
    }
    pub const fn payback_hurdle_pct(&self) -> f64 {
        self.payback_hurdle_pct
    }
    pub fn current_apr_pct(&self, symbol_id: SymbolId) -> Option<f64> {
        let state = self.symbols.get(&symbol_id)?;
        window_apr_pct(&state.funding_window, self.spec.window_days)
    }

    pub fn on_slice(
        &mut self,
        linear: &BTreeMap<SymbolId, Candle>,
        spot: &BTreeMap<SymbolId, Candle>,
    ) -> Result<Option<BTreeMap<SymbolId, f64>>, DomainError> {
        spot.iter()
            .try_for_each(|(&sym, bar)| check_candle(sym, bar))?;
        let syms = &self.symbols;
        for (&symbol_id, bar) in linear {
            check_candle(symbol_id, bar)?;
            let (curr_ns, fund) = (bar.open_ts_ns.raw(), bar.funding_rate_bps.raw());
            if !fund.is_finite() {
                return Err(DomainError::NonFiniteBps);
            }
            let prev_opt = syms.get(&symbol_id).and_then(|ent| ent.prev_linear_ts_ns);
            if let Some(prev_ns) = prev_opt.filter(|&prev| curr_ns <= prev) {
                return Err(DomainError::NonMonotonicTimestamp {
                    symbol_id: symbol_id.0,
                    prev_ns,
                    curr_ns,
                });
            }
        }
        let (win_days, mut dual) = (self.spec.window_days, BTreeMap::new());
        for (&sym, bar) in linear {
            let state = self.symbols.entry(sym).or_default();
            if state.funding_window.len() == win_days {
                state.funding_window.pop_front();
            }
            state.funding_window.push_back(bar.funding_rate_bps.raw());
            state.prev_linear_ts_ns = Some(bar.open_ts_ns.raw());
            if spot.contains_key(&sym) {
                if let Some(apr) = window_apr_pct(&state.funding_window, win_days) {
                    dual.insert(sym, apr);
                }
            }
        }
        if !self.is_ready && dual.is_empty() {
            return Ok(None);
        }
        self.is_ready = true;
        self.held_days.values_mut().for_each(|days| *days += 1);
        let next_days = match self.days_since_check {
            Some(days) if days + 1 < self.spec.check_cadence_days => days + 1,
            _ => 0,
        };
        self.days_since_check = Some(next_days);
        if next_days == 0 {
            self.select_holdings(&dual);
            let min_div = self.spec.min_diversification.min(self.spec.max_pairs);
            let cov = self.spec.coverage_ratio / (self.held_days.len().max(min_div) as f64);
            self.held_covered_notionals = self.held_days.keys().map(|&sym| (sym, cov)).collect();
        } else {
            for (&sym, state) in &mut self.symbols {
                let present = linear.contains_key(&sym) && spot.contains_key(&sym);
                if !present && self.held_days.remove(&sym).is_some() {
                    state.active = false;
                    self.held_covered_notionals.remove(&sym);
                }
            }
        }
        Ok(Some(self.held_covered_notionals.clone()))
    }

    fn select_holdings(&mut self, dual: &BTreeMap<SymbolId, f64>) {
        let spec = self.spec;
        if spec.selection_mode == CarrySelectionMode::BlindPeriodic {
            let mut cands = Vec::new();
            for (&sym, state) in &mut self.symbols {
                let apr_opt = dual.get(&sym).copied();
                state.active = apr_opt.is_some_and(|apr| {
                    (state.active && apr > spec.exit_apr_pct) || apr >= spec.entry_apr_pct
                });
                if let Some(apr) = apr_opt.filter(|_| state.active) {
                    cands.push(Cand { sym, apr });
                }
            }
            cands.sort_by(best_cmp);
            cands.truncate(spec.max_pairs);
            let prior = &self.held_days;
            self.held_days = cands
                .into_iter()
                .map(|item| (item.sym, prior.get(&item.sym).copied().unwrap_or(0)))
                .collect();
            return;
        }
        let held = &mut self.held_days;
        held.retain(|sym, _| dual.get(sym).is_some_and(|&apr| apr > spec.exit_apr_pct));
        let mut non_held: Vec<_> = dual
            .iter()
            .filter(|(sym, &apr)| !held.contains_key(sym) && apr >= spec.entry_apr_pct)
            .map(|(&sym, &apr)| Cand { sym, apr })
            .collect();
        non_held.sort_by(best_cmp);
        let free = spec.max_pairs.saturating_sub(held.len());
        for item in non_held.iter().take(free) {
            held.insert(item.sym, 0);
        }
        let mut evicted = BTreeSet::new();
        while held.len() == spec.max_pairs {
            let inc = held
                .iter()
                .filter(|(_, &days)| days >= spec.min_hold_days)
                .filter_map(|(&sym, _)| dual.get(&sym).map(|&apr| Cand { sym, apr }))
                .max_by(best_cmp);
            let chal = non_held
                .iter()
                .copied()
                .find(|item| !held.contains_key(&item.sym) && !evicted.contains(&item.sym));
            match inc.zip(chal) {
                Some((inc_c, chal_c)) if chal_c.apr - inc_c.apr >= self.payback_hurdle_pct => {
                    held.remove(&inc_c.sym);
                    evicted.insert(inc_c.sym);
                    held.insert(chal_c.sym, 0);
                }
                _ => break,
            }
        }
        for (&sym, state) in &mut self.symbols {
            state.active = held.contains_key(&sym);
        }
    }
}

fn check_candle(symbol_id: SymbolId, candle: &Candle) -> Result<(), DomainError> {
    if candle.symbol_id != symbol_id || !candle.volume.is_finite() || candle.volume < 0.0 {
        return Err(DomainError::InvalidCandle(symbol_id.0.to_string()));
    }
    Ok(())
}
fn best_cmp(left: &Cand, right: &Cand) -> Ordering {
    right
        .apr
        .total_cmp(&left.apr)
        .then_with(|| left.sym.cmp(&right.sym))
}
fn window_apr_pct(window: &VecDeque<f64>, window_days: usize) -> Option<f64> {
    let apr = (window.iter().sum::<f64>() / (window_days.max(1) as f64)) * DAYS_PER_YEAR
        / BPS_TO_PCT_DIVISOR;
    (window_days > 0 && window.len() == window_days && apr.is_finite()).then_some(apr)
}
