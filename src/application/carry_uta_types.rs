use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::application::macro_baselines::EquityCurveMetrics;
use crate::domain::error::DomainError;
pub use crate::domain::signals::carry_uta::{CarrySelectionMode, CarryUtaStrategySpec};
use crate::domain::stats::FalsificationReport;
use crate::domain::types::{Bps, FundingTimestampSemantics, SymbolId};

pub const DEFAULT_CARRY_UTA_SLIPPAGE_BPS: f64 = 1.5;
pub const DEFAULT_CARRY_UTA_BORROW_APR_PCT: f64 = 15.0;
pub const CRYPTO_DAYS_PER_YEAR: f64 = 365.0;

const fn blind_30d_spec(
    name: &'static str,
    coverage_ratio: f64,
    recenter_trigger_pct: f64,
) -> CarryUtaStrategySpec {
    CarryUtaStrategySpec {
        name,
        selection_mode: CarrySelectionMode::BlindPeriodic,
        coverage_ratio,
        recenter_trigger_pct,
        window_days: 30,
        entry_apr_pct: 8.0,
        exit_apr_pct: -2.0,
        max_pairs: 4,
        min_diversification: 2,
        check_cadence_days: 30,
        payback_days: 30,
        min_hold_days: 0,
    }
}

const fn amort_top4_spec(
    name: &'static str,
    coverage_ratio: f64,
    recenter_trigger_pct: f64,
) -> CarryUtaStrategySpec {
    CarryUtaStrategySpec {
        name,
        selection_mode: CarrySelectionMode::CostAmortized,
        coverage_ratio,
        recenter_trigger_pct,
        window_days: 30,
        entry_apr_pct: 6.0,
        exit_apr_pct: 0.0,
        max_pairs: 4,
        min_diversification: 2,
        check_cadence_days: 7,
        payback_days: 30,
        min_hold_days: 14,
    }
}

const fn amort_top2_spec(
    name: &'static str,
    coverage_ratio: f64,
    recenter_trigger_pct: f64,
) -> CarryUtaStrategySpec {
    CarryUtaStrategySpec {
        name,
        selection_mode: CarrySelectionMode::CostAmortized,
        coverage_ratio,
        recenter_trigger_pct,
        window_days: 30,
        entry_apr_pct: 8.0,
        exit_apr_pct: 0.0,
        max_pairs: 2,
        min_diversification: 2,
        check_cadence_days: 7,
        payback_days: 30,
        min_hold_days: 14,
    }
}

#[must_use]
pub const fn default_carry_uta_grid() -> [CarryUtaStrategySpec; 9] {
    [
        blind_30d_spec("Cash50_Blind30d", 0.50, 0.80),
        amort_top4_spec("Cash50_Amort30d_Top4", 0.50, 0.80),
        amort_top2_spec("Cash50_Amort30d_Top2", 0.50, 0.80),
        blind_30d_spec("Uta75_Blind30d", 0.75, 0.35),
        amort_top4_spec("Uta75_Amort30d_Top4", 0.75, 0.35),
        amort_top2_spec("Uta75_Amort30d_Top2", 0.75, 0.35),
        blind_30d_spec("Uta85_Blind30d", 0.85, 0.20),
        amort_top4_spec("Uta85_Amort30d_Top4", 0.85, 0.20),
        amort_top2_spec("Uta85_Amort30d_Top2", 0.85, 0.20),
    ]
}

#[derive(Clone, Debug, PartialEq)]
pub struct CarryUtaConfig {
    pub strategies: Vec<CarryUtaStrategySpec>,
    pub slippage_bps: Bps,
    pub borrow_apr_pct: f64,
    pub funding_semantics: FundingTimestampSemantics,
    pub days_per_year: f64,
}

impl Default for CarryUtaConfig {
    fn default() -> Self {
        Self {
            strategies: default_carry_uta_grid().to_vec(),
            slippage_bps: Bps::new(DEFAULT_CARRY_UTA_SLIPPAGE_BPS).unwrap_or(Bps::ZERO),
            borrow_apr_pct: DEFAULT_CARRY_UTA_BORROW_APR_PCT,
            funding_semantics: FundingTimestampSemantics::BarOpenSettlement,
            days_per_year: CRYPTO_DAYS_PER_YEAR,
        }
    }
}

impl CarryUtaConfig {
    pub fn validate(&self) -> Result<(), DomainError> {
        if self.strategies.is_empty() {
            return Err(DomainError::InvalidStatParam("carry_uta_strategies"));
        }
        self.strategies
            .iter()
            .try_for_each(CarryUtaStrategySpec::validate)?;
        if !self.slippage_bps.raw().is_finite() || self.slippage_bps.raw() < 0.0 {
            return Err(DomainError::InvalidFeeBps);
        }
        if !self.borrow_apr_pct.is_finite() || self.borrow_apr_pct < 0.0 {
            return Err(DomainError::InvalidStatParam("borrow_apr_pct"));
        }
        if !self.days_per_year.is_finite() || self.days_per_year <= 0.0 {
            return Err(DomainError::InvalidStatParam("days_per_year"));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CarryUtaDayRecord {
    pub strategy: String,
    pub decision_ts_ns: i64,
    pub entry_ts_ns: i64,
    pub exit_ts_ns: i64,
    pub is_eof_close: bool,
    pub weights: BTreeMap<SymbolId, f64>,
    pub total_covered_l: f64,
    pub unrealized_short_loss: f64,
    pub borrowed_usdt_frac: f64,
    pub basis_bps: f64,
    pub fund_bps: f64,
    pub borrow_bps: f64,
    pub rebalance_turn_l: f64,
    pub recenter_turn_l: f64,
    pub eof_close_turn_l: f64,
    pub total_turn_l: f64,
    pub recenter_events: u32,
    pub cost_vip0_bps: f64,
    pub cost_maker_diag_bps: f64,
    pub net_vip0_bps: f64,
    pub net_maker_diag_bps: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CarryUtaCellReport {
    pub strategy: String,
    pub selection_mode: CarrySelectionMode,
    pub coverage_ratio: f64,
    pub recenter_trigger_pct: f64,
    pub days_n: usize,
    pub mean_basis_bps: Option<f64>,
    pub mean_fund_bps: Option<f64>,
    pub mean_borrow_bps: Option<f64>,
    pub mean_cost_vip0_bps: Option<f64>,
    pub avg_daily_turn_l: Option<f64>,
    pub avg_coverage_pct: Option<f64>,
    pub recenter_count: u64,
    pub mean_net_vip0_bps: Option<f64>,
    pub mean_net_maker_diag_bps: Option<f64>,
    pub vip0_metrics: EquityCurveMetrics,
    pub maker_diag_metrics: EquityCurveMetrics,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CarryUtaReport {
    pub slippage_bps: f64,
    pub borrow_apr_pct: f64,
    pub days_per_year: f64,
    pub slices_processed: u64,
    pub linear_candles_processed: u64,
    pub spot_candles_processed: u64,
    pub report: FalsificationReport,
    pub diagnostics: Vec<CarryUtaCellReport>,
}
