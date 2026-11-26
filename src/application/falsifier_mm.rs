use crate::application::mm_accounting::{MmAccountLedger, FRONTIER_COUNT};
pub use crate::application::mm_accounting::{
    MmConfigDiagnostics, MmFalsificationReport, MmFalsifierConfig, MmFeeFrontierPoint,
    MM_FEE_FRONTIER,
};
use crate::application::mm_fifo_sim::{MmFifoConfig, MmFifoSimulator, MmFill};
use crate::domain::error::DomainError;
use crate::domain::fees::FeeSchedule;
use crate::domain::ports::MarketTapeReader;
use crate::domain::signals::market_making::{MmEstimatorConfig, MmQuoteParams, MmSignalEngine};
use crate::domain::stats::{evaluate_hypothesis_batch, BatchSummary, HypothesisCellInput};
use crate::domain::types::{Bps, PriceE8, QtyE8, SCALE_E8};

const SCALE_E8_F64: f64 = SCALE_E8 as f64;

#[derive(Clone, Debug, PartialEq)]
pub struct MmDetailedOutput {
    pub report: MmFalsificationReport,
    pub fills_by_config: Vec<Vec<MmFill>>,
    pub vip0_bucket_returns: Vec<Vec<f64>>,
}

#[must_use]
pub fn default_mm_grid(clip_usdt: f64, max_inv_clips: f64) -> Vec<MmQuoteParams> {
    let build = |name: &str,
                 gamma: f64,
                 theta_skew_bps: f64,
                 min_half_spread_bps: f64,
                 horizon_s: f64,
                 k_arrival: f64| MmQuoteParams {
        name: name.to_string(),
        gamma,
        theta_skew_bps,
        min_half_spread_bps,
        horizon_s,
        k_arrival,
        clip_usdt,
        max_inv_clips,
        tick_e8_override: None,
    };
    vec![
        build("AS_Base_Tight", 0.05, 0.0, 1.0, 30.0, 1.5),
        build("AS_Base_Wide", 0.15, 0.0, 2.5, 60.0, 1.0),
        build("AS_Skew_Tight", 0.05, 2.0, 1.0, 30.0, 1.5),
        build("AS_Skew_Mid", 0.10, 3.0, 2.0, 45.0, 1.2),
        build("AS_Skew_Wide", 0.20, 4.0, 3.5, 60.0, 0.8),
        build("AS_Skew_Deep", 0.30, 5.0, 5.0, 90.0, 0.6),
    ]
}

impl Default for MmFalsifierConfig {
    fn default() -> Self {
        Self {
            capital_usdt: 2_000.0,
            clip_usdt: 100.0,
            max_inv_clips: 4.0,
            max_gross_leverage: 2.0,
            bucket_ns: 60_000_000_000,
            estimator: MmEstimatorConfig::default(),
            fifo: MmFifoConfig::default(),
            grid: default_mm_grid(100.0, 4.0),
        }
    }
}

struct ConfigRunner {
    simulator: MmFifoSimulator,
    account: MmAccountLedger,
}

pub fn run_mm_falsification<R: MarketTapeReader>(
    reader: &mut R,
    config: &MmFalsifierConfig,
) -> Result<MmFalsificationReport, DomainError> {
    run_mm_internal(reader, config, false).map(|out| out.report)
}

pub fn run_mm_falsification_detailed<R: MarketTapeReader>(
    reader: &mut R,
    config: &MmFalsifierConfig,
) -> Result<MmDetailedOutput, DomainError> {
    run_mm_internal(reader, config, true)
}

fn run_mm_internal<R: MarketTapeReader>(
    reader: &mut R,
    config: &MmFalsifierConfig,
    collect_details: bool,
) -> Result<MmDetailedOutput, DomainError> {
    config.validate()?;
    let initial_equity_e8 = (config.capital_usdt * SCALE_E8_F64).round() as i128;
    let max_gross_inv_usdt = config.capital_usdt * config.max_gross_leverage;
    let vip0_schedule = FeeSchedule::bybit_vip0();
    let m_hypotheses = config.grid.len();

    let mut signal_engine = MmSignalEngine::new(config.estimator)?;
    let mut runners = Vec::with_capacity(m_hypotheses);
    for _ in 0..m_hypotheses {
        runners.push(ConfigRunner {
            simulator: MmFifoSimulator::new(config.fifo)?,
            account: MmAccountLedger::new(initial_equity_e8)?,
        });
    }

    let mut fills_by_config = if collect_details {
        vec![Vec::new(); m_hypotheses]
    } else {
        Vec::new()
    };
    let mut bucket_end_ns: Option<i64> = None;
    let mut events_processed = 0_u64;

    while let Some(event) = reader.next_event()? {
        events_processed += 1;
        let ts_ns = event.ts_ns.raw();

        if bucket_end_ns.is_some_and(|end| ts_ns >= end) {
            for runner in &mut runners {
                runner.account.flush_bucket()?;
            }
            bucket_end_ns = None;
        }
        if bucket_end_ns.is_none() {
            bucket_end_ns = Some(
                ts_ns
                    .checked_add(config.bucket_ns)
                    .ok_or(DomainError::ArithmeticOverflow("mm_bucket_end_ns"))?,
            );
        }

        let features = signal_engine.on_event(&event)?;
        for (idx, (runner, params)) in runners.iter_mut().zip(&config.grid).enumerate() {
            let fills = runner.simulator.on_event(&event)?;
            for fill in fills {
                runner.account.record_fill(
                    fill.symbol_id,
                    fill.side,
                    fill.fill_px,
                    fill.qty,
                    None,
                )?;
                if collect_details {
                    fills_by_config[idx].push(fill);
                }
            }

            let gross_inv_usdt = runner
                .account
                .update_book_and_gross(event.symbol_id, event.book);
            let symbol_qty_e8 = runner.account.symbol_qty_e8(event.symbol_id);
            let symbol_inv_usdt = ((symbol_qty_e8 as f64) / SCALE_E8_F64) * features.s_mid;
            let (mut bid_quote, mut ask_quote) = (None, None);
            if let Some(target) =
                signal_engine.quote_for_event(&event, &features, symbol_inv_usdt, params)?
            {
                let gross_capped = gross_inv_usdt >= max_gross_inv_usdt;
                bid_quote = size_quote_leg(
                    target.bid_target,
                    params.clip_usdt,
                    gross_capped && symbol_qty_e8 >= 0,
                );
                ask_quote = size_quote_leg(
                    target.ask_target,
                    params.clip_usdt,
                    gross_capped && symbol_qty_e8 <= 0,
                );
            }
            runner
                .simulator
                .on_quote_decision(event.symbol_id, ts_ns, bid_quote, ask_quote)?;
        }
    }

    if events_processed > 0 {
        for runner in &mut runners {
            runner.account.liquidate_eof(vip0_schedule.taker_bps)?;
        }
    }

    let vip0_bucket_returns = if collect_details {
        runners
            .iter()
            .map(|runner| runner.account.bucket_returns_bps[0].clone())
            .collect()
    } else {
        Vec::new()
    };

    let report = build_mm_report(config, events_processed, &vip0_schedule, &mut runners)?;
    Ok(MmDetailedOutput {
        report,
        fills_by_config,
        vip0_bucket_returns,
    })
}

fn build_mm_report(
    config: &MmFalsifierConfig,
    events_processed: u64,
    vip0_schedule: &FeeSchedule,
    runners: &mut [ConfigRunner],
) -> Result<MmFalsificationReport, DomainError> {
    let m_hypotheses = config.grid.len();
    let horizon_label = format_bucket_horizon(config.bucket_ns);
    let mut drawdowns_bps = vec![[0.0_f64; FRONTIER_COUNT]; m_hypotheses];
    let mut frontier_batches: Vec<BatchSummary> = Vec::with_capacity(FRONTIER_COUNT);

    for (profile_idx, spec) in MM_FEE_FRONTIER.iter().enumerate() {
        let schedule = FeeSchedule {
            name: spec.name,
            maker_bps: Bps::new(spec.maker_bps)?,
            taker_bps: vip0_schedule.taker_bps,
            is_binding_for_verdict: spec.is_binding,
        };
        let mut cell_inputs = Vec::with_capacity(m_hypotheses);
        for (cfg_idx, (runner, params)) in runners.iter_mut().zip(&config.grid).enumerate() {
            let returns = core::mem::take(&mut runner.account.bucket_returns_bps[profile_idx]);
            drawdowns_bps[cfg_idx][profile_idx] = max_drawdown_bps(&returns);
            cell_inputs.push(HypothesisCellInput {
                hypothesis: params.name.clone(),
                horizon: horizon_label.clone(),
                returns_bps: returns,
            });
        }
        frontier_batches.push(evaluate_hypothesis_batch(&cell_inputs, &schedule)?);
    }

    let mut configs = Vec::with_capacity(m_hypotheses);
    for (cfg_idx, (runner, params)) in runners.iter().zip(&config.grid).enumerate() {
        let sim_metrics = runner.simulator.metrics();
        let acct = &runner.account;
        let mut frontier = Vec::with_capacity(FRONTIER_COUNT);
        for (profile_idx, spec) in MM_FEE_FRONTIER.iter().enumerate() {
            let cell = &frontier_batches[profile_idx].cells[cfg_idx];
            frontier.push(MmFeeFrontierPoint {
                profile: cell.profile.clone(),
                maker_fee_bps: spec.maker_bps,
                is_binding: cell.is_binding,
                net_pnl_usdt: to_usdt(acct.prev_bucket_net_e8[profile_idx]),
                mean_bucket_bps: cell.mean_bps,
                std_bucket_bps: cell.std_bps,
                t_nw: cell.t_nw,
                t_crit: cell.t_crit,
                p_holm: cell.p_holm,
                dsr: cell.dsr,
                n_min: cell.n_min,
                max_drawdown_bps: drawdowns_bps[cfg_idx][profile_idx],
                verdict: cell.verdict,
            });
        }
        let eof_cost_e8 = acct.eof_spread_cost_e8 + acct.eof_taker_fee_e8;
        let net_vip0_e8 = acct.ledger.current_equity_e8() - acct.initial_equity_e8;
        configs.push(MmConfigDiagnostics {
            config_name: params.name.clone(),
            gamma: params.gamma,
            theta_skew_bps: params.theta_skew_bps,
            min_half_spread_bps: params.min_half_spread_bps,
            horizon_s: params.horizon_s,
            k_arrival: params.k_arrival,
            buckets_n: frontier_batches[0].cells[cfg_idx].n,
            fills_total: sim_metrics.fills_total,
            post_only_rejects: sim_metrics.post_only_rejects,
            adverse_in_flight_fills: sim_metrics.adverse_in_flight_fills,
            total_maker_volume_usdt: to_usdt(acct.total_maker_volume_e8),
            gross_pnl_usdt: to_usdt(acct.ledger.cumulative_gross_pnl_e8()),
            net_pnl_vip0_usdt: to_usdt(net_vip0_e8),
            mean_spread_capture_bps: sim_metrics.mean_spread_capture_bps(),
            mean_markout_1s_bps: sim_metrics.mean_markout_1s_bps(),
            mean_markout_5s_bps: sim_metrics.mean_markout_5s_bps(),
            mean_markout_15s_bps: sim_metrics.mean_markout_15s_bps(),
            mean_lvr_5s_bps: sim_metrics.mean_lvr_5s_bps(),
            peak_gross_inv_usdt: acct.peak_gross_inv_usdt,
            eof_liq_cost_usdt: to_usdt(eof_cost_e8),
            eof_liq_fee_usdt: to_usdt(acct.eof_taker_fee_e8),
            max_drawdown_bps: drawdowns_bps[cfg_idx][0],
            breakeven_maker_bps: acct.breakeven_maker_bps(),
            accounting_drift_e8: acct.accounting_drift_e8()?,
            frontier,
        });
    }

    let vip0_batch = frontier_batches.remove(0);
    Ok(MmFalsificationReport {
        track: "Track4-MarketMaking".to_string(),
        events_processed,
        capital_usdt: config.capital_usdt,
        clip_usdt: config.clip_usdt,
        max_inv_clips: config.max_inv_clips,
        max_gross_leverage: config.max_gross_leverage,
        bucket_s: (config.bucket_ns as f64) / 1e9,
        rtt_ms: (config.fifo.rtt_ns as f64) / 1e6,
        m_hypotheses,
        t_crit: vip0_batch.t_crit,
        sr_cross_std_vip0: vip0_batch.sr_cross_std,
        vip0_cells: vip0_batch.cells,
        configs,
    })
}

fn to_usdt(amount_e8: i128) -> f64 {
    (amount_e8 as f64) / SCALE_E8_F64
}

fn max_drawdown_bps(returns_bps: &[f64]) -> f64 {
    returns_bps
        .iter()
        .fold(
            (0.0_f64, 0.0_f64, 0.0_f64),
            |(equity, peak, max_dd), &ret| {
                let next_equity = equity + ret;
                let next_peak = peak.max(next_equity);
                (next_equity, next_peak, max_dd.max(next_peak - next_equity))
            },
        )
        .2
}

fn size_quote_leg(
    target_px: Option<PriceE8>,
    clip_usdt: f64,
    blocked: bool,
) -> Option<(PriceE8, QtyE8)> {
    let price = target_px.filter(|_| !blocked)?;
    let raw_qty = ((clip_usdt / price.as_f64()) * SCALE_E8_F64).round();
    if !raw_qty.is_finite() || !(1.0..=(i64::MAX as f64)).contains(&raw_qty) {
        return None;
    }
    QtyE8::new(raw_qty as i64).ok().map(|qty| (price, qty))
}

fn format_bucket_horizon(bucket_ns: i64) -> String {
    if bucket_ns % 1_000_000_000 == 0 {
        format!("{}s", bucket_ns / 1_000_000_000)
    } else {
        format!("{bucket_ns}ns")
    }
}
