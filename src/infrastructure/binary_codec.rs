use std::io::{self, Read};

use crate::domain::error::DomainError;
use crate::domain::types::{
    BookTop, Bps, Candle, PriceE8, QtyE8, Side, SymbolId, TapeEvent, TapeEventKind, TimestampNs,
};

pub const PAR2_MAGIC: [u8; 4] = *b"PAR2";
pub const PAR2_ROW_BYTES: usize = 256;
pub const VPZ1_MAGIC: [u8; 4] = *b"VPZ1";
pub const VPZ1_HEADER_BYTES: usize = 36;
pub const VKL1_MAGIC: [u8; 4] = *b"VKL1";
pub const VKL1_RECORD_BYTES: usize = 96;

const MAX_ORIG: usize = 4 * 1024 * 1024;

macro_rules! put_le { ($out:expr; $($v:expr),+ $(,)?) => { $($out.extend_from_slice(&($v).to_le_bytes());)+ }; }
macro_rules! get_le { ($c:expr => $r:expr; $($f:ident : $ty:ident),+ $(,)?) => { $($r.$f = $c.$ty();)+ }; }
macro_rules! chk {
    ($c:expr; $m:expr) => {
        ($c).then_some(()).ok_or_else(|| corrupt($m))?
    };
}
macro_rules! le_fn { ($($m:ident : $t:ty, $n:expr);+ $(;)?) => { $(fn $m(&mut self) -> $t { <$t>::from_le_bytes(self.bytes::<$n>()) })+ }; }

#[must_use]
pub fn fnv1a64(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |h, &b| {
        (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3)
    })
}

fn corrupt(msg: &str) -> DomainError {
    DomainError::FormatCorrupted(msg.to_string())
}

#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct Par2Row {
    pub symbol_id: u16,
    pub par2_kind: u8,
    pub side: u8,
    pub sequence_id: u64,
    pub ts_exchange_ns: i64,
    pub ts_local_ns: i64,
    pub price_ticks: i64,
    pub lot_qty: i64,
    pub best_bid_ticks: i64,
    pub best_ask_ticks: i64,
    pub best_bid_qty: i64,
    pub best_ask_qty: i64,
    pub mark_price_ticks: i64,
    pub ofi_bps: i32,
    pub tfi_bps: i32,
    pub geom_bps: i32,
    pub realized_vol_5m_bps: i64,
    pub open_interest_x100: i64,
    pub funding_rate_scaled_bps: i64,
    pub next_funding_ts_ns: i64,
    pub deep_bid_ticks: [i64; 4],
    pub deep_bid_qty: [i64; 4],
    pub deep_ask_ticks: [i64; 4],
    pub deep_ask_qty: [i64; 4],
}

fn lvl_notional(px: i64, qty: i64) -> Result<f64, DomainError> {
    if px <= 0 || qty <= 0 {
        return Ok(0.0);
    }
    Ok(PriceE8::new(px)?.as_f64() * QtyE8::new(qty)?.as_f64())
}

impl Par2Row {
    pub fn to_tape_event(&self) -> Result<TapeEvent, DomainError> {
        let ts = TimestampNs::new(self.ts_local_ns.max(self.ts_exchange_ns))?;
        let k = match self.par2_kind {
            1 => TapeEventKind::BookSnapshot,
            2 => TapeEventKind::Trade,
            3 => TapeEventKind::TickerMeta,
            4 => TapeEventKind::Liquidation,
            _ => return Err(corrupt("par2_kind")),
        };
        chk!(self.side <= 1; "bad side");
        let side = [Side::Long, Side::Short][usize::from(self.side)];
        let bb = PriceE8::new(self.best_bid_ticks)?;
        let ba = PriceE8::new(self.best_ask_ticks)?;
        let bq = QtyE8::new(self.best_bid_qty)?;
        let aq = QtyE8::new(self.best_ask_qty)?;
        let bk = BookTop::new(bb, ba, bq, aq)?;
        let pr = matches!(k, TapeEventKind::Trade | TapeEventKind::Liquidation);
        let (p, q) = (self.price_ticks, self.lot_qty);
        let tx = (pr && p > 0).then(|| PriceE8::new(p)).transpose()?;
        let tq = (pr && q > 0).then(|| QtyE8::new(q)).transpose()?;
        let mut not = bb.as_f64() * bq.as_f64() + ba.as_f64() * aq.as_f64();
        for l in 0..4 {
            not += lvl_notional(self.deep_bid_ticks[l], self.deep_bid_qty[l])?
                + lvl_notional(self.deep_ask_ticks[l], self.deep_ask_qty[l])?;
        }
        TapeEvent::new(SymbolId(self.symbol_id), ts, k, side, bk, tx, tq, not, None)
    }

    fn write_to(&self, out: &mut Vec<u8>) {
        let s = self;
        put_le!(out; s.symbol_id, s.par2_kind, s.side, s.sequence_id, s.ts_exchange_ns, s.ts_local_ns);
        put_le!(out; s.price_ticks, s.lot_qty, s.best_bid_ticks, s.best_ask_ticks, s.best_bid_qty, s.best_ask_qty);
        put_le!(out; s.mark_price_ticks, s.ofi_bps, s.tfi_bps, s.geom_bps, s.realized_vol_5m_bps);
        put_le!(out; s.open_interest_x100, s.funding_rate_scaled_bps, s.next_funding_ts_ns);
        for l in 0..4 {
            put_le!(out; s.deep_bid_ticks[l], s.deep_bid_qty[l], s.deep_ask_ticks[l], s.deep_ask_qty[l]);
        }
    }

    fn read_from(buf: &[u8]) -> Result<Self, DomainError> {
        chk!(buf.len() == PAR2_ROW_BYTES; "invalid par2 row len");
        let mut c = Le(buf, 0);
        let mut r = Self::default();
        get_le!(c => r; symbol_id: u16, par2_kind: u8, side: u8, sequence_id: u64);
        get_le!(c => r; ts_exchange_ns: i64, ts_local_ns: i64, price_ticks: i64, lot_qty: i64);
        get_le!(c => r; best_bid_ticks: i64, best_ask_ticks: i64, best_bid_qty: i64, best_ask_qty: i64);
        get_le!(c => r; mark_price_ticks: i64, ofi_bps: i32, tfi_bps: i32, geom_bps: i32);
        get_le!(c => r; realized_vol_5m_bps: i64, open_interest_x100: i64, funding_rate_scaled_bps: i64, next_funding_ts_ns: i64);
        for l in 0..4 {
            (r.deep_bid_ticks[l], r.deep_bid_qty[l]) = (c.i64(), c.i64());
            (r.deep_ask_ticks[l], r.deep_ask_qty[l]) = (c.i64(), c.i64());
        }
        Ok(r)
    }
}

fn par2_bounds(buf: &[u8]) -> Result<(usize, usize), DomainError> {
    chk!(buf.len() >= 20 && buf[0..4] == PAR2_MAGIC; "invalid par2 header");
    let n = Le(buf, 4).u32() as usize;
    chk!(n <= MAX_ORIG / PAR2_ROW_BYTES; "par2 row count too large");
    Ok((n, n * PAR2_ROW_BYTES + 20))
}

pub fn decode_par2_frame(buf: &[u8]) -> Result<Vec<Par2Row>, DomainError> {
    let (n, expected) = par2_bounds(buf)?;
    chk!(buf.len() == expected && buf[buf.len() - 4..] == PAR2_MAGIC; "par2 frame mismatch");
    let end = buf.len() - 12;
    chk!(fnv1a64(&buf[..end]) == Le(buf, end).u64(); "par2 checksum mismatch");
    (0..n)
        .map(|i| Par2Row::read_from(&buf[8 + i * 256..8 + (i + 1) * 256]))
        .collect()
}

pub fn encode_par2_frame(rows: &[Par2Row]) -> Result<Vec<u8>, DomainError> {
    let n = u32::try_from(rows.len()).map_err(|_| corrupt("par2 row count overflow"))?;
    let mut buf = Vec::with_capacity(20 + rows.len() * PAR2_ROW_BYTES);
    buf.extend_from_slice(&PAR2_MAGIC);
    put_le!(buf; n);
    rows.iter().for_each(|r| r.write_to(&mut buf));
    put_le!(buf; fnv1a64(&buf));
    buf.extend_from_slice(&PAR2_MAGIC);
    Ok(buf)
}

pub fn par2_first_ts_ns(buf: &[u8]) -> Result<i64, DomainError> {
    let f = buf
        .get(..par2_bounds(buf)?.1)
        .ok_or_else(|| corrupt("truncated par2"))?;
    let rows = decode_par2_frame(f)?;
    let r = rows.first().ok_or_else(|| corrupt("empty par2 frame"))?;
    Ok(r.ts_local_ns.max(r.ts_exchange_ns))
}

fn sb(buf: &[u8], i: usize) -> u8 {
    i.checked_sub(PAR2_ROW_BYTES).map_or(0, |k| buf[k])
}

#[must_use]
pub fn compress_vpz1_stride_zrle(raw: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(raw.len() / 2 + 16);
    let mut i = 0_usize;
    while i < raw.len() {
        let d = raw[i] ^ sb(raw, i);
        if d != 0 {
            out.push(d);
            i += 1;
        } else {
            let lim = 255.min(raw.len() - i);
            let r = (1..lim)
                .take_while(|&k| (raw[i + k] ^ sb(raw, i + k)) == 0)
                .count()
                + 1;
            out.extend_from_slice(&[0, r as u8]);
            i += r;
        }
    }
    out
}

pub fn decompress_vpz1_stride_zrle(
    comp: &[u8],
    expected_len: usize,
) -> Result<Vec<u8>, DomainError> {
    chk!(expected_len <= MAX_ORIG; "vpz1 frame exceeds max");
    let mut out = Vec::with_capacity(expected_len);
    let mut i = 0_usize;
    while i < comp.len() {
        let b = comp[i];
        if b != 0 {
            chk!(out.len() < expected_len; "vpz1 overflow");
            out.push(b ^ sb(&out, out.len()));
            i += 1;
        } else {
            let &r = comp.get(i + 1).ok_or_else(|| corrupt("vpz1 rle"))?;
            chk!(r != 0 && out.len().saturating_add(usize::from(r)) <= expected_len; "vpz1 bad rle");
            (0..r).for_each(|_| out.push(sb(&out, out.len())));
            i += 2;
        }
    }
    chk!(out.len() == expected_len; "vpz1 length mismatch");
    Ok(out)
}

pub fn encode_vpz1_frame(seq: u64, raw: &[u8]) -> Result<Vec<u8>, DomainError> {
    chk!(!raw.is_empty() && raw.len() <= MAX_ORIG; "invalid vpz1 raw len");
    let comp = compress_vpz1_stride_zrle(raw);
    let mut frame = Vec::with_capacity(VPZ1_HEADER_BYTES + comp.len());
    frame.extend_from_slice(&VPZ1_MAGIC);
    put_le!(frame; seq, raw.len() as u32, comp.len() as u32, fnv1a64(raw), fnv1a64(&comp));
    frame.extend_from_slice(&comp);
    Ok(frame)
}

pub fn read_next_vpz1_frame<R: Read>(
    reader: &mut R,
) -> Result<Option<(u64, Vec<u8>)>, DomainError> {
    let mut hdr = [0_u8; VPZ1_HEADER_BYTES];
    let got = read_or_eof(reader, &mut hdr)?;
    if got == 0 {
        return Ok(None);
    }
    chk!(got == VPZ1_HEADER_BYTES && hdr[0..4] == VPZ1_MAGIC; "corrupted vpz1 header");
    let mut c = Le(&hdr, 4);
    let (seq, orig_len, comp_len) = (c.u64(), c.u32() as usize, c.u32() as usize);
    let (orig_crc, comp_crc) = (c.u64(), c.u64());
    chk!(orig_len > 0 && orig_len <= MAX_ORIG && comp_len <= MAX_ORIG * 2; "vpz1 bad lens");
    let mut comp = vec![0_u8; comp_len];
    chk!(read_or_eof(reader, &mut comp)? == comp_len; "vpz1 truncated payload");
    chk!(fnv1a64(&comp) == comp_crc; "vpz1 comp_crc mismatch");
    let raw = decompress_vpz1_stride_zrle(&comp, orig_len)?;
    chk!(fnv1a64(&raw) == orig_crc; "vpz1 orig_crc mismatch");
    Ok(Some((seq, raw)))
}

#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct Vkl1Row {
    pub magic: u32,
    pub slot_idx: u8,
    pub tf_code: u8,
    pub wave_regime: u8,
    pub flags: u8,
    pub open_ts_ns: i64,
    pub open_ticks: i64,
    pub high_ticks: i64,
    pub low_ticks: i64,
    pub close_ticks: i64,
    pub volume_lots: i64,
    pub open_interest_x100: i64,
    pub trade_count: u32,
    pub ofi_mean_pm: i16,
    pub tfi_mean_pm: i16,
    pub geom_mean_pm: i16,
    pub funding_sbps: i16,
    pub hurst_pm: u16,
    pub ob_bull_pm: u16,
    pub ob_bear_pm: u16,
    pub spoof_vetoes: u16,
    pub liq_buy_lots_c: u16,
    pub liq_sell_lots_c: u16,
    pub fnv1a64: u64,
}

impl Vkl1Row {
    pub fn to_candle(&self) -> Result<Candle, DomainError> {
        if self.volume_lots < 0 {
            return Err(DomainError::InvalidCandle("negative volume_lots".into()));
        }
        let p = PriceE8::new;
        let (o, h) = (p(self.open_ticks)?, p(self.high_ticks)?);
        let (l, c) = (p(self.low_ticks)?, p(self.close_ticks)?);
        let f = Bps::new(f64::from(self.funding_sbps) / 100.0)?;
        let sym = SymbolId(u16::from(self.slot_idx));
        let ts = TimestampNs::new(self.open_ts_ns)?;
        Candle::new(sym, ts, o, h, l, c, (self.volume_lots as f64) / 1e8, f)
    }
}

pub fn decode_vkl1_record(buf: &[u8; VKL1_RECORD_BYTES]) -> Result<Vkl1Row, DomainError> {
    chk!(buf[0..4] == VKL1_MAGIC && buf[5] <= 6; "invalid vkl1 header");
    let stored_crc = Le(buf, 88).u64();
    chk!(fnv1a64(&buf[..88]) == stored_crc; "vkl1 checksum mismatch");
    let mut c = Le(buf, 0);
    let mut r = Vkl1Row::default();
    get_le!(c => r; magic: u32, slot_idx: u8, tf_code: u8, wave_regime: u8, flags: u8);
    get_le!(c => r; open_ts_ns: i64, open_ticks: i64, high_ticks: i64, low_ticks: i64, close_ticks: i64);
    get_le!(c => r; volume_lots: i64, open_interest_x100: i64, trade_count: u32);
    get_le!(c => r; ofi_mean_pm: i16, tfi_mean_pm: i16, geom_mean_pm: i16, funding_sbps: i16);
    get_le!(c => r; hurst_pm: u16, ob_bull_pm: u16, ob_bear_pm: u16, spoof_vetoes: u16, liq_buy_lots_c: u16, liq_sell_lots_c: u16);
    r.fnv1a64 = stored_crc;
    Ok(r)
}

#[must_use]
pub fn encode_vkl1_record(row: &Vkl1Row) -> [u8; VKL1_RECORD_BYTES] {
    let mut v = Vec::with_capacity(VKL1_RECORD_BYTES);
    let r = row;
    v.extend_from_slice(&VKL1_MAGIC);
    put_le!(v; r.slot_idx, r.tf_code, r.wave_regime, r.flags, r.open_ts_ns, r.open_ticks, r.high_ticks);
    put_le!(v; r.low_ticks, r.close_ticks, r.volume_lots, r.open_interest_x100, r.trade_count);
    put_le!(v; r.ofi_mean_pm, r.tfi_mean_pm, r.geom_mean_pm, r.funding_sbps, r.hurst_pm, r.ob_bull_pm);
    put_le!(v; r.ob_bear_pm, r.spoof_vetoes, r.liq_buy_lots_c, r.liq_sell_lots_c, fnv1a64(&v));
    v.try_into().unwrap_or([0; VKL1_RECORD_BYTES])
}

fn read_or_eof<R: Read>(reader: &mut R, buf: &mut [u8]) -> Result<usize, DomainError> {
    let mut got = 0_usize;
    while got < buf.len() {
        match reader.read(&mut buf[got..]) {
            Ok(0) => break,
            Ok(n) => got += n,
            Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
            Err(e) => return Err(DomainError::Io(format!("vpz1 read error: {e}"))),
        }
    }
    Ok(got)
}

struct Le<'a>(&'a [u8], usize);
impl Le<'_> {
    fn u8(&mut self) -> u8 {
        self.1 += 1;
        self.0[self.1 - 1]
    }
    fn bytes<const N: usize>(&mut self) -> [u8; N] {
        let a = self.0[self.1..self.1 + N].try_into().unwrap_or([0; N]);
        self.1 += N;
        a
    }
    le_fn!(u16: u16, 2; i16: i16, 2; u32: u32, 4; i32: i32, 4; u64: u64, 8; i64: i64, 8);
}
