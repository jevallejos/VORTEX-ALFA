use std::error::Error;
use std::fs::{self, File};
use std::io::Write;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use vortex_alfa::application::falsifier_meso::{
    run_meso_falsification_detailed, MesoFalsifierConfig,
};
use vortex_alfa::application::falsifier_micro::{
    run_micro_falsification_detailed, MicroFalsifierConfig,
};
use vortex_alfa::domain::error::DomainError;
use vortex_alfa::domain::fees::{book_roundtrip_net_bps, FeeSchedule};
use vortex_alfa::domain::ports::{CandleSeriesReader, MarketTapeReader};
use vortex_alfa::domain::signals::meso_energy::{MesoConfig, MesoEnergyEngine};
use vortex_alfa::domain::signals::micro_tail::{
    MicroDetectorConfig, MicroHypothesis, MicroTailEngine,
};
use vortex_alfa::domain::stats::{FalsificationReport, GateVerdict};
use vortex_alfa::domain::types::{
    BookTop, Bps, Candle, PriceE8, QtyE8, Side, SymbolId, TapeEvent, TapeEventKind, TimestampNs,
    SCALE_E8,
};
use vortex_alfa::infrastructure::binary_codec::{
    encode_par2_frame, encode_vkl1_record, encode_vpz1_frame, Par2Row, Vkl1Row, VKL1_MAGIC,
};
use vortex_alfa::infrastructure::tape_reader::{StreamingTapeReader, TapeFormat};
use vortex_alfa::infrastructure::vkl1_reader::StreamingVkl1Reader;
use vortex_alfa::interfaces::cli::run_cli;

type TestResult = Result<(), Box<dyn Error>>;

static TEMP_SEQ: AtomicU64 = AtomicU64::new(1);

struct TempDirGuard {
    path: PathBuf,
}

impl TempDirGuard {
    fn new(prefix: &str) -> Result<Self, Box<dyn Error>> {
        let id = TEMP_SEQ.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "vortex_alfa_{prefix}_{}_{}",
            std::process::id(),
            id
        ));
        if path.exists() {
            fs::remove_dir_all(&path)?;
        }
        fs::create_dir_all(&path)?;
        Ok(Self { path })
    }
}

impl Drop for TempDirGuard {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

struct VecTapeReader {
    events: Vec<TapeEvent>,
    idx: usize,
}

impl MarketTapeReader for VecTapeReader {
    fn next_event(&mut self) -> Result<Option<TapeEvent>, DomainError> {
        if self.idx >= self.events.len() {
            return Ok(None);
        }
        let ev = self.events[self.idx];
        self.idx += 1;
        Ok(Some(ev))
    }
}

struct VecCandleReader {
    candles: Vec<Candle>,
    idx: usize,
}

impl CandleSeriesReader for VecCandleReader {
    fn next_candle(&mut self) -> Result<Option<Candle>, DomainError> {
        if self.idx >= self.candles.len() {
            return Ok(None);
        }
        let c = self.candles[self.idx];
        self.idx += 1;
        Ok(Some(c))
    }
}

fn px(v: f64) -> Result<PriceE8, DomainError> {
    PriceE8::new((v * (SCALE_E8 as f64)).round() as i64)
}

fn qty(v: f64) -> Result<QtyE8, DomainError> {
    QtyE8::new((v * (SCALE_E8 as f64)).round() as i64)
}

fn build_symbol_a_tape(sym: SymbolId) -> Result<Vec<TapeEvent>, DomainError> {
    let mut events = Vec::new();
    let mut ts = 1_000_000_000_i64;
    // 1. Seed 105 rapid buy trades to drive Hawkes buy intensity above crit.
    for i in 0..105 {
        let mid = 100.0 + (i as f64) * 0.001;
        let book = BookTop::new(px(mid - 0.05)?, px(mid + 0.05)?, qty(10.0)?, qty(10.0)?)?;
        let t_qty = if i == 104 { 200.0 } else { 1.0 };
        events.push(TapeEvent::new(
            sym,
            TimestampNs::new(ts)?,
            TapeEventKind::Trade,
            Side::Long,
            book,
            Some(px(mid + 0.05)?),
            Some(qty(t_qty)?),
            10_000.0,
            Some(px(mid)?),
        )?);
        ts += 20_000_000; // 20 ms
    }

    // 2. Quiet baseline + jump for ShiryaevRobertsJump and CrossExchangeShock.
    for i in 0..35 {
        ts += 1_000_000_000;
        let mid = if i >= 25 { 101.2 } else { 100.1 };
        let bn_mid = if i == 26 { mid * 1.004 } else { mid };
        let book = BookTop::new(px(mid - 0.05)?, px(mid + 0.05)?, qty(10.0)?, qty(10.0)?)?;
        events.push(TapeEvent::new(
            sym,
            TimestampNs::new(ts)?,
            TapeEventKind::BookSnapshot,
            Side::Long,
            book,
            None,
            None,
            10_000.0,
            Some(px(bn_mid)?),
        )?);
    }

    // 3. Liquidation cascade trigger.
    ts += 1_000_000_000;
    let liq_book = BookTop::new(px(99.0)?, px(99.1)?, qty(10.0)?, qty(10.0)?)?;
    events.push(TapeEvent::new(
        sym,
        TimestampNs::new(ts)?,
        TapeEventKind::Liquidation,
        Side::Short,
        liq_book,
        Some(px(99.0)?),
        Some(qty(500.0)?),
        5_000.0,
        Some(px(99.05)?),
    )?);

    // 4. Trailing events over +120s so horizons close cleanly.
    for i in 0..30 {
        ts += 5_000_000_000;
        let mid = 99.5 + (i as f64) * 0.05;
        let book = BookTop::new(px(mid - 0.05)?, px(mid + 0.05)?, qty(10.0)?, qty(10.0)?)?;
        events.push(TapeEvent::new(
            sym,
            TimestampNs::new(ts)?,
            TapeEventKind::BookSnapshot,
            Side::Long,
            book,
            None,
            None,
            10_000.0,
            Some(px(mid)?),
        )?);
    }
    Ok(events)
}

fn build_symbol_a_candles(sym: SymbolId) -> Result<Vec<Candle>, DomainError> {
    let mut candles = Vec::new();
    let step = 300_000_000_000_i64;
    for i in 0..70 {
        let ts = TimestampNs::new((i as i64) * step)?;
        let (o, h, l, c, vol, fund) = if i < 12 {
            // High volatility bars to raise slow GK EMA
            (
                100.0,
                118.0,
                82.0,
                100.0 + ((i % 2) as f64) * 0.5,
                500.0,
                (i % 3) as f64,
            )
        } else if i < 36 {
            // Quiet compressed range
            (
                100.0,
                100.2,
                99.9,
                100.1,
                100.0,
                1.0 + ((i % 2) as f64) * 0.5,
            )
        } else if i < 45 {
            // Breakout + funding crowding spike
            let base = 100.0 + ((i - 35) as f64) * 0.8;
            let f = if i == 38 { 80.0 } else { 1.5 };
            (base - 0.2, base + 0.2, base - 0.2, base + 0.2, 300.0, f)
        } else {
            let base = 107.0 + ((i - 45) as f64) * 0.2;
            (base, base + 0.3, base - 0.3, base + 0.1, 150.0, 1.0)
        };
        candles.push(Candle::new(
            sym,
            ts,
            px(o)?,
            px(h)?,
            px(l)?,
            px(c)?,
            vol,
            Bps::new(fund)?,
        )?);
    }
    Ok(candles)
}

#[test]
fn test_strict_symbol_isolation_track1_micro() -> TestResult {
    let sym_a = SymbolId(1);
    let sym_b = SymbolId(2);
    let tape_a = build_symbol_a_tape(sym_a)?;

    // Interleave extreme 100x spikes and huge liquidations on Symbol B.
    let mut interleaved = Vec::with_capacity(tape_a.len() * 2);
    for (idx, &ev_a) in tape_a.iter().enumerate() {
        interleaved.push(ev_a);
        let extreme_px = if idx % 2 == 0 { 10_000.0 } else { 50_000.0 };
        let b_book = BookTop::new(
            px(extreme_px)?,
            px(extreme_px + 50.0)?,
            qty(1_000.0)?,
            qty(1_000.0)?,
        )?;
        interleaved.push(TapeEvent::new(
            sym_b,
            TimestampNs::new(ev_a.ts_ns.raw() + 1_000)?,
            TapeEventKind::Liquidation,
            Side::Short,
            b_book,
            Some(px(extreme_px)?),
            Some(qty(10_000.0)?),
            1.0,
            Some(px(extreme_px * 1.5)?),
        )?);
    }

    // 1. Engine-level bit-for-bit isolation
    let mut eng_alone = MicroTailEngine::new(MicroDetectorConfig::default())?;
    let mut sigs_alone = Vec::new();
    for ev in &tape_a {
        sigs_alone.extend(eng_alone.on_event(ev)?);
    }
    assert!(!sigs_alone.is_empty());

    let mut eng_mixed = MicroTailEngine::new(MicroDetectorConfig::default())?;
    let mut sigs_mixed_a = Vec::new();
    for ev in &interleaved {
        for s in eng_mixed.on_event(ev)? {
            if s.symbol_id == sym_a {
                sigs_mixed_a.push(s);
            }
        }
    }
    assert_eq!(sigs_alone.len(), sigs_mixed_a.len());
    for (s1, s2) in sigs_alone.iter().zip(sigs_mixed_a.iter()) {
        assert_eq!(s1.hypothesis, s2.hypothesis);
        assert_eq!(s1.ts_ns, s2.ts_ns);
        assert_eq!(s1.side, s2.side);
        assert_eq!(s1.score.to_bits(), s2.score.to_bits());
    }

    // 2. Falsifier-level bit-for-bit trade isolation
    let cfg = MicroFalsifierConfig {
        horizons_ns: vec![5_000_000_000, 15_000_000_000],
        ..MicroFalsifierConfig::default()
    };
    let mut r_alone = VecTapeReader {
        events: tape_a,
        idx: 0,
    };
    let (_, trades_alone) = run_micro_falsification_detailed(&mut r_alone, &cfg)?;
    assert!(!trades_alone.is_empty());

    let mut r_mixed = VecTapeReader {
        events: interleaved,
        idx: 0,
    };
    let (_, trades_mixed) = run_micro_falsification_detailed(&mut r_mixed, &cfg)?;
    let trades_mixed_a: Vec<_> = trades_mixed
        .into_iter()
        .filter(|t| t.symbol_id == sym_a)
        .collect();
    assert_eq!(trades_alone.len(), trades_mixed_a.len());
    for (t1, t2) in trades_alone.iter().zip(trades_mixed_a.iter()) {
        assert_eq!(t1.hypothesis, t2.hypothesis);
        assert_eq!(t1.horizon_ns, t2.horizon_ns);
        assert_eq!(t1.side, t2.side);
        assert_eq!(t1.signal_ts_ns, t2.signal_ts_ns);
        assert_eq!(t1.entry_ts_ns, t2.entry_ts_ns);
        assert_eq!(t1.exit_ts_ns, t2.exit_ts_ns);
        assert_eq!(t1.net_bps_vip0.to_bits(), t2.net_bps_vip0.to_bits());
        assert_eq!(t1.net_bps_diag.to_bits(), t2.net_bps_diag.to_bits());
    }
    Ok(())
}

#[test]
fn test_strict_symbol_isolation_track2_meso() -> TestResult {
    let sym_a = SymbolId(1);
    let sym_b = SymbolId(2);
    let candles_a = build_symbol_a_candles(sym_a)?;

    let mut interleaved = Vec::with_capacity(candles_a.len() * 2);
    for (idx, &ca) in candles_a.iter().enumerate() {
        interleaved.push(ca);
        let mult = if idx % 2 == 0 { 100.0 } else { 300.0 };
        interleaved.push(Candle::new(
            sym_b,
            TimestampNs::new(ca.open_ts_ns.raw() + 1_000)?,
            px(100.0 * mult)?,
            px(150.0 * mult)?,
            px(50.0 * mult)?,
            px(120.0 * mult)?,
            1_000_000.0 * (idx as f64 + 1.0),
            Bps::new(if idx % 2 == 0 { 2500.0 } else { -2500.0 })?,
        )?);
    }

    // 1. Engine-level bit-for-bit isolation
    let mut eng_alone = MesoEnergyEngine::new(MesoConfig::default())?;
    let mut outs_alone = Vec::new();
    for c in &candles_a {
        outs_alone.push(eng_alone.on_candle(c)?);
    }

    let mut eng_mixed = MesoEnergyEngine::new(MesoConfig::default())?;
    let mut outs_mixed_a = Vec::new();
    for c in &interleaved {
        let out = eng_mixed.on_candle(c)?;
        if c.symbol_id == sym_a {
            outs_mixed_a.push(out);
        }
    }

    assert_eq!(outs_alone.len(), outs_mixed_a.len());
    for (o1, o2) in outs_alone.iter().zip(outs_mixed_a.iter()) {
        assert_eq!(o1.gk_vol.to_bits(), o2.gk_vol.to_bits());
        assert_eq!(o1.r_comp.to_bits(), o2.r_comp.to_bits());
        assert_eq!(o1.tsmom_vw.map(f64::to_bits), o2.tsmom_vw.map(f64::to_bits));
        assert_eq!(o1.z_fund.map(f64::to_bits), o2.z_fund.map(f64::to_bits));
        assert_eq!(o1.signals.len(), o2.signals.len());
        for (s1, s2) in o1.signals.iter().zip(o2.signals.iter()) {
            assert_eq!(s1.hypothesis, s2.hypothesis);
            assert_eq!(s1.side, s2.side);
            assert_eq!(s1.ts_ns, s2.ts_ns);
            assert_eq!(s1.score.to_bits(), s2.score.to_bits());
        }
    }

    // 2. Falsifier-level bit-for-bit isolation
    let cfg = MesoFalsifierConfig {
        horizons_bars: vec![1, 3, 6],
        ..MesoFalsifierConfig::default()
    };
    let mut r_alone = VecCandleReader {
        candles: candles_a,
        idx: 0,
    };
    let (_, trades_alone) = run_meso_falsification_detailed(&mut r_alone, &cfg)?;
    assert!(!trades_alone.is_empty());

    let mut r_mixed = VecCandleReader {
        candles: interleaved,
        idx: 0,
    };
    let (_, trades_mixed) = run_meso_falsification_detailed(&mut r_mixed, &cfg)?;
    let trades_mixed_a: Vec<_> = trades_mixed
        .into_iter()
        .filter(|t| t.symbol_id == sym_a)
        .collect();
    assert_eq!(trades_alone.len(), trades_mixed_a.len());
    for (t1, t2) in trades_alone.iter().zip(trades_mixed_a.iter()) {
        assert_eq!(t1.hypothesis, t2.hypothesis);
        assert_eq!(t1.horizon_bars, t2.horizon_bars);
        assert_eq!(t1.side, t2.side);
        assert_eq!(t1.signal_ts_ns, t2.signal_ts_ns);
        assert_eq!(t1.entry_ts_ns, t2.entry_ts_ns);
        assert_eq!(t1.exit_ts_ns, t2.exit_ts_ns);
        assert_eq!(t1.funding_cost_bps.to_bits(), t2.funding_cost_bps.to_bits());
        assert_eq!(t1.net_bps_vip0.to_bits(), t2.net_bps_vip0.to_bits());
        assert_eq!(t1.net_bps_diag.to_bits(), t2.net_bps_diag.to_bits());
    }
    Ok(())
}

#[test]
fn test_zero_lookahead_end_to_end_track1_micro() -> TestResult {
    let tape_orig = build_symbol_a_tape(SymbolId(1))?;
    let k = tape_orig.len() - 10;
    let cutoff_ts = tape_orig[k].ts_ns.raw();

    let mut tape_mut = tape_orig.clone();
    for ev in tape_mut.iter_mut().skip(k + 1) {
        ev.side = match ev.side {
            Side::Long => Side::Short,
            Side::Short => Side::Long,
        };
        ev.book = BookTop::new(px(1_000.0)?, px(1_005.0)?, qty(50.0)?, qty(50.0)?)?;
        ev.trade_price = Some(px(1_002.0)?);
        ev.trade_qty = Some(qty(9_999.0)?);
        ev.binance_mid = Some(px(1_500.0)?);
    }

    let cfg = MicroFalsifierConfig {
        horizons_ns: vec![5_000_000_000, 15_000_000_000],
        ..MicroFalsifierConfig::default()
    };
    let mut r1 = VecTapeReader {
        events: tape_orig,
        idx: 0,
    };
    let (_, trades1) = run_micro_falsification_detailed(&mut r1, &cfg)?;

    let mut r2 = VecTapeReader {
        events: tape_mut,
        idx: 0,
    };
    let (_, trades2) = run_micro_falsification_detailed(&mut r2, &cfg)?;

    let prefix1: Vec<_> = trades1
        .into_iter()
        .filter(|t| t.exit_ts_ns <= cutoff_ts)
        .collect();
    let prefix2: Vec<_> = trades2
        .into_iter()
        .filter(|t| t.exit_ts_ns <= cutoff_ts)
        .collect();

    assert!(!prefix1.is_empty());
    assert_eq!(prefix1.len(), prefix2.len());
    for (t1, t2) in prefix1.iter().zip(prefix2.iter()) {
        assert_eq!(t1.hypothesis, t2.hypothesis);
        assert_eq!(t1.horizon_ns, t2.horizon_ns);
        assert_eq!(t1.side, t2.side);
        assert_eq!(t1.signal_ts_ns, t2.signal_ts_ns);
        assert_eq!(t1.entry_ts_ns, t2.entry_ts_ns);
        assert_eq!(t1.exit_ts_ns, t2.exit_ts_ns);
        assert_eq!(t1.net_bps_vip0.to_bits(), t2.net_bps_vip0.to_bits());
        assert_eq!(t1.net_bps_diag.to_bits(), t2.net_bps_diag.to_bits());
    }
    Ok(())
}

#[test]
fn test_zero_lookahead_end_to_end_track2_meso() -> TestResult {
    let candles_orig = build_symbol_a_candles(SymbolId(1))?;
    let k = 50_usize;
    let cutoff_ts = candles_orig[k].open_ts_ns.raw();

    let mut candles_mut = candles_orig.clone();
    for c in candles_mut.iter_mut().skip(k + 1) {
        c.open = px(1_000.0)?;
        c.high = px(1_500.0)?;
        c.low = px(500.0)?;
        c.close = px(1_200.0)?;
        c.volume = 999_999.0;
        c.funding_rate_bps = Bps::new(-500.0)?;
    }

    let cfg = MesoFalsifierConfig {
        horizons_bars: vec![1, 3, 6],
        ..MesoFalsifierConfig::default()
    };
    let mut r1 = VecCandleReader {
        candles: candles_orig,
        idx: 0,
    };
    let (_, trades1) = run_meso_falsification_detailed(&mut r1, &cfg)?;

    let mut r2 = VecCandleReader {
        candles: candles_mut,
        idx: 0,
    };
    let (_, trades2) = run_meso_falsification_detailed(&mut r2, &cfg)?;

    let prefix1: Vec<_> = trades1
        .into_iter()
        .filter(|t| t.exit_ts_ns <= cutoff_ts)
        .collect();
    let prefix2: Vec<_> = trades2
        .into_iter()
        .filter(|t| t.exit_ts_ns <= cutoff_ts)
        .collect();

    assert!(!prefix1.is_empty());
    assert_eq!(prefix1.len(), prefix2.len());
    for (t1, t2) in prefix1.iter().zip(prefix2.iter()) {
        assert_eq!(t1.hypothesis, t2.hypothesis);
        assert_eq!(t1.horizon_bars, t2.horizon_bars);
        assert_eq!(t1.side, t2.side);
        assert_eq!(t1.signal_ts_ns, t2.signal_ts_ns);
        assert_eq!(t1.entry_ts_ns, t2.entry_ts_ns);
        assert_eq!(t1.exit_ts_ns, t2.exit_ts_ns);
        assert_eq!(t1.funding_cost_bps.to_bits(), t2.funding_cost_bps.to_bits());
        assert_eq!(t1.net_bps_vip0.to_bits(), t2.net_bps_vip0.to_bits());
        assert_eq!(t1.net_bps_diag.to_bits(), t2.net_bps_diag.to_bits());
    }
    Ok(())
}

fn make_par2_row(seq: u64, ts_ns: i64, kind: u8, bid: f64, ask: f64) -> Par2Row {
    let mid_e8 = (((bid + ask) * 0.5) * 1e8).round() as i64;
    Par2Row {
        symbol_id: 1,
        par2_kind: kind,
        side: 0,
        sequence_id: seq,
        ts_exchange_ns: ts_ns,
        ts_local_ns: ts_ns,
        price_ticks: mid_e8,
        lot_qty: SCALE_E8 as i64,
        best_bid_ticks: (bid * 1e8).round() as i64,
        best_ask_ticks: (ask * 1e8).round() as i64,
        best_bid_qty: 10 * (SCALE_E8 as i64),
        best_ask_qty: 10 * (SCALE_E8 as i64),
        mark_price_ticks: mid_e8,
        ..Par2Row::default()
    }
}

#[test]
fn test_multifile_par2_vpz1_vkl1_streaming_and_cli() -> TestResult {
    let dir = TempDirGuard::new("multifile_stream")?;
    let par2_dir = dir.path.join("par2_tape");
    let vpz_dir = dir.path.join("vpz_tape");
    let vkl1_dir = dir.path.join("vkl1_bars");
    fs::create_dir_all(&par2_dir)?;
    fs::create_dir_all(&vpz_dir)?;
    fs::create_dir_all(&vkl1_dir)?;

    // Build seg_001.parquet (t = 0s..29s): 25 quiet events, then SR jump at t=25s, entry at t=26s.
    let mut rows_seg1 = Vec::new();
    for s in 0..30_i64 {
        let ts = s * 1_000_000_000;
        let (bid, ask) = if s >= 25 {
            (101.0, 101.1)
        } else {
            (99.95, 100.05)
        };
        rows_seg1.push(make_par2_row(s as u64, ts, 1, bid, ask));
    }

    // Build seg_002.parquet (t = 30s..120s): exit horizons (15s, 60s) land here!
    let mut rows_seg2 = Vec::new();
    for s in 30..=120_i64 {
        let ts = s * 1_000_000_000;
        rows_seg2.push(make_par2_row(s as u64, ts, 1, 102.0, 102.1));
    }

    let frame1 = encode_par2_frame(&rows_seg1)?;
    let frame2 = encode_par2_frame(&rows_seg2)?;
    fs::write(par2_dir.join("seg_001.parquet"), &frame1)?;
    fs::write(par2_dir.join("seg_002.parquet"), &frame2)?;

    // Stream PAR2 multi-file directory
    let cfg = MicroFalsifierConfig {
        horizons_ns: vec![15_000_000_000, 60_000_000_000],
        ..MicroFalsifierConfig::default()
    };
    let mut par2_reader = StreamingTapeReader::open(&par2_dir, TapeFormat::Par2)?;
    let (par2_report, par2_trades) = run_micro_falsification_detailed(&mut par2_reader, &cfg)?;
    assert_eq!(par2_report.events_processed, 121);

    let seg2_start_ns = 30_000_000_000_i64;
    let cross_file_trade = par2_trades
        .iter()
        .find(|t| {
            t.hypothesis == MicroHypothesis::ShiryaevRobertsJump
                && t.entry_ts_ns < seg2_start_ns
                && t.exit_ts_ns >= seg2_start_ns
        })
        .ok_or("expected cross-file ShiryaevRobertsJump trade spanning seg_001 -> seg_002")?;

    let expected_net = book_roundtrip_net_bps(
        Side::Long,
        BookTop::new(px(101.0)?, px(101.1)?, qty(10.0)?, qty(10.0)?)?,
        BookTop::new(px(102.0)?, px(102.1)?, qty(10.0)?, qty(10.0)?)?,
        &FeeSchedule::bybit_vip0(),
        true,
        true,
        Bps::ZERO,
    )?
    .raw();
    assert!((cross_file_trade.net_bps_vip0 - expected_net).abs() < 1e-12);

    // CrossExchangeShock MUST have n == 0 and InsufficientSample across all horizons on PAR2
    let cross_cells_vip0: Vec<_> = par2_report
        .vip0_cells
        .iter()
        .filter(|c| c.hypothesis == "CrossExchangeShock")
        .collect();
    assert_eq!(cross_cells_vip0.len(), cfg.horizons_ns.len());
    for c in cross_cells_vip0 {
        assert_eq!(c.n, 0);
        assert_eq!(c.verdict, GateVerdict::InsufficientSample);
    }

    // VPZ1 (.par2z) round-trip produces bit-identical report and trades
    let vpz1 = encode_vpz1_frame(1, &frame1)?;
    let vpz2 = encode_vpz1_frame(2, &frame2)?;
    fs::write(vpz_dir.join("seg_001.par2z"), &vpz1)?;
    fs::write(vpz_dir.join("seg_002.par2z"), &vpz2)?;
    let mut vpz_reader = StreamingTapeReader::open(&vpz_dir, TapeFormat::Vpz1)?;
    let (vpz_report, vpz_trades) = run_micro_falsification_detailed(&mut vpz_reader, &cfg)?;
    assert_eq!(par2_report, vpz_report);
    assert_eq!(par2_trades, vpz_trades);

    // VKL1 directory streaming across 2 .vkl1 files
    let candles = build_symbol_a_candles(SymbolId(1))?;
    let split = 40_usize;
    let mut f1 = File::create(vkl1_dir.join("bars_001.vkl1"))?;
    let mut f2 = File::create(vkl1_dir.join("bars_002.vkl1"))?;
    for (idx, c) in candles.iter().enumerate() {
        let row = Vkl1Row {
            magic: u32::from_le_bytes(VKL1_MAGIC),
            slot_idx: c.symbol_id.0 as u8,
            tf_code: 1,
            open_ts_ns: c.open_ts_ns.raw(),
            open_ticks: c.open.raw(),
            high_ticks: c.high.raw(),
            low_ticks: c.low.raw(),
            close_ticks: c.close.raw(),
            volume_lots: (c.volume * 1e8).round() as i64,
            funding_sbps: (c.funding_rate_bps.raw() * 100.0).round() as i16,
            ..Vkl1Row::default()
        };
        let bytes = encode_vkl1_record(&row);
        if idx < split {
            f1.write_all(&bytes)?;
        } else {
            f2.write_all(&bytes)?;
        }
    }
    drop(f1);
    drop(f2);

    let meso_cfg = MesoFalsifierConfig {
        horizons_bars: vec![1, 3, 6],
        ..MesoFalsifierConfig::default()
    };
    let mut vkl1_reader = StreamingVkl1Reader::open(&vkl1_dir)?;
    let (vkl1_report, vkl1_trades) = run_meso_falsification_detailed(&mut vkl1_reader, &meso_cfg)?;
    assert_eq!(vkl1_report.events_processed, candles.len() as u64);
    let file2_start_ns = candles[split].open_ts_ns.raw();
    assert!(vkl1_trades
        .iter()
        .any(|t| t.entry_ts_ns < file2_start_ns && t.exit_ts_ns >= file2_start_ns));

    // CLI scan-micro & scan-meso --out <json>
    let micro_out = dir.path.join("micro_report.json");
    let meso_out = dir.path.join("meso_report.json");
    let micro_ascii = run_cli([
        "vortex-alfa",
        "scan-micro",
        "--format",
        "par2",
        "--input",
        &par2_dir.to_string_lossy(),
        "--out",
        &micro_out.to_string_lossy(),
    ])?;
    assert!(micro_ascii.contains("Track1-MicroTail"));
    let micro_json: FalsificationReport = serde_json::from_str(&fs::read_to_string(&micro_out)?)?;
    assert_eq!(micro_json.events_processed, 121);
    assert_eq!(micro_json.track, "Track1-MicroTail");

    let meso_ascii = run_cli([
        "vortex-alfa",
        "scan-meso",
        "--format",
        "vkl1",
        "--input",
        &vkl1_dir.to_string_lossy(),
        "--slippage-bps",
        "1.5",
        "--out",
        &meso_out.to_string_lossy(),
    ])?;
    assert!(meso_ascii.contains("Track2-MesoEnergy"));
    let meso_json: FalsificationReport = serde_json::from_str(&fs::read_to_string(&meso_out)?)?;
    assert_eq!(meso_json.events_processed, candles.len() as u64);
    assert_eq!(meso_json.track, "Track2-MesoEnergy");
    Ok(())
}
