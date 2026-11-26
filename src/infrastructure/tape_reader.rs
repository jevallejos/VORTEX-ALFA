use std::collections::VecDeque;
use std::fs::{self, File};
use std::io::{self, BufRead, BufReader, Read};
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::domain::error::DomainError;
use crate::domain::ports::MarketTapeReader;
use crate::domain::types::{
    BookTop, PriceE8, QtyE8, Side, SymbolId, TapeEvent, TapeEventKind, TimestampNs,
};
use crate::infrastructure::binary_codec::{
    decode_par2_frame, read_next_vpz1_frame, PAR2_MAGIC, PAR2_ROW_BYTES,
};

const PAR2_FRAME_OVERHEAD: usize = 20;
const MAX_PAR2_FRAME_ROWS: usize = 65_536;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum TapeFormat {
    Par2,
    Vpz1,
    Jsonl,
}

#[derive(Deserialize)]
struct JsonlTapeRow {
    symbol_id: u16,
    ts_ns: i64,
    kind: String,
    side: String,
    bid_e8: i64,
    ask_e8: i64,
    bid_qty_e8: i64,
    ask_qty_e8: i64,
    trade_price_e8: Option<i64>,
    trade_qty_e8: Option<i64>,
    l1_to_l5_notional: f64,
    binance_mid_e8: Option<i64>,
}

pub struct StreamingTapeReader {
    format: TapeFormat,
    files: Vec<PathBuf>,
    file_idx: usize,
    reader: Option<BufReader<File>>,
    buffer: VecDeque<TapeEvent>,
    line_buf: String,
}

impl StreamingTapeReader {
    pub fn open(path: &Path, format: TapeFormat) -> Result<Self, DomainError> {
        let meta = fs::metadata(path)
            .map_err(|e| DomainError::Io(format!("stat {}: {e}", path.display())))?;
        let files = if meta.is_dir() {
            collect_and_sort_files(path, format)?
        } else {
            vec![path.to_path_buf()]
        };
        Ok(Self {
            format,
            files,
            file_idx: 0,
            reader: None,
            buffer: VecDeque::new(),
            line_buf: String::new(),
        })
    }

    fn open_next_file(&mut self) -> Result<bool, DomainError> {
        self.reader = None;
        if self.file_idx >= self.files.len() {
            return Ok(false);
        }
        let path = &self.files[self.file_idx];
        self.file_idx += 1;
        let file = File::open(path)
            .map_err(|e| DomainError::Io(format!("open {}: {e}", path.display())))?;
        self.reader = Some(BufReader::new(file));
        Ok(true)
    }

    fn refill_from_current(&mut self) -> Result<bool, DomainError> {
        loop {
            if self.reader.is_none() && !self.open_next_file()? {
                return Ok(false);
            }
            let Some(reader) = self.reader.as_mut() else {
                return Ok(false);
            };
            match self.format {
                TapeFormat::Par2 => match read_next_par2_frame(reader)? {
                    Some(frame) => {
                        for row in decode_par2_frame(&frame)? {
                            self.buffer.push_back(row.to_tape_event()?);
                        }
                        if !self.buffer.is_empty() {
                            return Ok(true);
                        }
                    }
                    None => self.reader = None,
                },
                TapeFormat::Vpz1 => match read_next_vpz1_frame(reader)? {
                    Some((_seq, raw)) => {
                        for row in decode_par2_frame(&raw)? {
                            self.buffer.push_back(row.to_tape_event()?);
                        }
                        if !self.buffer.is_empty() {
                            return Ok(true);
                        }
                    }
                    None => self.reader = None,
                },
                TapeFormat::Jsonl => {
                    self.line_buf.clear();
                    let read = reader
                        .read_line(&mut self.line_buf)
                        .map_err(|e| DomainError::Io(format!("jsonl read failed: {e}")))?;
                    if read == 0 {
                        self.reader = None;
                        continue;
                    }
                    let trimmed = self.line_buf.trim();
                    if trimmed.is_empty() {
                        continue;
                    }
                    self.buffer.push_back(parse_jsonl_line(trimmed)?);
                    return Ok(true);
                }
            }
        }
    }
}

impl MarketTapeReader for StreamingTapeReader {
    fn next_event(&mut self) -> Result<Option<TapeEvent>, DomainError> {
        if let Some(ev) = self.buffer.pop_front() {
            return Ok(Some(ev));
        }
        if !self.refill_from_current()? {
            return Ok(None);
        }
        Ok(self.buffer.pop_front())
    }
}

fn collect_and_sort_files(dir: &Path, format: TapeFormat) -> Result<Vec<PathBuf>, DomainError> {
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
            } else if matches_format(&p, format) {
                let first_ts = probe_first_ts(&p, format)?.unwrap_or(i64::MAX);
                candidates.push((first_ts, p));
            }
        }
    }
    if candidates.is_empty() {
        return Err(DomainError::Io(format!(
            "no matching {:?} files in {}",
            format,
            dir.display()
        )));
    }
    candidates.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(&b.1)));
    Ok(candidates.into_iter().map(|(_, p)| p).collect())
}

fn matches_format(path: &Path, format: TapeFormat) -> bool {
    let Some(ext) = path.extension().and_then(|s| s.to_str()) else {
        return false;
    };
    match format {
        TapeFormat::Par2 => ext.eq_ignore_ascii_case("par2") || ext.eq_ignore_ascii_case("parquet"),
        TapeFormat::Vpz1 => ext.eq_ignore_ascii_case("par2z"),
        TapeFormat::Jsonl => ext.eq_ignore_ascii_case("jsonl"),
    }
}

fn probe_first_ts(path: &Path, format: TapeFormat) -> Result<Option<i64>, DomainError> {
    let file =
        File::open(path).map_err(|e| DomainError::Io(format!("open {}: {e}", path.display())))?;
    let mut reader = BufReader::new(file);
    match format {
        TapeFormat::Par2 => {
            while let Some(frame) = read_next_par2_frame(&mut reader)? {
                let rows = decode_par2_frame(&frame)?;
                if let Some(first) = rows.first() {
                    return Ok(Some(first.ts_local_ns.max(first.ts_exchange_ns)));
                }
            }
            Ok(None)
        }
        TapeFormat::Vpz1 => {
            while let Some((_seq, raw)) = read_next_vpz1_frame(&mut reader)? {
                let rows = decode_par2_frame(&raw)?;
                if let Some(first) = rows.first() {
                    return Ok(Some(first.ts_local_ns.max(first.ts_exchange_ns)));
                }
            }
            Ok(None)
        }
        TapeFormat::Jsonl => {
            let mut line = String::new();
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
                    let ev = parse_jsonl_line(trimmed)?;
                    return Ok(Some(ev.ts_ns.raw()));
                }
            }
        }
    }
}

fn read_next_par2_frame<R: Read>(reader: &mut R) -> Result<Option<Vec<u8>>, DomainError> {
    let mut hdr = [0_u8; 8];
    let got = read_exact_or_eof(reader, &mut hdr)?;
    if got == 0 {
        return Ok(None);
    }
    if got < 8 || hdr[0..4] != PAR2_MAGIC {
        return Err(DomainError::FormatCorrupted(
            "corrupted par2 frame header".to_string(),
        ));
    }
    let n = u32::from_le_bytes([hdr[4], hdr[5], hdr[6], hdr[7]]) as usize;
    if n > MAX_PAR2_FRAME_ROWS {
        return Err(DomainError::FormatCorrupted(
            "par2 frame row count too large".to_string(),
        ));
    }
    let total_len = n * PAR2_ROW_BYTES + PAR2_FRAME_OVERHEAD;
    let mut frame = vec![0_u8; total_len];
    frame[0..8].copy_from_slice(&hdr);
    let rem = total_len - 8;
    if read_exact_or_eof(reader, &mut frame[8..])? != rem {
        return Err(DomainError::FormatCorrupted(
            "truncated par2 frame body".to_string(),
        ));
    }
    Ok(Some(frame))
}

fn read_exact_or_eof<R: Read>(reader: &mut R, buf: &mut [u8]) -> Result<usize, DomainError> {
    let mut got = 0_usize;
    while got < buf.len() {
        match reader.read(&mut buf[got..]) {
            Ok(0) => break,
            Ok(n) => got += n,
            Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
            Err(e) => return Err(DomainError::Io(format!("tape read error: {e}"))),
        }
    }
    Ok(got)
}

fn parse_jsonl_line(line: &str) -> Result<TapeEvent, DomainError> {
    let row: JsonlTapeRow = serde_json::from_str(line)
        .map_err(|e| DomainError::FormatCorrupted(format!("invalid jsonl row: {e}")))?;
    let kind = match row.kind.to_ascii_lowercase().as_str() {
        "booksnapshot" | "l2booksnapshot" | "book_snapshot" => TapeEventKind::BookSnapshot,
        "trade" | "publictradeprint" => TapeEventKind::Trade,
        "tickermeta" | "ticker_meta" => TapeEventKind::TickerMeta,
        "liquidation" | "liquidationprint" => TapeEventKind::Liquidation,
        other => {
            return Err(DomainError::FormatCorrupted(format!(
                "unknown jsonl kind '{other}'"
            )));
        }
    };
    let side = match row.side.to_ascii_lowercase().as_str() {
        "long" | "bid" | "buy" => Side::Long,
        "short" | "ask" | "sell" => Side::Short,
        other => {
            return Err(DomainError::FormatCorrupted(format!(
                "unknown jsonl side '{other}'"
            )));
        }
    };
    let book = BookTop::new(
        PriceE8::new(row.bid_e8)?,
        PriceE8::new(row.ask_e8)?,
        QtyE8::new(row.bid_qty_e8)?,
        QtyE8::new(row.ask_qty_e8)?,
    )?;
    let trade_price = match row.trade_price_e8 {
        Some(v) => Some(PriceE8::new(v)?),
        None => None,
    };
    let trade_qty = match row.trade_qty_e8 {
        Some(v) => Some(QtyE8::new(v)?),
        None => None,
    };
    let binance_mid = match row.binance_mid_e8 {
        Some(v) => Some(PriceE8::new(v)?),
        None => None,
    };
    TapeEvent::new(
        SymbolId(row.symbol_id),
        TimestampNs::new(row.ts_ns)?,
        kind,
        side,
        book,
        trade_price,
        trade_qty,
        row.l1_to_l5_notional,
        binance_mid,
    )
}
