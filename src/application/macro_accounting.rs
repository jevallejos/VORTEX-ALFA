use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::application::macro_baselines::{
    compute_equity_metrics, weight_turnover, MacroDailySlice, MacroFamily, MacroStrategyRow,
};
use crate::domain::error::DomainError;
use crate::domain::fees::FeeSchedule;
use crate::domain::stats::welford_mean;
use crate::domain::types::{Bps, Candle, FundingTimestampSemantics, SymbolId, BPS_PER_UNIT};

pub const SPOT_VIP0_FEE_BPS: f64 = 10.00;

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct CostPerTurnover {
    pub vip0_bps: f64,
    pub maker_diag_bps: f64,
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct MacroFeeProfiles {
    pub perp_vip0_taker: FeeSchedule,
    pub perp_maker_diag: FeeSchedule,
    pub spot_vip0: FeeSchedule,
    pub slippage_bps: f64,
}

impl MacroFeeProfiles {
    pub fn new(slippage_bps: Bps) -> Result<Self, DomainError> {
        let slippage = slippage_bps.raw();
        if !slippage.is_finite() || slippage < 0.0 {
            return Err(DomainError::InvalidFeeBps);
        }
        let perp_vip0_taker = FeeSchedule::bybit_vip0();
        let (maker_bps, taker_bps) = (perp_vip0_taker.maker_bps, perp_vip0_taker.taker_bps);
        let perp_maker_diag = FeeSchedule::custom("VIP0-MakerDiag", maker_bps, taker_bps, false)?;
        let spot_fee_bps = Bps::new(SPOT_VIP0_FEE_BPS)?;
        let spot_vip0 = FeeSchedule::custom("Spot-VIP0", spot_fee_bps, spot_fee_bps, true)?;
        Ok(Self {
            perp_vip0_taker,
            perp_maker_diag,
            spot_vip0,
            slippage_bps: slippage,
        })
    }

    pub fn perp_costs(&self) -> CostPerTurnover {
        CostPerTurnover {
            vip0_bps: self.perp_vip0_taker.taker_bps.raw() + self.slippage_bps,
            maker_diag_bps: self.perp_maker_diag.maker_bps.raw(),
        }
    }

    pub fn spot_costs(&self) -> CostPerTurnover {
        CostPerTurnover {
            vip0_bps: self.spot_vip0.taker_bps.raw() + self.slippage_bps,
            maker_diag_bps: self.spot_vip0.maker_bps.raw(),
        }
    }

    pub fn carry_costs(&self) -> CostPerTurnover {
        let (perp_cost, spot_cost) = (self.perp_costs(), self.spot_costs());
        CostPerTurnover {
            vip0_bps: spot_cost.vip0_bps + perp_cost.vip0_bps,
            maker_diag_bps: spot_cost.maker_diag_bps + perp_cost.maker_diag_bps,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MacroDayRecord {
    pub strategy: String,
    pub family: MacroFamily,
    pub decision_ts_ns: i64,
    pub entry_ts_ns: i64,
    pub exit_ts_ns: i64,
    pub is_eof_close: bool,
    pub weights: BTreeMap<SymbolId, f64>,
    pub price_or_basis_bps: f64,
    pub fund_bps: f64,
    pub turnover: f64,
    pub cost_vip0_bps: f64,
    pub cost_maker_bps: f64,
    pub net_vip0_bps: f64,
    pub net_maker_bps: f64,
}

#[derive(Copy, Clone, Debug)]
pub(crate) struct HeldEntryLeg {
    pub(crate) weight: f64,
    pub(crate) linear_open: f64,
    pub(crate) linear_close: f64,
    pub(crate) spot_open: Option<f64>,
    pub(crate) spot_close: Option<f64>,
    pub(crate) bar_close_funding_bps: f64,
}

#[derive(Clone, Debug)]
pub(crate) struct ActiveTradingDay {
    pub(crate) decision_ts_ns: i64,
    pub(crate) entry_ts_ns: i64,
    pub(crate) legs: BTreeMap<SymbolId, HeldEntryLeg>,
    pub(crate) entry_turnover: f64,
}

#[derive(Clone, Debug)]
pub(crate) struct OpenedDay {
    pub(crate) active_day: ActiveTradingDay,
    pub(crate) opened_weights: BTreeMap<SymbolId, f64>,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct DailyStrategySeries {
    pub(crate) price_or_basis_bps: Vec<f64>,
    pub(crate) fund_bps: Vec<f64>,
    pub(crate) turnover: Vec<f64>,
    pub(crate) cost_vip0_bps: Vec<f64>,
    pub(crate) net_vip0_bps: Vec<f64>,
    pub(crate) net_maker_bps: Vec<f64>,
}

pub(crate) fn open_trading_day(
    family: MacroFamily,
    decision_ts_ns: i64,
    target_weights: &BTreeMap<SymbolId, f64>,
    previous_weights: &BTreeMap<SymbolId, f64>,
    slice: &MacroDailySlice,
    funding_semantics: FundingTimestampSemantics,
) -> OpenedDay {
    let is_carry = family == MacroFamily::SpotPerpCarry;
    let is_bar_close = funding_semantics == FundingTimestampSemantics::BarCloseSettlement;
    let mut opened_weights = BTreeMap::new();
    let mut legs = BTreeMap::new();
    for (&symbol_id, &weight) in target_weights {
        let Some(linear_candle) = slice.linear.get(&symbol_id).filter(|_| weight.abs() > 0.0)
        else {
            continue;
        };
        let spot_candle = is_carry.then(|| slice.spot.get(&symbol_id)).flatten();
        if is_carry && spot_candle.is_none() {
            continue;
        }
        let close_fund = if is_bar_close {
            linear_candle.funding_rate_bps.raw()
        } else {
            0.0
        };
        opened_weights.insert(symbol_id, weight);
        legs.insert(
            symbol_id,
            HeldEntryLeg {
                weight,
                linear_open: linear_candle.open.as_f64(),
                linear_close: linear_candle.close.as_f64(),
                spot_open: spot_candle.map(|candle| candle.open.as_f64()),
                spot_close: spot_candle.map(|candle| candle.close.as_f64()),
                bar_close_funding_bps: close_fund,
            },
        );
    }
    OpenedDay {
        active_day: ActiveTradingDay {
            decision_ts_ns,
            entry_ts_ns: slice.ts_ns.raw(),
            legs,
            entry_turnover: weight_turnover(previous_weights, &opened_weights),
        },
        opened_weights,
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn settle_trading_day(
    strategy_name: &str,
    family: MacroFamily,
    active_day: ActiveTradingDay,
    slice: &MacroDailySlice,
    is_eof_close: bool,
    funding_semantics: FundingTimestampSemantics,
    cost_rates: CostPerTurnover,
    series: &mut DailyStrategySeries,
    records: &mut Vec<MacroDayRecord>,
) -> Result<(), DomainError> {
    let (mut price_or_basis_bps, mut fund_bps, mut close_turnover) = (0.0_f64, 0.0_f64, 0.0_f64);
    let mut held_weights = BTreeMap::new();
    let is_bar_open = funding_semantics == FundingTimestampSemantics::BarOpenSettlement;
    let is_carry = family == MacroFamily::SpotPerpCarry;

    for (&symbol_id, leg) in &active_day.legs {
        held_weights.insert(symbol_id, leg.weight);
        close_turnover += leg.weight.abs();
        let linear_candle = slice.linear.get(&symbol_id);
        let linear_exit = candle_exit_price(linear_candle, leg.linear_close, is_eof_close);
        let perp_return = (linear_exit - leg.linear_open) / leg.linear_open;
        let funding_rate_bps = match (is_bar_open, is_eof_close) {
            (false, _) => leg.bar_close_funding_bps,
            (true, true) => 0.0,
            (true, false) => linear_candle.map_or(0.0, |candle| candle.funding_rate_bps.raw()),
        };
        if is_carry {
            let (Some(spot_open), Some(spot_close)) = (leg.spot_open, leg.spot_close) else {
                return Err(DomainError::InvalidStatParam("carry_spot_leg"));
            };
            let spot_exit = candle_exit_price(slice.spot.get(&symbol_id), spot_close, is_eof_close);
            let spot_return = (spot_exit - spot_open) / spot_open;
            price_or_basis_bps += leg.weight * (spot_return - perp_return) * BPS_PER_UNIT;
            fund_bps += leg.weight * funding_rate_bps;
        } else {
            price_or_basis_bps += leg.weight * perp_return * BPS_PER_UNIT;
            fund_bps -= leg.weight * funding_rate_bps;
        }
    }

    let turnover = active_day.entry_turnover + if is_eof_close { close_turnover } else { 0.0 };
    let cost_vip0_bps = turnover * cost_rates.vip0_bps;
    let cost_maker_bps = turnover * cost_rates.maker_diag_bps;
    let net_vip0_bps = price_or_basis_bps + fund_bps - cost_vip0_bps;
    let net_maker_bps = price_or_basis_bps + fund_bps - cost_maker_bps;
    if !price_or_basis_bps.is_finite()
        || !fund_bps.is_finite()
        || !turnover.is_finite()
        || !net_vip0_bps.is_finite()
        || !net_maker_bps.is_finite()
    {
        return Err(DomainError::NonFiniteValue("macro_day_return"));
    }

    series.price_or_basis_bps.push(price_or_basis_bps);
    series.fund_bps.push(fund_bps);
    series.turnover.push(turnover);
    series.cost_vip0_bps.push(cost_vip0_bps);
    series.net_vip0_bps.push(net_vip0_bps);
    series.net_maker_bps.push(net_maker_bps);
    records.push(MacroDayRecord {
        strategy: strategy_name.to_string(),
        family,
        decision_ts_ns: active_day.decision_ts_ns,
        entry_ts_ns: active_day.entry_ts_ns,
        exit_ts_ns: slice.ts_ns.raw(),
        is_eof_close,
        weights: held_weights,
        price_or_basis_bps,
        fund_bps,
        turnover,
        cost_vip0_bps,
        cost_maker_bps,
        net_vip0_bps,
        net_maker_bps,
    });
    Ok(())
}

pub(crate) fn build_strategy_row(
    strategy: &str,
    family: MacroFamily,
    series: &DailyStrategySeries,
    days_per_year: f64,
) -> Result<MacroStrategyRow, DomainError> {
    Ok(MacroStrategyRow {
        strategy: strategy.to_string(),
        family,
        days_n: series.net_vip0_bps.len(),
        mean_price_or_basis_bps: welford_mean(&series.price_or_basis_bps)?,
        mean_fund_bps: welford_mean(&series.fund_bps)?,
        avg_daily_turnover: welford_mean(&series.turnover)?,
        mean_cost_vip0_bps: welford_mean(&series.cost_vip0_bps)?,
        mean_net_vip0_bps: welford_mean(&series.net_vip0_bps)?,
        mean_net_maker_bps: welford_mean(&series.net_maker_bps)?,
        vip0_metrics: compute_equity_metrics(&series.net_vip0_bps, days_per_year)?,
        maker_metrics: compute_equity_metrics(&series.net_maker_bps, days_per_year)?,
    })
}

fn candle_exit_price(candle: Option<&Candle>, fallback: f64, is_eof: bool) -> f64 {
    match candle {
        Some(bar) if is_eof => bar.close.as_f64(),
        Some(bar) => bar.open.as_f64(),
        None => fallback,
    }
}
