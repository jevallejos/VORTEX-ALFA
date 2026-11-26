use std::collections::BTreeMap;

use crate::application::macro_accounting::CostPerTurnover;
use crate::application::xvenue_carry_accounting::{
    build_xvenue_cell_report, open_xvenue_bar, settle_xvenue_bar, ActiveXVenueBar, BarXVenueSeries,
    XVenueOpenPosition,
};
pub use crate::application::xvenue_carry_types::*;
use crate::domain::error::DomainError;
use crate::domain::ports::CandleSeriesReader;
use crate::domain::stats::{
    compute_n_min, deflated_sharpe_ratio, evaluate_gate, evaluate_hypothesis_batch, t_crit_holm,
    BatchSummary, CellEvaluation, FalsificationReport, HypothesisCellInput, Moments4,
};
use crate::domain::types::{FundingTimestampSemantics, SymbolId};

struct PendingDecision {
    ts_ns: i64,
    weights: BTreeMap<SymbolId, f64>,
}

struct XVenueSlot {
    spec: XVenueStrategySpec,
    engine: XVenueCarryEngine,
    positions: BTreeMap<SymbolId, XVenueOpenPosition>,
    active_bar: Option<ActiveXVenueBar>,
    pending: Option<PendingDecision>,
    prev_weights: BTreeMap<SymbolId, f64>,
    imbalance_frac: f64,
    series: BarXVenueSeries,
}

#[derive(Copy, Clone)]
struct RunCtx {
    sem: FundingTimestampSemantics,
    costs: CostPerTurnover,
    bridge: BridgeConfig,
}

fn settle_slot(
    slot: &mut XVenueSlot,
    slice: &XVenueBarSlice,
    eof: bool,
    ctx: RunCtx,
    records: &mut Vec<XVenueBarRecord>,
) -> Result<(), DomainError> {
    let Some(bar) = slot.active_bar.take() else {
        return Ok(());
    };
    settle_xvenue_bar(
        slot.spec.name,
        bar,
        slice,
        eof,
        ctx.sem,
        ctx.costs,
        ctx.bridge,
        &mut slot.imbalance_frac,
        &mut slot.series,
        records,
    )
}

pub fn run_xvenue_carry_falsification_detailed<RB: CandleSeriesReader, RH: CandleSeriesReader>(
    bybit_reader: &mut RB,
    hl_reader: &mut RH,
    config: &XVenueCarryConfig,
) -> Result<(XVenueCarryReport, Vec<XVenueBarRecord>), DomainError> {
    config.validate()?;
    let fee_profiles = XVenueFeeProfiles::new(config.slippage_bps)?;
    let ctx = RunCtx {
        sem: config.funding_semantics,
        costs: fee_profiles.xvenue_costs(),
        bridge: BridgeConfig {
            capital_usdt: config.capital_usdt,
            bridge_fee_usdt: config.bridge_fee_usdt,
            imbalance_trigger_frac: config.venue_imbalance_trigger,
        },
    };
    let mut grouped = BTreeMap::<i64, XVenueBarSlice>::new();
    let bybit_n = ingest_stream(bybit_reader, true, &mut grouped)?;
    let hl_n = ingest_stream(hl_reader, false, &mut grouped)?;
    let slices: Vec<XVenueBarSlice> = grouped.into_values().collect();
    let mut slots = build_slots(config, ctx.costs.vip0_bps)?;
    let mut records = Vec::new();
    for slice in &slices {
        for slot in &mut slots {
            settle_slot(slot, slice, false, ctx, &mut records)?;
            if let Some(p) = slot.pending.take() {
                let sp = slot.spec;
                let opened = open_xvenue_bar(
                    p.ts_ns,
                    &p.weights,
                    &slot.prev_weights,
                    &mut slot.positions,
                    slice,
                    sp.total_notional_l,
                    sp.recenter_trigger_pct,
                )?;
                slot.prev_weights = opened.opened_weights;
                slot.active_bar = Some(opened.active_bar);
            }
            if let Some(weights) = slot.engine.on_slice(&slice.bybit, &slice.hl)? {
                slot.pending = Some(PendingDecision {
                    ts_ns: slice.ts_ns.raw(),
                    weights,
                });
            }
        }
    }
    if let Some(last) = slices.last() {
        for slot in &mut slots {
            settle_slot(slot, last, true, ctx, &mut records)?;
        }
    }
    let (mut vip0_in, mut maker_in) = (
        Vec::with_capacity(slots.len()),
        Vec::with_capacity(slots.len()),
    );
    for s in &slots {
        vip0_in.push(cell_in(s.spec.name, &s.series.net_vip0_bps));
        maker_in.push(cell_in(s.spec.name, &s.series.net_maker_diag_bps));
    }
    let vip0_batch = evaluate_hypothesis_batch(&vip0_in, &fee_profiles.binding_schedule)?;
    let maker_batch = evaluate_hypothesis_batch(&maker_in, &fee_profiles.maker_diag_schedule)?;
    let t_crit_cum116 = t_crit_holm(CUMULATIVE_HYPOTHESES_M)?;
    let bpy = config.bars_per_year;
    let diagnostics = slots
        .iter()
        .zip(&vip0_batch.cells)
        .map(|(slot, cell)| build_diag_cell(slot, cell, &vip0_batch, t_crit_cum116, bpy))
        .collect::<Result<Vec<_>, _>>()?;
    let report = XVenueCarryReport {
        slippage_bps: config.slippage_bps.raw(),
        capital_usdt: config.capital_usdt,
        bridge_fee_usdt: config.bridge_fee_usdt,
        venue_imbalance_trigger: config.venue_imbalance_trigger,
        bars_per_year: bpy,
        m_cumulative: CUMULATIVE_HYPOTHESES_M,
        t_crit_cum116,
        slices_processed: slices.len() as u64,
        bybit_candles_processed: bybit_n,
        hl_candles_processed: hl_n,
        report: FalsificationReport {
            track: "Track7-XVenueCarry".to_string(),
            events_processed: bybit_n + hl_n,
            m_hypotheses: slots.len(),
            t_crit: vip0_batch.t_crit,
            sr_cross_std_vip0: vip0_batch.sr_cross_std,
            sr_cross_std_diag: maker_batch.sr_cross_std,
            vip0_cells: vip0_batch.cells,
            diag_zero_cells: maker_batch.cells,
        },
        diagnostics,
    };
    Ok((report, records))
}

pub fn run_xvenue_carry_falsification<RB: CandleSeriesReader, RH: CandleSeriesReader>(
    bybit_reader: &mut RB,
    hl_reader: &mut RH,
    config: &XVenueCarryConfig,
) -> Result<XVenueCarryReport, DomainError> {
    let (report, _) = run_xvenue_carry_falsification_detailed(bybit_reader, hl_reader, config)?;
    Ok(report)
}

fn cell_in(name: &str, ret: &[f64]) -> HypothesisCellInput {
    HypothesisCellInput {
        hypothesis: name.to_string(),
        horizon: "4h".to_string(),
        returns_bps: ret.to_vec(),
    }
}

fn build_diag_cell(
    slot: &XVenueSlot,
    cell: &CellEvaluation,
    batch: &BatchSummary,
    t_crit_116: f64,
    bars_per_year: f64,
) -> Result<XVenueCellReport, DomainError> {
    let moments = Moments4::compute(&slot.series.net_vip0_bps)?;
    let eff = batch
        .sr_cross_std
        .unwrap_or_else(|| 1.0 / (cell.n.max(1) as f64).sqrt());
    let n_min = moments.and_then(|m| compute_n_min(m.mean, m.std_dev, t_crit_116));
    let dsr = moments
        .map(|m| deflated_sharpe_ratio(&m, CUMULATIVE_HYPOTHESES_M, eff))
        .transpose()?
        .flatten();
    let mean = cell.mean_bps.unwrap_or(0.0);
    let verdict = evaluate_gate(cell.n, mean, cell.t_nw, t_crit_116, n_min, dsr, true);
    build_xvenue_cell_report(&slot.spec, &slot.series, n_min, dsr, verdict, bars_per_year)
}

fn ingest_stream<R: CandleSeriesReader>(
    reader: &mut R,
    is_bybit: bool,
    grouped: &mut BTreeMap<i64, XVenueBarSlice>,
) -> Result<u64, DomainError> {
    let mut processed = 0_u64;
    while let Some(candle) = reader.next_candle()? {
        if !candle.volume.is_finite() || candle.volume < 0.0 {
            return Err(DomainError::InvalidCandle("invalid volume".to_string()));
        }
        if !candle.funding_rate_bps.raw().is_finite() {
            return Err(DomainError::NonFiniteBps);
        }
        processed += 1;
        let (ts_ns, sym) = (candle.open_ts_ns.raw(), candle.symbol_id);
        let slice = grouped.entry(ts_ns).or_insert_with(|| XVenueBarSlice {
            ts_ns: candle.open_ts_ns,
            bybit: BTreeMap::new(),
            hl: BTreeMap::new(),
        });
        let leg = if is_bybit {
            &mut slice.bybit
        } else {
            &mut slice.hl
        };
        if leg.insert(sym, candle).is_some() {
            return Err(DomainError::NonMonotonicTimestamp {
                symbol_id: sym.0,
                prev_ns: ts_ns,
                curr_ns: ts_ns,
            });
        }
    }
    Ok(processed)
}

fn build_slots(config: &XVenueCarryConfig, cost_bps: f64) -> Result<Vec<XVenueSlot>, DomainError> {
    config
        .strategies
        .iter()
        .map(|&spec| {
            Ok(XVenueSlot {
                spec,
                engine: XVenueCarryEngine::new(spec, cost_bps)?,
                positions: BTreeMap::new(),
                active_bar: None,
                pending: None,
                prev_weights: BTreeMap::new(),
                imbalance_frac: 0.0,
                series: BarXVenueSeries::default(),
            })
        })
        .collect()
}
