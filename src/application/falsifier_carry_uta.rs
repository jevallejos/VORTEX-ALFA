use std::collections::BTreeMap;

use crate::application::carry_uta_accounting::{
    build_carry_uta_cell_report, open_carry_uta_day, settle_carry_uta_day, ActiveCarryUtaDay,
    DailyCarryUtaSeries, UtaOpenPosition,
};
pub use crate::application::carry_uta_types::*;
use crate::application::macro_accounting::{CostPerTurnover, MacroFeeProfiles};
use crate::application::macro_baselines::MacroDailySlice;
use crate::domain::error::DomainError;
use crate::domain::ports::CandleSeriesReader;
use crate::domain::signals::carry_uta::CarryUtaEngine;
use crate::domain::stats::{evaluate_hypothesis_batch, FalsificationReport, HypothesisCellInput};
use crate::domain::types::{FundingTimestampSemantics, SymbolId};

struct PendingCarryDecision {
    decision_ts_ns: i64,
    weights: BTreeMap<SymbolId, f64>,
}

struct CarryUtaSlot {
    spec: CarryUtaStrategySpec,
    engine: CarryUtaEngine,
    positions: BTreeMap<SymbolId, UtaOpenPosition>,
    active_day: Option<ActiveCarryUtaDay>,
    pending: Option<PendingCarryDecision>,
    previous_weights: BTreeMap<SymbolId, f64>,
    series: DailyCarryUtaSeries,
}

fn settle_slot(
    slot: &mut CarryUtaSlot,
    slice: &MacroDailySlice,
    is_eof_close: bool,
    funding_semantics: FundingTimestampSemantics,
    cost_rates: CostPerTurnover,
    records: &mut Vec<CarryUtaDayRecord>,
) -> Result<(), DomainError> {
    let Some(active_day) = slot.active_day.take() else {
        return Ok(());
    };
    settle_carry_uta_day(
        slot.spec.name,
        active_day,
        slice,
        is_eof_close,
        funding_semantics,
        cost_rates,
        &mut slot.series,
        records,
    )
}

pub fn run_carry_uta_falsification_detailed<RL: CandleSeriesReader, RS: CandleSeriesReader>(
    linear_reader: &mut RL,
    spot_reader: &mut RS,
    config: &CarryUtaConfig,
) -> Result<(CarryUtaReport, Vec<CarryUtaDayRecord>), DomainError> {
    config.validate()?;
    let fee_profiles = MacroFeeProfiles::new(config.slippage_bps)?;
    let carry_costs = fee_profiles.carry_costs();

    let mut grouped = BTreeMap::<i64, MacroDailySlice>::new();
    let linear_candles_processed = ingest_stream(linear_reader, false, &mut grouped)?;
    let spot_candles_processed = ingest_stream(spot_reader, true, &mut grouped)?;
    let slices: Vec<MacroDailySlice> = grouped.into_values().collect();

    let mut slots = build_slots(config, carry_costs.vip0_bps)?;
    let mut records = Vec::new();
    let semantics = config.funding_semantics;
    for slice in &slices {
        for slot in &mut slots {
            settle_slot(slot, slice, false, semantics, carry_costs, &mut records)?;
            if let Some(pending) = slot.pending.take() {
                let opened = open_carry_uta_day(
                    pending.decision_ts_ns,
                    &pending.weights,
                    &slot.previous_weights,
                    &mut slot.positions,
                    slice,
                    slot.spec.recenter_trigger_pct,
                    config.borrow_apr_pct,
                    config.days_per_year,
                    semantics,
                )?;
                slot.previous_weights = opened.opened_weights;
                slot.active_day = Some(opened.active_day);
            }
        }
        for slot in &mut slots {
            if let Some(weights) = slot.engine.on_slice(&slice.linear, &slice.spot)? {
                slot.pending = Some(PendingCarryDecision {
                    decision_ts_ns: slice.ts_ns.raw(),
                    weights,
                });
            }
        }
    }
    if let Some(last) = slices.last() {
        for slot in &mut slots {
            settle_slot(slot, last, true, semantics, carry_costs, &mut records)?;
        }
    }

    let mut vip0_cells_in = Vec::with_capacity(slots.len());
    let mut maker_cells_in = Vec::with_capacity(slots.len());
    let mut diagnostics = Vec::with_capacity(slots.len());
    for slot in &mut slots {
        diagnostics.push(build_carry_uta_cell_report(
            &slot.spec,
            &slot.series,
            config.days_per_year,
        )?);
        vip0_cells_in.push(HypothesisCellInput {
            hypothesis: slot.spec.name.to_string(),
            horizon: "1d".to_string(),
            returns_bps: core::mem::take(&mut slot.series.net_vip0_bps),
        });
        maker_cells_in.push(HypothesisCellInput {
            hypothesis: slot.spec.name.to_string(),
            horizon: "1d".to_string(),
            returns_bps: core::mem::take(&mut slot.series.net_maker_diag_bps),
        });
    }
    let vip0_batch = evaluate_hypothesis_batch(&vip0_cells_in, &fee_profiles.perp_vip0_taker)?;
    let maker_batch = evaluate_hypothesis_batch(&maker_cells_in, &fee_profiles.perp_maker_diag)?;
    let report = CarryUtaReport {
        slippage_bps: config.slippage_bps.raw(),
        borrow_apr_pct: config.borrow_apr_pct,
        days_per_year: config.days_per_year,
        slices_processed: slices.len() as u64,
        linear_candles_processed,
        spot_candles_processed,
        report: FalsificationReport {
            track: "Track6-CarryUTA".to_string(),
            events_processed: linear_candles_processed + spot_candles_processed,
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

pub fn run_carry_uta_falsification<RL: CandleSeriesReader, RS: CandleSeriesReader>(
    linear_reader: &mut RL,
    spot_reader: &mut RS,
    config: &CarryUtaConfig,
) -> Result<CarryUtaReport, DomainError> {
    let (report, _) = run_carry_uta_falsification_detailed(linear_reader, spot_reader, config)?;
    Ok(report)
}

fn ingest_stream<R: CandleSeriesReader>(
    reader: &mut R,
    is_spot: bool,
    grouped: &mut BTreeMap<i64, MacroDailySlice>,
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
        let ts_ns = candle.open_ts_ns.raw();
        let symbol_id = candle.symbol_id;
        let slice = grouped.entry(ts_ns).or_insert_with(|| MacroDailySlice {
            ts_ns: candle.open_ts_ns,
            linear: BTreeMap::new(),
            spot: BTreeMap::new(),
        });
        let leg_map = if is_spot {
            &mut slice.spot
        } else {
            &mut slice.linear
        };
        if leg_map.insert(symbol_id, candle).is_some() {
            return Err(DomainError::NonMonotonicTimestamp {
                symbol_id: symbol_id.0,
                prev_ns: ts_ns,
                curr_ns: ts_ns,
            });
        }
    }
    Ok(processed)
}

fn build_slots(
    config: &CarryUtaConfig,
    carry_cost_vip0_bps: f64,
) -> Result<Vec<CarryUtaSlot>, DomainError> {
    let mut slots = Vec::with_capacity(config.strategies.len());
    for &spec in &config.strategies {
        slots.push(CarryUtaSlot {
            spec,
            engine: CarryUtaEngine::new(spec, carry_cost_vip0_bps)?,
            positions: BTreeMap::new(),
            active_day: None,
            pending: None,
            previous_weights: BTreeMap::new(),
            series: DailyCarryUtaSeries::default(),
        });
    }
    Ok(slots)
}
