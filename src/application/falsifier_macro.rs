use std::collections::BTreeMap;

use crate::application::macro_accounting::{
    build_strategy_row, open_trading_day, settle_trading_day, ActiveTradingDay, DailyStrategySeries,
};
pub use crate::application::macro_accounting::{
    CostPerTurnover, MacroDayRecord, MacroFeeProfiles, SPOT_VIP0_FEE_BPS,
};
pub use crate::application::macro_baselines::{
    compute_equity_metrics, compute_passive_baselines, weight_turnover, EquityCurveMetrics,
    MacroBaselineRow, MacroConfig, MacroDailySlice, MacroFalsificationReport, MacroFamily,
    MacroStrategyRow,
};
use crate::domain::error::DomainError;
use crate::domain::ports::CandleSeriesReader;
use crate::domain::signals::macro_donchian::DonchianBreakoutStrategy;
use crate::domain::signals::macro_trend::{MacroTrendFeatureEngine, VolTargetTrendStrategy};
use crate::domain::signals::spot_perp_carry::SpotPerpCarryEngine;
use crate::domain::stats::{evaluate_hypothesis_batch, FalsificationReport, HypothesisCellInput};
use crate::domain::types::{FundingTimestampSemantics, SymbolId};

enum StrategyRunner {
    Trend(VolTargetTrendStrategy),
    Donchian(DonchianBreakoutStrategy),
    Carry(SpotPerpCarryEngine),
}

struct PendingDecision {
    decision_ts_ns: i64,
    weights: BTreeMap<SymbolId, f64>,
}

struct StrategySlot {
    name: String,
    family: MacroFamily,
    runner: StrategyRunner,
    cost_rates: CostPerTurnover,
    active_day: Option<ActiveTradingDay>,
    pending: Option<PendingDecision>,
    previous_weights: BTreeMap<SymbolId, f64>,
    series: DailyStrategySeries,
}

fn settle_slot(
    slot: &mut StrategySlot,
    slice: &MacroDailySlice,
    is_eof_close: bool,
    funding_semantics: FundingTimestampSemantics,
    records: &mut Vec<MacroDayRecord>,
) -> Result<(), DomainError> {
    let Some(active_day) = slot.active_day.take() else {
        return Ok(());
    };
    settle_trading_day(
        &slot.name,
        slot.family,
        active_day,
        slice,
        is_eof_close,
        funding_semantics,
        slot.cost_rates,
        &mut slot.series,
        records,
    )
}

pub fn run_macro_falsification_detailed<RL: CandleSeriesReader, RS: CandleSeriesReader>(
    linear_reader: &mut RL,
    spot_reader: &mut RS,
    config: &MacroConfig,
) -> Result<(MacroFalsificationReport, Vec<MacroDayRecord>), DomainError> {
    config.validate()?;
    let fee_profiles = MacroFeeProfiles::new(config.slippage_bps)?;
    let mut grouped = BTreeMap::<i64, MacroDailySlice>::new();
    let linear_candles_processed = ingest_stream(linear_reader, false, &mut grouped)?;
    let spot_candles_processed = ingest_stream(spot_reader, true, &mut grouped)?;
    let slices: Vec<MacroDailySlice> = grouped.into_values().collect();

    let mut engine = MacroTrendFeatureEngine::new(config.trend_vol)?;
    let mut slots = build_strategy_slots(config, &fee_profiles)?;
    let mut records = Vec::new();
    let semantics = config.funding_semantics;
    for slice in &slices {
        for slot in &mut slots {
            settle_slot(slot, slice, false, semantics, &mut records)?;
            if let Some(pending) = slot.pending.take() {
                let opened = open_trading_day(
                    slot.family,
                    pending.decision_ts_ns,
                    &pending.weights,
                    &slot.previous_weights,
                    slice,
                    semantics,
                );
                slot.previous_weights = opened.opened_weights;
                slot.active_day = Some(opened.active_day);
            }
        }
        let active_symbols = engine.on_slice(&slice.linear)?;
        for slot in &mut slots {
            let decision = match &mut slot.runner {
                StrategyRunner::Trend(runner) => runner.evaluate(&active_symbols, &engine)?,
                StrategyRunner::Donchian(runner) => runner.evaluate(&active_symbols, &engine)?,
                StrategyRunner::Carry(runner) => runner.on_slice(&slice.linear, &slice.spot)?,
            };
            if let Some(weights) = decision {
                slot.pending = Some(PendingDecision {
                    decision_ts_ns: slice.ts_ns.raw(),
                    weights,
                });
            }
        }
    }
    if let Some(last) = slices.last() {
        for slot in &mut slots {
            settle_slot(slot, last, true, semantics, &mut records)?;
        }
    }

    let mut vip0_cells_in = Vec::with_capacity(slots.len());
    let mut maker_cells_in = Vec::with_capacity(slots.len());
    let mut diagnostics = Vec::with_capacity(slots.len());
    for slot in &mut slots {
        let row = build_strategy_row(&slot.name, slot.family, &slot.series, config.days_per_year)?;
        diagnostics.push(row);
        vip0_cells_in.push(HypothesisCellInput {
            hypothesis: slot.name.clone(),
            horizon: "1d".to_string(),
            returns_bps: core::mem::take(&mut slot.series.net_vip0_bps),
        });
        maker_cells_in.push(HypothesisCellInput {
            hypothesis: slot.name.clone(),
            horizon: "1d".to_string(),
            returns_bps: core::mem::take(&mut slot.series.net_maker_bps),
        });
    }
    let vip0_batch = evaluate_hypothesis_batch(&vip0_cells_in, &fee_profiles.perp_vip0_taker)?;
    let maker_batch = evaluate_hypothesis_batch(&maker_cells_in, &fee_profiles.perp_maker_diag)?;
    let spot_costs = fee_profiles.spot_costs();
    let baselines = compute_passive_baselines(
        &slices,
        spot_costs.vip0_bps,
        spot_costs.maker_diag_bps,
        config.days_per_year,
        config.benchmark_symbol,
    )?;
    let report = MacroFalsificationReport {
        slippage_bps: config.slippage_bps.raw(),
        days_per_year: config.days_per_year,
        slices_processed: slices.len() as u64,
        linear_candles_processed,
        spot_candles_processed,
        report: FalsificationReport {
            track: "Track5-Macro1D".to_string(),
            events_processed: linear_candles_processed + spot_candles_processed,
            m_hypotheses: slots.len(),
            t_crit: vip0_batch.t_crit,
            sr_cross_std_vip0: vip0_batch.sr_cross_std,
            sr_cross_std_diag: maker_batch.sr_cross_std,
            vip0_cells: vip0_batch.cells,
            diag_zero_cells: maker_batch.cells,
        },
        diagnostics,
        baselines,
    };
    Ok((report, records))
}

pub fn run_macro_falsification<RL: CandleSeriesReader, RS: CandleSeriesReader>(
    linear_reader: &mut RL,
    spot_reader: &mut RS,
    config: &MacroConfig,
) -> Result<MacroFalsificationReport, DomainError> {
    run_macro_falsification_detailed(linear_reader, spot_reader, config).map(|(report, _)| report)
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

fn build_strategy_slots(
    config: &MacroConfig,
    fee_profiles: &MacroFeeProfiles,
) -> Result<Vec<StrategySlot>, DomainError> {
    let (perp_costs, carry_costs) = (fee_profiles.perp_costs(), fee_profiles.carry_costs());
    let mut slots = Vec::new();
    let mut push_slot = |name: &str, runner: StrategyRunner, cost_rates| {
        let family = match &runner {
            StrategyRunner::Trend(_) => MacroFamily::VolTargetTrend,
            StrategyRunner::Donchian(_) => MacroFamily::DonchianBreakout,
            StrategyRunner::Carry(_) => MacroFamily::SpotPerpCarry,
        };
        slots.push(StrategySlot {
            name: name.to_string(),
            family,
            runner,
            cost_rates,
            active_day: None,
            pending: None,
            previous_weights: BTreeMap::new(),
            series: DailyStrategySeries::default(),
        });
    };
    for &params in &config.trend_strategies {
        push_slot(
            params.name,
            StrategyRunner::Trend(VolTargetTrendStrategy::new(params)?),
            perp_costs,
        );
    }
    for &params in &config.donchian_strategies {
        push_slot(
            params.name,
            StrategyRunner::Donchian(DonchianBreakoutStrategy::new(params)?),
            perp_costs,
        );
    }
    for &params in &config.carry_strategies {
        push_slot(
            params.name,
            StrategyRunner::Carry(SpotPerpCarryEngine::new(params)?),
            carry_costs,
        );
    }
    Ok(slots)
}
