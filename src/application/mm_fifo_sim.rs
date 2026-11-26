use std::collections::BTreeMap;

use crate::application::mm_markout::MarkoutTracker;
pub use crate::application::mm_markout::{
    MmFill, MmSimMetrics, MARKOUT_15S_NS, MARKOUT_1S_NS, MARKOUT_5S_NS,
};
use crate::domain::error::DomainError;
use crate::domain::types::{
    BookTop, PriceE8, QtyE8, Side, SymbolId, TapeEvent, TapeEventKind, BPS_PER_UNIT,
};

pub const DEFAULT_RTT_NS: i64 = 5_000_000;
pub const DEFAULT_MIN_ORDER_LIFE_NS: i64 = 100_000_000;
const BEHIND_L1_QUEUE_E8: i64 = i64::MAX / 4;

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct MmFifoConfig {
    pub rtt_ns: i64,
    pub requote_threshold_bps: f64,
    pub min_order_life_ns: i64,
}

impl Default for MmFifoConfig {
    fn default() -> Self {
        Self {
            rtt_ns: DEFAULT_RTT_NS,
            requote_threshold_bps: 0.5,
            min_order_life_ns: DEFAULT_MIN_ORDER_LIFE_NS,
        }
    }
}

impl MmFifoConfig {
    pub fn validate(&self) -> Result<(), DomainError> {
        let valid = self.rtt_ns >= 0
            && self.min_order_life_ns >= 0
            && self.requote_threshold_bps.is_finite()
            && self.requote_threshold_bps >= 0.0;
        valid
            .then_some(())
            .ok_or(DomainError::InvalidStatParam("mm_fifo_config"))
    }
}

#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum OrderState {
    #[default]
    Idle,
    PendingPlace {
        target_px: PriceE8,
        qty_e8: QtyE8,
        active_at_ns: i64,
    },
    Resting {
        px: PriceE8,
        qty_e8: QtyE8,
        q_ahead_e8: i64,
        placed_ts_ns: i64,
    },
    PendingReplace {
        current_px: PriceE8,
        current_qty_e8: QtyE8,
        current_q_ahead_e8: i64,
        new_px: PriceE8,
        new_qty_e8: QtyE8,
        active_at_ns: i64,
    },
    PendingCancel {
        current_px: PriceE8,
        current_qty_e8: QtyE8,
        current_q_ahead_e8: i64,
        active_at_ns: i64,
    },
}

#[derive(Copy, Clone, Debug)]
struct OrderTicket {
    px: PriceE8,
    qty_e8: QtyE8,
    active_at_ns: i64,
}

#[derive(Copy, Clone, Debug)]
struct LiveOrder {
    px: PriceE8,
    qty_e8: QtyE8,
    q_ahead_e8: i64,
    placed_ts_ns: i64,
}

#[derive(Copy, Clone, Debug, Default)]
enum SideSlot {
    #[default]
    Idle,
    PendingPlace(OrderTicket),
    Resting(LiveOrder),
    PendingReplace(LiveOrder, OrderTicket),
    PendingCancel(LiveOrder, i64),
}

impl SideSlot {
    fn to_order_state(self) -> OrderState {
        match self {
            Self::Idle => OrderState::Idle,
            Self::PendingPlace(ticket) => OrderState::PendingPlace {
                target_px: ticket.px,
                qty_e8: ticket.qty_e8,
                active_at_ns: ticket.active_at_ns,
            },
            Self::Resting(order) => OrderState::Resting {
                px: order.px,
                qty_e8: order.qty_e8,
                q_ahead_e8: order.q_ahead_e8,
                placed_ts_ns: order.placed_ts_ns,
            },
            Self::PendingReplace(order, ticket) => OrderState::PendingReplace {
                current_px: order.px,
                current_qty_e8: order.qty_e8,
                current_q_ahead_e8: order.q_ahead_e8,
                new_px: ticket.px,
                new_qty_e8: ticket.qty_e8,
                active_at_ns: ticket.active_at_ns,
            },
            Self::PendingCancel(order, active_at_ns) => OrderState::PendingCancel {
                current_px: order.px,
                current_qty_e8: order.qty_e8,
                current_q_ahead_e8: order.q_ahead_e8,
                active_at_ns,
            },
        }
    }
}

#[derive(Clone, Debug, Default)]
struct SymbolBookState {
    prev_ts_ns: Option<i64>,
    bid: SideSlot,
    ask: SideSlot,
    markouts: MarkoutTracker,
}

/// Conservative FIFO L1 queue simulator with 5 ms RTT place/replace/cancel latency.
#[derive(Clone, Debug, Default)]
pub struct MmFifoSimulator {
    config: MmFifoConfig,
    metrics: MmSimMetrics,
    symbols: BTreeMap<SymbolId, SymbolBookState>,
}

impl MmFifoSimulator {
    pub fn new(config: MmFifoConfig) -> Result<Self, DomainError> {
        config.validate()?;
        Ok(Self {
            config,
            ..Self::default()
        })
    }

    pub fn on_event(&mut self, event: &TapeEvent) -> Result<Vec<MmFill>, DomainError> {
        let ts_ns = event.ts_ns.raw();
        let sym_state = self.symbols.entry(event.symbol_id).or_default();
        if let Some(prev_ns) = sym_state.prev_ts_ns.filter(|&prev| ts_ns < prev) {
            return Err(DomainError::NonMonotonicTimestamp {
                symbol_id: event.symbol_id.0,
                prev_ns,
                curr_ns: ts_ns,
            });
        }
        sym_state.prev_ts_ns = Some(ts_ns);

        let mid = event.book.mid_f64();
        let mut fills = Vec::new();
        for (side, slot) in [
            (Side::Long, &mut sym_state.bid),
            (Side::Short, &mut sym_state.ask),
        ] {
            let (live_order, is_adverse) = match slot {
                SideSlot::PendingPlace(ticket) if ts_ns >= ticket.active_at_ns => {
                    *slot = activate_order(&mut self.metrics, side, *ticket, ts_ns, event.book);
                    continue;
                }
                SideSlot::PendingReplace(_, ticket) if ts_ns >= ticket.active_at_ns => {
                    *slot = activate_order(&mut self.metrics, side, *ticket, ts_ns, event.book);
                    continue;
                }
                SideSlot::PendingCancel(_, active_at_ns) if ts_ns >= *active_at_ns => {
                    *slot = SideSlot::Idle;
                    continue;
                }
                SideSlot::Resting(live) => (live, false),
                SideSlot::PendingCancel(live, _) | SideSlot::PendingReplace(live, _) => {
                    (live, true)
                }
                _ => continue,
            };
            if check_side_fill(
                side,
                live_order.px,
                live_order.qty_e8,
                &mut live_order.q_ahead_e8,
                event,
            ) {
                let fill = sym_state.markouts.record_fill(
                    &mut self.metrics,
                    event,
                    side,
                    live_order.px,
                    live_order.qty_e8,
                    mid,
                    is_adverse,
                );
                fills.push(fill);
                *slot = SideSlot::Idle;
            }
        }
        sym_state.markouts.evaluate(&mut self.metrics, ts_ns, mid);
        Ok(fills)
    }

    pub fn on_quote_decision(
        &mut self,
        symbol_id: SymbolId,
        ts_ns: i64,
        bid: Option<(PriceE8, QtyE8)>,
        ask: Option<(PriceE8, QtyE8)>,
    ) -> Result<(), DomainError> {
        if ts_ns < 0 {
            return Err(DomainError::InvalidTimestamp(ts_ns));
        }
        let active_at_ns = ts_ns
            .checked_add(self.config.rtt_ns)
            .ok_or(DomainError::ArithmeticOverflow("mm_rtt_ts"))?;
        let sym_state = self.symbols.entry(symbol_id).or_default();
        for (slot, target) in [(&mut sym_state.bid, bid), (&mut sym_state.ask, ask)] {
            match (*slot, target) {
                (SideSlot::Idle, Some((px, qty_e8))) => {
                    *slot = SideSlot::PendingPlace(OrderTicket {
                        px,
                        qty_e8,
                        active_at_ns,
                    });
                }
                (SideSlot::Resting(live), None) => {
                    *slot = SideSlot::PendingCancel(live, active_at_ns);
                }
                (SideSlot::Resting(live), Some((new_px, new_qty_e8)))
                    if new_px != live.px
                        && ts_ns.saturating_sub(live.placed_ts_ns)
                            >= self.config.min_order_life_ns =>
                {
                    let diff_bps = ((new_px.as_f64() - live.px.as_f64()).abs() / live.px.as_f64())
                        * BPS_PER_UNIT;
                    if diff_bps >= self.config.requote_threshold_bps {
                        let next = OrderTicket {
                            px: new_px,
                            qty_e8: new_qty_e8,
                            active_at_ns,
                        };
                        *slot = SideSlot::PendingReplace(live, next);
                    }
                }
                _ => {}
            }
        }
        Ok(())
    }

    #[must_use]
    pub fn order_states(&self, symbol_id: SymbolId) -> Option<(OrderState, OrderState)> {
        self.symbols
            .get(&symbol_id)
            .map(|s| (s.bid.to_order_state(), s.ask.to_order_state()))
    }

    #[must_use]
    pub const fn metrics(&self) -> &MmSimMetrics {
        &self.metrics
    }
}

fn activate_order(
    metrics: &mut MmSimMetrics,
    side: Side,
    ticket: OrderTicket,
    ts_ns: i64,
    book: BookTop,
) -> SideSlot {
    let (crosses_book, q_ahead_e8) = evaluate_book_queue(side, ticket.px, book);
    if crosses_book {
        metrics.post_only_rejects += 1;
        SideSlot::Idle
    } else {
        SideSlot::Resting(LiveOrder {
            px: ticket.px,
            qty_e8: ticket.qty_e8,
            q_ahead_e8,
            placed_ts_ns: ts_ns,
        })
    }
}

fn evaluate_book_queue(side: Side, px: PriceE8, book: BookTop) -> (bool, i64) {
    let dir = side.sign_i128();
    let (own_px, opp_px, own_qty) = match side {
        Side::Long => (book.bid().raw(), book.ask().raw(), book.bid_qty().raw()),
        Side::Short => (book.ask().raw(), book.bid().raw(), book.ask_qty().raw()),
    };
    let px_i128 = i128::from(px.raw());
    let crosses_book = dir * px_i128 >= dir * i128::from(opp_px);
    let q_ahead_e8 = if dir * px_i128 > dir * i128::from(own_px) {
        0
    } else if px.raw() == own_px {
        own_qty
    } else {
        BEHIND_L1_QUEUE_E8
    };
    (crosses_book, q_ahead_e8)
}

fn check_side_fill(
    side: Side,
    px: PriceE8,
    qty_e8: QtyE8,
    q_ahead_e8: &mut i64,
    event: &TapeEvent,
) -> bool {
    let (book_crossed, l1_queue_cap) = evaluate_book_queue(side, px, event.book);
    if book_crossed {
        return true;
    }
    *q_ahead_e8 = (*q_ahead_e8).min(l1_queue_cap);
    if let (TapeEventKind::Trade, Some(trade_px), Some(trade_qty)) =
        (event.kind, event.trade_price, event.trade_qty)
    {
        if side.sign_i128() * i128::from(trade_px.raw()) < side.sign_i128() * i128::from(px.raw()) {
            return true;
        }
        if trade_px == px && event.side != side {
            let order_qty_i128 = i128::from(qty_e8.raw());
            let remaining = i128::from(*q_ahead_e8) - i128::from(trade_qty.raw());
            *q_ahead_e8 = remaining.clamp(-order_qty_i128, i128::from(BEHIND_L1_QUEUE_E8)) as i64;
            return remaining <= -order_qty_i128;
        }
    }
    false
}
