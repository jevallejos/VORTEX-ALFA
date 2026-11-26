use std::collections::{BTreeMap, VecDeque};

use crate::domain::error::DomainError;
use crate::domain::types::SymbolId;

#[derive(Copy, Clone, Debug)]
struct HawkesSymState {
    prev_ts_ns: i64,
    prev_r: f64,
}

/// Self-exciting exponential-kernel Hawkes process with isolated per-symbol state.
#[derive(Clone, Debug)]
pub struct HawkesProcess {
    beta: f64,
    alpha_exc: f64,
    mu_0: f64,
    states: BTreeMap<SymbolId, HawkesSymState>,
}

impl HawkesProcess {
    pub fn new(beta: f64, alpha_exc: f64, mu_0: f64) -> Result<Self, DomainError> {
        if !beta.is_finite() || beta <= 0.0 {
            return Err(DomainError::InvalidStatParam("hawkes_beta"));
        }
        if !alpha_exc.is_finite() || alpha_exc < 0.0 {
            return Err(DomainError::InvalidStatParam("hawkes_alpha_exc"));
        }
        if !mu_0.is_finite() || mu_0 <= 0.0 {
            return Err(DomainError::InvalidStatParam("hawkes_mu_0"));
        }
        Ok(Self {
            beta,
            alpha_exc,
            mu_0,
            states: BTreeMap::new(),
        })
    }

    pub fn on_event(&mut self, ts_ns: i64, symbol_id: SymbolId) -> Result<(f64, f64), DomainError> {
        if ts_ns < 0 {
            return Err(DomainError::InvalidTimestamp(ts_ns));
        }
        match self.states.get_mut(&symbol_id) {
            None => {
                self.states.insert(
                    symbol_id,
                    HawkesSymState {
                        prev_ts_ns: ts_ns,
                        prev_r: 0.0,
                    },
                );
                Ok((0.0, self.mu_0))
            }
            Some(state) => {
                if ts_ns < state.prev_ts_ns {
                    return Err(DomainError::NonMonotonicTimestamp {
                        symbol_id: symbol_id.0,
                        prev_ns: state.prev_ts_ns,
                        curr_ns: ts_ns,
                    });
                }
                let dt_s = ((ts_ns - state.prev_ts_ns) as f64) * 1e-9;
                let r_k = (-self.beta * dt_s).exp() * (1.0 + state.prev_r);
                let lambda = self.mu_0 + self.alpha_exc * r_k;
                state.prev_ts_ns = ts_ns;
                state.prev_r = r_k;
                Ok((r_k, lambda))
            }
        }
    }

    pub fn intensity_at(&self, ts_ns: i64, symbol_id: SymbolId) -> Result<f64, DomainError> {
        if ts_ns < 0 {
            return Err(DomainError::InvalidTimestamp(ts_ns));
        }
        let Some(state) = self.states.get(&symbol_id) else {
            return Ok(self.mu_0);
        };
        if ts_ns < state.prev_ts_ns {
            return Err(DomainError::NonMonotonicTimestamp {
                symbol_id: symbol_id.0,
                prev_ns: state.prev_ts_ns,
                curr_ns: ts_ns,
            });
        }
        let dt_s = ((ts_ns - state.prev_ts_ns) as f64) * 1e-9;
        let r_decayed = (-self.beta * dt_s).exp() * (1.0 + state.prev_r);
        Ok(self.mu_0 + self.alpha_exc * r_decayed)
    }
}

/// Numerically stable `ln(exp(a) + exp(b))` handling `-inf` inputs cleanly.
#[must_use]
pub fn log_add_exp(a: f64, b: f64) -> f64 {
    if a == f64::NEG_INFINITY {
        return b;
    }
    if b == f64::NEG_INFINITY {
        return a;
    }
    let (hi, lo) = if a >= b { (a, b) } else { (b, a) };
    hi + (lo - hi).exp().ln_1p()
}

/// Log-space Shiryaev-Roberts changepoint detector with strictly causal EWMA scale.
#[derive(Clone, Debug)]
pub struct ShiryaevRobertsDetector {
    threshold_log_r: f64,
    ewma_alpha: f64,
    warmup: usize,
    ewma_var: f64,
    log_r: f64,
    count: usize,
}

impl ShiryaevRobertsDetector {
    pub fn new(threshold_log_r: f64, ewma_alpha: f64, warmup: usize) -> Result<Self, DomainError> {
        if !threshold_log_r.is_finite() || threshold_log_r <= 0.0 {
            return Err(DomainError::InvalidStatParam("sr_threshold_log_r"));
        }
        if !ewma_alpha.is_finite() || ewma_alpha <= 0.0 || ewma_alpha > 1.0 {
            return Err(DomainError::InvalidStatParam("sr_ewma_alpha"));
        }
        Ok(Self {
            threshold_log_r,
            ewma_alpha,
            warmup,
            ewma_var: 0.0,
            log_r: f64::NEG_INFINITY,
            count: 0,
        })
    }

    pub fn update(&mut self, x_t: f64) -> Result<(bool, f64), DomainError> {
        if !x_t.is_finite() {
            return Err(DomainError::NonFiniteValue("sr_detector_input"));
        }
        // Baseline scale sigma_0 strictly PRIOR to incorporating x_t.
        let sigma_0 = self.ewma_var.sqrt().max(0.1);
        let x_sq = x_t * x_t;

        if self.count < self.warmup {
            self.update_ewma_var(x_sq);
            self.count += 1;
            return Ok((false, f64::NEG_INFINITY));
        }

        let z = x_t / sigma_0;
        let log_llr = (0.375 * z * z - core::f64::consts::LN_2).clamp(-20.0, 20.0);
        self.log_r = log_add_exp(0.0, self.log_r) + log_llr;

        // Update variance AFTER computing log_llr so current observation cannot mask itself.
        self.update_ewma_var(x_sq);
        self.count += 1;

        if self.log_r >= self.threshold_log_r {
            let triggered_val = self.log_r;
            self.log_r = f64::NEG_INFINITY;
            Ok((true, triggered_val))
        } else {
            Ok((false, self.log_r))
        }
    }

    fn update_ewma_var(&mut self, x_sq: f64) {
        if self.count == 0 {
            self.ewma_var = x_sq;
        } else {
            self.ewma_var = self.ewma_alpha * x_sq + (1.0 - self.ewma_alpha) * self.ewma_var;
        }
    }
}

/// Rolling causal quantile window that evaluates thresholds strictly from prior items
/// BEFORE pushing the current observation.
#[derive(Clone, Debug)]
pub struct CausalQuantileWindow {
    capacity: usize,
    min_samples: usize,
    buf: VecDeque<f64>,
}

impl CausalQuantileWindow {
    pub fn new(capacity: usize, min_samples: usize) -> Result<Self, DomainError> {
        if capacity == 0 {
            return Err(DomainError::InvalidStatParam("quantile_window_capacity"));
        }
        if min_samples == 0 || min_samples > capacity {
            return Err(DomainError::InvalidStatParam("quantile_window_min_samples"));
        }
        Ok(Self {
            capacity,
            min_samples,
            buf: VecDeque::with_capacity(capacity),
        })
    }

    pub fn check_and_push(&mut self, val: f64, q: f64) -> Result<(bool, Option<f64>), DomainError> {
        if !val.is_finite() {
            return Err(DomainError::NonFiniteValue("causal_quantile_val"));
        }
        if !q.is_finite() || !(0.0..=1.0).contains(&q) {
            return Err(DomainError::InvalidStatParam("causal_quantile_q"));
        }

        let outcome = if self.buf.len() >= self.min_samples {
            let mut sorted: Vec<f64> = self.buf.iter().copied().collect();
            sorted.sort_by(f64::total_cmp);
            let pos = q * ((sorted.len() - 1) as f64);
            let lo = pos.floor() as usize;
            let hi = pos.ceil() as usize;
            let frac = pos - (lo as f64);
            let thresh = sorted[lo] + frac * (sorted[hi] - sorted[lo]);
            (val >= thresh, Some(thresh))
        } else {
            (false, None)
        };

        if self.buf.len() == self.capacity {
            self.buf.pop_front();
        }
        self.buf.push_back(val);
        Ok(outcome)
    }
}
