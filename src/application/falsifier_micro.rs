use std::collections::BTreeMap;

use crate::domain::error::DomainError;
use crate::domain::fees::{book_roundtrip_net_bps, FeeSchedule};
use crate::domain::ports::MarketTapeReader;
use crate::domain::signals::micro_tail::{MicroDetectorConfig, MicroHypothesis, MicroTailEngine};
use crate::domain::stats::{evaluate_hypothesis_batch, FalsificationReport, HypothesisCellInput};
use crate::domain::types::{BookTop, Bps, Side, SymbolId};

#[derive(Clone, Debug)]
pub struct MicroFalsifierConfig {
    pub rtt_ns: i64,
    pub cooldown_ns: i64,
    pub horizons_ns: Vec<i64>,
    pub detector: MicroDetectorConfig,
    pub entry_is_taker: bool,
    pub exit_is_taker: bool,
}

impl Default for MicroFalsifierConfig {
    fn default() -> Self {
        Self {
            rtt_ns: 5_000_000,
            cooldown_ns: 10_000_000_000,
            horizons_ns: vec![
                15_000_000_000,
                60_000_000_000,
                180_000_000_000,
                600_000_000_000,
                900_000_000_000,
            ],
            detector: MicroDetectorConfig::default(),
            entry_is_taker: true,
            exit_is_taker: true,
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct MicroTradeRecord {
    pub symbol_id: SymbolId,
    pub hypothesis: MicroHypothesis,
    pub horizon_ns: i64,
    pub side: Side,
    pub signal_ts_ns: i64,
    pub entry_ts_ns: i64,
    pub exit_ts_ns: i64,
    pub net_bps_vip0: f64,
    pub net_bps_diag: f64,
}

#[derive(Copy, Clone, Debug)]
enum PositionState {
    PendingEntry {
        signal_ts_ns: i64,
        side: Side,
        target_entry_ts_ns: i64,
    },
    OpenPosition {
        signal_ts_ns: i64,
        entry_ts_ns: i64,
        side: Side,
        entry_book: BookTop,
        target_exit_ts_ns: i64,
    },
}

#[derive(Copy, Clone, Debug, Default)]
struct SlotEntry {
    state: Option<PositionState>,
    last_signal_ts_ns: Option<i64>,
}

pub fn run_micro_falsification_detailed<R: MarketTapeReader>(
    reader: &mut R,
    config: &MicroFalsifierConfig,
) -> Result<(FalsificationReport, Vec<MicroTradeRecord>), DomainError> {
    if config.rtt_ns < 0 {
        return Err(DomainError::InvalidStatParam("rtt_ns must be >= 0"));
    }
    if config.cooldown_ns < 0 {
        return Err(DomainError::InvalidStatParam("cooldown_ns must be >= 0"));
    }
    if config.horizons_ns.is_empty() {
        return Err(DomainError::InvalidStatParam(
            "horizons_ns must be non-empty",
        ));
    }
    for &h in &config.horizons_ns {
        if h <= 0 {
            return Err(DomainError::InvalidStatParam("horizon_ns must be > 0"));
        }
    }

    let mut engine = MicroTailEngine::new(config.detector.clone())?;
    let vip0_sched = FeeSchedule::bybit_vip0();
    let diag_sched = FeeSchedule::diag_zero_maker();
    let zero_funding = Bps::new(0.0)?;

    let n_hyp = MicroHypothesis::ALL.len();
    let n_hor = config.horizons_ns.len();
    let total_cells = n_hyp * n_hor;

    let mut vip0_returns = vec![Vec::<f64>::new(); total_cells];
    let mut diag_returns = vec![Vec::<f64>::new(); total_cells];
    let mut symbol_slots: BTreeMap<SymbolId, Vec<SlotEntry>> = BTreeMap::new();
    let mut trades = Vec::new();
    let mut events_processed = 0_u64;

    while let Some(event) = reader.next_event()? {
        events_processed += 1;
        let ev_ts = event.ts_ns.raw();

        if let Some(sym_slots) = symbol_slots.get_mut(&event.symbol_id) {
            for (slot_idx, entry) in sym_slots.iter_mut().enumerate() {
                let hor_idx = slot_idx % n_hor;
                let h_idx = slot_idx / n_hor;
                let horizon_ns = config.horizons_ns[hor_idx];
                let hypothesis = MicroHypothesis::ALL[h_idx];

                match entry.state {
                    Some(PositionState::PendingEntry {
                        signal_ts_ns,
                        side,
                        target_entry_ts_ns,
                    }) if ev_ts >= target_entry_ts_ns => {
                        let target_exit_ts_ns = ev_ts
                            .checked_add(horizon_ns)
                            .ok_or(DomainError::ArithmeticOverflow("micro_target_exit_ts"))?;
                        entry.state = Some(PositionState::OpenPosition {
                            signal_ts_ns,
                            entry_ts_ns: ev_ts,
                            side,
                            entry_book: event.book,
                            target_exit_ts_ns,
                        });
                    }
                    Some(PositionState::OpenPosition {
                        signal_ts_ns,
                        entry_ts_ns,
                        side,
                        entry_book,
                        target_exit_ts_ns,
                    }) if ev_ts >= target_exit_ts_ns => {
                        let net_vip0 = book_roundtrip_net_bps(
                            side,
                            entry_book,
                            event.book,
                            &vip0_sched,
                            config.entry_is_taker,
                            config.exit_is_taker,
                            zero_funding,
                        )?
                        .raw();
                        let net_diag = book_roundtrip_net_bps(
                            side,
                            entry_book,
                            event.book,
                            &diag_sched,
                            config.entry_is_taker,
                            config.exit_is_taker,
                            zero_funding,
                        )?
                        .raw();
                        vip0_returns[slot_idx].push(net_vip0);
                        diag_returns[slot_idx].push(net_diag);
                        trades.push(MicroTradeRecord {
                            symbol_id: event.symbol_id,
                            hypothesis,
                            horizon_ns,
                            side,
                            signal_ts_ns,
                            entry_ts_ns,
                            exit_ts_ns: ev_ts,
                            net_bps_vip0: net_vip0,
                            net_bps_diag: net_diag,
                        });
                        entry.state = None;
                    }
                    _ => {}
                }
            }
        }

        let signals = engine.on_event(&event)?;
        for sig in signals {
            let sym_slots = symbol_slots
                .entry(sig.symbol_id)
                .or_insert_with(|| vec![SlotEntry::default(); total_cells]);
            let h_idx = hypothesis_index(sig.hypothesis);
            let sig_ts = sig.ts_ns.raw();

            for hor_idx in 0..n_hor {
                let slot_idx = h_idx * n_hor + hor_idx;
                let entry = &mut sym_slots[slot_idx];
                let cooldown_ok = match entry.last_signal_ts_ns {
                    None => true,
                    Some(prev) => {
                        let min_next = prev
                            .checked_add(config.cooldown_ns)
                            .ok_or(DomainError::ArithmeticOverflow("micro_cooldown_ts"))?;
                        sig_ts >= min_next
                    }
                };
                if entry.state.is_none() && cooldown_ok {
                    let target_entry_ts_ns = sig_ts
                        .checked_add(config.rtt_ns)
                        .ok_or(DomainError::ArithmeticOverflow("micro_target_entry_ts"))?;
                    entry.last_signal_ts_ns = Some(sig_ts);
                    entry.state = Some(PositionState::PendingEntry {
                        signal_ts_ns: sig_ts,
                        side: sig.side,
                        target_entry_ts_ns,
                    });
                }
            }
        }
    }

    let mut vip0_inputs = Vec::with_capacity(total_cells);
    let mut diag_inputs = Vec::with_capacity(total_cells);
    for (h_idx, hyp) in MicroHypothesis::ALL.iter().enumerate() {
        for (hor_idx, &hor_ns) in config.horizons_ns.iter().enumerate() {
            let slot_idx = h_idx * n_hor + hor_idx;
            let h_label = hyp.as_str().to_string();
            let hor_label = format_horizon_ns(hor_ns);
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
        track: "Track1-MicroTail".to_string(),
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

pub fn run_micro_falsification<R: MarketTapeReader>(
    reader: &mut R,
    config: &MicroFalsifierConfig,
) -> Result<FalsificationReport, DomainError> {
    let (report, _) = run_micro_falsification_detailed(reader, config)?;
    Ok(report)
}

const fn hypothesis_index(h: MicroHypothesis) -> usize {
    match h {
        MicroHypothesis::HawkesTail => 0,
        MicroHypothesis::ShiryaevRobertsJump => 1,
        MicroHypothesis::LiquidationCascade => 2,
        MicroHypothesis::CrossExchangeShock => 3,
    }
}

fn format_horizon_ns(ns: i64) -> String {
    if ns % 1_000_000_000 == 0 {
        format!("{}s", ns / 1_000_000_000)
    } else if ns % 1_000_000 == 0 {
        format!("{}ms", ns / 1_000_000)
    } else {
        format!("{ns}ns")
    }
}
