use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::domain::error::DomainError;
use crate::domain::signals::macro_trend::MacroTrendFeatureEngine;
use crate::domain::signals::macro_vol::{
    is_positive_finite, scale_vol_target_weights, SizedCandidate,
};
use crate::domain::types::SymbolId;

#[derive(Copy, Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DonchianParams {
    pub name: &'static str,
    pub entry_n: usize,
    pub exit_n: usize,
    pub long_only: bool,
    pub target_vol_ann: f64,
    pub max_gross_leverage: f64,
}

impl DonchianParams {
    pub fn validate(&self) -> Result<(), DomainError> {
        let valid = !self.name.is_empty()
            && is_positive_finite(self.target_vol_ann)
            && is_positive_finite(self.max_gross_leverage)
            && self.entry_n >= 1
            && self.exit_n >= 1
            && self.exit_n <= self.entry_n;
        if !valid {
            return Err(DomainError::InvalidStatParam("donchian_params"));
        }
        Ok(())
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DonchianChannels {
    pub upper_in: f64,
    pub lower_in: f64,
    pub upper_out: f64,
    pub lower_out: f64,
}

const fn donchian_spec(
    name: &'static str,
    entry_n: usize,
    exit_n: usize,
    long_only: bool,
) -> DonchianParams {
    DonchianParams {
        name,
        entry_n,
        exit_n,
        long_only,
        target_vol_ann: 0.20,
        max_gross_leverage: 1.0,
    }
}

#[must_use]
pub const fn default_donchian_params() -> [DonchianParams; 3] {
    [
        donchian_spec("Donchian_20_10_LS", 20, 10, false),
        donchian_spec("Donchian_55_20_LS", 55, 20, false),
        donchian_spec("Donchian_55_20_LO", 55, 20, true),
    ]
}

#[must_use]
pub fn prior_donchian_channel(
    engine: &MacroTrendFeatureEngine,
    symbol_id: SymbolId,
    entry_n: usize,
    exit_n: usize,
) -> Option<DonchianChannels> {
    if entry_n == 0 || exit_n == 0 || exit_n > entry_n {
        return None;
    }
    let entry_bounds = engine.prior_high_low_bounds(symbol_id, entry_n)?;
    let exit_bounds = engine.prior_high_low_bounds(symbol_id, exit_n)?;
    Some(DonchianChannels {
        upper_in: entry_bounds.highest_high,
        lower_in: entry_bounds.lowest_low,
        upper_out: exit_bounds.highest_high,
        lower_out: exit_bounds.lowest_low,
    })
}

#[derive(Copy, Clone, Debug)]
struct DonchianCandidate {
    symbol_id: SymbolId,
    close: f64,
    sigma_ann: f64,
    channel: DonchianChannels,
}

/// Causal turtle-style Donchian breakout strategy with asymmetric exit channels and vol sizing.
#[derive(Clone, Debug)]
pub struct DonchianBreakoutStrategy {
    params: DonchianParams,
    is_ready: bool,
    positions: BTreeMap<SymbolId, i8>,
}

impl DonchianBreakoutStrategy {
    pub fn new(params: DonchianParams) -> Result<Self, DomainError> {
        params.validate()?;
        Ok(Self {
            params,
            is_ready: false,
            positions: BTreeMap::new(),
        })
    }

    #[must_use]
    pub const fn params(&self) -> &DonchianParams {
        &self.params
    }

    #[must_use]
    pub fn position(&self, symbol_id: SymbolId) -> i8 {
        self.positions.get(&symbol_id).copied().unwrap_or(0)
    }

    pub fn evaluate(
        &mut self,
        active_symbols: &[SymbolId],
        engine: &MacroTrendFeatureEngine,
    ) -> Result<Option<BTreeMap<SymbolId, f64>>, DomainError> {
        let active_set: BTreeSet<SymbolId> = active_symbols.iter().copied().collect();
        self.positions
            .retain(|symbol_id, _| active_set.contains(symbol_id));

        let params = self.params;
        let mut eligible = Vec::new();
        for &symbol_id in active_symbols {
            let Some(snapshot) = engine.snapshot(symbol_id) else {
                continue;
            };
            let Some(sigma_ann) = snapshot.sigma_ann else {
                continue;
            };
            if let Some(channel) =
                prior_donchian_channel(engine, symbol_id, params.entry_n, params.exit_n)
            {
                eligible.push(DonchianCandidate {
                    symbol_id,
                    close: snapshot.close,
                    sigma_ann,
                    channel,
                });
            }
        }
        if eligible.is_empty() && !self.is_ready {
            return Ok(None);
        }
        self.is_ready = true;

        let mut sized_candidates = Vec::with_capacity(eligible.len());
        for candidate in eligible {
            let mut next_position = self.position(candidate.symbol_id);
            if candidate.close > candidate.channel.upper_in {
                next_position = 1;
            } else if !params.long_only && candidate.close < candidate.channel.lower_in {
                next_position = -1;
            } else if (next_position == 1 && candidate.close < candidate.channel.lower_out)
                || (next_position == -1 && candidate.close > candidate.channel.upper_out)
            {
                next_position = 0;
            }
            if next_position == 0 {
                self.positions.remove(&candidate.symbol_id);
            } else {
                self.positions.insert(candidate.symbol_id, next_position);
            }
            sized_candidates.push(SizedCandidate {
                symbol_id: candidate.symbol_id,
                sigma_ann: candidate.sigma_ann,
                direction: f64::from(next_position),
            });
        }

        let mut weights = scale_vol_target_weights(
            &sized_candidates,
            params.target_vol_ann,
            params.max_gross_leverage,
        )?;
        weights.retain(|_, weight| weight.abs() > 0.0);
        Ok(Some(weights))
    }
}
