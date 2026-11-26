use std::collections::{BTreeMap, VecDeque};

use serde::{Deserialize, Serialize};

use crate::domain::error::DomainError;
use crate::domain::types::{Candle, Side, SymbolId, TimestampNs};

#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum MesoHypothesis {
    VolCompressionBreakout,
    TsmomVolumeWeighted,
    FundingCrowdingReversal,
}

impl MesoHypothesis {
    pub const ALL: [Self; 3] = [
        Self::VolCompressionBreakout,
        Self::TsmomVolumeWeighted,
        Self::FundingCrowdingReversal,
    ];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::VolCompressionBreakout => "VolCompressionBreakout",
            Self::TsmomVolumeWeighted => "TsmomVolumeWeighted",
            Self::FundingCrowdingReversal => "FundingCrowdingReversal",
        }
    }
}

/// Garman-Klass range-based bar volatility. Returns exact `0.0` when `O == H == L == C`.
pub fn garman_klass_vol(candle: &Candle) -> Result<f64, DomainError> {
    if candle.open == candle.high && candle.high == candle.low && candle.low == candle.close {
        return Ok(0.0);
    }
    let o = candle.open.as_f64();
    let h = candle.high.as_f64();
    let l = candle.low.as_f64();
    let c = candle.close.as_f64();
    if h < l || o <= 0.0 || l <= 0.0 {
        return Err(DomainError::InvalidCandle(
            "invalid OHLC for Garman-Klass".to_string(),
        ));
    }
    let hl = (h / l).ln();
    let co = (c / o).ln();
    let gk_var = (0.5 * hl * hl - (2.0 * core::f64::consts::LN_2 - 1.0) * co * co).max(0.0);
    let gk = gk_var.sqrt();
    if !gk.is_finite() {
        return Err(DomainError::NonFiniteValue("garman_klass_vol"));
    }
    Ok(gk)
}

#[derive(Clone, Debug)]
pub struct MesoConfig {
    pub ema_fast_span: usize,
    pub ema_slow_span: usize,
    pub r_comp_threshold: f64,
    pub tsmom_window: usize,
    pub vol_ema_span: usize,
    pub vol_ratio_cap: f64,
    pub tsmom_threshold: f64,
    pub funding_warmup: usize,
    pub funding_z_threshold: f64,
}

impl Default for MesoConfig {
    fn default() -> Self {
        Self {
            ema_fast_span: 6,
            ema_slow_span: 48,
            r_comp_threshold: 0.70,
            tsmom_window: 12,
            vol_ema_span: 24,
            vol_ratio_cap: 5.0,
            tsmom_threshold: 0.005,
            funding_warmup: 30,
            funding_z_threshold: 2.0,
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct MesoSignal {
    pub hypothesis: MesoHypothesis,
    pub symbol_id: SymbolId,
    pub ts_ns: TimestampNs,
    pub side: Side,
    pub score: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct MesoBarOutput {
    pub gk_vol: f64,
    pub r_comp: f64,
    pub tsmom_vw: Option<f64>,
    pub z_fund: Option<f64>,
    pub signals: Vec<MesoSignal>,
}

#[derive(Clone, Debug)]
struct SymbolMesoState {
    prev_ts_ns: Option<i64>,
    bar_count: usize,
    ema_fast_gk: f64,
    ema_slow_gk: f64,
    ema_vol_prev: Option<f64>,
    close_history: VecDeque<f64>,
    fund_count: usize,
    fund_mean: f64,
    fund_m2: f64,
}

impl SymbolMesoState {
    fn new(tsmom_window: usize) -> Self {
        Self {
            prev_ts_ns: None,
            bar_count: 0,
            ema_fast_gk: 0.0,
            ema_slow_gk: 0.0,
            ema_vol_prev: None,
            close_history: VecDeque::with_capacity(tsmom_window),
            fund_count: 0,
            fund_mean: 0.0,
            fund_m2: 0.0,
        }
    }
}

/// Causal meso-horizon energy & crowding engine with 100% isolated per-symbol state.
#[derive(Clone, Debug)]
pub struct MesoEnergyEngine {
    config: MesoConfig,
    symbols: BTreeMap<SymbolId, SymbolMesoState>,
}

impl MesoEnergyEngine {
    pub fn new(config: MesoConfig) -> Result<Self, DomainError> {
        if config.ema_fast_span == 0 || config.ema_slow_span == 0 || config.vol_ema_span == 0 {
            return Err(DomainError::InvalidStatParam("meso_ema_span"));
        }
        if config.tsmom_window == 0 {
            return Err(DomainError::InvalidStatParam("tsmom_window"));
        }
        if !config.r_comp_threshold.is_finite() || config.r_comp_threshold <= 0.0 {
            return Err(DomainError::InvalidStatParam("r_comp_threshold"));
        }
        if !config.vol_ratio_cap.is_finite() || config.vol_ratio_cap <= 0.0 {
            return Err(DomainError::InvalidStatParam("vol_ratio_cap"));
        }
        if !config.tsmom_threshold.is_finite() || config.tsmom_threshold <= 0.0 {
            return Err(DomainError::InvalidStatParam("tsmom_threshold"));
        }
        if !config.funding_z_threshold.is_finite() || config.funding_z_threshold <= 0.0 {
            return Err(DomainError::InvalidStatParam("funding_z_threshold"));
        }
        Ok(Self {
            config,
            symbols: BTreeMap::new(),
        })
    }

    #[must_use]
    pub const fn config(&self) -> &MesoConfig {
        &self.config
    }

    pub fn on_candle(&mut self, candle: &Candle) -> Result<MesoBarOutput, DomainError> {
        if !candle.volume.is_finite() || candle.volume < 0.0 {
            return Err(DomainError::InvalidCandle(format!(
                "invalid volume: {}",
                candle.volume
            )));
        }
        let fund = candle.funding_rate_bps.raw();
        if !fund.is_finite() {
            return Err(DomainError::NonFiniteBps);
        }

        let gk_vol = garman_klass_vol(candle)?;
        let ts_ns = candle.open_ts_ns.raw();
        let state = self
            .symbols
            .entry(candle.symbol_id)
            .or_insert_with(|| SymbolMesoState::new(self.config.tsmom_window));

        if let Some(prev_ns) = state.prev_ts_ns {
            if ts_ns <= prev_ns {
                return Err(DomainError::NonMonotonicTimestamp {
                    symbol_id: candle.symbol_id.0,
                    prev_ns,
                    curr_ns: ts_ns,
                });
            }
        }

        // Fast/slow Garman-Klass volatility EMAs & compression ratio.
        let alpha_fast = 2.0 / ((self.config.ema_fast_span as f64) + 1.0);
        let alpha_slow = 2.0 / ((self.config.ema_slow_span as f64) + 1.0);
        if state.bar_count == 0 {
            state.ema_fast_gk = gk_vol;
            state.ema_slow_gk = gk_vol;
        } else {
            state.ema_fast_gk = alpha_fast * gk_vol + (1.0 - alpha_fast) * state.ema_fast_gk;
            state.ema_slow_gk = alpha_slow * gk_vol + (1.0 - alpha_slow) * state.ema_slow_gk;
        }
        let r_comp = if state.ema_slow_gk > 0.0 {
            state.ema_fast_gk / state.ema_slow_gk
        } else {
            1.0
        };

        // Causal volume ratio using strictly PRIOR ema_vol_prev.
        let vol_ratio = match state.ema_vol_prev {
            Some(ev) if ev > 0.0 => (candle.volume / ev).min(self.config.vol_ratio_cap),
            _ => 1.0,
        };
        let open_f = candle.open.as_f64();
        let close_f = candle.close.as_f64();
        let tsmom_vw = if state.close_history.len() >= self.config.tsmom_window {
            state
                .close_history
                .front()
                .map(|&old_close| (close_f / old_close).ln() * vol_ratio)
        } else {
            None
        };

        // Update volume EMA and close history AFTER computing vol_ratio & tsmom_vw.
        let alpha_vol = 2.0 / ((self.config.vol_ema_span as f64) + 1.0);
        state.ema_vol_prev = Some(match state.ema_vol_prev {
            Some(prev) => alpha_vol * candle.volume + (1.0 - alpha_vol) * prev,
            None => candle.volume,
        });
        if state.close_history.len() == self.config.tsmom_window {
            state.close_history.pop_front();
        }
        state.close_history.push_back(close_f);

        // Causal funding z-score using strictly PRIOR Welford mean & std.
        let z_fund = if state.fund_count >= self.config.funding_warmup && state.fund_count >= 2 {
            let var = state.fund_m2 / ((state.fund_count - 1) as f64);
            if var.is_finite() && var > 0.0 {
                Some((fund - state.fund_mean) / var.sqrt())
            } else {
                None
            }
        } else {
            None
        };

        // Update Welford funding moments AFTER computing z_fund.
        state.fund_count += 1;
        let delta = fund - state.fund_mean;
        state.fund_mean += delta / (state.fund_count as f64);
        let delta2 = fund - state.fund_mean;
        state.fund_m2 += delta * delta2;

        let mut signals = Vec::new();

        // 1. VolCompressionBreakout.
        if r_comp <= self.config.r_comp_threshold {
            let breakout = match tsmom_vw {
                Some(tw) if tw.abs() >= self.config.tsmom_threshold => {
                    let side = if tw > 0.0 { Side::Long } else { Side::Short };
                    Some((side, tw.abs()))
                }
                None if candle.close != candle.open => {
                    let bar_ret = (close_f - open_f) / open_f;
                    let side = if bar_ret > 0.0 {
                        Side::Long
                    } else {
                        Side::Short
                    };
                    Some((side, bar_ret.abs()))
                }
                _ => None,
            };
            if let Some((side, score)) = breakout {
                signals.push(MesoSignal {
                    hypothesis: MesoHypothesis::VolCompressionBreakout,
                    symbol_id: candle.symbol_id,
                    ts_ns: candle.open_ts_ns,
                    side,
                    score,
                });
            }
        }

        // 2. TsmomVolumeWeighted.
        if let Some(tw) = tsmom_vw {
            if tw.abs() >= self.config.tsmom_threshold {
                let side = if tw > 0.0 { Side::Long } else { Side::Short };
                signals.push(MesoSignal {
                    hypothesis: MesoHypothesis::TsmomVolumeWeighted,
                    symbol_id: candle.symbol_id,
                    ts_ns: candle.open_ts_ns,
                    side,
                    score: tw.abs(),
                });
            }
        }

        // 3. FundingCrowdingReversal (contrarian to extreme funding).
        if let Some(zf) = z_fund {
            if zf.abs() >= self.config.funding_z_threshold {
                let side = if zf > 0.0 { Side::Short } else { Side::Long };
                signals.push(MesoSignal {
                    hypothesis: MesoHypothesis::FundingCrowdingReversal,
                    symbol_id: candle.symbol_id,
                    ts_ns: candle.open_ts_ns,
                    side,
                    score: zf.abs(),
                });
            }
        }

        state.prev_ts_ns = Some(ts_ns);
        state.bar_count += 1;

        Ok(MesoBarOutput {
            gk_vol,
            r_comp,
            tsmom_vw,
            z_fund,
            signals,
        })
    }
}
