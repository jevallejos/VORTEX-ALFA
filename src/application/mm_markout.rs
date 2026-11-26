use std::collections::VecDeque;

use crate::domain::types::{PriceE8, QtyE8, Side, SymbolId, TapeEvent, BPS_PER_UNIT};

pub const MARKOUT_1S_NS: i64 = 1_000_000_000;
pub const MARKOUT_5S_NS: i64 = 5_000_000_000;
pub const MARKOUT_15S_NS: i64 = 15_000_000_000;
const MARKOUT_HORIZONS_NS: [i64; 3] = [MARKOUT_1S_NS, MARKOUT_5S_NS, MARKOUT_15S_NS];

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct MmFill {
    pub fill_id: u64,
    pub symbol_id: SymbolId,
    pub side: Side,
    pub fill_px: PriceE8,
    pub qty: QtyE8,
    pub fill_ts_ns: i64,
    pub mid_at_fill: f64,
    pub spread_capture_bps: f64,
    pub is_adverse_in_flight: bool,
}

#[derive(Copy, Clone, Debug, Default, PartialEq)]
struct RunningMean {
    sum: f64,
    count: u64,
}

impl RunningMean {
    fn record(&mut self, value: f64) {
        self.sum += value;
        self.count += 1;
    }

    #[must_use]
    fn mean(self) -> Option<f64> {
        (self.count > 0).then(|| self.sum / (self.count as f64))
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct MmSimMetrics {
    pub fills_total: u64,
    pub post_only_rejects: u64,
    pub adverse_in_flight_fills: u64,
    spread_capture: RunningMean,
    markout_horizons: [RunningMean; 3],
    lvr_horizons: [RunningMean; 3],
}

impl MmSimMetrics {
    #[must_use]
    pub fn mean_spread_capture_bps(&self) -> Option<f64> {
        self.spread_capture.mean()
    }
    #[must_use]
    pub fn mean_markout_1s_bps(&self) -> Option<f64> {
        self.markout_horizons[0].mean()
    }
    #[must_use]
    pub fn mean_markout_5s_bps(&self) -> Option<f64> {
        self.markout_horizons[1].mean()
    }
    #[must_use]
    pub fn mean_markout_15s_bps(&self) -> Option<f64> {
        self.markout_horizons[2].mean()
    }
    #[must_use]
    pub fn mean_lvr_1s_bps(&self) -> Option<f64> {
        self.lvr_horizons[0].mean()
    }
    #[must_use]
    pub fn mean_lvr_5s_bps(&self) -> Option<f64> {
        self.lvr_horizons[1].mean()
    }
    #[must_use]
    pub fn mean_lvr_15s_bps(&self) -> Option<f64> {
        self.lvr_horizons[2].mean()
    }
}

#[derive(Copy, Clone, Debug)]
struct PendingMarkout {
    side: Side,
    fill_px: f64,
    fill_ts_ns: i64,
    done: [bool; 3],
}

/// Causal per-symbol 1s / 5s / 15s markout and Loss-Versus-Rebalancing (LVR) tracker.
#[derive(Clone, Debug, Default)]
pub(crate) struct MarkoutTracker {
    pending: VecDeque<PendingMarkout>,
}

impl MarkoutTracker {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn record_fill(
        &mut self,
        metrics: &mut MmSimMetrics,
        event: &TapeEvent,
        side: Side,
        fill_px: PriceE8,
        qty: QtyE8,
        mid_at_fill: f64,
        is_adverse_in_flight: bool,
    ) -> MmFill {
        let px_f64 = fill_px.as_f64();
        let spread_capture_bps = side.sign_f64() * ((mid_at_fill - px_f64) / px_f64) * BPS_PER_UNIT;
        let fill_ts_ns = event.ts_ns.raw();
        metrics.fills_total += 1;
        metrics.adverse_in_flight_fills += u64::from(is_adverse_in_flight);
        metrics.spread_capture.record(spread_capture_bps);
        self.pending.push_back(PendingMarkout {
            side,
            fill_px: px_f64,
            fill_ts_ns,
            done: [false; 3],
        });
        MmFill {
            fill_id: metrics.fills_total,
            symbol_id: event.symbol_id,
            side,
            fill_px,
            qty,
            fill_ts_ns,
            mid_at_fill,
            spread_capture_bps,
            is_adverse_in_flight,
        }
    }

    pub(crate) fn evaluate(&mut self, metrics: &mut MmSimMetrics, ts_ns: i64, mid_now: f64) {
        for entry in self.pending.iter_mut() {
            let elapsed_ns = ts_ns.saturating_sub(entry.fill_ts_ns);
            if elapsed_ns < MARKOUT_1S_NS {
                break;
            }
            let markout_bps =
                entry.side.sign_f64() * ((mid_now - entry.fill_px) / entry.fill_px) * BPS_PER_UNIT;
            let lvr_bps = (-markout_bps).max(0.0);
            for (idx, &horizon_ns) in MARKOUT_HORIZONS_NS.iter().enumerate() {
                if !entry.done[idx] && elapsed_ns >= horizon_ns {
                    entry.done[idx] = true;
                    metrics.markout_horizons[idx].record(markout_bps);
                    metrics.lvr_horizons[idx].record(lvr_bps);
                }
            }
        }
        while self
            .pending
            .front()
            .is_some_and(|entry| entry.done == [true, true, true])
        {
            self.pending.pop_front();
        }
    }
}
