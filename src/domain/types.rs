use serde::{Deserialize, Serialize};

use crate::domain::error::DomainError;

/// Fixed-point 1e8 integer scale factor.
pub const SCALE_E8: i128 = 100_000_000;

/// Basis points per unit return (10,000 bps = 100%).
pub const BPS_PER_UNIT: f64 = 10_000.0;

#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct PriceE8(i64);

impl PriceE8 {
    pub fn new(raw: i64) -> Result<Self, DomainError> {
        if raw <= 0 {
            return Err(DomainError::InvalidPrice(raw));
        }
        Ok(Self(raw))
    }

    #[must_use]
    pub const fn raw(self) -> i64 {
        self.0
    }

    #[must_use]
    pub fn as_f64(self) -> f64 {
        (self.0 as f64) / (SCALE_E8 as f64)
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct QtyE8(i64);

impl QtyE8 {
    pub fn new(raw: i64) -> Result<Self, DomainError> {
        if raw <= 0 {
            return Err(DomainError::InvalidQty(raw));
        }
        Ok(Self(raw))
    }

    #[must_use]
    pub const fn raw(self) -> i64 {
        self.0
    }

    #[must_use]
    pub fn as_f64(self) -> f64 {
        (self.0 as f64) / (SCALE_E8 as f64)
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct NotionalE8(i128);

impl NotionalE8 {
    pub fn new(raw: i128) -> Result<Self, DomainError> {
        if raw < 0 {
            return Err(DomainError::InvalidNotional(raw));
        }
        Ok(Self(raw))
    }

    pub fn from_price_qty(
        price: PriceE8,
        qty: QtyE8,
        min_notional_e8: i128,
    ) -> Result<Self, DomainError> {
        let prod = i128::from(price.raw())
            .checked_mul(i128::from(qty.raw()))
            .ok_or(DomainError::ArithmeticOverflow("notional_mul"))?;
        let notional_e8 = prod / SCALE_E8;
        if notional_e8 < min_notional_e8 {
            return Err(DomainError::DustNotional {
                notional_e8,
                min_notional_e8,
            });
        }
        Ok(Self(notional_e8))
    }

    #[must_use]
    pub const fn raw(self) -> i128 {
        self.0
    }
}

#[derive(Copy, Clone, Debug, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct Bps(f64);

impl Bps {
    pub const ZERO: Self = Self(0.0);

    pub fn new(val: f64) -> Result<Self, DomainError> {
        if !val.is_finite() {
            return Err(DomainError::NonFiniteBps);
        }
        Ok(Self(val))
    }

    #[must_use]
    pub const fn raw(self) -> f64 {
        self.0
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct TimestampNs(i64);

impl TimestampNs {
    pub fn new(ns: i64) -> Result<Self, DomainError> {
        if ns < 0 {
            return Err(DomainError::InvalidTimestamp(ns));
        }
        Ok(Self(ns))
    }

    #[must_use]
    pub const fn raw(self) -> i64 {
        self.0
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Side {
    Long,
    Short,
}

impl Side {
    #[must_use]
    pub const fn sign_f64(self) -> f64 {
        match self {
            Self::Long => 1.0,
            Self::Short => -1.0,
        }
    }

    #[must_use]
    pub const fn sign_i128(self) -> i128 {
        match self {
            Self::Long => 1,
            Self::Short => -1,
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct SymbolId(pub u16);

#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BookTop {
    bid: PriceE8,
    ask: PriceE8,
    bid_qty: QtyE8,
    ask_qty: QtyE8,
}

impl BookTop {
    pub fn new(
        bid: PriceE8,
        ask: PriceE8,
        bid_qty: QtyE8,
        ask_qty: QtyE8,
    ) -> Result<Self, DomainError> {
        if ask.raw() < bid.raw() {
            return Err(DomainError::CrossedBook {
                bid_e8: bid.raw(),
                ask_e8: ask.raw(),
            });
        }
        Ok(Self {
            bid,
            ask,
            bid_qty,
            ask_qty,
        })
    }

    #[must_use]
    pub const fn bid(self) -> PriceE8 {
        self.bid
    }

    #[must_use]
    pub const fn ask(self) -> PriceE8 {
        self.ask
    }

    #[must_use]
    pub const fn bid_qty(self) -> QtyE8 {
        self.bid_qty
    }

    #[must_use]
    pub const fn ask_qty(self) -> QtyE8 {
        self.ask_qty
    }

    #[must_use]
    pub fn mid_f64(self) -> f64 {
        0.5 * (self.bid.as_f64() + self.ask.as_f64())
    }

    #[must_use]
    pub fn spread_bps(self) -> f64 {
        let mid = self.mid_f64();
        ((self.ask.as_f64() - self.bid.as_f64()) / mid) * BPS_PER_UNIT
    }
}

#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum FundingTimestampSemantics {
    #[default]
    BarOpenSettlement,
    BarCloseSettlement,
}

#[derive(Copy, Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Candle {
    pub symbol_id: SymbolId,
    pub open_ts_ns: TimestampNs,
    pub open: PriceE8,
    pub high: PriceE8,
    pub low: PriceE8,
    pub close: PriceE8,
    pub volume: f64,
    pub funding_rate_bps: Bps,
}

impl Candle {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        symbol_id: SymbolId,
        open_ts_ns: TimestampNs,
        open: PriceE8,
        high: PriceE8,
        low: PriceE8,
        close: PriceE8,
        volume: f64,
        funding_rate_bps: Bps,
    ) -> Result<Self, DomainError> {
        let max_oc = open.raw().max(close.raw());
        let min_oc = open.raw().min(close.raw());
        if high.raw() < max_oc || low.raw() > min_oc {
            return Err(DomainError::InvalidCandle(format!(
                "OHLC bounds violated: O={}, H={}, L={}, C={}",
                open.raw(),
                high.raw(),
                low.raw(),
                close.raw()
            )));
        }
        if !volume.is_finite() || volume < 0.0 {
            return Err(DomainError::InvalidCandle(format!(
                "invalid volume: {volume}"
            )));
        }
        Ok(Self {
            symbol_id,
            open_ts_ns,
            open,
            high,
            low,
            close,
            volume,
            funding_rate_bps,
        })
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum TapeEventKind {
    BookSnapshot,
    Trade,
    TickerMeta,
    Liquidation,
}

#[derive(Copy, Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TapeEvent {
    pub symbol_id: SymbolId,
    pub ts_ns: TimestampNs,
    pub kind: TapeEventKind,
    pub side: Side,
    pub book: BookTop,
    pub trade_price: Option<PriceE8>,
    pub trade_qty: Option<QtyE8>,
    pub l1_to_l5_notional: f64,
    pub binance_mid: Option<PriceE8>,
}

impl TapeEvent {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        symbol_id: SymbolId,
        ts_ns: TimestampNs,
        kind: TapeEventKind,
        side: Side,
        book: BookTop,
        trade_price: Option<PriceE8>,
        trade_qty: Option<QtyE8>,
        l1_to_l5_notional: f64,
        binance_mid: Option<PriceE8>,
    ) -> Result<Self, DomainError> {
        if !l1_to_l5_notional.is_finite() || l1_to_l5_notional < 0.0 {
            return Err(DomainError::NonFiniteValue("l1_to_l5_notional"));
        }
        Ok(Self {
            symbol_id,
            ts_ns,
            kind,
            side,
            book,
            trade_price,
            trade_qty,
            l1_to_l5_notional,
            binance_mid,
        })
    }

    #[must_use]
    pub fn event_notional(&self) -> f64 {
        match (self.trade_price, self.trade_qty) {
            (Some(px), Some(qty)) => px.as_f64() * qty.as_f64(),
            _ => 0.0,
        }
    }
}
