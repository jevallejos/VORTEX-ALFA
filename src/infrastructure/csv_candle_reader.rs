use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

use crate::domain::error::DomainError;
use crate::domain::ports::CandleSeriesReader;
use crate::domain::types::{Bps, Candle, PriceE8, SymbolId, TimestampNs, BPS_PER_UNIT};

#[derive(Copy, Clone)]
enum TsCol {
    Ns(usize),
    Ms(usize),
}

#[derive(Copy, Clone)]
enum FundingCol {
    Bps(usize),
    Decimal(usize),
}

struct CsvSchema {
    col_count: usize,
    symbol_id: usize,
    ts: TsCol,
    open: usize,
    high: usize,
    low: usize,
    close: usize,
    volume: usize,
    funding: FundingCol,
}

pub struct CsvCandleReader {
    files: Vec<PathBuf>,
    file_idx: usize,
    reader: Option<BufReader<File>>,
    schema: Option<CsvSchema>,
    line_buf: String,
    last_ts_by_symbol: HashMap<u16, i64>,
}

impl CsvCandleReader {
    pub fn open(path: &Path) -> Result<Self, DomainError> {
        let meta = fs::metadata(path)
            .map_err(|e| DomainError::Io(format!("stat {}: {e}", path.display())))?;
        let files = if meta.is_dir() {
            collect_and_sort_csv_files(path)?
        } else {
            vec![path.to_path_buf()]
        };
        Ok(Self {
            files,
            file_idx: 0,
            reader: None,
            schema: None,
            line_buf: String::new(),
            last_ts_by_symbol: HashMap::new(),
        })
    }

    fn open_next_file(&mut self) -> Result<bool, DomainError> {
        self.reader = None;
        self.schema = None;
        if self.file_idx >= self.files.len() {
            return Ok(false);
        }
        let path = &self.files[self.file_idx];
        self.file_idx += 1;
        let file = File::open(path)
            .map_err(|e| DomainError::Io(format!("open {}: {e}", path.display())))?;
        let mut reader = BufReader::new(file);
        self.line_buf.clear();
        let schema = read_header_schema(&mut reader, &mut self.line_buf, path)?;
        self.reader = Some(reader);
        self.schema = Some(schema);
        Ok(true)
    }
}

impl CandleSeriesReader for CsvCandleReader {
    fn next_candle(&mut self) -> Result<Option<Candle>, DomainError> {
        loop {
            if self.reader.is_none() && !self.open_next_file()? {
                return Ok(None);
            }
            let Some(reader) = self.reader.as_mut() else {
                return Ok(None);
            };
            let Some(schema) = self.schema.as_ref() else {
                return Err(DomainError::FormatCorrupted(
                    "missing csv schema".to_string(),
                ));
            };
            self.line_buf.clear();
            let n = reader
                .read_line(&mut self.line_buf)
                .map_err(|e| DomainError::Io(format!("csv read failed: {e}")))?;
            if n == 0 {
                self.reader = None;
                self.schema = None;
                continue;
            }
            let trimmed = self.line_buf.trim();
            if trimmed.is_empty() {
                continue;
            }
            let candle = parse_csv_row(trimmed, schema)?;
            let sym = candle.symbol_id.0;
            let ts = candle.open_ts_ns.raw();
            if let Some(&prev_ns) = self.last_ts_by_symbol.get(&sym) {
                if ts <= prev_ns {
                    return Err(DomainError::NonMonotonicTimestamp {
                        symbol_id: sym,
                        prev_ns,
                        curr_ns: ts,
                    });
                }
            }
            self.last_ts_by_symbol.insert(sym, ts);
            return Ok(Some(candle));
        }
    }
}

fn collect_and_sort_csv_files(dir: &Path) -> Result<Vec<PathBuf>, DomainError> {
    let mut stack = vec![dir.to_path_buf()];
    let mut candidates = Vec::new();
    while let Some(cur) = stack.pop() {
        let entries = fs::read_dir(&cur)
            .map_err(|e| DomainError::Io(format!("read_dir {}: {e}", cur.display())))?;
        for entry_res in entries {
            let entry = entry_res
                .map_err(|e| DomainError::Io(format!("dir entry {}: {e}", cur.display())))?;
            let p = entry.path();
            if p.is_dir() {
                stack.push(p);
            } else if p
                .extension()
                .and_then(|s| s.to_str())
                .is_some_and(|ext| ext.eq_ignore_ascii_case("csv"))
            {
                let first_ts = probe_first_csv_ts(&p)?.unwrap_or(i64::MAX);
                candidates.push((first_ts, p));
            }
        }
    }
    if candidates.is_empty() {
        return Err(DomainError::Io(format!(
            "no .csv files in {}",
            dir.display()
        )));
    }
    candidates.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(&b.1)));
    Ok(candidates.into_iter().map(|(_, p)| p).collect())
}

fn probe_first_csv_ts(path: &Path) -> Result<Option<i64>, DomainError> {
    let file =
        File::open(path).map_err(|e| DomainError::Io(format!("open {}: {e}", path.display())))?;
    let mut reader = BufReader::new(file);
    let mut line = String::new();
    let schema = read_header_schema(&mut reader, &mut line, path)?;
    loop {
        line.clear();
        let n = reader
            .read_line(&mut line)
            .map_err(|e| DomainError::Io(format!("read {}: {e}", path.display())))?;
        if n == 0 {
            return Ok(None);
        }
        let trimmed = line.trim();
        if !trimmed.is_empty() {
            let c = parse_csv_row(trimmed, &schema)?;
            return Ok(Some(c.open_ts_ns.raw()));
        }
    }
}

fn read_header_schema(
    reader: &mut BufReader<File>,
    buf: &mut String,
    path: &Path,
) -> Result<CsvSchema, DomainError> {
    loop {
        buf.clear();
        let n = reader
            .read_line(buf)
            .map_err(|e| DomainError::Io(format!("read {}: {e}", path.display())))?;
        if n == 0 {
            return Err(DomainError::FormatCorrupted(format!(
                "empty csv file {}",
                path.display()
            )));
        }
        let trimmed = buf.trim().trim_start_matches('\u{feff}');
        if !trimmed.is_empty() {
            return parse_header(trimmed);
        }
    }
}

fn parse_header(header_line: &str) -> Result<CsvSchema, DomainError> {
    let cols: Vec<&str> = header_line.split(',').map(str::trim).collect();
    let find = |name: &str| cols.iter().position(|&c| c == name);
    let req = |name: &str| {
        find(name)
            .ok_or_else(|| DomainError::FormatCorrupted(format!("missing csv column '{name}'")))
    };
    let ts = match (find("ts_ns"), find("ts_ms")) {
        (Some(idx), _) => TsCol::Ns(idx),
        (None, Some(idx)) => TsCol::Ms(idx),
        (None, None) => {
            return Err(DomainError::FormatCorrupted(
                "missing csv timestamp column ('ts_ns' or 'ts_ms')".to_string(),
            ));
        }
    };
    let funding = match (find("funding_rate_bps"), find("funding_rate")) {
        (Some(idx), _) => FundingCol::Bps(idx),
        (None, Some(idx)) => FundingCol::Decimal(idx),
        (None, None) => {
            return Err(DomainError::FormatCorrupted(
                "missing csv funding column ('funding_rate_bps' or 'funding_rate')".to_string(),
            ));
        }
    };
    Ok(CsvSchema {
        col_count: cols.len(),
        symbol_id: req("symbol_id")?,
        ts,
        open: req("open")?,
        high: req("high")?,
        low: req("low")?,
        close: req("close")?,
        volume: req("volume")?,
        funding,
    })
}

fn parse_csv_row(line: &str, s: &CsvSchema) -> Result<Candle, DomainError> {
    let cols: Vec<&str> = line.split(',').map(str::trim).collect();
    if cols.len() != s.col_count {
        return Err(DomainError::FormatCorrupted(format!(
            "csv column count mismatch: expected {}, got {}",
            s.col_count,
            cols.len()
        )));
    }
    let sym_raw: u16 = cols[s.symbol_id].parse().map_err(|_| {
        DomainError::FormatCorrupted(format!("invalid symbol_id '{}'", cols[s.symbol_id]))
    })?;
    let ts_ns_raw = match s.ts {
        TsCol::Ns(idx) => cols[idx]
            .parse::<i64>()
            .map_err(|_| DomainError::FormatCorrupted(format!("invalid ts_ns '{}'", cols[idx])))?,
        TsCol::Ms(idx) => {
            let ms = cols[idx].parse::<i64>().map_err(|_| {
                DomainError::FormatCorrupted(format!("invalid ts_ms '{}'", cols[idx]))
            })?;
            ms.checked_mul(1_000_000)
                .ok_or(DomainError::ArithmeticOverflow("csv_ts_ms_to_ns"))?
        }
    };
    let open = parse_price_e8(cols[s.open], "open")?;
    let high = parse_price_e8(cols[s.high], "high")?;
    let low = parse_price_e8(cols[s.low], "low")?;
    let close = parse_price_e8(cols[s.close], "close")?;
    let volume: f64 = cols[s.volume]
        .parse()
        .map_err(|_| DomainError::InvalidCandle(format!("invalid volume '{}'", cols[s.volume])))?;
    if !volume.is_finite() {
        return Err(DomainError::NonFiniteValue("volume"));
    }
    let funding_bps = match s.funding {
        FundingCol::Bps(idx) => {
            let v: f64 = cols[idx].parse().map_err(|_| {
                DomainError::FormatCorrupted(format!("invalid funding '{}'", cols[idx]))
            })?;
            Bps::new(v)?
        }
        FundingCol::Decimal(idx) => {
            let v: f64 = cols[idx].parse().map_err(|_| {
                DomainError::FormatCorrupted(format!("invalid funding '{}'", cols[idx]))
            })?;
            if !v.is_finite() {
                return Err(DomainError::NonFiniteBps);
            }
            Bps::new(v * BPS_PER_UNIT)?
        }
    };
    Candle::new(
        SymbolId(sym_raw),
        TimestampNs::new(ts_ns_raw)?,
        open,
        high,
        low,
        close,
        volume,
        funding_bps,
    )
}

fn parse_price_e8(raw: &str, field: &'static str) -> Result<PriceE8, DomainError> {
    let px: f64 = raw
        .parse()
        .map_err(|_| DomainError::InvalidCandle(format!("invalid {field} '{raw}'")))?;
    if !px.is_finite() {
        return Err(DomainError::NonFiniteValue(field));
    }
    if px <= 0.0 {
        return Err(DomainError::InvalidPrice(0));
    }
    let scaled = (px * 1e8).round();
    if !scaled.is_finite() || scaled > (i64::MAX as f64) || scaled < 1.0 {
        return Err(DomainError::InvalidPrice(0));
    }
    PriceE8::new(scaled as i64)
}
