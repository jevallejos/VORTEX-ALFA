use std::collections::{BTreeMap, BTreeSet, VecDeque};

use serde::{Deserialize, Serialize};

use crate::domain::error::DomainError;
use crate::domain::types::{Candle, SymbolId};

const DAYS_PER_YEAR: f64 = 365.0;
const BPS_TO_PCT_DIVISOR: f64 = 100.0;

#[derive(Copy, Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SpotPerpCarryParams {
    pub name: &'static str,
    pub window_days: usize,
    pub entry_apr_pct: f64,
    pub exit_apr_pct: f64,
    pub max_pairs: usize,
    pub min_diversification: usize,
    pub min_rebalance_days: usize,
}

impl SpotPerpCarryParams {
    pub fn validate(&self) -> Result<(), DomainError> {
        check(!self.name.is_empty(), "carry_strategy_name")?;
        check(self.window_days >= 1, "window_days")?;
        check(self.max_pairs >= 1, "max_pairs")?;
        check(self.min_diversification >= 1, "min_diversification")?;
        check(self.min_rebalance_days >= 1, "min_rebalance_days")?;
        check(
            self.entry_apr_pct.is_finite() && self.exit_apr_pct.is_finite(),
            "carry_apr_pct",
        )?;
        check(self.entry_apr_pct > self.exit_apr_pct, "carry_hysteresis")
    }
}

const fn carry_param(
    name: &'static str,
    window_days: usize,
    entry_apr_pct: f64,
    exit_apr_pct: f64,
    min_rebalance_days: usize,
) -> SpotPerpCarryParams {
    SpotPerpCarryParams {
        name,
        window_days,
        entry_apr_pct,
        exit_apr_pct,
        max_pairs: 4,
        min_diversification: 2,
        min_rebalance_days,
    }
}

#[must_use]
pub const fn default_carry_params() -> [SpotPerpCarryParams; 3] {
    [
        carry_param("Carry_SpotPerp_10Apr", 7, 10.0, 0.0, 1),
        carry_param("Carry_SpotPerp_15Apr", 7, 15.0, 2.0, 1),
        carry_param("Carry_SpotPerp_TopCarry30d", 30, 8.0, -2.0, 30),
    ]
}

#[derive(Clone, Debug, Default)]
struct SymbolCarryState {
    prev_linear_ts_ns: Option<i64>,
    funding_window: VecDeque<f64>,
    active: bool,
}

#[derive(Copy, Clone, Debug)]
struct CarryCandidate {
    symbol_id: SymbolId,
    apr_pct: f64,
}

/// Causal spot-long / perp-short funding carry engine with rolling APR hysteresis.
#[derive(Clone, Debug)]
pub struct SpotPerpCarryEngine {
    params: SpotPerpCarryParams,
    is_ready: bool,
    days_since_rebalance: Option<usize>,
    symbols: BTreeMap<SymbolId, SymbolCarryState>,
    held_covered_notionals: BTreeMap<SymbolId, f64>,
}

impl SpotPerpCarryEngine {
    pub fn new(params: SpotPerpCarryParams) -> Result<Self, DomainError> {
        params.validate()?;
        Ok(Self {
            params,
            is_ready: false,
            days_since_rebalance: None,
            symbols: BTreeMap::new(),
            held_covered_notionals: BTreeMap::new(),
        })
    }

    #[must_use]
    pub const fn params(&self) -> &SpotPerpCarryParams {
        &self.params
    }

    #[must_use]
    pub const fn is_ready(&self) -> bool {
        self.is_ready
    }

    #[must_use]
    pub fn current_apr_pct(&self, symbol_id: SymbolId) -> Option<f64> {
        let state = self.symbols.get(&symbol_id)?;
        window_apr_pct(&state.funding_window, self.params.window_days)
    }

    pub fn on_slice(
        &mut self,
        linear: &BTreeMap<SymbolId, Candle>,
        spot: &BTreeMap<SymbolId, Candle>,
    ) -> Result<Option<BTreeMap<SymbolId, f64>>, DomainError> {
        for (&symbol_id, candle) in linear {
            if candle.symbol_id != symbol_id || !candle.volume.is_finite() || candle.volume < 0.0 {
                return Err(DomainError::InvalidCandle(format!(
                    "symbol {}",
                    symbol_id.0
                )));
            }
            if !candle.funding_rate_bps.raw().is_finite() {
                return Err(DomainError::NonFiniteBps);
            }
            let ts_ns = candle.open_ts_ns.raw();
            if let Some(prev_ns) = self
                .symbols
                .get(&symbol_id)
                .and_then(|st| st.prev_linear_ts_ns)
            {
                if ts_ns <= prev_ns {
                    return Err(DomainError::NonMonotonicTimestamp {
                        symbol_id: symbol_id.0,
                        prev_ns,
                        curr_ns: ts_ns,
                    });
                }
            }
        }
        for (&symbol_id, candle) in linear {
            let state = self.symbols.entry(symbol_id).or_default();
            if state.funding_window.len() == self.params.window_days {
                state.funding_window.pop_front();
            }
            state
                .funding_window
                .push_back(candle.funding_rate_bps.raw());
            state.prev_linear_ts_ns = Some(candle.open_ts_ns.raw());
        }

        let dual_eligible: BTreeSet<SymbolId> = self
            .symbols
            .iter()
            .filter(|(&sym, st)| {
                linear.contains_key(&sym)
                    && spot.contains_key(&sym)
                    && st.funding_window.len() == self.params.window_days
            })
            .map(|(&sym, _)| sym)
            .collect();

        if !self.is_ready && dual_eligible.is_empty() {
            return Ok(None);
        }
        self.is_ready = true;

        let (next_days, rebalance_now) = match self.days_since_rebalance {
            None => (Some(0), true),
            Some(days) if days + 1 >= self.params.min_rebalance_days => (Some(0), true),
            Some(days) => (Some(days + 1), false),
        };
        self.days_since_rebalance = next_days;

        if rebalance_now {
            let mut candidates = Vec::new();
            for (&symbol_id, state) in &mut self.symbols {
                let apr_opt = dual_eligible
                    .contains(&symbol_id)
                    .then(|| window_apr_pct(&state.funding_window, self.params.window_days))
                    .flatten();
                let Some(apr_pct) = apr_opt else {
                    state.active = false;
                    continue;
                };
                if state.active && apr_pct <= self.params.exit_apr_pct {
                    state.active = false;
                } else if !state.active && apr_pct >= self.params.entry_apr_pct {
                    state.active = true;
                }
                if state.active {
                    candidates.push(CarryCandidate { symbol_id, apr_pct });
                }
            }
            candidates.sort_by(|left, right| {
                right
                    .apr_pct
                    .total_cmp(&left.apr_pct)
                    .then_with(|| left.symbol_id.cmp(&right.symbol_id))
            });
            candidates.truncate(self.params.max_pairs);
            self.held_covered_notionals.clear();
            if !candidates.is_empty() {
                let alloc = 1.0 / (candidates.len().max(self.params.min_diversification) as f64);
                let covered_notional = 0.5 * alloc;
                for candidate in candidates {
                    self.held_covered_notionals
                        .insert(candidate.symbol_id, covered_notional);
                }
            }
        } else {
            self.held_covered_notionals.retain(|symbol_id, _| {
                let both_present = linear.contains_key(symbol_id) && spot.contains_key(symbol_id);
                if !both_present {
                    if let Some(state) = self.symbols.get_mut(symbol_id) {
                        state.active = false;
                    }
                }
                both_present
            });
        }
        Ok(Some(self.held_covered_notionals.clone()))
    }
}

fn window_apr_pct(window: &VecDeque<f64>, window_days: usize) -> Option<f64> {
    if window.len() != window_days || window_days == 0 {
        return None;
    }
    let apr =
        (window.iter().sum::<f64>() / (window_days as f64)) * DAYS_PER_YEAR / BPS_TO_PCT_DIVISOR;
    apr.is_finite().then_some(apr)
}

fn check(ok: bool, name: &'static str) -> Result<(), DomainError> {
    ok.then_some(()).ok_or(DomainError::InvalidStatParam(name))
}
