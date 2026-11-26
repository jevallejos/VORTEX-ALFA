use std::collections::BTreeMap;

use crate::application::carry_uta_types::{
    CarryUtaCellReport, CarryUtaDayRecord, CarryUtaStrategySpec,
};
use crate::application::macro_accounting::CostPerTurnover;
use crate::application::macro_baselines::{
    compute_equity_metrics, weight_turnover, MacroDailySlice,
};
use crate::domain::error::DomainError;
use crate::domain::stats::welford_mean;
use crate::domain::types::{FundingTimestampSemantics, SymbolId, BPS_PER_UNIT};

const PCT_SCALE: f64 = 100.0;

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct UtaOpenPosition {
    pub covered_l: f64,
    pub entry_perp_px: f64,
    pub entry_spot_px: f64,
}

#[derive(Copy, Clone, Debug)]
pub struct HeldCarryUtaLeg {
    pub covered_l: f64,
    pub linear_open: f64,
    pub linear_close: f64,
    pub spot_open: f64,
    pub spot_close: f64,
    pub bar_close_funding_bps: f64,
}

#[derive(Clone, Debug)]
pub struct ActiveCarryUtaDay {
    pub decision_ts_ns: i64,
    pub entry_ts_ns: i64,
    pub legs: BTreeMap<SymbolId, HeldCarryUtaLeg>,
    pub total_covered_l: f64,
    pub unrealized_short_loss: f64,
    pub borrowed_usdt_frac: f64,
    pub borrow_bps: f64,
    pub rebalance_turn_l: f64,
    pub recenter_turn_l: f64,
    pub recenter_events: u32,
}

#[derive(Clone, Debug)]
pub struct OpenedCarryUtaDay {
    pub active_day: ActiveCarryUtaDay,
    pub opened_weights: BTreeMap<SymbolId, f64>,
}

#[derive(Clone, Debug, Default)]
pub struct DailyCarryUtaSeries {
    pub basis_bps: Vec<f64>,
    pub fund_bps: Vec<f64>,
    pub borrow_bps: Vec<f64>,
    pub cost_vip0_bps: Vec<f64>,
    pub total_turn_l: Vec<f64>,
    pub coverage_pct: Vec<f64>,
    pub recenter_count: u64,
    pub net_vip0_bps: Vec<f64>,
    pub net_maker_diag_bps: Vec<f64>,
}

#[allow(clippy::too_many_arguments)]
pub fn open_carry_uta_day(
    decision_ts_ns: i64,
    target_weights: &BTreeMap<SymbolId, f64>,
    previous_weights: &BTreeMap<SymbolId, f64>,
    positions: &mut BTreeMap<SymbolId, UtaOpenPosition>,
    slice: &MacroDailySlice,
    recenter_trigger_pct: f64,
    borrow_apr_pct: f64,
    days_per_year: f64,
    funding_semantics: FundingTimestampSemantics,
) -> Result<OpenedCarryUtaDay, DomainError> {
    if !(recenter_trigger_pct > 0.0 && recenter_trigger_pct.is_finite())
        || !(borrow_apr_pct >= 0.0 && borrow_apr_pct.is_finite())
        || !(days_per_year > 0.0 && days_per_year.is_finite())
    {
        return Err(DomainError::InvalidStatParam("carry_uta_open_params"));
    }
    if target_weights.values().any(|weight| !weight.is_finite()) {
        return Err(DomainError::NonFiniteValue("carry_uta_weight"));
    }
    let mut opened_weights = BTreeMap::new();
    let (mut total_covered_l, mut unrealized_short_loss) = (0.0_f64, 0.0_f64);
    let (mut recenter_turn_l, mut recenter_events) = (0.0_f64, 0_u32);
    let mut legs = BTreeMap::new();
    let is_bar_close = funding_semantics == FundingTimestampSemantics::BarCloseSettlement;
    for (&symbol_id, &new_l) in target_weights {
        let (Some(lin), Some(spot)) = (slice.linear.get(&symbol_id), slice.spot.get(&symbol_id))
        else {
            continue;
        };
        if new_l <= 0.0 {
            continue;
        }
        opened_weights.insert(symbol_id, new_l);
        let (lin_open, spot_open) = (lin.open.as_f64(), spot.open.as_f64());
        let pos = positions.entry(symbol_id).or_insert(UtaOpenPosition {
            covered_l: 0.0,
            entry_perp_px: lin_open,
            entry_spot_px: spot_open,
        });
        if pos.covered_l > 0.0 && new_l > pos.covered_l {
            let add = new_l - pos.covered_l;
            pos.entry_perp_px = new_l / (pos.covered_l / pos.entry_perp_px + add / lin_open);
            pos.entry_spot_px = new_l / (pos.covered_l / pos.entry_spot_px + add / spot_open);
        }
        pos.covered_l = new_l;
        total_covered_l += pos.covered_l;
        let perp_move = (lin_open - pos.entry_perp_px) / pos.entry_perp_px;
        unrealized_short_loss += pos.covered_l * perp_move.max(0.0);
        if perp_move >= recenter_trigger_pct {
            recenter_turn_l += 2.0 * pos.covered_l;
            recenter_events += 1;
            pos.entry_perp_px = lin_open;
            pos.entry_spot_px = spot_open;
        }
        let fund_close = if is_bar_close {
            lin.funding_rate_bps.raw()
        } else {
            0.0
        };
        legs.insert(
            symbol_id,
            HeldCarryUtaLeg {
                covered_l: pos.covered_l,
                linear_open: lin_open,
                linear_close: lin.close.as_f64(),
                spot_open,
                spot_close: spot.close.as_f64(),
                bar_close_funding_bps: fund_close,
            },
        );
    }
    positions.retain(|symbol_id, _| opened_weights.contains_key(symbol_id));
    let rebalance_turn_l = weight_turnover(previous_weights, &opened_weights);
    let borrowed_usdt_frac = (unrealized_short_loss - (1.0 - total_covered_l).max(0.0)).max(0.0);
    let borrow_bps = borrowed_usdt_frac * (borrow_apr_pct * PCT_SCALE / days_per_year);
    let margin_ok = total_covered_l.is_finite() && unrealized_short_loss.is_finite();
    let rate_ok = borrowed_usdt_frac.is_finite() && borrow_bps.is_finite();
    let turn_ok = rebalance_turn_l.is_finite() && recenter_turn_l.is_finite();
    if !margin_ok || !rate_ok || !turn_ok {
        return Err(DomainError::NonFiniteValue("carry_uta_open_day"));
    }
    Ok(OpenedCarryUtaDay {
        active_day: ActiveCarryUtaDay {
            decision_ts_ns,
            entry_ts_ns: slice.ts_ns.raw(),
            legs,
            total_covered_l,
            unrealized_short_loss,
            borrowed_usdt_frac,
            borrow_bps,
            rebalance_turn_l,
            recenter_turn_l,
            recenter_events,
        },
        opened_weights,
    })
}

#[allow(clippy::too_many_arguments)]
pub fn settle_carry_uta_day(
    strategy_name: &str,
    active_day: ActiveCarryUtaDay,
    slice: &MacroDailySlice,
    is_eof_close: bool,
    funding_semantics: FundingTimestampSemantics,
    cost_rates: CostPerTurnover,
    series: &mut DailyCarryUtaSeries,
    records: &mut Vec<CarryUtaDayRecord>,
) -> Result<(), DomainError> {
    let (mut basis_bps, mut fund_bps, mut close_turnover) = (0.0_f64, 0.0_f64, 0.0_f64);
    let mut held_weights = BTreeMap::new();
    let is_bar_open = funding_semantics == FundingTimestampSemantics::BarOpenSettlement;
    for (&symbol_id, leg) in &active_day.legs {
        held_weights.insert(symbol_id, leg.covered_l);
        close_turnover += leg.covered_l.abs();
        let linear_candle = slice.linear.get(&symbol_id);
        let linear_exit = linear_candle.map_or(leg.linear_close, |bar| {
            if is_eof_close { bar.close } else { bar.open }.as_f64()
        });
        let spot_exit = slice.spot.get(&symbol_id).map_or(leg.spot_close, |bar| {
            if is_eof_close { bar.close } else { bar.open }.as_f64()
        });
        let perp_return = (linear_exit - leg.linear_open) / leg.linear_open;
        let spot_return = (spot_exit - leg.spot_open) / leg.spot_open;
        let funding_bps = match (is_bar_open, is_eof_close) {
            (false, _) => leg.bar_close_funding_bps,
            (true, true) => 0.0,
            (true, false) => linear_candle.map_or(0.0, |candle| candle.funding_rate_bps.raw()),
        };
        basis_bps += leg.covered_l * (spot_return - perp_return) * BPS_PER_UNIT;
        fund_bps += leg.covered_l * funding_bps;
    }
    let eof_close_turn_l = if is_eof_close { close_turnover } else { 0.0 };
    let total_turn_l = active_day.rebalance_turn_l + active_day.recenter_turn_l + eof_close_turn_l;
    let cost_vip0_bps = total_turn_l * cost_rates.vip0_bps;
    let cost_maker_diag_bps = total_turn_l * cost_rates.maker_diag_bps;
    let net_vip0_bps = basis_bps + fund_bps - active_day.borrow_bps - cost_vip0_bps;
    let net_maker_diag_bps = basis_bps + fund_bps - active_day.borrow_bps - cost_maker_diag_bps;
    let core_ok =
        basis_bps.is_finite() && fund_bps.is_finite() && active_day.borrow_bps.is_finite();
    let turn_ok = total_turn_l.is_finite() && active_day.total_covered_l.is_finite();
    let cost_ok = cost_vip0_bps.is_finite() && cost_maker_diag_bps.is_finite();
    let net_ok = net_vip0_bps.is_finite() && net_maker_diag_bps.is_finite();
    if !core_ok || !turn_ok || !cost_ok || !net_ok {
        return Err(DomainError::NonFiniteValue("carry_uta_day_return"));
    }
    let coverage_pct = active_day.total_covered_l * PCT_SCALE;
    series.basis_bps.push(basis_bps);
    series.fund_bps.push(fund_bps);
    series.borrow_bps.push(active_day.borrow_bps);
    series.cost_vip0_bps.push(cost_vip0_bps);
    series.total_turn_l.push(total_turn_l);
    series.coverage_pct.push(coverage_pct);
    series.recenter_count += u64::from(active_day.recenter_events);
    series.net_vip0_bps.push(net_vip0_bps);
    series.net_maker_diag_bps.push(net_maker_diag_bps);
    records.push(CarryUtaDayRecord {
        strategy: strategy_name.to_string(),
        decision_ts_ns: active_day.decision_ts_ns,
        entry_ts_ns: active_day.entry_ts_ns,
        exit_ts_ns: slice.ts_ns.raw(),
        is_eof_close,
        weights: held_weights,
        total_covered_l: active_day.total_covered_l,
        unrealized_short_loss: active_day.unrealized_short_loss,
        borrowed_usdt_frac: active_day.borrowed_usdt_frac,
        basis_bps,
        fund_bps,
        borrow_bps: active_day.borrow_bps,
        rebalance_turn_l: active_day.rebalance_turn_l,
        recenter_turn_l: active_day.recenter_turn_l,
        eof_close_turn_l,
        total_turn_l,
        recenter_events: active_day.recenter_events,
        cost_vip0_bps,
        cost_maker_diag_bps,
        net_vip0_bps,
        net_maker_diag_bps,
    });
    Ok(())
}

pub fn build_carry_uta_cell_report(
    spec: &CarryUtaStrategySpec,
    series: &DailyCarryUtaSeries,
    days_per_year: f64,
) -> Result<CarryUtaCellReport, DomainError> {
    Ok(CarryUtaCellReport {
        strategy: spec.name.to_string(),
        selection_mode: spec.selection_mode,
        coverage_ratio: spec.coverage_ratio,
        recenter_trigger_pct: spec.recenter_trigger_pct,
        days_n: series.net_vip0_bps.len(),
        mean_basis_bps: welford_mean(&series.basis_bps)?,
        mean_fund_bps: welford_mean(&series.fund_bps)?,
        mean_borrow_bps: welford_mean(&series.borrow_bps)?,
        mean_cost_vip0_bps: welford_mean(&series.cost_vip0_bps)?,
        avg_daily_turn_l: welford_mean(&series.total_turn_l)?,
        avg_coverage_pct: welford_mean(&series.coverage_pct)?,
        recenter_count: series.recenter_count,
        mean_net_vip0_bps: welford_mean(&series.net_vip0_bps)?,
        mean_net_maker_diag_bps: welford_mean(&series.net_maker_diag_bps)?,
        vip0_metrics: compute_equity_metrics(&series.net_vip0_bps, days_per_year)?,
        maker_diag_metrics: compute_equity_metrics(&series.net_maker_diag_bps, days_per_year)?,
    })
}
