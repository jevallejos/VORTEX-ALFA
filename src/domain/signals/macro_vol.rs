use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::domain::error::DomainError;
use crate::domain::types::SymbolId;

pub const CRYPTO_DAYS_PER_YEAR: f64 = 365.0;
pub const EWMA_VOL_SPAN_DAYS: usize = 20;
pub const MIN_SIGMA_ANN: f64 = 0.15;
pub const MAX_SIGMA_ANN: f64 = 3.00;

#[derive(Copy, Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TrendVolConfig {
    pub ewma_span: usize,
    pub min_sigma_ann: f64,
    pub max_sigma_ann: f64,
    pub days_per_year: f64,
}

impl Default for TrendVolConfig {
    fn default() -> Self {
        Self {
            ewma_span: EWMA_VOL_SPAN_DAYS,
            min_sigma_ann: MIN_SIGMA_ANN,
            max_sigma_ann: MAX_SIGMA_ANN,
            days_per_year: CRYPTO_DAYS_PER_YEAR,
        }
    }
}

impl TrendVolConfig {
    pub fn validate(&self) -> Result<(), DomainError> {
        let valid = self.ewma_span >= 2
            && is_positive_finite(self.min_sigma_ann)
            && self.max_sigma_ann.is_finite()
            && self.max_sigma_ann >= self.min_sigma_ann
            && is_positive_finite(self.days_per_year);
        if !valid {
            return Err(DomainError::InvalidStatParam("trend_vol_config"));
        }
        Ok(())
    }
}

/// Causal EWMA annualized volatility estimator seeded by sample variance over `ewma_span` returns.
#[derive(Clone, Debug)]
pub struct CausalEwmaVol {
    config: TrendVolConfig,
    alpha: f64,
    init_returns: Vec<f64>,
    ewma_var: Option<f64>,
}

impl CausalEwmaVol {
    pub fn new(config: TrendVolConfig) -> Result<Self, DomainError> {
        config.validate()?;
        let alpha = 2.0 / ((config.ewma_span as f64) + 1.0);
        Ok(Self {
            config,
            alpha,
            init_returns: Vec::with_capacity(config.ewma_span),
            ewma_var: None,
        })
    }

    pub fn update(&mut self, log_return: f64) -> Result<Option<f64>, DomainError> {
        if !log_return.is_finite() {
            return Err(DomainError::NonFiniteValue("macro_log_return"));
        }
        if let Some(prev_var) = self.ewma_var {
            self.ewma_var =
                Some((1.0 - self.alpha) * prev_var + self.alpha * log_return * log_return);
        } else {
            self.init_returns.push(log_return);
            if self.init_returns.len() == self.config.ewma_span {
                let count = self.config.ewma_span as f64;
                let mean = self.init_returns.iter().sum::<f64>() / count;
                let mut sum_sq = 0.0_f64;
                for &ret_val in &self.init_returns {
                    let diff = ret_val - mean;
                    sum_sq += diff * diff;
                }
                self.ewma_var = Some(sum_sq / ((self.config.ewma_span - 1) as f64));
                self.init_returns.clear();
            }
        }
        if self.ewma_var.is_some_and(|var| !var.is_finite()) {
            return Err(DomainError::NonFiniteValue("macro_ewma_var"));
        }
        Ok(self.current_sigma_ann())
    }

    #[must_use]
    pub fn current_sigma_ann(&self) -> Option<f64> {
        self.ewma_var.map(|var| {
            let raw_sigma = (var * self.config.days_per_year).sqrt();
            raw_sigma.clamp(self.config.min_sigma_ann, self.config.max_sigma_ann)
        })
    }

    #[must_use]
    pub const fn current_variance(&self) -> Option<f64> {
        self.ewma_var
    }
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub(crate) struct SizedCandidate {
    pub(crate) symbol_id: SymbolId,
    pub(crate) sigma_ann: f64,
    pub(crate) direction: f64,
}

pub(crate) fn scale_vol_target_weights(
    candidates: &[SizedCandidate],
    target_vol_ann: f64,
    max_gross_leverage: f64,
) -> Result<BTreeMap<SymbolId, f64>, DomainError> {
    let denominator = candidates.len().max(1) as f64;
    let mut weights = BTreeMap::new();
    for candidate in candidates {
        let raw_weight =
            (candidate.direction / denominator) * (target_vol_ann / candidate.sigma_ann);
        if !raw_weight.is_finite() {
            return Err(DomainError::NonFiniteValue("macro_raw_weight"));
        }
        weights.insert(candidate.symbol_id, raw_weight);
    }
    cap_gross_leverage(&mut weights, max_gross_leverage, 0.0);
    Ok(weights)
}

pub(crate) fn cap_gross_leverage(
    weights: &mut BTreeMap<SymbolId, f64>,
    max_gross_leverage: f64,
    tolerance: f64,
) {
    let gross_leverage: f64 = weights.values().map(|weight| weight.abs()).sum();
    if gross_leverage > max_gross_leverage + tolerance && gross_leverage > 0.0 {
        let scale = max_gross_leverage / gross_leverage;
        for weight in weights.values_mut() {
            *weight *= scale;
        }
    }
}

#[must_use]
pub(crate) fn is_positive_finite(value: f64) -> bool {
    value.is_finite() && value > 0.0
}
