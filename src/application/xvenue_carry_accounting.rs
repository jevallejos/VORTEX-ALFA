use crate::application::macro_accounting::CostPerTurnover;
use crate::application::macro_baselines::{compute_equity_metrics, weight_turnover};
pub use crate::application::xvenue_carry_types::{
    BridgeConfig, XVenueBarRecord, XVenueBarSlice, XVenueCellReport, XVenueStrategySpec,
};
use crate::domain::error::DomainError;
use crate::domain::stats::{welford_mean, GateVerdict};
use crate::domain::types::{Candle, FundingTimestampSemantics, SymbolId, BPS_PER_UNIT};
use std::collections::BTreeMap;

const PCT_SCALE: f64 = 100.0;

#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct XVenueOpenPosition {
    pub signed_weight: f64,
    pub entry_bybit_px: f64,
    pub entry_hl_px: f64,
}
#[derive(Copy, Clone, Debug)]
pub struct HeldXVenueLeg {
    pub signed_weight: f64,
    pub bybit: Candle,
    pub hl: Candle,
}
#[derive(Clone, Debug)]
pub struct ActiveXVenueBar {
    pub decision_ts_ns: i64,
    pub entry_ts_ns: i64,
    pub legs: BTreeMap<SymbolId, HeldXVenueLeg>,
    pub total_gross_l: f64,
    pub util_pct: f64,
    pub rebalance_turn_l: f64,
    pub recenter_turn_l: f64,
    pub recenter_events: u32,
}
#[derive(Clone, Debug)]
pub struct OpenedXVenueBar {
    pub active_bar: ActiveXVenueBar,
    pub opened_weights: BTreeMap<SymbolId, f64>,
}
#[derive(Clone, Debug, Default)]
pub struct BarXVenueSeries {
    pub basis_bps: Vec<f64>,
    pub fund_spread_bps: Vec<f64>,
    pub bridge_bps: Vec<f64>,
    pub cost_vip0_bps: Vec<f64>,
    pub total_turn_l: Vec<f64>,
    pub gross_l: Vec<f64>,
    pub util_pct: Vec<f64>,
    pub recenter_count: u64,
    pub bridge_count: u64,
    pub net_vip0_bps: Vec<f64>,
    pub net_maker_diag_bps: Vec<f64>,
}

#[allow(clippy::too_many_arguments)]
pub fn open_xvenue_bar(
    decision_ts_ns: i64,
    target_weights: &BTreeMap<SymbolId, f64>,
    prev_weights: &BTreeMap<SymbolId, f64>,
    positions: &mut BTreeMap<SymbolId, XVenueOpenPosition>,
    slice: &XVenueBarSlice,
    target_l: f64,
    recenter_pct: f64,
) -> Result<OpenedXVenueBar, DomainError> {
    if !pos_fin(target_l) || !pos_fin(recenter_pct) {
        return Err(DomainError::InvalidStatParam("xvenue_open_params"));
    }
    if target_weights.values().any(|w| !w.is_finite()) {
        return Err(DomainError::NonFiniteValue("xvenue_weight"));
    }
    let (mut opened_weights, mut legs) = (BTreeMap::new(), BTreeMap::new());
    let (mut gross_l, mut rec_turn, mut rec_ev) = (0.0_f64, 0.0_f64, 0_u32);
    for (&sym, &new_w) in target_weights.iter().filter(|(_, w)| **w != 0.0) {
        let (Some(byb), Some(hl)) = (slice.bybit.get(&sym), slice.hl.get(&sym)) else {
            continue;
        };
        let (byb_o, hl_o) = (byb.open.as_f64(), hl.open.as_f64());
        opened_weights.insert(sym, new_w);
        let pos = positions.entry(sym).or_default();
        let (old_abs, new_abs) = (pos.signed_weight.abs(), new_w.abs());
        if old_abs == 0.0 || pos.signed_weight * new_w < 0.0 {
            (pos.entry_bybit_px, pos.entry_hl_px) = (byb_o, hl_o);
        } else if new_abs > old_abs {
            let add_l = new_abs - old_abs;
            pos.entry_bybit_px = new_abs / (old_abs / pos.entry_bybit_px + add_l / byb_o);
            pos.entry_hl_px = new_abs / (old_abs / pos.entry_hl_px + add_l / hl_o);
        }
        pos.signed_weight = new_w;
        gross_l += new_abs;
        let byb_mv = ((byb_o - pos.entry_bybit_px) / pos.entry_bybit_px).abs();
        let hl_mv = ((hl_o - pos.entry_hl_px) / pos.entry_hl_px).abs();
        if byb_mv.max(hl_mv) >= recenter_pct {
            rec_turn += 2.0 * new_abs;
            rec_ev += 1;
            (pos.entry_bybit_px, pos.entry_hl_px) = (byb_o, hl_o);
        }
        let leg = HeldXVenueLeg {
            signed_weight: new_w,
            bybit: *byb,
            hl: *hl,
        };
        legs.insert(sym, leg);
    }
    positions.retain(|sym, _| opened_weights.contains_key(sym));
    let reb_turn = weight_turnover(prev_weights, &opened_weights);
    let util_pct = (gross_l / target_l) * PCT_SCALE;
    if !all_fin(&[gross_l, util_pct, reb_turn, rec_turn]) {
        return Err(DomainError::NonFiniteValue("xvenue_open_bar"));
    }
    Ok(OpenedXVenueBar {
        active_bar: ActiveXVenueBar {
            decision_ts_ns,
            entry_ts_ns: slice.ts_ns.raw(),
            legs,
            total_gross_l: gross_l,
            util_pct,
            rebalance_turn_l: reb_turn,
            recenter_turn_l: rec_turn,
            recenter_events: rec_ev,
        },
        opened_weights,
    })
}

#[allow(clippy::too_many_arguments)]
pub fn settle_xvenue_bar(
    strategy_name: &str,
    bar: ActiveXVenueBar,
    slice: &XVenueBarSlice,
    eof: bool,
    sem: FundingTimestampSemantics,
    costs: CostPerTurnover,
    bridge: BridgeConfig,
    imb: &mut f64,
    series: &mut BarXVenueSeries,
    records: &mut Vec<XVenueBarRecord>,
) -> Result<(), DomainError> {
    let fee_ok = bridge.bridge_fee_usdt.is_finite() && bridge.bridge_fee_usdt >= 0.0;
    if !pos_fin(bridge.capital_usdt) || !pos_fin(bridge.imbalance_trigger_frac) || !fee_ok {
        return Err(DomainError::InvalidStatParam("xvenue_bridge_cfg"));
    }
    if !imb.is_finite() {
        return Err(DomainError::NonFiniteValue("venue_imbalance_frac"));
    }
    let (mut basis, mut fund, mut close_turn, mut byb_pnl) = (0.0_f64, 0.0_f64, 0.0_f64, 0.0_f64);
    let mut weights = BTreeMap::new();
    let bar_open = sem == FundingTimestampSemantics::BarOpenSettlement;
    for (&sym, leg) in &bar.legs {
        weights.insert(sym, leg.signed_weight);
        close_turn += leg.signed_weight.abs();
        let (byb_c, hl_c) = (slice.bybit.get(&sym), slice.hl.get(&sym));
        let (byb_ret, hl_ret) = (leg_ret(byb_c, &leg.bybit, eof), leg_ret(hl_c, &leg.hl, eof));
        let byb_f = bar_fund(byb_c, &leg.bybit, bar_open, eof);
        let hl_f = bar_fund(hl_c, &leg.hl, bar_open, eof);
        basis += leg.signed_weight * (byb_ret - hl_ret) * BPS_PER_UNIT;
        fund += leg.signed_weight * (hl_f - byb_f);
        byb_pnl += leg.signed_weight * (byb_ret - byb_f / BPS_PER_UNIT);
    }
    *imb += byb_pnl;
    let (mut bridge_bps, mut bridge_events) = (0.0_f64, 0_u32);
    if imb.abs() >= bridge.imbalance_trigger_frac {
        bridge_bps = (bridge.bridge_fee_usdt / bridge.capital_usdt) * BPS_PER_UNIT;
        bridge_events = 1;
        *imb = 0.0;
    }
    let eof_turn = if eof { close_turn } else { 0.0 };
    let turn = bar.rebalance_turn_l + bar.recenter_turn_l + eof_turn;
    let (cost_v, cost_m) = (turn * costs.vip0_bps, turn * costs.maker_diag_bps);
    let net_v = basis + fund - bridge_bps - cost_v;
    let net_m = basis + fund - bridge_bps - cost_m;
    if !all_fin(&[basis, fund, bridge_bps, *imb, turn]) || !all_fin(&[cost_v, cost_m, net_v, net_m])
    {
        return Err(DomainError::NonFiniteValue("xvenue_bar_return"));
    }
    series.basis_bps.push(basis);
    series.fund_spread_bps.push(fund);
    series.bridge_bps.push(bridge_bps);
    series.cost_vip0_bps.push(cost_v);
    series.total_turn_l.push(turn);
    series.gross_l.push(bar.total_gross_l);
    series.util_pct.push(bar.util_pct);
    series.recenter_count += u64::from(bar.recenter_events);
    series.bridge_count += u64::from(bridge_events);
    series.net_vip0_bps.push(net_v);
    series.net_maker_diag_bps.push(net_m);
    records.push(XVenueBarRecord {
        strategy: strategy_name.to_string(),
        decision_ts_ns: bar.decision_ts_ns,
        entry_ts_ns: bar.entry_ts_ns,
        exit_ts_ns: slice.ts_ns.raw(),
        is_eof_close: eof,
        weights,
        total_gross_l: bar.total_gross_l,
        basis_bps: basis,
        fund_spread_bps: fund,
        bridge_bps,
        rebalance_turn_l: bar.rebalance_turn_l,
        recenter_turn_l: bar.recenter_turn_l,
        eof_close_turn_l: eof_turn,
        total_turn_l: turn,
        recenter_events: bar.recenter_events,
        bridge_events,
        venue_imbalance_after: *imb,
        cost_vip0_bps: cost_v,
        cost_maker_diag_bps: cost_m,
        net_vip0_bps: net_v,
        net_maker_diag_bps: net_m,
    });
    Ok(())
}

pub fn build_xvenue_cell_report(
    spec: &XVenueStrategySpec,
    series: &BarXVenueSeries,
    n_min_cum116: Option<u64>,
    dsr_cum116: Option<f64>,
    verdict_cum116: GateVerdict,
    bars_per_year: f64,
) -> Result<XVenueCellReport, DomainError> {
    Ok(XVenueCellReport {
        strategy: spec.name.to_string(),
        total_notional_l: spec.total_notional_l,
        recenter_trigger_pct: spec.recenter_trigger_pct,
        bars_n: series.net_vip0_bps.len(),
        n_min_cum116,
        dsr_cum116,
        verdict_cum116,
        mean_basis_bps: welford_mean(&series.basis_bps)?,
        mean_fund_spread_bps: welford_mean(&series.fund_spread_bps)?,
        mean_bridge_bps: welford_mean(&series.bridge_bps)?,
        mean_cost_vip0_bps: welford_mean(&series.cost_vip0_bps)?,
        avg_bar_turn_l: welford_mean(&series.total_turn_l)?,
        avg_gross_l: welford_mean(&series.gross_l)?,
        avg_util_pct: welford_mean(&series.util_pct)?,
        recenter_count: series.recenter_count,
        bridge_count: series.bridge_count,
        mean_net_vip0_bps: welford_mean(&series.net_vip0_bps)?,
        mean_net_maker_diag_bps: welford_mean(&series.net_maker_diag_bps)?,
        vip0_metrics: compute_equity_metrics(&series.net_vip0_bps, bars_per_year)?,
        maker_diag_metrics: compute_equity_metrics(&series.net_maker_diag_bps, bars_per_year)?,
    })
}

fn pos_fin(val: f64) -> bool {
    val.is_finite() && val > 0.0
}
fn all_fin(vals: &[f64]) -> bool {
    vals.iter().all(|v| v.is_finite())
}
fn leg_ret(candle: Option<&Candle>, entry: &Candle, eof: bool) -> f64 {
    let bar = candle.map_or(entry.close, |b| if eof { b.close } else { b.open });
    (bar.as_f64() - entry.open.as_f64()) / entry.open.as_f64()
}
fn bar_fund(candle: Option<&Candle>, entry: &Candle, bar_open: bool, eof: bool) -> f64 {
    match (bar_open, eof) {
        (false, _) => entry.funding_rate_bps.raw(),
        (true, true) => 0.0,
        (true, false) => candle.map_or(0.0, |b| b.funding_rate_bps.raw()),
    }
}
