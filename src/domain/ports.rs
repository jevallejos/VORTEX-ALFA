use crate::domain::error::DomainError;
use crate::domain::stats::FalsificationReport;
use crate::domain::types::{Candle, TapeEvent};

/// Streaming input port for Track 1 market microstructure events.
pub trait MarketTapeReader {
    fn next_event(&mut self) -> Result<Option<TapeEvent>, DomainError>;
}

/// Streaming input port for Track 2 mesoscopic OHLCV + funding bars.
pub trait CandleSeriesReader {
    fn next_candle(&mut self) -> Result<Option<Candle>, DomainError>;
}

/// Output port for persisting falsification reports.
pub trait ReportSink {
    fn write_report(&mut self, report: &FalsificationReport) -> Result<(), DomainError>;
}
