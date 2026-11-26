use thiserror::Error;

/// Typed domain, codec, and CLI errors across all Hexagonal layers.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum DomainError {
    #[error("invalid price e8 (must be > 0): {0}")]
    InvalidPrice(i64),
    #[error("invalid qty e8 (must be > 0): {0}")]
    InvalidQty(i64),
    #[error("invalid notional e8 (must be >= 0): {0}")]
    InvalidNotional(i128),
    #[error("non-finite bps value")]
    NonFiniteBps,
    #[error("invalid fee bps (must be finite and >= 0)")]
    InvalidFeeBps,
    #[error("invalid timestamp ns (must be >= 0): {0}")]
    InvalidTimestamp(i64),
    #[error("non-monotonic timestamp for symbol {symbol_id}: prev={prev_ns}, curr={curr_ns}")]
    NonMonotonicTimestamp {
        symbol_id: u16,
        prev_ns: i64,
        curr_ns: i64,
    },
    #[error("crossed book: bid_e8={bid_e8} > ask_e8={ask_e8}")]
    CrossedBook { bid_e8: i64, ask_e8: i64 },
    #[error("invalid candle: {0}")]
    InvalidCandle(String),
    #[error("arithmetic overflow in {0}")]
    ArithmeticOverflow(&'static str),
    #[error("dust notional {notional_e8} < min_notional {min_notional_e8}")]
    DustNotional {
        notional_e8: i128,
        min_notional_e8: i128,
    },
    #[error("non-finite value in {0}")]
    NonFiniteValue(&'static str),
    #[error("invalid statistical parameter: {0}")]
    InvalidStatParam(&'static str),
    #[error("corrupted format: {0}")]
    FormatCorrupted(String),
    #[error("io error: {0}")]
    Io(String),
    #[error("cli error: {0}")]
    Cli(String),
}
