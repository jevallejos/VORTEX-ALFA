use crate::application::macro_accounting::CostPerTurnover;
use crate::application::macro_baselines::EquityCurveMetrics;
use crate::domain::error::DomainError;
use crate::domain::fees::FeeSchedule;
pub use crate::domain::signals::xvenue_carry::*;
use crate::domain::stats::{FalsificationReport, GateVerdict};
use crate::domain::types::{Bps, Candle, FundingTimestampSemantics, SymbolId, TimestampNs};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const HYPERLIQUID_TIER0_TAKER_BPS: f64 = 4.50;
pub const HYPERLIQUID_TIER0_MAKER_BPS: f64 = 1.50;
pub const DEFAULT_XVENUE_SLIPPAGE_BPS: f64 = 1.5;
pub const DEFAULT_XVENUE_CAPITAL_USDT: f64 = 2_000.0;
pub const DEFAULT_XVENUE_BRIDGE_FEE_USDT: f64 = 1.0;
pub const DEFAULT_VENUE_IMBALANCE_TRIGGER: f64 = 0.15;
pub const CUMULATIVE_HYPOTHESES_M: usize = 116;
macro_rules! xv_spec {
    ($n:literal, $l:expr, $r:expr, [$w:expr, $p:expr, $c:expr, $h:expr, $k:expr], $e:expr) => {
        XVenueStrategySpec {
            name: $n,
            total_notional_l: $l,
            recenter_trigger_pct: $r,
            window_bars: $w,
            payback_days: $p,
            check_cadence_bars: $c,
            min_hold_bars: $h,
            max_pairs: $k,
            min_diversification: 2,
            entry_apr_pct: $e,
            exit_apr_pct: 0.0,
        }
    };
}

pub const fn default_xvenue_grid() -> [XVenueStrategySpec; 9] {
    [
        xv_spec!("Lev1x_Fast7d_Top4", 0.50, 0.60, [18, 7, 6, 18, 4], 15.0),
        xv_spec!("Lev1x_Mid14d_Top4", 0.50, 0.60, [42, 14, 18, 42, 4], 10.0),
        xv_spec!("Lev1x_Slow30d_Top2", 0.50, 0.60, [180, 30, 42, 84, 2], 8.0),
        xv_spec!("Lev2x_Fast7d_Top4", 1.00, 0.30, [18, 7, 6, 18, 4], 15.0),
        xv_spec!("Lev2x_Mid14d_Top4", 1.00, 0.30, [42, 14, 18, 42, 4], 10.0),
        xv_spec!("Lev2x_Slow30d_Top2", 1.00, 0.30, [180, 30, 42, 84, 2], 8.0),
        xv_spec!("Lev3x_Fast7d_Top4", 1.50, 0.18, [18, 7, 6, 18, 4], 15.0),
        xv_spec!("Lev3x_Mid14d_Top4", 1.50, 0.18, [42, 14, 18, 42, 4], 10.0),
        xv_spec!("Lev3x_Slow30d_Top2", 1.50, 0.18, [180, 30, 42, 84, 2], 8.0),
    ]
}

#[derive(Clone, Debug)]
pub struct XVenueBarSlice {
    pub ts_ns: TimestampNs,
    pub bybit: BTreeMap<SymbolId, Candle>,
    pub hl: BTreeMap<SymbolId, Candle>,
}
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct BridgeConfig {
    pub capital_usdt: f64,
    pub bridge_fee_usdt: f64,
    pub imbalance_trigger_frac: f64,
}
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct XVenueFeeProfiles {
    pub bybit_vip0: FeeSchedule,
    pub hl_tier0: FeeSchedule,
    pub binding_schedule: FeeSchedule,
    pub maker_diag_schedule: FeeSchedule,
    pub slippage_bps: f64,
}
impl XVenueFeeProfiles {
    pub fn new(slippage_bps: Bps) -> Result<Self, DomainError> {
        let slip = slippage_bps.raw();
        if !slip.is_finite() || slip < 0.0 {
            return Err(DomainError::InvalidFeeBps);
        }
        let bv = FeeSchedule::bybit_vip0();
        let hm = Bps::new(HYPERLIQUID_TIER0_MAKER_BPS)?;
        let ht = Bps::new(HYPERLIQUID_TIER0_TAKER_BPS)?;
        let hl_tier0 = FeeSchedule::custom("HL-Tier0", hm, ht, true)?;
        let mk = Bps::new(bv.maker_bps.raw() + hm.raw())?;
        let tk = Bps::new(bv.taker_bps.raw() + ht.raw())?;
        let binding_schedule = FeeSchedule::custom("XVenue-VIP0", mk, tk, true)?;
        let maker_diag_schedule = FeeSchedule::custom("XVenue-MakerDiag", mk, tk, false)?;
        Ok(Self {
            bybit_vip0: bv,
            hl_tier0,
            binding_schedule,
            maker_diag_schedule,
            slippage_bps: slip,
        })
    }
    pub fn xvenue_costs(&self) -> CostPerTurnover {
        let taker = self.bybit_vip0.taker_bps.raw() + self.hl_tier0.taker_bps.raw();
        let maker = self.bybit_vip0.maker_bps.raw() + self.hl_tier0.maker_bps.raw();
        CostPerTurnover {
            vip0_bps: taker + 2.0 * self.slippage_bps,
            maker_diag_bps: maker,
        }
    }
}
#[derive(Clone, Debug, PartialEq)]
pub struct XVenueCarryConfig {
    pub strategies: Vec<XVenueStrategySpec>,
    pub slippage_bps: Bps,
    pub capital_usdt: f64,
    pub bridge_fee_usdt: f64,
    pub venue_imbalance_trigger: f64,
    pub funding_semantics: FundingTimestampSemantics,
    pub bars_per_year: f64,
}
impl Default for XVenueCarryConfig {
    fn default() -> Self {
        Self {
            strategies: default_xvenue_grid().to_vec(),
            slippage_bps: Bps::new(DEFAULT_XVENUE_SLIPPAGE_BPS).unwrap_or(Bps::ZERO),
            capital_usdt: DEFAULT_XVENUE_CAPITAL_USDT,
            bridge_fee_usdt: DEFAULT_XVENUE_BRIDGE_FEE_USDT,
            venue_imbalance_trigger: DEFAULT_VENUE_IMBALANCE_TRIGGER,
            funding_semantics: FundingTimestampSemantics::BarOpenSettlement,
            bars_per_year: BARS_4H_PER_YEAR,
        }
    }
}
impl XVenueCarryConfig {
    pub fn validate(&self) -> Result<(), DomainError> {
        if self.strategies.is_empty() {
            return Err(DomainError::InvalidStatParam("xvenue_strategies"));
        }
        self.strategies
            .iter()
            .try_for_each(XVenueStrategySpec::validate)?;
        if !self.slippage_bps.raw().is_finite() || self.slippage_bps.raw() < 0.0 {
            return Err(DomainError::InvalidFeeBps);
        }
        let cap_ok = self.capital_usdt.is_finite() && self.capital_usdt > 0.0;
        let trig = self.venue_imbalance_trigger;
        let trig_ok = trig.is_finite() && trig > 0.0;
        let bpy_ok = self.bars_per_year.is_finite() && self.bars_per_year > 0.0;
        let fee_ok = self.bridge_fee_usdt.is_finite() && self.bridge_fee_usdt >= 0.0;
        if !cap_ok || !trig_ok || !bpy_ok || !fee_ok {
            return Err(DomainError::InvalidStatParam("xvenue_carry_config"));
        }
        Ok(())
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct XVenueBarRecord {
    pub strategy: String,
    pub decision_ts_ns: i64,
    pub entry_ts_ns: i64,
    pub exit_ts_ns: i64,
    pub is_eof_close: bool,
    pub weights: BTreeMap<SymbolId, f64>,
    pub total_gross_l: f64,
    pub basis_bps: f64,
    pub fund_spread_bps: f64,
    pub bridge_bps: f64,
    pub rebalance_turn_l: f64,
    pub recenter_turn_l: f64,
    pub eof_close_turn_l: f64,
    pub total_turn_l: f64,
    pub recenter_events: u32,
    pub bridge_events: u32,
    pub venue_imbalance_after: f64,
    pub cost_vip0_bps: f64,
    pub cost_maker_diag_bps: f64,
    pub net_vip0_bps: f64,
    pub net_maker_diag_bps: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct XVenueCellReport {
    pub strategy: String,
    pub total_notional_l: f64,
    pub recenter_trigger_pct: f64,
    pub bars_n: usize,
    pub n_min_cum116: Option<u64>,
    pub dsr_cum116: Option<f64>,
    pub verdict_cum116: GateVerdict,
    pub mean_basis_bps: Option<f64>,
    pub mean_fund_spread_bps: Option<f64>,
    pub mean_bridge_bps: Option<f64>,
    pub mean_cost_vip0_bps: Option<f64>,
    pub avg_bar_turn_l: Option<f64>,
    pub avg_gross_l: Option<f64>,
    pub avg_util_pct: Option<f64>,
    pub recenter_count: u64,
    pub bridge_count: u64,
    pub mean_net_vip0_bps: Option<f64>,
    pub mean_net_maker_diag_bps: Option<f64>,
    pub vip0_metrics: EquityCurveMetrics,
    pub maker_diag_metrics: EquityCurveMetrics,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct XVenueCarryReport {
    pub slippage_bps: f64,
    pub capital_usdt: f64,
    pub bridge_fee_usdt: f64,
    pub venue_imbalance_trigger: f64,
    pub bars_per_year: f64,
    pub m_cumulative: usize,
    pub t_crit_cum116: f64,
    pub slices_processed: u64,
    pub bybit_candles_processed: u64,
    pub hl_candles_processed: u64,
    pub report: FalsificationReport,
    pub diagnostics: Vec<XVenueCellReport>,
}
