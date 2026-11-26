use std::collections::btree_map::Entry;
use std::collections::{BTreeMap, VecDeque};

use serde::{Deserialize, Serialize};

use crate::domain::error::DomainError;
use crate::domain::signals::macro_vol::{
    cap_gross_leverage, is_positive_finite, scale_vol_target_weights, SizedCandidate,
};
pub use crate::domain::signals::macro_vol::{
    CausalEwmaVol, TrendVolConfig, CRYPTO_DAYS_PER_YEAR, EWMA_VOL_SPAN_DAYS, MAX_SIGMA_ANN,
    MIN_SIGMA_ANN,
};
use crate::domain::signals::meso_energy::garman_klass_vol;
use crate::domain::types::{Candle, SymbolId};

pub const TREND_HORIZONS: [usize; 3] = [20, 60, 120];
const MAX_HISTORY_BARS: usize = 121;

#[derive(Copy, Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MacroTrendParams {
    pub name: &'static str,
    pub long_only: bool,
    pub target_vol_ann: f64,
    pub max_gross_leverage: f64,
    pub rebalance_band: f64,
}

impl MacroTrendParams {
    pub fn validate(&self) -> Result<(), DomainError> {
        let valid = !self.name.is_empty()
            && is_positive_finite(self.target_vol_ann)
            && is_positive_finite(self.max_gross_leverage)
            && self.rebalance_band.is_finite()
            && self.rebalance_band >= 0.0;
        if !valid {
            return Err(DomainError::InvalidStatParam("macro_trend_params"));
        }
        Ok(())
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SymbolTrendSnapshot {
    pub close: f64,
    pub gk_vol_daily: f64,
    pub sigma_ann: Option<f64>,
    pub trend_signal_ls: Option<f64>,
}

const fn build_trend_param(
    name: &'static str,
    long_only: bool,
    target_vol_ann: f64,
    max_gross_leverage: f64,
) -> MacroTrendParams {
    MacroTrendParams {
        name,
        long_only,
        target_vol_ann,
        max_gross_leverage,
        rebalance_band: 0.02,
    }
}

#[must_use]
pub const fn default_trend_params() -> [MacroTrendParams; 3] {
    [
        build_trend_param("MacroTrend_LS_Vol15", false, 0.15, 1.0),
        build_trend_param("MacroTrend_LS_Vol30", false, 0.30, 1.5),
        build_trend_param("MacroTrend_LO_Vol20", true, 0.20, 1.0),
    ]
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub(crate) struct HighLowBounds {
    pub(crate) highest_high: f64,
    pub(crate) lowest_low: f64,
}

#[derive(Copy, Clone, Debug)]
struct PriceBar {
    high: f64,
    low: f64,
    close: f64,
}

#[derive(Clone, Debug)]
struct SymbolTrendState {
    prev_ts_ns: Option<i64>,
    ewma_vol: CausalEwmaVol,
    bars: VecDeque<PriceBar>,
    last_snapshot: Option<SymbolTrendSnapshot>,
}

impl SymbolTrendState {
    fn new(config: TrendVolConfig) -> Result<Self, DomainError> {
        Ok(Self {
            prev_ts_ns: None,
            ewma_vol: CausalEwmaVol::new(config)?,
            bars: VecDeque::with_capacity(MAX_HISTORY_BARS),
            last_snapshot: None,
        })
    }
}

/// Causal daily feature engine for Garman-Klass range vol, EWMA return vol, and multi-horizon TSMOM.
#[derive(Clone, Debug)]
pub struct MacroTrendFeatureEngine {
    config: TrendVolConfig,
    symbols: BTreeMap<SymbolId, SymbolTrendState>,
}

impl MacroTrendFeatureEngine {
    pub fn new(config: TrendVolConfig) -> Result<Self, DomainError> {
        config.validate()?;
        Ok(Self {
            config,
            symbols: BTreeMap::new(),
        })
    }

    pub fn on_slice(
        &mut self,
        linear_candles: &BTreeMap<SymbolId, Candle>,
    ) -> Result<Vec<SymbolId>, DomainError> {
        for (&symbol_id, candle) in linear_candles {
            if candle.symbol_id != symbol_id || !candle.volume.is_finite() || candle.volume < 0.0 {
                return Err(DomainError::InvalidCandle(symbol_id.0.to_string()));
            }
            if !candle.funding_rate_bps.raw().is_finite() {
                return Err(DomainError::NonFiniteBps);
            }
            let gk_vol_daily = garman_klass_vol(candle)?;
            let curr_ts_ns = candle.open_ts_ns.raw();
            let high = candle.high.as_f64();
            let low = candle.low.as_f64();
            let close = candle.close.as_f64();

            let state = match self.symbols.entry(symbol_id) {
                Entry::Occupied(entry) => entry.into_mut(),
                Entry::Vacant(entry) => entry.insert(SymbolTrendState::new(self.config)?),
            };
            if let Some(prev_ts_ns) = state.prev_ts_ns.filter(|&prev| curr_ts_ns <= prev) {
                return Err(DomainError::NonMonotonicTimestamp {
                    symbol_id: symbol_id.0,
                    prev_ns: prev_ts_ns,
                    curr_ns: curr_ts_ns,
                });
            }

            let sigma_ann = if let Some(prev_bar) = state.bars.back() {
                let log_return = (close / prev_bar.close).ln();
                state.ewma_vol.update(log_return)?
            } else {
                state.ewma_vol.current_sigma_ann()
            };

            if state.bars.len() == MAX_HISTORY_BARS {
                state.bars.pop_front();
            }
            state.bars.push_back(PriceBar { high, low, close });

            let trend_signal_ls = compute_multi_horizon_signal(&state.bars, close)?;
            state.prev_ts_ns = Some(curr_ts_ns);
            state.last_snapshot = Some(SymbolTrendSnapshot {
                close,
                gk_vol_daily,
                sigma_ann,
                trend_signal_ls,
            });
        }
        Ok(linear_candles.keys().copied().collect())
    }

    #[must_use]
    pub fn snapshot(&self, symbol_id: SymbolId) -> Option<SymbolTrendSnapshot> {
        self.symbols
            .get(&symbol_id)
            .and_then(|state| state.last_snapshot)
    }

    pub(crate) fn prior_high_low_bounds(
        &self,
        symbol_id: SymbolId,
        lookback_bars: usize,
    ) -> Option<HighLowBounds> {
        let bars = &self.symbols.get(&symbol_id)?.bars;
        if lookback_bars == 0 || bars.len() <= lookback_bars {
            return None;
        }
        let end_idx = bars.len() - 1;
        let mut bounds = HighLowBounds {
            highest_high: f64::NEG_INFINITY,
            lowest_low: f64::INFINITY,
        };
        for bar in bars.range(end_idx - lookback_bars..end_idx) {
            bounds.highest_high = bounds.highest_high.max(bar.high);
            bounds.lowest_low = bounds.lowest_low.min(bar.low);
        }
        Some(bounds)
    }
}

fn compute_multi_horizon_signal(
    bars: &VecDeque<PriceBar>,
    current_close: f64,
) -> Result<Option<f64>, DomainError> {
    if bars.len() < MAX_HISTORY_BARS {
        return Ok(None);
    }
    let mut signal_sum = 0.0_f64;
    for horizon in TREND_HORIZONS {
        let past_close = bars[MAX_HISTORY_BARS - 1 - horizon].close;
        let log_return = (current_close / past_close).ln();
        if !log_return.is_finite() {
            return Err(DomainError::NonFiniteValue("trend_return"));
        }
        if log_return != 0.0 {
            signal_sum += log_return.signum();
        }
    }
    Ok(Some(signal_sum / (TREND_HORIZONS.len() as f64)))
}

/// Volatility-targeted multi-horizon TSMOM strategy with anti-churn rebalance band.
#[derive(Clone, Debug)]
pub struct VolTargetTrendStrategy {
    params: MacroTrendParams,
    is_ready: bool,
    prev_weights: BTreeMap<SymbolId, f64>,
}

impl VolTargetTrendStrategy {
    pub fn new(params: MacroTrendParams) -> Result<Self, DomainError> {
        params.validate()?;
        Ok(Self {
            params,
            is_ready: false,
            prev_weights: BTreeMap::new(),
        })
    }

    #[must_use]
    pub const fn params(&self) -> &MacroTrendParams {
        &self.params
    }

    pub fn evaluate(
        &mut self,
        active_symbols: &[SymbolId],
        engine: &MacroTrendFeatureEngine,
    ) -> Result<Option<BTreeMap<SymbolId, f64>>, DomainError> {
        let params = self.params;
        let mut eligible = Vec::new();
        for &symbol_id in active_symbols {
            if let Some(snapshot) = engine.snapshot(symbol_id) {
                if let (Some(sigma_ann), Some(signal)) =
                    (snapshot.sigma_ann, snapshot.trend_signal_ls)
                {
                    let direction = if params.long_only {
                        signal.max(0.0)
                    } else {
                        signal
                    };
                    eligible.push(SizedCandidate {
                        symbol_id,
                        sigma_ann,
                        direction,
                    });
                }
            }
        }
        if eligible.is_empty() && !self.is_ready {
            return Ok(None);
        }
        self.is_ready = true;
        let targets =
            scale_vol_target_weights(&eligible, params.target_vol_ann, params.max_gross_leverage)?;
        let mut weights = BTreeMap::new();
        for (symbol_id, target_weight) in targets {
            let prev_weight = self.prev_weights.get(&symbol_id).copied().unwrap_or(0.0);
            let chosen_weight = if (target_weight - prev_weight).abs() >= params.rebalance_band {
                target_weight
            } else {
                prev_weight
            };
            if chosen_weight.abs() > 0.0 {
                weights.insert(symbol_id, chosen_weight);
            }
        }
        cap_gross_leverage(&mut weights, params.max_gross_leverage, 1e-12);
        self.prev_weights = weights.clone();
        Ok(Some(weights))
    }
}
