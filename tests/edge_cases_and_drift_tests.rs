use std::error::Error;
use std::fs;
use std::io::Cursor;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use vortex_alfa::application::falsifier_meso::{
    run_meso_falsification_detailed, MesoFalsifierConfig,
};
use vortex_alfa::application::falsifier_micro::{
    run_micro_falsification_detailed, MicroFalsifierConfig,
};
use vortex_alfa::domain::error::DomainError;
use vortex_alfa::domain::fees::{leg_fee_e8, FeeSchedule};
use vortex_alfa::domain::ledger::EquityLedgerE8;
use vortex_alfa::domain::normal_dist::{normal_cdf, normal_inv_cdf};
use vortex_alfa::domain::ports::{CandleSeriesReader, MarketTapeReader};
use vortex_alfa::domain::signals::meso_energy::{garman_klass_vol, MesoConfig, MesoEnergyEngine};
use vortex_alfa::domain::signals::micro_detectors::{
    CausalQuantileWindow, HawkesProcess, ShiryaevRobertsDetector,
};
use vortex_alfa::domain::signals::micro_tail::{MicroDetectorConfig, MicroTailEngine};
use vortex_alfa::domain::stats::{
    evaluate_hypothesis_batch, newey_west, GateVerdict, HypothesisCellInput, Moments4,
};
use vortex_alfa::domain::types::{
    BookTop, Bps, Candle, NotionalE8, PriceE8, QtyE8, Side, SymbolId, TapeEvent, TapeEventKind,
    TimestampNs, SCALE_E8,
};
use vortex_alfa::infrastructure::binary_codec::{
    decode_par2_frame, decode_vkl1_record, encode_par2_frame, encode_vkl1_record,
    encode_vpz1_frame, fnv1a64, read_next_vpz1_frame, Par2Row, Vkl1Row, VKL1_MAGIC,
};
use vortex_alfa::infrastructure::csv_candle_reader::CsvCandleReader;
use vortex_alfa::infrastructure::tape_reader::{StreamingTapeReader, TapeFormat};
use vortex_alfa::infrastructure::vkl1_reader::StreamingVkl1Reader;

type TestResult = Result<(), Box<dyn Error>>;

static TEMP_SEQ: AtomicU64 = AtomicU64::new(1);

struct TempDirGuard {
    path: PathBuf,
}

impl TempDirGuard {
    fn new(prefix: &str) -> Result<Self, Box<dyn Error>> {
        let id = TEMP_SEQ.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "vortex_alfa_edge_{prefix}_{}_{}",
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

#[test]
fn test_reject_non_finite_across_all_components() -> TestResult {
    for &bad in &[f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert!(matches!(Bps::new(bad), Err(DomainError::NonFiniteBps)));
        assert!(matches!(
            Moments4::compute(&[1.0, 2.0, bad, 4.0, 5.0]),
            Err(DomainError::NonFiniteValue(_))
        ));
        assert!(matches!(
            newey_west(&[1.0, 2.0, bad, 4.0, 5.0]),
            Err(DomainError::NonFiniteValue(_))
        ));
        assert!(matches!(
            normal_inv_cdf(bad),
            Err(DomainError::InvalidStatParam(_))
        ));
        let mut sr = ShiryaevRobertsDetector::new(4.6, 0.05, 5)?;
        assert!(matches!(
            sr.update(bad),
            Err(DomainError::NonFiniteValue(_))
        ));
        let mut qwin = CausalQuantileWindow::new(10, 2)?;
        assert!(matches!(
            qwin.check_and_push(bad, 0.9),
            Err(DomainError::NonFiniteValue(_))
        ));
        assert!(matches!(
            qwin.check_and_push(1.0, bad),
            Err(DomainError::InvalidStatParam(_))
        ));
    }

    assert!(matches!(
        normal_cdf(f64::NAN),
        Err(DomainError::NonFiniteValue(_))
    ));
    assert_eq!(normal_cdf(f64::INFINITY)?, 1.0);
    assert_eq!(normal_cdf(f64::NEG_INFINITY)?, 0.0);

    // CSV candle reader rejects NaN / inf / -inf in price, volume, and funding
    let dir = TempDirGuard::new("csv_nonfinite")?;
    let bad_rows = [
        "1,1000,NaN,105,95,100,10,1.0",
        "1,1000,100,inf,95,100,10,1.0",
        "1,1000,100,105,-inf,100,10,1.0",
        "1,1000,100,105,95,100,NaN,1.0",
        "1,1000,100,105,95,100,10,inf",
        "1,1000,100,105,95,100,10,-inf",
    ];
    for (i, row) in bad_rows.iter().enumerate() {
        let p = dir.path.join(format!("bad_{i}.csv"));
        fs::write(
            &p,
            format!("symbol_id,ts_ms,open,high,low,close,volume,funding_rate_bps\n{row}\n"),
        )?;
        let mut r = CsvCandleReader::open(&p)?;
        assert!(r.next_candle().is_err());
    }
    Ok(())
}

#[test]
fn test_zero_variance_never_passes_or_nans() -> TestResult {
    let const_pos = vec![5.0_f64; 10];
    let const_zero = vec![0.0_f64; 10];
    assert_eq!(Moments4::compute(&const_pos)?, None);
    assert_eq!(newey_west(&const_pos)?, None);
    assert_eq!(Moments4::compute(&const_zero)?, None);
    assert_eq!(newey_west(&const_zero)?, None);

    let cells = vec![
        HypothesisCellInput {
            hypothesis: "ConstPos".to_string(),
            horizon: "15s".to_string(),
            returns_bps: const_pos,
        },
        HypothesisCellInput {
            hypothesis: "ConstZero".to_string(),
            horizon: "15s".to_string(),
            returns_bps: const_zero,
        },
    ];
    let summary = evaluate_hypothesis_batch(&cells, &FeeSchedule::bybit_vip0())?;
    for c in &summary.cells {
        assert_ne!(c.verdict, GateVerdict::Pass);
        assert!(matches!(
            c.verdict,
            GateVerdict::Fail | GateVerdict::InsufficientSample
        ));
        assert!(c.dsr.is_none());
        assert!(c.t_nw.is_none());
        if let Some(m) = c.mean_bps {
            assert!(m.is_finite());
        }
    }
    Ok(())
}

#[test]
fn test_flat_candle_garman_klass_zero() -> TestResult {
    let flat = Candle::new(
        SymbolId(3),
        TimestampNs::new(100)?,
        px(250.0)?,
        px(250.0)?,
        px(250.0)?,
        px(250.0)?,
        42.0,
        Bps::ZERO,
    )?;
    let gk = garman_klass_vol(&flat)?;
    assert_eq!(gk.to_bits(), 0.0_f64.to_bits());
    Ok(())
}

#[test]
fn test_flash_crash_minus_30_percent_bar_and_tick() -> TestResult {
    // 1. Single-bar -30% crash on a Long Meso position
    let sym = SymbolId(1);
    let step = 300_000_000_000_i64;
    let candles = vec![
        Candle::new(
            sym,
            TimestampNs::new(0)?,
            px(100.0)?,
            px(120.0)?,
            px(80.0)?,
            px(100.0)?,
            100.0,
            Bps::ZERO,
        )?,
        Candle::new(
            sym,
            TimestampNs::new(step)?,
            px(100.0)?,
            px(100.8)?,
            px(100.0)?,
            px(100.8)?,
            100.0,
            Bps::ZERO,
        )?,
        // Entry at 100.0, single-bar flash crash to 70.0 (-30%)
        Candle::new(
            sym,
            TimestampNs::new(2 * step)?,
            px(100.0)?,
            px(100.0)?,
            px(70.0)?,
            px(70.0)?,
            5_000.0,
            Bps::ZERO,
        )?,
    ];
    let gk_crash = garman_klass_vol(&candles[2])?;
    assert!(gk_crash.is_finite() && gk_crash > 0.0);

    let meso_cfg = MesoFalsifierConfig {
        horizons_bars: vec![1],
        slippage_bps: Bps::ZERO,
        detector: MesoConfig {
            ema_fast_span: 1,
            ema_slow_span: 10,
            r_comp_threshold: 0.70,
            tsmom_window: 1,
            tsmom_threshold: 0.005,
            funding_warmup: 100,
            ..MesoConfig::default()
        },
        ..MesoFalsifierConfig::default()
    };
    let mut c_reader = VecCandleReader { candles, idx: 0 };
    let (_, meso_trades) = run_meso_falsification_detailed(&mut c_reader, &meso_cfg)?;
    let crash_meso = meso_trades
        .iter()
        .find(|t| t.signal_ts_ns == step)
        .ok_or("expected meso flash-crash trade")?;
    assert_eq!(crash_meso.side, Side::Long);
    assert!(crash_meso.net_bps_vip0.is_finite());
    // -30% = -3000 bps minus 11 bps VIP-0 taker fees = -3011.0 bps
    assert!((crash_meso.net_bps_vip0 - (-3011.0)).abs() < 1e-6);

    // 2. Single-tick -30% crash on a Long Micro position
    let mut events = Vec::new();
    for s in 0..25_i64 {
        let book = BookTop::new(px(99.95)?, px(100.05)?, qty(10.0)?, qty(10.0)?)?;
        events.push(TapeEvent::new(
            sym,
            TimestampNs::new(s * 1_000_000_000)?,
            TapeEventKind::BookSnapshot,
            Side::Long,
            book,
            None,
            None,
            10_000.0,
            None,
        )?);
    }
    // Jump up to trigger Long ShiryaevRobertsJump at t=25s
    events.push(TapeEvent::new(
        sym,
        TimestampNs::new(25_000_000_000)?,
        TapeEventKind::BookSnapshot,
        Side::Long,
        BookTop::new(px(101.0)?, px(101.0)?, qty(10.0)?, qty(10.0)?)?,
        None,
        None,
        10_000.0,
        None,
    )?);
    // Entry at t=26s at ask=100.0
    events.push(TapeEvent::new(
        sym,
        TimestampNs::new(26_000_000_000)?,
        TapeEventKind::BookSnapshot,
        Side::Long,
        BookTop::new(px(100.0)?, px(100.0)?, qty(10.0)?, qty(10.0)?)?,
        None,
        None,
        10_000.0,
        None,
    )?);
    // Exit at t=41s after -30% flash crash to bid=70.0
    events.push(TapeEvent::new(
        sym,
        TimestampNs::new(41_000_000_000)?,
        TapeEventKind::BookSnapshot,
        Side::Short,
        BookTop::new(px(70.0)?, px(70.1)?, qty(10.0)?, qty(10.0)?)?,
        None,
        None,
        10_000.0,
        None,
    )?);

    let micro_cfg = MicroFalsifierConfig {
        horizons_ns: vec![15_000_000_000],
        ..MicroFalsifierConfig::default()
    };
    let mut t_reader = VecTapeReader { events, idx: 0 };
    let (_, micro_trades) = run_micro_falsification_detailed(&mut t_reader, &micro_cfg)?;
    let crash_micro = micro_trades
        .first()
        .ok_or("expected micro flash-crash trade")?;
    assert_eq!(crash_micro.side, Side::Long);
    assert!(crash_micro.net_bps_vip0.is_finite());
    assert!((crash_micro.net_bps_vip0 - (-3011.0)).abs() < 1e-6);
    Ok(())
}

#[test]
fn test_dust_notional_crossed_book_and_invalid_ohlc_rejected() -> TestResult {
    // Dust notional: 1.00 * 0.001 = 0.001 (100_000 e8) < min 10_000_000 e8 (0.10)
    let dust = NotionalE8::from_price_qty(px(1.0)?, qty(0.001)?, 10_000_000);
    assert!(matches!(
        dust,
        Err(DomainError::DustNotional {
            notional_e8: 100_000,
            min_notional_e8: 10_000_000
        })
    ));

    // Crossed book: ask < bid
    let crossed = BookTop::new(px(100.5)?, px(100.0)?, qty(1.0)?, qty(1.0)?);
    assert!(matches!(crossed, Err(DomainError::CrossedBook { .. })));

    // Invalid OHLC: H < C and L > O
    let sym = SymbolId(1);
    let ts = TimestampNs::new(1_000)?;
    let bad_high = Candle::new(
        sym,
        ts,
        px(100.0)?,
        px(104.0)?,
        px(95.0)?,
        px(105.0)?,
        10.0,
        Bps::ZERO,
    );
    assert!(matches!(bad_high, Err(DomainError::InvalidCandle(_))));

    let bad_low = Candle::new(
        sym,
        ts,
        px(100.0)?,
        px(105.0)?,
        px(101.0)?,
        px(102.0)?,
        10.0,
        Bps::ZERO,
    );
    assert!(matches!(bad_low, Err(DomainError::InvalidCandle(_))));
    Ok(())
}

#[test]
fn test_duplicate_and_backward_timestamps_rejected() -> TestResult {
    let sym = SymbolId(1);

    // 1. HawkesProcess rejects backward timestamps
    let mut hp = HawkesProcess::new(1.0, 0.65, 2.0)?;
    hp.on_event(1_000, sym)?;
    assert!(matches!(
        hp.on_event(999, sym),
        Err(DomainError::NonMonotonicTimestamp { .. })
    ));

    // 2. MicroTailEngine rejects backward timestamps
    let book = BookTop::new(px(100.0)?, px(100.1)?, qty(1.0)?, qty(1.0)?)?;
    let ev1 = TapeEvent::new(
        sym,
        TimestampNs::new(2_000)?,
        TapeEventKind::BookSnapshot,
        Side::Long,
        book,
        None,
        None,
        100.0,
        None,
    )?;
    let ev_back = TapeEvent::new(
        sym,
        TimestampNs::new(1_999)?,
        TapeEventKind::BookSnapshot,
        Side::Long,
        book,
        None,
        None,
        100.0,
        None,
    )?;
    let mut micro = MicroTailEngine::new(MicroDetectorConfig::default())?;
    micro.on_event(&ev1)?;
    assert!(matches!(
        micro.on_event(&ev_back),
        Err(DomainError::NonMonotonicTimestamp { .. })
    ));

    // 3. MesoEnergyEngine rejects BOTH duplicate and backward timestamps
    let c1 = Candle::new(
        sym,
        TimestampNs::new(5_000)?,
        px(100.0)?,
        px(101.0)?,
        px(99.0)?,
        px(100.5)?,
        10.0,
        Bps::ZERO,
    )?;
    let c_dup = Candle::new(
        sym,
        TimestampNs::new(5_000)?,
        px(100.5)?,
        px(101.5)?,
        px(100.0)?,
        px(101.0)?,
        10.0,
        Bps::ZERO,
    )?;
    let c_back = Candle::new(
        sym,
        TimestampNs::new(4_999)?,
        px(100.5)?,
        px(101.5)?,
        px(100.0)?,
        px(101.0)?,
        10.0,
        Bps::ZERO,
    )?;
    let mut meso = MesoEnergyEngine::new(MesoConfig::default())?;
    meso.on_candle(&c1)?;
    assert!(matches!(
        meso.on_candle(&c_dup),
        Err(DomainError::NonMonotonicTimestamp { .. })
    ));
    assert!(matches!(
        meso.on_candle(&c_back),
        Err(DomainError::NonMonotonicTimestamp { .. })
    ));

    // 4. CsvCandleReader rejects BOTH duplicate and backward timestamps
    let dir = TempDirGuard::new("csv_ts")?;
    let dup_path = dir.path.join("dup.csv");
    fs::write(
        &dup_path,
        "symbol_id,ts_ns,open,high,low,close,volume,funding_rate_bps\n\
         1,5000,100,101,99,100.5,10,0\n\
         1,5000,100.5,101.5,100,101,10,0\n",
    )?;
    let mut csv_dup = CsvCandleReader::open(&dup_path)?;
    assert!(csv_dup.next_candle()?.is_some());
    assert!(matches!(
        csv_dup.next_candle(),
        Err(DomainError::NonMonotonicTimestamp { .. })
    ));

    let back_path = dir.path.join("back.csv");
    fs::write(
        &back_path,
        "symbol_id,ts_ns,open,high,low,close,volume,funding_rate_bps\n\
         1,5000,100,101,99,100.5,10,0\n\
         1,4999,100.5,101.5,100,101,10,0\n",
    )?;
    let mut csv_back = CsvCandleReader::open(&back_path)?;
    assert!(csv_back.next_candle()?.is_some());
    assert!(matches!(
        csv_back.next_candle(),
        Err(DomainError::NonMonotonicTimestamp { .. })
    ));
    Ok(())
}

#[test]
fn test_corrupted_binary_formats_rejected() -> TestResult {
    let dir = TempDirGuard::new("corrupt_bin")?;

    // 1. PAR2 corrupted magic & FNV-1a checksum
    let row = Par2Row {
        symbol_id: 1,
        par2_kind: 1,
        side: 0,
        ts_exchange_ns: 1_000,
        ts_local_ns: 1_000,
        best_bid_ticks: 100 * (SCALE_E8 as i64),
        best_ask_ticks: 101 * (SCALE_E8 as i64),
        best_bid_qty: SCALE_E8 as i64,
        best_ask_qty: SCALE_E8 as i64,
        ..Par2Row::default()
    };
    let par2_valid = encode_par2_frame(&[row])?;

    let mut par2_bad_magic = par2_valid.clone();
    par2_bad_magic[0] = b'X';
    assert!(matches!(
        decode_par2_frame(&par2_bad_magic),
        Err(DomainError::FormatCorrupted(_))
    ));
    let p_magic = dir.path.join("bad_magic.parquet");
    fs::write(&p_magic, &par2_bad_magic)?;
    assert!(matches!(
        StreamingTapeReader::open(&p_magic, TapeFormat::Par2)?.next_event(),
        Err(DomainError::FormatCorrupted(_))
    ));

    let mut par2_bad_crc = par2_valid.clone();
    par2_bad_crc[16] ^= 0x01;
    assert!(matches!(
        decode_par2_frame(&par2_bad_crc),
        Err(DomainError::FormatCorrupted(_))
    ));
    let p_crc = dir.path.join("bad_crc.parquet");
    fs::write(&p_crc, &par2_bad_crc)?;
    assert!(matches!(
        StreamingTapeReader::open(&p_crc, TapeFormat::Par2)?.next_event(),
        Err(DomainError::FormatCorrupted(_))
    ));

    // 2. VPZ1 corrupted comp_crc and orig_crc
    let vpz_valid = encode_vpz1_frame(1, &par2_valid)?;
    // Header layout: [0..4] magic, [4..12] seq, [12..16] orig_len, [16..20] comp_len,
    //                [20..28] orig_crc, [28..36] comp_crc, [36..] comp
    let mut vpz_bad_comp_crc = vpz_valid.clone();
    vpz_bad_comp_crc[28] ^= 0xFF;
    assert!(matches!(
        read_next_vpz1_frame(&mut Cursor::new(&vpz_bad_comp_crc)),
        Err(DomainError::FormatCorrupted(_))
    ));

    let mut vpz_bad_orig_crc = vpz_valid.clone();
    vpz_bad_orig_crc[20] ^= 0xFF;
    assert!(matches!(
        read_next_vpz1_frame(&mut Cursor::new(&vpz_bad_orig_crc)),
        Err(DomainError::FormatCorrupted(_))
    ));
    let p_vpz = dir.path.join("bad_orig.par2z");
    fs::write(&p_vpz, &vpz_bad_orig_crc)?;
    assert!(matches!(
        StreamingTapeReader::open(&p_vpz, TapeFormat::Vpz1)?.next_event(),
        Err(DomainError::FormatCorrupted(_))
    ));

    // Also test mutated payload with recomputed comp_crc so orig_crc check catches it
    let mut vpz_mut_payload = vpz_valid;
    let payload_last = vpz_mut_payload.len() - 1;
    vpz_mut_payload[payload_last] ^= 0x01;
    let new_comp_crc = fnv1a64(&vpz_mut_payload[36..]);
    vpz_mut_payload[28..36].copy_from_slice(&new_comp_crc.to_le_bytes());
    assert!(matches!(
        read_next_vpz1_frame(&mut Cursor::new(&vpz_mut_payload)),
        Err(DomainError::FormatCorrupted(_))
    ));

    // 3. VKL1 corrupted checksum & corrupted file length
    let vkl_row = Vkl1Row {
        magic: u32::from_le_bytes(VKL1_MAGIC),
        slot_idx: 1,
        tf_code: 1,
        open_ts_ns: 1_000,
        open_ticks: 100 * (SCALE_E8 as i64),
        high_ticks: 101 * (SCALE_E8 as i64),
        low_ticks: 99 * (SCALE_E8 as i64),
        close_ticks: 100 * (SCALE_E8 as i64),
        volume_lots: SCALE_E8 as i64,
        ..Vkl1Row::default()
    };
    let vkl_valid = encode_vkl1_record(&vkl_row);
    let mut vkl_bad_crc = vkl_valid;
    vkl_bad_crc[88] ^= 0x01;
    assert!(matches!(
        decode_vkl1_record(&vkl_bad_crc),
        Err(DomainError::FormatCorrupted(_))
    ));
    let p_vkl_crc = dir.path.join("bad_crc.vkl1");
    fs::write(&p_vkl_crc, vkl_bad_crc)?;
    assert!(matches!(
        StreamingVkl1Reader::open(&p_vkl_crc)?.next_candle(),
        Err(DomainError::FormatCorrupted(_))
    ));

    let p_vkl_len = dir.path.join("bad_len.vkl1");
    fs::write(&p_vkl_len, &vkl_valid[..50])?;
    assert!(matches!(
        StreamingVkl1Reader::open(&p_vkl_len),
        Err(DomainError::FormatCorrupted(_))
    ));
    Ok(())
}

#[test]
fn test_equity_ledger_10000_trades_zero_drift() -> TestResult {
    let initial_e8 = 100_000_i128 * SCALE_E8;
    let mut ledger = EquityLedgerE8::new(initial_e8)?;
    let vip0 = FeeSchedule::bybit_vip0();
    let notional = NotionalE8::new(1_000_000_003)?; // odd notional to exercise ceil fee rounding

    let entry_fee = leg_fee_e8(notional, vip0.taker_bps)?;
    let exit_fee = leg_fee_e8(notional, vip0.maker_bps)?;

    let mut expected_equity = initial_e8;
    for i in 0..10_000_i128 {
        let gross_pnl = if i % 2 == 0 {
            50_000_000 + (i % 97)
        } else {
            -40_000_000 - (i % 53)
        };
        let funding_cost = if i % 3 == 0 {
            125_000
        } else if i % 3 == 1 {
            -75_000
        } else {
            0
        };
        let net = ledger.record_trade(gross_pnl, entry_fee, exit_fee, funding_cost)?;
        expected_equity += gross_pnl - entry_fee - exit_fee - funding_cost;
        assert_eq!(net, gross_pnl - entry_fee - exit_fee - funding_cost);
    }

    assert_eq!(ledger.trade_count(), 10_000);
    assert_eq!(ledger.drift_e8(), Ok(0));
    assert_eq!(ledger.current_equity_e8(), expected_equity);
    Ok(())
}
