use std::collections::{BTreeMap, VecDeque};

use serde::{Deserialize, Serialize};

use crate::domain::error::DomainError;
use crate::domain::signals::micro_detectors::{
    CausalQuantileWindow, HawkesProcess, ShiryaevRobertsDetector,
};
use crate::domain::types::{Side, SymbolId, TapeEvent, TapeEventKind, TimestampNs, BPS_PER_UNIT};

#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum MicroHypothesis {
    HawkesTail,
    ShiryaevRobertsJump,
    LiquidationCascade,
    CrossExchangeShock,
}

impl MicroHypothesis {
    pub const ALL: [Self; 4] = [
        Self::HawkesTail,
        Self::ShiryaevRobertsJump,
        Self::LiquidationCascade,
        Self::CrossExchangeShock,
    ];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::HawkesTail => "HawkesTail",
            Self::ShiryaevRobertsJump => "ShiryaevRobertsJump",
            Self::LiquidationCascade => "LiquidationCascade",
            Self::CrossExchangeShock => "CrossExchangeShock",
        }
    }
}

#[derive(Clone, Debug)]
pub struct MicroDetectorConfig {
    pub hawkes_beta: f64,
    pub hawkes_alpha: f64,
    pub hawkes_mu0: f64,
    pub hawkes_intensity_crit: f64,
    pub hawkes_imb_min: f64,
    pub sr_threshold_log_r: f64,
    pub sr_ewma_alpha: f64,
    pub sr_warmup: usize,
    pub liq_window_ns: i64,
    pub liq_pressure_threshold: f64,
    pub cross_tau_s: f64,
    pub cross_shock_bps: f64,
    pub cross_warmup: usize,
    pub tail_quantile: f64,
    pub tail_window_cap: usize,
    pub tail_min_samples: usize,
}

impl Default for MicroDetectorConfig {
    fn default() -> Self {
        Self {
            hawkes_beta: 1.0,
            hawkes_alpha: 0.65,
            hawkes_mu0: 2.0,
            hawkes_intensity_crit: 5.0,
            hawkes_imb_min: 0.25,
            sr_threshold_log_r: 100.0_f64.ln(),
            sr_ewma_alpha: 0.05,
            sr_warmup: 20,
            liq_window_ns: 5_000_000_000,
            liq_pressure_threshold: 1.5,
            cross_tau_s: 60.0,
            cross_shock_bps: 15.0,
            cross_warmup: 5,
            tail_quantile: 0.99,
            tail_window_cap: 500,
            tail_min_samples: 100,
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct MicroSignal {
    pub hypothesis: MicroHypothesis,
    pub symbol_id: SymbolId,
    pub ts_ns: TimestampNs,
    pub side: Side,
    pub score: f64,
}

#[derive(Clone, Debug)]
struct SymbolMicroState {
    prev_ts_ns: Option<i64>,
    hawkes_buy: HawkesProcess,
    hawkes_sell: HawkesProcess,
    tail_window: CausalQuantileWindow,
    sr: ShiryaevRobertsDetector,
    prev_mid: Option<f64>,
    liq_window: VecDeque<(i64, Side, f64)>,
    cross_ewma_bps: f64,
    cross_prev_ts_ns: Option<i64>,
    cross_count: usize,
}

impl SymbolMicroState {
    fn new(cfg: &MicroDetectorConfig) -> Result<Self, DomainError> {
        Ok(Self {
            prev_ts_ns: None,
            hawkes_buy: HawkesProcess::new(cfg.hawkes_beta, cfg.hawkes_alpha, cfg.hawkes_mu0)?,
            hawkes_sell: HawkesProcess::new(cfg.hawkes_beta, cfg.hawkes_alpha, cfg.hawkes_mu0)?,
            tail_window: CausalQuantileWindow::new(cfg.tail_window_cap, cfg.tail_min_samples)?,
            sr: ShiryaevRobertsDetector::new(
                cfg.sr_threshold_log_r,
                cfg.sr_ewma_alpha,
                cfg.sr_warmup,
            )?,
            prev_mid: None,
            liq_window: VecDeque::new(),
            cross_ewma_bps: 0.0,
            cross_prev_ts_ns: None,
            cross_count: 0,
        })
    }
}

/// Causal micro-regime signal engine with 100% isolated per-symbol state.
#[derive(Clone, Debug)]
pub struct MicroTailEngine {
    config: MicroDetectorConfig,
    symbols: BTreeMap<SymbolId, SymbolMicroState>,
}

impl MicroTailEngine {
    pub fn new(config: MicroDetectorConfig) -> Result<Self, DomainError> {
        // Validate detector parameters eagerly via a prototype state.
        let _ = SymbolMicroState::new(&config)?;
        if !config.hawkes_intensity_crit.is_finite() || config.hawkes_intensity_crit < 0.0 {
            return Err(DomainError::InvalidStatParam("hawkes_intensity_crit"));
        }
        if !config.hawkes_imb_min.is_finite() || !(0.0..=1.0).contains(&config.hawkes_imb_min) {
            return Err(DomainError::InvalidStatParam("hawkes_imb_min"));
        }
        if config.liq_window_ns <= 0 {
            return Err(DomainError::InvalidStatParam("liq_window_ns"));
        }
        if !config.liq_pressure_threshold.is_finite() || config.liq_pressure_threshold <= 0.0 {
            return Err(DomainError::InvalidStatParam("liq_pressure_threshold"));
        }
        if !config.cross_tau_s.is_finite() || config.cross_tau_s <= 0.0 {
            return Err(DomainError::InvalidStatParam("cross_tau_s"));
        }
        if !config.cross_shock_bps.is_finite() || config.cross_shock_bps <= 0.0 {
            return Err(DomainError::InvalidStatParam("cross_shock_bps"));
        }
        if !config.tail_quantile.is_finite() || !(0.0..=1.0).contains(&config.tail_quantile) {
            return Err(DomainError::InvalidStatParam("tail_quantile"));
        }
        Ok(Self {
            config,
            symbols: BTreeMap::new(),
        })
    }

    #[must_use]
    pub const fn config(&self) -> &MicroDetectorConfig {
        &self.config
    }

    pub fn on_event(&mut self, event: &TapeEvent) -> Result<Vec<MicroSignal>, DomainError> {
        if !event.l1_to_l5_notional.is_finite() || event.l1_to_l5_notional < 0.0 {
            return Err(DomainError::NonFiniteValue("l1_to_l5_notional"));
        }
        let ts_ns = event.ts_ns.raw();
        if !self.symbols.contains_key(&event.symbol_id) {
            let st = SymbolMicroState::new(&self.config)?;
            self.symbols.insert(event.symbol_id, st);
        }
        let Some(state) = self.symbols.get_mut(&event.symbol_id) else {
            return Ok(Vec::new());
        };

        if let Some(prev_ns) = state.prev_ts_ns {
            if ts_ns < prev_ns {
                return Err(DomainError::NonMonotonicTimestamp {
                    symbol_id: event.symbol_id.0,
                    prev_ns,
                    curr_ns: ts_ns,
                });
            }
        }

        let mut signals = Vec::new();

        // 1. HawkesTail on trades using causal tail quantile + buy/sell intensity imbalance.
        if event.kind == TapeEventKind::Trade {
            let (lambda_buy, lambda_sell) = match event.side {
                Side::Long => {
                    let (_, lb) = state.hawkes_buy.on_event(ts_ns, event.symbol_id)?;
                    let ls = state.hawkes_sell.intensity_at(ts_ns, event.symbol_id)?;
                    (lb, ls)
                }
                Side::Short => {
                    let lb = state.hawkes_buy.intensity_at(ts_ns, event.symbol_id)?;
                    let (_, ls) = state.hawkes_sell.on_event(ts_ns, event.symbol_id)?;
                    (lb, ls)
                }
            };
            let total_lambda = lambda_buy + lambda_sell;
            let imb = if total_lambda > 0.0 {
                (lambda_buy - lambda_sell) / total_lambda
            } else {
                0.0
            };
            let notional = event.event_notional();
            let (is_tail, _) = state
                .tail_window
                .check_and_push(notional, self.config.tail_quantile)?;
            if is_tail
                && total_lambda >= self.config.hawkes_intensity_crit
                && imb.abs() >= self.config.hawkes_imb_min
            {
                let side = if imb > 0.0 { Side::Long } else { Side::Short };
                signals.push(MicroSignal {
                    hypothesis: MicroHypothesis::HawkesTail,
                    symbol_id: event.symbol_id,
                    ts_ns: event.ts_ns,
                    side,
                    score: imb.abs() * total_lambda,
                });
            }
        }

        // 2. ShiryaevRobertsJump on mid-price bps returns.
        let mid = event.book.mid_f64();
        if let Some(prev_mid) = state.prev_mid {
            if prev_mid > 0.0 {
                let dx_bps = ((mid - prev_mid) / prev_mid) * BPS_PER_UNIT;
                let (triggered, log_r) = state.sr.update(dx_bps)?;
                if triggered && dx_bps != 0.0 {
                    let side = if dx_bps > 0.0 {
                        Side::Long
                    } else {
                        Side::Short
                    };
                    signals.push(MicroSignal {
                        hypothesis: MicroHypothesis::ShiryaevRobertsJump,
                        symbol_id: event.symbol_id,
                        ts_ns: event.ts_ns,
                        side,
                        score: log_r,
                    });
                }
            }
        }
        state.prev_mid = Some(mid);

        // 3. LiquidationCascade on 5s rolling liquidation notional / max(l1_to_l5_notional, 1e-6).
        let cutoff = ts_ns.saturating_sub(self.config.liq_window_ns);
        while let Some(&(front_ts, _, _)) = state.liq_window.front() {
            if front_ts < cutoff {
                state.liq_window.pop_front();
            } else {
                break;
            }
        }
        if event.kind == TapeEventKind::Liquidation {
            let liq_notional = event.event_notional();
            state
                .liq_window
                .push_back((ts_ns, event.side, liq_notional));
            let (total_liq, signed_liq) = state
                .liq_window
                .iter()
                .fold((0.0_f64, 0.0_f64), |(tot, sig), &(_, s, n)| {
                    (tot + n, sig + s.sign_f64() * n)
                });
            let pressure = total_liq / event.l1_to_l5_notional.max(1e-6);
            if pressure >= self.config.liq_pressure_threshold && signed_liq != 0.0 {
                let side = if signed_liq > 0.0 {
                    Side::Long
                } else {
                    Side::Short
                };
                signals.push(MicroSignal {
                    hypothesis: MicroHypothesis::LiquidationCascade,
                    symbol_id: event.symbol_id,
                    ts_ns: event.ts_ns,
                    side,
                    score: pressure,
                });
            }
        }

        // 4. CrossExchangeShock ONLY when binance_mid is Some (uses strictly prior EWMA baseline).
        if let Some(bn_mid_px) = event.binance_mid {
            let bn_mid = bn_mid_px.as_f64();
            if mid > 0.0 {
                let basis_bps = ((bn_mid - mid) / mid) * BPS_PER_UNIT;
                if state.cross_count >= self.config.cross_warmup {
                    let shock_bps = basis_bps - state.cross_ewma_bps;
                    if shock_bps.abs() >= self.config.cross_shock_bps {
                        let side = if shock_bps > 0.0 {
                            Side::Long
                        } else {
                            Side::Short
                        };
                        signals.push(MicroSignal {
                            hypothesis: MicroHypothesis::CrossExchangeShock,
                            symbol_id: event.symbol_id,
                            ts_ns: event.ts_ns,
                            side,
                            score: shock_bps.abs(),
                        });
                    }
                }
                if let Some(prev_cross_ts) = state.cross_prev_ts_ns {
                    let dt_s = ((ts_ns - prev_cross_ts) as f64) * 1e-9;
                    let alpha = 1.0 - (-dt_s / self.config.cross_tau_s).exp();
                    state.cross_ewma_bps += alpha * (basis_bps - state.cross_ewma_bps);
                } else {
                    state.cross_ewma_bps = basis_bps;
                }
                state.cross_prev_ts_ns = Some(ts_ns);
                state.cross_count += 1;
            }
        }

        state.prev_ts_ns = Some(ts_ns);
        Ok(signals)
    }
}
