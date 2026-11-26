use std::collections::BTreeMap;

use crate::domain::error::DomainError;
use crate::domain::fees::{candle_roundtrip_net_bps, FeeSchedule};
use crate::domain::ports::CandleSeriesReader;
use crate::domain::signals::meso_energy::{MesoConfig, MesoEnergyEngine, MesoHypothesis};
use crate::domain::stats::{evaluate_hypothesis_batch, FalsificationReport, HypothesisCellInput};
use crate::domain::types::{Bps, Candle, FundingTimestampSemantics, PriceE8, Side, SymbolId};

#[derive(Clone, Debug)]
pub struct MesoFalsifierConfig {
    pub horizons_bars: Vec<usize>,
    pub slippage_bps: Bps,
    pub funding_semantics: FundingTimestampSemantics,
    pub detector: MesoConfig,
    pub entry_is_taker: bool,
    pub exit_is_taker: bool,
}

impl Default for MesoFalsifierConfig {
    fn default() -> Self {
        let default_slip = Bps::new(1.5).unwrap_or(FeeSchedule::diag_zero_maker().maker_bps);
        Self {
            horizons_bars: vec![1, 3, 6, 12],
            slippage_bps: default_slip,
            funding_semantics: FundingTimestampSemantics::BarOpenSettlement,
            detector: MesoConfig::default(),
            entry_is_taker: true,
            exit_is_taker: true,
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct MesoTradeRecord {
    pub symbol_id: SymbolId,
    pub hypothesis: MesoHypothesis,
    pub horizon_bars: usize,
    pub side: Side,
    pub signal_ts_ns: i64,
    pub entry_ts_ns: i64,
    pub exit_ts_ns: i64,
    pub funding_cost_bps: f64,
    pub net_bps_vip0: f64,
    pub net_bps_diag: f64,
}

#[derive(Copy, Clone, Debug)]
enum MesoPosState {
    PendingEntry {
        signal_ts_ns: i64,
        side: Side,
        entry_bar: usize,
    },
    Open {
        signal_ts_ns: i64,
        side: Side,
        entry_open: PriceE8,
        entry_ts_ns: i64,
        exit_bar: usize,
        funding_sum_bps: f64,
    },
}

struct SymbolMesoBook {
    bar_idx: usize,
    slots: Vec<Option<MesoPosState>>,
}

struct CloseContext<'a> {
    config: &'a MesoFalsifierConfig,
    vip0_sched: &'a FeeSchedule,
    diag_sched: &'a FeeSchedule,
    candle: &'a Candle,
    hypothesis: MesoHypothesis,
    horizon_bars: usize,
}

struct CloseFill {
    signal_ts_ns: i64,
    side: Side,
    entry_open: PriceE8,
    entry_ts_ns: i64,
    funding_sum_bps: f64,
}

fn settle_meso_trade(
    ctx: &CloseContext<'_>,
    fill: CloseFill,
    vip0_ret: &mut Vec<f64>,
    diag_ret: &mut Vec<f64>,
    trades: &mut Vec<MesoTradeRecord>,
) -> Result<(), DomainError> {
    let funding_cost_bps = fill.side.sign_f64() * fill.funding_sum_bps;
    let funding_bps = Bps::new(funding_cost_bps)?;
    let net_vip0 = candle_roundtrip_net_bps(
        fill.side,
        fill.entry_open,
        ctx.candle.close,
        ctx.config.slippage_bps,
        ctx.vip0_sched,
        ctx.config.entry_is_taker,
        ctx.config.exit_is_taker,
        funding_bps,
    )?
    .raw();
    let net_diag = candle_roundtrip_net_bps(
        fill.side,
        fill.entry_open,
        ctx.candle.close,
        ctx.config.slippage_bps,
        ctx.diag_sched,
        ctx.config.entry_is_taker,
        ctx.config.exit_is_taker,
        funding_bps,
    )?
    .raw();
    vip0_ret.push(net_vip0);
    diag_ret.push(net_diag);
    trades.push(MesoTradeRecord {
        symbol_id: ctx.candle.symbol_id,
        hypothesis: ctx.hypothesis,
        horizon_bars: ctx.horizon_bars,
        side: fill.side,
        signal_ts_ns: fill.signal_ts_ns,
        entry_ts_ns: fill.entry_ts_ns,
        exit_ts_ns: ctx.candle.open_ts_ns.raw(),
        funding_cost_bps,
        net_bps_vip0: net_vip0,
        net_bps_diag: net_diag,
    });
    Ok(())
}

pub fn run_meso_falsification_detailed<R: CandleSeriesReader>(
    reader: &mut R,
    config: &MesoFalsifierConfig,
) -> Result<(FalsificationReport, Vec<MesoTradeRecord>), DomainError> {
    if config.horizons_bars.is_empty() {
        return Err(DomainError::InvalidStatParam(
            "horizons_bars must be non-empty",
        ));
    }
    if config.horizons_bars.contains(&0) {
        return Err(DomainError::InvalidStatParam("horizon_bars must be >= 1"));
    }
    if config.slippage_bps.raw() < 0.0 {
        return Err(DomainError::InvalidFeeBps);
    }

    let mut engine = MesoEnergyEngine::new(config.detector.clone())?;
    let vip0_sched = FeeSchedule::bybit_vip0();
    let diag_sched = FeeSchedule::diag_zero_maker();
    let n_hor = config.horizons_bars.len();
    let total_cells = MesoHypothesis::ALL.len() * n_hor;

    let mut vip0_returns = vec![Vec::<f64>::new(); total_cells];
    let mut diag_returns = vec![Vec::<f64>::new(); total_cells];
    let mut symbol_books: BTreeMap<SymbolId, SymbolMesoBook> = BTreeMap::new();
    let mut trades = Vec::new();
    let mut events_processed = 0_u64;

    while let Some(candle) = reader.next_candle()? {
        events_processed += 1;
        let sym = symbol_books
            .entry(candle.symbol_id)
            .or_insert_with(|| SymbolMesoBook {
                bar_idx: 0,
                slots: vec![None; total_cells],
            });
        let bar_idx = sym.bar_idx;
        let candle_ts = candle.open_ts_ns.raw();

        for (slot_idx, slot) in sym.slots.iter_mut().enumerate() {
            let h_bars = config.horizons_bars[slot_idx % n_hor];
            let hypothesis = MesoHypothesis::ALL[slot_idx / n_hor];
            let ctx = CloseContext {
                config,
                vip0_sched: &vip0_sched,
                diag_sched: &diag_sched,
                candle: &candle,
                hypothesis,
                horizon_bars: h_bars,
            };

            match *slot {
                Some(MesoPosState::PendingEntry {
                    signal_ts_ns,
                    side,
                    entry_bar,
                }) if bar_idx == entry_bar => {
                    let exit_bar = entry_bar + h_bars - 1;
                    let init_funding = match config.funding_semantics {
                        FundingTimestampSemantics::BarOpenSettlement => 0.0,
                        FundingTimestampSemantics::BarCloseSettlement => {
                            candle.funding_rate_bps.raw()
                        }
                    };
                    if bar_idx == exit_bar {
                        settle_meso_trade(
                            &ctx,
                            CloseFill {
                                signal_ts_ns,
                                side,
                                entry_open: candle.open,
                                entry_ts_ns: candle_ts,
                                funding_sum_bps: init_funding,
                            },
                            &mut vip0_returns[slot_idx],
                            &mut diag_returns[slot_idx],
                            &mut trades,
                        )?;
                        *slot = None;
                    } else {
                        *slot = Some(MesoPosState::Open {
                            signal_ts_ns,
                            side,
                            entry_open: candle.open,
                            entry_ts_ns: candle_ts,
                            exit_bar,
                            funding_sum_bps: init_funding,
                        });
                    }
                }
                Some(MesoPosState::Open {
                    signal_ts_ns,
                    side,
                    entry_open,
                    entry_ts_ns,
                    exit_bar,
                    mut funding_sum_bps,
                }) if bar_idx <= exit_bar => {
                    funding_sum_bps += candle.funding_rate_bps.raw();
                    if bar_idx == exit_bar {
                        settle_meso_trade(
                            &ctx,
                            CloseFill {
                                signal_ts_ns,
                                side,
                                entry_open,
                                entry_ts_ns,
                                funding_sum_bps,
                            },
                            &mut vip0_returns[slot_idx],
                            &mut diag_returns[slot_idx],
                            &mut trades,
                        )?;
                        *slot = None;
                    } else {
                        *slot = Some(MesoPosState::Open {
                            signal_ts_ns,
                            side,
                            entry_open,
                            entry_ts_ns,
                            exit_bar,
                            funding_sum_bps,
                        });
                    }
                }
                _ => {}
            }
        }

        let bar_out = engine.on_candle(&candle)?;
        for sig in bar_out.signals {
            let h_idx = hypothesis_index(sig.hypothesis);
            for hor_idx in 0..n_hor {
                let slot_idx = h_idx * n_hor + hor_idx;
                if sym.slots[slot_idx].is_none() {
                    sym.slots[slot_idx] = Some(MesoPosState::PendingEntry {
                        signal_ts_ns: sig.ts_ns.raw(),
                        side: sig.side,
                        entry_bar: bar_idx + 1,
                    });
                }
            }
        }
        sym.bar_idx += 1;
    }

    let mut vip0_inputs = Vec::with_capacity(total_cells);
    let mut diag_inputs = Vec::with_capacity(total_cells);
    for (h_idx, hyp) in MesoHypothesis::ALL.iter().enumerate() {
        for (hor_idx, &h_bars) in config.horizons_bars.iter().enumerate() {
            let slot_idx = h_idx * n_hor + hor_idx;
            let h_label = hyp.as_str().to_string();
            let hor_label = format!("{h_bars}b");
            vip0_inputs.push(HypothesisCellInput {
                hypothesis: h_label.clone(),
                horizon: hor_label.clone(),
                returns_bps: core::mem::take(&mut vip0_returns[slot_idx]),
            });
            diag_inputs.push(HypothesisCellInput {
                hypothesis: h_label,
                horizon: hor_label,
                returns_bps: core::mem::take(&mut diag_returns[slot_idx]),
            });
        }
    }

    let vip0 = evaluate_hypothesis_batch(&vip0_inputs, &vip0_sched)?;
    let diag = evaluate_hypothesis_batch(&diag_inputs, &diag_sched)?;
    let report = FalsificationReport {
        track: "Track2-MesoEnergy".to_string(),
        events_processed,
        m_hypotheses: total_cells,
        t_crit: vip0.t_crit,
        sr_cross_std_vip0: vip0.sr_cross_std,
        sr_cross_std_diag: diag.sr_cross_std,
        vip0_cells: vip0.cells,
        diag_zero_cells: diag.cells,
    };
    Ok((report, trades))
}

pub fn run_meso_falsification<R: CandleSeriesReader>(
    reader: &mut R,
    config: &MesoFalsifierConfig,
) -> Result<FalsificationReport, DomainError> {
    let (report, _) = run_meso_falsification_detailed(reader, config)?;
    Ok(report)
}

const fn hypothesis_index(h: MesoHypothesis) -> usize {
    match h {
        MesoHypothesis::VolCompressionBreakout => 0,
        MesoHypothesis::TsmomVolumeWeighted => 1,
        MesoHypothesis::FundingCrowdingReversal => 2,
    }
}
