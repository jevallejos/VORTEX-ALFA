use std::error::Error;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use vortex_alfa::application::falsifier_mm::{
    default_mm_grid, run_mm_falsification, run_mm_falsification_detailed, MmDetailedOutput,
    MmFalsificationReport, MmFalsifierConfig, MM_FEE_FRONTIER,
};
use vortex_alfa::application::mm_accounting::MmAccountLedger;
use vortex_alfa::application::mm_fifo_sim::{
    MmFifoConfig, MmFifoSimulator, MmFill, OrderState, DEFAULT_MIN_ORDER_LIFE_NS, DEFAULT_RTT_NS,
};
use vortex_alfa::domain::error::DomainError;
use vortex_alfa::domain::ports::MarketTapeReader;
use vortex_alfa::domain::signals::market_making::{
    compute_as_quote, stoikov_microprice, MmEstimatorConfig, MmMarketFeatures, MmQuoteParams,
    MmSignalEngine,
};
use vortex_alfa::domain::stats::GateVerdict;
use vortex_alfa::domain::types::{
    BookTop, Bps, PriceE8, QtyE8, Side, SymbolId, TapeEvent, TapeEventKind, TimestampNs,
    BPS_PER_UNIT, SCALE_E8,
};
use vortex_alfa::infrastructure::binary_codec::{encode_par2_frame, Par2Row};
use vortex_alfa::interfaces::cli::run_cli;

type TestResult = Result<(), Box<dyn Error>>;

const NS_PER_MS: i64 = 1_000_000;
const NS_PER_S: i64 = 1_000_000_000;
const SCALE_F64: f64 = SCALE_E8 as f64;

static TEMP_SEQ: AtomicU64 = AtomicU64::new(1);

struct TempDirGuard {
    path: PathBuf,
}

impl TempDirGuard {
    fn new(prefix: &str) -> Result<Self, Box<dyn Error>> {
        let id = TEMP_SEQ.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "vortex_alfa_mm_{prefix}_{}_{}",
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

impl VecTapeReader {
    fn new(events: Vec<TapeEvent>) -> Self {
        Self { events, idx: 0 }
    }
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

fn px(v: f64) -> Result<PriceE8, DomainError> {
    PriceE8::new((v * SCALE_F64).round() as i64)
}

fn qty(v: f64) -> Result<QtyE8, DomainError> {
    QtyE8::new((v * SCALE_F64).round() as i64)
}

fn book(bid: f64, ask: f64, bid_qty: f64, ask_qty: f64) -> Result<BookTop, DomainError> {
    BookTop::new(px(bid)?, px(ask)?, qty(bid_qty)?, qty(ask_qty)?)
}

fn snap_event(sym: SymbolId, ts_ns: i64, b: BookTop) -> Result<TapeEvent, DomainError> {
    TapeEvent::new(
        sym,
        TimestampNs::new(ts_ns)?,
        TapeEventKind::BookSnapshot,
        Side::Long,
        b,
        None,
        None,
        10_000.0,
        None,
    )
}

fn trade_event(
    sym: SymbolId,
    ts_ns: i64,
    side: Side,
    b: BookTop,
    trade_px: f64,
    trade_qty: f64,
) -> Result<TapeEvent, DomainError> {
    TapeEvent::new(
        sym,
        TimestampNs::new(ts_ns)?,
        TapeEventKind::Trade,
        side,
        b,
        Some(px(trade_px)?),
        Some(qty(trade_qty)?),
        10_000.0,
        None,
    )
}

fn assert_close(actual: f64, expected: f64, tol: f64, label: &str) -> TestResult {
    if !actual.is_finite() || (actual - expected).abs() > tol {
        return Err(format!(
            "{label}: actual={actual:.16}, expected={expected:.16}, diff={:.3e}, tol={tol:.3e}",
            (actual - expected).abs()
        )
        .into());
    }
    Ok(())
}

fn base_quote_params(name: &str) -> MmQuoteParams {
    MmQuoteParams {
        name: name.to_string(),
        gamma: 0.10,
        theta_skew_bps: 3.0,
        min_half_spread_bps: 1.0,
        horizon_s: 45.0,
        k_arrival: 1.2,
        clip_usdt: 100.0,
        max_inv_clips: 4.0,
        tick_e8_override: Some(1_000_000),
    }
}

#[test]
fn test_mm_stoikov_ofi_tfi_as_and_postonly_analytical() -> TestResult {
    // 1. Stoikov L1 microprice closed form: (ask * bid_qty + bid * ask_qty) / (bid_qty + ask_qty)
    let asym_book = book(100.0, 101.0, 3.0, 1.0)?;
    assert_close(
        stoikov_microprice(asym_book),
        100.75,
        1e-12,
        "microprice_asym",
    )?;
    let sym_book = book(100.0, 101.0, 2.0, 2.0)?;
    assert_close(
        stoikov_microprice(sym_book),
        100.50,
        1e-12,
        "microprice_sym",
    )?;

    // 2. 3-step Causal OFI (Cont-Kukanov-Stoikov L1) with exp(-dt / 2.0) decay & warmup gating
    let est_cfg = MmEstimatorConfig {
        tau_ofi_s: 2.0,
        tau_tfi_s: 5.0,
        tau_vol_s: 30.0,
        min_sigma: 1e-6,
        max_sigma: 0.05,
        max_inst_var: 0.05 * 0.05,
        min_book_updates: 4,
    };
    let mut engine = MmSignalEngine::new(est_cfg)?;
    let sym1 = SymbolId(1);
    let q_params = base_quote_params("AS_Test");

    // Step 0 (init): bid=100.0 (2.0), ask=101.0 (3.0)
    let ev0 = snap_event(sym1, 0, book(100.0, 101.0, 2.0, 3.0)?)?;
    let f0 = engine.on_event(&ev0)?;
    assert_eq!(f0.book_updates, 1);
    assert!(!f0.is_warm);
    assert_close(f0.ofi_norm, 0.0, 1e-12, "ofi_step0")?;
    assert_close(f0.tfi_norm, 0.0, 1e-12, "tfi_step0")?;
    assert!(engine.quote_for_event(&ev0, &f0, 0.0, &q_params)?.is_none());

    // Step 1 (t=1.0s): same bid price + larger bid_qty (2.0 -> 5.0), ask unchanged
    let ev1 = snap_event(sym1, NS_PER_S, book(100.0, 101.0, 5.0, 3.0)?)?;
    let f1 = engine.on_event(&ev1)?;
    assert_eq!(f1.book_updates, 2);
    assert!(!f1.is_warm);
    let k1 = (-1.0_f64 / 2.0).exp();
    let ofi_num1 = 0.0 * k1 + 3.0;
    let ofi_den1 = 0.0 * k1 + 3.0;
    assert_close(f1.ofi_norm, ofi_num1 / ofi_den1, 1e-12, "ofi_step1")?;
    assert!(engine.quote_for_event(&ev1, &f1, 0.0, &q_params)?.is_none());

    // Step 2 (t=2.0s): bid price up (100.2, qty=4.0), ask price up (101.2, qty=6.0)
    // CKS: e_b = +bid_qty = +4.0, e_a = +prev_ask_qty = +3.0
    let ev2 = snap_event(sym1, 2 * NS_PER_S, book(100.2, 101.2, 4.0, 6.0)?)?;
    let f2 = engine.on_event(&ev2)?;
    assert_eq!(f2.book_updates, 3);
    assert!(!f2.is_warm);
    let k2 = (-1.0_f64 / 2.0).exp();
    let ofi_num2 = ofi_num1 * k2 + (4.0 + 3.0);
    let ofi_den2 = ofi_den1 * k2 + (4.0 + 3.0);
    assert_close(f2.ofi_norm, ofi_num2 / ofi_den2, 1e-12, "ofi_step2")?;
    assert!(engine.quote_for_event(&ev2, &f2, 0.0, &q_params)?.is_none());

    // Step 3 (t=3.0s): bid price down (100.1, qty=2.5), ask price down (101.1, qty=5.0)
    // CKS: e_b = -prev_bid_qty = -4.0, e_a = -ask_qty = -5.0
    let ev3 = snap_event(sym1, 3 * NS_PER_S, book(100.1, 101.1, 2.5, 5.0)?)?;
    let f3 = engine.on_event(&ev3)?;
    assert_eq!(f3.book_updates, 4);
    assert!(f3.is_warm);
    let k3 = (-1.0_f64 / 2.0).exp();
    let ofi_num3 = ofi_num2 * k3 + (-4.0 - 5.0);
    let ofi_den3 = ofi_den2 * k3 + (4.0 + 5.0);
    let expected_ofi3 = ofi_num3 / ofi_den3;
    assert_close(f3.ofi_norm, expected_ofi3, 1e-12, "ofi_step3")?;
    assert!(engine.quote_for_event(&ev3, &f3, 0.0, &q_params)?.is_some());

    // 3. Causal TFI exponential decay (tau_tfi_s = 5.0) & book changes without Trade
    // Step 4 (t=4.0s): Buy trade (qty=4.0) with unchanged book -> OFI unchanged, TFI = +1.0
    let b3 = book(100.1, 101.1, 2.5, 5.0)?;
    let ev4 = trade_event(sym1, 4 * NS_PER_S, Side::Long, b3, 101.1, 4.0)?;
    let f4 = engine.on_event(&ev4)?;
    assert_eq!(f4.book_updates, 4);
    assert_close(f4.ofi_norm, expected_ofi3, 1e-12, "ofi_unchanged_on_trade")?;
    assert_close(f4.tfi_norm, 1.0, 1e-12, "tfi_buy_init")?;
    assert_close(
        f4.alpha_skew,
        0.5 * expected_ofi3 + 0.5 * 1.0,
        1e-12,
        "alpha_skew_step4",
    )?;

    // Step 5 (t=5.0s): BookSnapshot changing bid_qty (2.5 -> 3.0) without Trade -> TFI MUST NOT change
    let b5 = book(100.1, 101.1, 3.0, 5.0)?;
    let ev5 = snap_event(sym1, 5 * NS_PER_S, b5)?;
    let f5 = engine.on_event(&ev5)?;
    assert_eq!(f5.book_updates, 5);
    let k_ofi5 = (-2.0_f64 / 2.0).exp(); // 2.0s elapsed since last book update at t=3s
    let ofi_num5 = ofi_num3 * k_ofi5 + 0.5;
    let ofi_den5 = ofi_den3 * k_ofi5 + 0.5;
    let expected_ofi5 = ofi_num5 / ofi_den5;
    assert_close(f5.ofi_norm, expected_ofi5, 1e-12, "ofi_step5")?;
    assert_close(f5.tfi_norm, 1.0, 1e-12, "tfi_unchanged_on_book_snapshot")?;

    // Step 6 (t=6.5s, dt_tfi=2.5s since t=4.0s): Sell trade (qty=6.0) with same book b5
    let ev6 = trade_event(sym1, 6_500_000_000, Side::Short, b5, 100.1, 6.0)?;
    let f6 = engine.on_event(&ev6)?;
    let k_tfi = (-2.5_f64 / 5.0).exp();
    let expected_tfi6 = (4.0 * k_tfi - 6.0) / (4.0 * k_tfi + 6.0);
    let expected_alpha6 = 0.5 * expected_ofi5 + 0.5 * expected_tfi6;
    assert_close(f6.ofi_norm, expected_ofi5, 1e-12, "ofi_unchanged_step6")?;
    assert_close(f6.tfi_norm, expected_tfi6, 1e-12, "tfi_decay_step6")?;
    assert_close(f6.alpha_skew, expected_alpha6, 1e-12, "alpha_skew_step6")?;

    // 4. EWMA volatility (tau_vol_s = 30.0, max_inst_var cap) + warmup gating on SymbolId(2)
    let sym2 = SymbolId(2);
    let v_ev0 = snap_event(sym2, 0, book(99.95, 100.05, 1.0, 1.0)?)?; // mid = 100.0
    let vf0 = engine.on_event(&v_ev0)?;
    assert!(!vf0.is_warm);
    assert_close(vf0.sigma_per_sqrt_s, 1e-6, 1e-15, "vol_pre_init")?;

    // Known mid jump 100.0 -> 100.1 over dt = 1.0s
    let v_ev1 = snap_event(sym2, NS_PER_S, book(100.05, 100.15, 1.0, 1.0)?)?;
    let vf1 = engine.on_event(&v_ev1)?;
    let r1 = (100.1_f64 / 100.0_f64).ln();
    let iv1 = (r1 * r1 / 1.0).min(est_cfg.max_inst_var);
    assert_close(
        vf1.sigma_per_sqrt_s,
        iv1.sqrt(),
        1e-12,
        "ewma_vol_after_jump",
    )?;

    // Extreme jump 100.1 -> 120.0 over dt = 1.0s triggers max_inst_var cap with tau_vol_s=30.0 EWMA
    let v_ev2 = snap_event(sym2, 2 * NS_PER_S, book(119.95, 120.05, 1.0, 1.0)?)?;
    let vf2 = engine.on_event(&v_ev2)?;
    let k_vol = (-1.0_f64 / 30.0).exp();
    let expected_var2 = k_vol * iv1 + (1.0 - k_vol) * est_cfg.max_inst_var;
    assert_close(
        vf2.sigma_per_sqrt_s,
        expected_var2.sqrt(),
        1e-12,
        "ewma_vol_capped_ewma",
    )?;

    // 5. Avellaneda-Stoikov equations in homogeneous [bps] units & tick rounding
    let wide_book = book(99.60, 100.20, 2.0, 2.0)?;
    let as_features = MmMarketFeatures {
        symbol_id: sym1,
        ts_ns: TimestampNs::new(10 * NS_PER_S)?,
        s_mid: 99.90,
        s_micro: 100.02,
        ofi_norm: 0.4,
        tfi_norm: 0.4,
        alpha_skew: 0.4,
        sigma_per_sqrt_s: 0.0002,
        tick_e8: 1_000_000,
        book_updates: 10,
        is_warm: true,
    };
    let as_q = compute_as_quote(wide_book, &as_features, 150.0, &q_params)?;
    let sigma_bps = 0.0002 * BPS_PER_UNIT;
    let inv_var_bps = q_params.gamma * sigma_bps * sigma_bps * q_params.horizon_s;
    let q_clips = 150.0 / q_params.clip_usdt;
    let shift_bps = q_params.theta_skew_bps * 0.4 - q_clips * inv_var_bps;
    let expected_res = 100.02 * (1.0 + shift_bps / BPS_PER_UNIT);
    let expected_dh = q_params
        .min_half_spread_bps
        .max(0.5 * inv_var_bps + (1.0 + q_params.gamma / q_params.k_arrival).ln() / q_params.gamma);
    let expected_df = expected_dh / BPS_PER_UNIT;
    let expected_bid = ((expected_res * (1.0 - expected_df)) / 0.01).floor() * 0.01;
    let expected_ask = ((expected_res * (1.0 + expected_df)) / 0.01).ceil() * 0.01;

    assert_close(as_q.reservation_px, expected_res, 1e-12, "as_r_t")?;
    assert_close(as_q.delta_half_bps, expected_dh, 1e-12, "as_delta_half")?;
    assert_close(
        as_q.delta_half_frac,
        expected_df,
        1e-12,
        "as_delta_half_frac",
    )?;
    assert_eq!(as_q.tick_e8, 1_000_000);
    let bid_px = as_q.bid_target.ok_or("expected Some(bid_target)")?;
    let ask_px = as_q.ask_target.ok_or("expected Some(ask_target)")?;
    assert_close(bid_px.as_f64(), expected_bid, 1e-12, "as_bid_tick_floor")?;
    assert_close(ask_px.as_f64(), expected_ask, 1e-12, "as_ask_tick_ceil")?;

    // Strict PostOnly guard under extreme positive and negative skew
    let tight_book = book(100.00, 100.01, 2.0, 2.0)?;
    let skew_params = MmQuoteParams {
        gamma: 0.01,
        theta_skew_bps: 50.0,
        min_half_spread_bps: 0.5,
        horizon_s: 1.0,
        k_arrival: 100.0,
        ..base_quote_params("PostOnly_Skew")
    };
    let pos_skew_f = MmMarketFeatures {
        s_mid: 100.005,
        s_micro: 100.005,
        alpha_skew: 1.0,
        sigma_per_sqrt_s: 1e-6,
        tick_e8: 1_000_000,
        ..as_features
    };
    let pos_q = compute_as_quote(tight_book, &pos_skew_f, 0.0, &skew_params)?;
    let pb = pos_q.bid_target.ok_or("pos_skew bid")?;
    let pa = pos_q.ask_target.ok_or("pos_skew ask")?;
    assert!(pb.raw() <= tight_book.ask().raw() - 1_000_000);
    assert!(pa.raw() >= tight_book.bid().raw() + 1_000_000);
    assert!(pb < pa);

    let neg_skew_f = MmMarketFeatures {
        alpha_skew: -1.0,
        ..pos_skew_f
    };
    let neg_q = compute_as_quote(tight_book, &neg_skew_f, 0.0, &skew_params)?;
    let nb = neg_q.bid_target.ok_or("neg_skew bid")?;
    let na = neg_q.ask_target.ok_or("neg_skew ask")?;
    assert!(nb.raw() <= tight_book.ask().raw() - 1_000_000);
    assert!(na.raw() >= tight_book.bid().raw() + 1_000_000);
    assert!(nb < na);

    // Locked book (bid == ask) widens cleanly around the locked level
    let locked_book = book(100.00, 100.00, 1.0, 1.0)?;
    let locked_f = MmMarketFeatures {
        s_mid: 100.00,
        s_micro: 100.00,
        alpha_skew: 0.0,
        sigma_per_sqrt_s: 1e-6,
        tick_e8: 1_000_000,
        ..as_features
    };
    let zero_spread_params = MmQuoteParams {
        min_half_spread_bps: 0.0,
        gamma: 0.01,
        k_arrival: 1000.0,
        ..base_quote_params("Locked_Widen")
    };
    let locked_q = compute_as_quote(locked_book, &locked_f, 0.0, &zero_spread_params)?;
    let lb = locked_q.bid_target.ok_or("locked bid")?;
    let la = locked_q.ask_target.ok_or("locked ask")?;
    assert!(lb < locked_book.bid());
    assert!(la > locked_book.ask());
    assert!(lb < la);

    // Zero-spread collision on wide book (raw_b == raw_a) widens by tick cleanly
    let wider_book = book(99.90, 100.10, 1.0, 1.0)?;
    let widen_q = compute_as_quote(wider_book, &locked_f, 0.0, &zero_spread_params)?;
    let wb = widen_q.bid_target.ok_or("widen bid")?;
    let wa = widen_q.ask_target.ok_or("widen ask")?;
    assert!(wb < wa);
    assert!(wb < wider_book.ask());
    assert!(wa > wider_book.bid());

    // Inventory bounds: >= max_inv_clips suppresses bid, <= -max_inv_clips suppresses ask
    let max_long = compute_as_quote(wide_book, &as_features, 400.0, &q_params)?;
    assert!(max_long.bid_target.is_none());
    assert!(max_long.ask_target.is_some());
    let over_long = compute_as_quote(wide_book, &as_features, 500.0, &q_params)?;
    assert!(over_long.bid_target.is_none());
    assert!(over_long.ask_target.is_some());

    let max_short = compute_as_quote(wide_book, &as_features, -400.0, &q_params)?;
    assert!(max_short.bid_target.is_some());
    assert!(max_short.ask_target.is_none());
    let over_short = compute_as_quote(wide_book, &as_features, -500.0, &q_params)?;
    assert!(over_short.bid_target.is_some());
    assert!(over_short.ask_target.is_none());

    // 6. Error rejection checks
    let non_mono = snap_event(sym1, 6 * NS_PER_S, b5)?;
    assert!(matches!(
        engine.on_event(&non_mono),
        Err(DomainError::NonMonotonicTimestamp { .. })
    ));

    let bad_gamma = MmQuoteParams {
        gamma: 0.0,
        ..base_quote_params("BadGamma")
    };
    assert!(matches!(
        compute_as_quote(wide_book, &as_features, 0.0, &bad_gamma),
        Err(DomainError::InvalidStatParam("gamma"))
    ));
    let bad_k = MmQuoteParams {
        k_arrival: -1.0,
        ..base_quote_params("BadK")
    };
    assert!(matches!(
        compute_as_quote(wide_book, &as_features, 0.0, &bad_k),
        Err(DomainError::InvalidStatParam("k_arrival"))
    ));
    let bad_name = base_quote_params("   ");
    assert!(matches!(
        compute_as_quote(wide_book, &as_features, 0.0, &bad_name),
        Err(DomainError::InvalidStatParam("mm_name"))
    ));
    let bad_theta = MmQuoteParams {
        theta_skew_bps: -0.01,
        ..base_quote_params("BadTheta")
    };
    assert!(matches!(
        compute_as_quote(wide_book, &as_features, 0.0, &bad_theta),
        Err(DomainError::InvalidStatParam("theta_skew_bps"))
    ));
    assert!(matches!(
        compute_as_quote(wide_book, &as_features, f64::NAN, &q_params),
        Err(DomainError::NonFiniteValue("inventory_notional_usdt"))
    ));
    assert!(matches!(
        compute_as_quote(wide_book, &as_features, f64::INFINITY, &q_params),
        Err(DomainError::NonFiniteValue("inventory_notional_usdt"))
    ));
    Ok(())
}

#[test]
fn test_mm_fifo_queue_conservation_and_5ms_adverse_latency() -> TestResult {
    let sym = SymbolId(1);
    let mut sim = MmFifoSimulator::new(MmFifoConfig::default())?;
    assert_eq!(DEFAULT_RTT_NS, 5 * NS_PER_MS);
    assert_eq!(DEFAULT_MIN_ORDER_LIFE_NS, 100 * NS_PER_MS);

    // Scenario A: FIFO queue conservation (zero phantom fills)
    let b_q = book(100.00, 101.00, 10.0, 10.0)?;
    assert!(sim.on_event(&snap_event(sym, 0, b_q)?)?.is_empty());
    sim.on_quote_decision(sym, 0, Some((px(100.00)?, qty(1.0)?)), None)?;
    assert!(matches!(
        sim.order_states(sym),
        Some((OrderState::PendingPlace { .. }, OrderState::Idle))
    ));

    // Activate at 5ms -> Resting with q_ahead = 10.0
    assert!(sim
        .on_event(&snap_event(sym, 5 * NS_PER_MS, b_q)?)?
        .is_empty());
    assert!(matches!(
        sim.order_states(sym),
        Some((
            OrderState::Resting { q_ahead_e8, .. },
            OrderState::Idle
        )) if q_ahead_e8 == 10 * SCALE_E8 as i64
    ));

    // Partial trade 6.0 at 100.00 -> q_ahead becomes 4.0, 0 fills
    let f_part1 = sim.on_event(&trade_event(
        sym,
        10 * NS_PER_MS,
        Side::Short,
        b_q,
        100.00,
        6.0,
    )?)?;
    assert!(f_part1.is_empty());

    // Partial trade 4.5 at 100.00 (4.0 - 4.5 = -0.5 > -1.0) -> still 0 fills (conservative!)
    let f_part2 = sim.on_event(&trade_event(
        sym,
        15 * NS_PER_MS,
        Side::Short,
        b_q,
        100.00,
        4.5,
    )?)?;
    assert!(f_part2.is_empty());

    // Final trade 0.5 at 100.00 (-0.5 - 0.5 = -1.0 <= -1.0) -> exact 1 fill at 100.00
    let f_full = sim.on_event(&trade_event(
        sym,
        20 * NS_PER_MS,
        Side::Short,
        b_q,
        100.00,
        0.5,
    )?)?;
    assert_eq!(f_full.len(), 1);
    assert_eq!(f_full[0].fill_px, px(100.00)?);
    assert_eq!(f_full[0].qty, qty(1.0)?);
    assert!(!f_full[0].is_adverse_in_flight);
    assert_eq!(
        sim.order_states(sym),
        Some((OrderState::Idle, OrderState::Idle))
    );

    // Scenario B: PendingCancel < 5 ms adverse fill vs safe cancel >= 5 ms
    let b_shallow = book(100.00, 101.00, 0.5, 5.0)?;
    sim.on_quote_decision(sym, 25 * NS_PER_MS, Some((px(100.00)?, qty(1.0)?)), None)?;
    assert!(sim
        .on_event(&snap_event(sym, 30 * NS_PER_MS, b_shallow)?)?
        .is_empty());
    let t0 = 35 * NS_PER_MS;
    sim.on_quote_decision(sym, t0, None, None)?;
    assert!(matches!(
        sim.order_states(sym),
        Some((OrderState::PendingCancel { active_at_ns, .. }, OrderState::Idle))
        if active_at_ns == t0 + 5 * NS_PER_MS
    ));

    // Crossing trade at t0 + 3ms (< 5ms RTT) -> adverse in-flight fill!
    let b_cross = book(99.80, 99.90, 5.0, 5.0)?;
    let adv_fills = sim.on_event(&trade_event(
        sym,
        t0 + 3 * NS_PER_MS,
        Side::Short,
        b_cross,
        99.90,
        1.0,
    )?)?;
    assert_eq!(adv_fills.len(), 1);
    assert!(adv_fills[0].is_adverse_in_flight);
    assert_eq!(sim.metrics().adverse_in_flight_fills, 1);
    assert_eq!(
        sim.order_states(sym),
        Some((OrderState::Idle, OrderState::Idle))
    );

    // Second order canceled at t1, crossing trade arrives at t1 + 6ms (>= 5ms RTT) -> 0 fills
    let t_place2 = 50 * NS_PER_MS;
    assert!(sim
        .on_event(&snap_event(sym, t_place2, b_shallow)?)?
        .is_empty());
    sim.on_quote_decision(sym, t_place2, Some((px(100.00)?, qty(1.0)?)), None)?;
    assert!(sim
        .on_event(&snap_event(sym, t_place2 + 5 * NS_PER_MS, b_shallow)?)?
        .is_empty());
    let t1 = 60 * NS_PER_MS;
    sim.on_quote_decision(sym, t1, None, None)?;
    let safe_fills = sim.on_event(&trade_event(
        sym,
        t1 + 6 * NS_PER_MS,
        Side::Short,
        b_cross,
        99.90,
        1.0,
    )?)?;
    assert!(safe_fills.is_empty());
    assert_eq!(sim.metrics().adverse_in_flight_fills, 1);
    assert_eq!(
        sim.order_states(sym),
        Some((OrderState::Idle, OrderState::Idle))
    );

    // Scenario C: PendingPlace PostOnlyReject at t + 5 ms when book crosses our quote
    let t_p = 80 * NS_PER_MS;
    sim.on_quote_decision(sym, t_p, Some((px(100.00)?, qty(1.0)?)), None)?;
    let b_down = book(99.90, 99.95, 5.0, 5.0)?;
    let rej_fills = sim.on_event(&snap_event(sym, t_p + 5 * NS_PER_MS, b_down)?)?;
    assert!(rej_fills.is_empty());
    assert_eq!(sim.metrics().post_only_rejects, 1);
    assert_eq!(
        sim.order_states(sym),
        Some((OrderState::Idle, OrderState::Idle))
    );

    // Scenario D: Anti-churn guard (min_order_life_ns = 100ms AND requote_threshold_bps = 0.5)
    let t_d0 = 100 * NS_PER_MS;
    sim.on_quote_decision(sym, t_d0, Some((px(100.00)?, qty(1.0)?)), None)?;
    let t_active = t_d0 + 5 * NS_PER_MS;
    assert!(sim.on_event(&snap_event(sym, t_active, b_q)?)?.is_empty());
    assert!(matches!(
        sim.order_states(sym),
        Some((OrderState::Resting { placed_ts_ns, .. }, OrderState::Idle)) if placed_ts_ns == t_active
    ));

    // At +50ms (< 100ms min_order_life_ns): large 50 bps requote is ignored
    sim.on_quote_decision(
        sym,
        t_active + 50 * NS_PER_MS,
        Some((px(99.50)?, qty(1.0)?)),
        None,
    )?;
    assert!(matches!(
        sim.order_states(sym),
        Some((OrderState::Resting { px: cur, .. }, OrderState::Idle)) if cur == px(100.00)?
    ));

    // At +120ms (>= 100ms): sub-0.5 bps move (0.4 bps) WITH changed qty_e8 does NOT trigger PendingReplace
    sim.on_quote_decision(
        sym,
        t_active + 120 * NS_PER_MS,
        Some((px(100.004)?, qty(1.25)?)),
        None,
    )?;
    assert!(matches!(
        sim.order_states(sym),
        Some((OrderState::Resting { px: cur, qty_e8: q, .. }, OrderState::Idle))
            if cur == px(100.00)? && q == qty(1.0)?
    ));

    // At +125ms (>= 100ms): >= 0.5 bps move (0.6 bps) DOES trigger PendingReplace
    sim.on_quote_decision(
        sym,
        t_active + 125 * NS_PER_MS,
        Some((px(100.006)?, qty(1.25)?)),
        None,
    )?;
    assert!(matches!(
        sim.order_states(sym),
        Some((
            OrderState::PendingReplace {
                current_px,
                new_px,
                new_qty_e8,
                ..
            },
            OrderState::Idle
        )) if current_px == px(100.00)? && new_px == px(100.006)? && new_qty_e8 == qty(1.25)?
    ));
    Ok(())
}

#[test]
fn test_mm_markout_lvr_eof_liquidation_breakeven_and_zero_drift() -> TestResult {
    let sym = SymbolId(1);
    let custom_params = MmQuoteParams {
        name: "MM_Exact_Test".to_string(),
        gamma: 1e-5,
        theta_skew_bps: 0.0,
        min_half_spread_bps: 60.0,
        horizon_s: 1.0,
        k_arrival: 1000.0,
        clip_usdt: 100.0,
        max_inv_clips: 4.0,
        tick_e8_override: Some(1_000_000),
    };
    let mm_cfg = MmFalsifierConfig {
        capital_usdt: 1000.0,
        clip_usdt: 100.0,
        max_inv_clips: 4.0,
        max_gross_leverage: 2.0,
        bucket_ns: 60 * NS_PER_S,
        estimator: MmEstimatorConfig {
            min_book_updates: 2,
            ..MmEstimatorConfig::default()
        },
        fifo: MmFifoConfig::default(),
        grid: vec![custom_params],
    };

    // Book at t=0..3s: 99.91 / 101.31 -> mid = 100.61 -> 60 bps bid floor = 100.00
    let b_init = book(99.91, 101.31, 5.0, 5.0)?;
    let events = vec![
        snap_event(sym, 0, b_init)?,
        snap_event(sym, NS_PER_S, b_init)?,
        snap_event(sym, 2 * NS_PER_S, b_init)?,
        trade_event(sym, 3 * NS_PER_S, Side::Short, b_init, 100.00, 1.0)?,
        snap_event(sym, 4 * NS_PER_S, book(99.40, 99.60, 5.0, 5.0)?)?, // +1s: mid = 99.50
        snap_event(
            sym,
            4 * NS_PER_S + 10 * NS_PER_MS,
            book(99.40, 99.60, 5.0, 5.0)?,
        )?, // activate ask=100.10 before +5s
        snap_event(sym, 8 * NS_PER_S, book(99.70, 99.90, 5.0, 5.0)?)?, // +5s: mid = 99.80
        snap_event(sym, 18 * NS_PER_S, book(100.10, 100.30, 5.0, 5.0)?)?, // +15s: mid = 100.20
    ];

    let mut reader = VecTapeReader::new(events);
    let detailed: MmDetailedOutput = run_mm_falsification_detailed(&mut reader, &mm_cfg)?;
    let fills = &detailed.fills_by_config[0];
    assert_eq!(fills.len(), 1);
    assert_eq!(fills[0].side, Side::Long);
    assert_eq!(fills[0].fill_px, px(100.00)?);
    assert_eq!(fills[0].qty, qty(1.0)?);

    let diag = &detailed.report.configs[0];
    assert_eq!(diag.fills_total, 1);
    assert_eq!(diag.accounting_drift_e8, 0);

    // Independent first-principles calculations
    let exp_spread_cap = ((100.61 - 100.00) / 100.00) * 10_000.0;
    let exp_mo_1s = ((99.50 - 100.00) / 100.00) * 10_000.0;
    let exp_mo_5s: f64 = ((99.80 - 100.00) / 100.00) * 10_000.0;
    let exp_mo_15s = ((100.20 - 100.00) / 100.00) * 10_000.0;
    let exp_lvr_5s = (-exp_mo_5s).max(0.0);

    assert_close(
        diag.mean_spread_capture_bps.ok_or("spread_cap")?,
        exp_spread_cap,
        1e-9,
        "spread_capture_bps",
    )?;
    assert_close(
        diag.mean_markout_1s_bps.ok_or("mo1s")?,
        exp_mo_1s,
        1e-9,
        "markout_1s_bps",
    )?;
    assert_close(
        diag.mean_markout_5s_bps.ok_or("mo5s")?,
        exp_mo_5s,
        1e-9,
        "markout_5s_bps",
    )?;
    assert_close(
        diag.mean_markout_15s_bps.ok_or("mo15s")?,
        exp_mo_15s,
        1e-9,
        "markout_15s_bps",
    )?;
    assert_close(
        diag.mean_lvr_5s_bps.ok_or("lvr5s")?,
        exp_lvr_5s,
        1e-9,
        "mean_lvr_5s_bps",
    )?;

    // EOF liquidation at last_book.bid = 100.10 with 5.50 bps Taker fee
    let eof_notional_e8 = 100_1000_0000_i128; // 100.10 * 1.0 * 1e8
    let taker_scaled = 55_000_i128; // 5.50 bps * 10_000
    let fee_denom = 100_000_000_i128;
    let exp_eof_fee_e8 = (eof_notional_e8 * taker_scaled + (fee_denom - 1)) / fee_denom;
    let exp_eof_spread_e8 = 10_000_000_i128; // |100.20 - 100.10| * 1.0 * 1e8 = 0.10 USDT
    let exp_eof_cost_usdt = ((exp_eof_fee_e8 + exp_eof_spread_e8) as f64) / SCALE_F64;
    let exp_gross_e8 = 10_000_000_i128; // (100.10 - 100.00) * 1.0 * 1e8 = +0.10 USDT
    let maker_vol_e8 = 100_0000_0000_i128;
    let exp_breakeven_bps =
        (((exp_gross_e8 - exp_eof_fee_e8) as f64) / (maker_vol_e8 as f64)) * 10_000.0;

    assert_close(
        diag.eof_liq_fee_usdt,
        (exp_eof_fee_e8 as f64) / SCALE_F64,
        1e-12,
        "eof_liq_fee_usdt",
    )?;
    assert_close(
        diag.eof_liq_cost_usdt,
        exp_eof_cost_usdt,
        1e-12,
        "eof_liq_cost_usdt",
    )?;
    assert_close(diag.gross_pnl_usdt, 0.10, 1e-12, "gross_pnl_usdt")?;
    assert_close(
        diag.breakeven_maker_bps.ok_or("breakeven")?,
        exp_breakeven_bps,
        1e-12,
        "breakeven_maker_bps",
    )?;

    // Exact net PnL across VIP0_Maker + all 5 frontier points and strict monotonic ordering
    let expected_frontier_fees_e8 = [2_000_000_i128, 1_000_000, 0, -500_000, -1_000_000];
    assert_eq!(diag.frontier.len(), 5);
    assert_close(
        diag.net_pnl_vip0_usdt,
        ((exp_gross_e8 - exp_eof_fee_e8 - expected_frontier_fees_e8[0]) as f64) / SCALE_F64,
        1e-12,
        "net_pnl_vip0_usdt",
    )?;
    for (pt, &mkr_fee_e8) in diag.frontier.iter().zip(&expected_frontier_fees_e8) {
        let exp_net = ((exp_gross_e8 - exp_eof_fee_e8 - mkr_fee_e8) as f64) / SCALE_F64;
        assert_close(pt.net_pnl_usdt, exp_net, 1e-12, &pt.profile)?;
    }
    for pair in diag.frontier.windows(2) {
        assert!(
            pair[0].net_pnl_usdt < pair[1].net_pnl_usdt,
            "expected strict monotonic frontier: {} ({}) < {} ({})",
            pair[0].profile,
            pair[0].net_pnl_usdt,
            pair[1].profile,
            pair[1].net_pnl_usdt
        );
    }

    // Direct MmAccountLedger test: partial close + flips across zero + EOF liquidation
    let mut ledger = MmAccountLedger::new(1000 * SCALE_E8)?;
    ledger.update_book_and_gross(sym, book(99.90, 100.10, 5.0, 5.0)?);
    ledger.record_fill(sym, Side::Long, px(100.00)?, qty(2.0)?, None)?; // +2.0
    assert_eq!(ledger.accounting_drift_e8()?, 0);
    ledger.record_fill(sym, Side::Short, px(101.00)?, qty(0.5)?, None)?; // partial close -> +1.5
    assert_eq!(ledger.accounting_drift_e8()?, 0);
    ledger.record_fill(sym, Side::Short, px(99.50)?, qty(2.5)?, None)?; // flip across zero -> -1.0
    assert_eq!(ledger.symbol_qty_e8(sym), -SCALE_E8);
    assert_eq!(ledger.accounting_drift_e8()?, 0);
    ledger.record_fill(sym, Side::Long, px(99.00)?, qty(1.5)?, None)?; // flip across zero -> +0.5
    assert_eq!(ledger.symbol_qty_e8(sym), SCALE_E8 / 2);
    assert_eq!(ledger.accounting_drift_e8()?, 0);

    ledger.update_book_and_gross(sym, book(99.20, 99.40, 5.0, 5.0)?);
    ledger.liquidate_eof(Bps::new(5.50)?)?;
    assert_eq!(ledger.symbol_qty_e8(sym), 0);
    assert_eq!(ledger.accounting_drift_e8()?, 0);
    assert!(ledger.breakeven_maker_bps().is_some());
    Ok(())
}

#[test]
fn test_mm_diagnostic_frontier_never_passes_binding_gate() -> TestResult {
    let sym = SymbolId(1);
    let params = MmQuoteParams {
        name: "AS_Tight_Diag".to_string(),
        gamma: 1e-5,
        theta_skew_bps: 0.0,
        min_half_spread_bps: 1.45, // rounds to 9998.50 / 10001.50 -> +1.5 bps/leg = +3.0 bps gross RT
        horizon_s: 1.0,
        k_arrival: 1000.0,
        clip_usdt: 1000.0,
        max_inv_clips: 4.0,
        tick_e8_override: Some(10_000_000), // $0.10 tick at $10,000 mid
    };
    let cfg = MmFalsifierConfig {
        capital_usdt: 2_000.0,
        clip_usdt: 1_000.0,
        max_inv_clips: 4.0,
        max_gross_leverage: 4.0,
        bucket_ns: 60 * NS_PER_S,
        estimator: MmEstimatorConfig {
            min_book_updates: 2,
            ..MmEstimatorConfig::default()
        },
        fifo: MmFifoConfig::default(),
        grid: vec![params],
    };

    let b = book(9_998.00, 10_002.00, 5.0, 5.0)?; // mid = 10_000.00 -> quotes 9998.50 / 10001.50
    let mut events = vec![
        snap_event(sym, 0, b)?,
        snap_event(sym, NS_PER_S, b)?,
        snap_event(sym, NS_PER_S + 10 * NS_PER_MS, b)?,
    ];

    // 20 60-second buckets (>= 18) with slight round-trip count variance so std > 0
    let rts_per_bucket: [usize; 20] = [4, 5, 6, 5, 4, 6, 5, 5, 6, 4, 5, 6, 4, 5, 6, 5, 4, 6, 5, 5];
    for (bucket_idx, &n_rts) in rts_per_bucket.iter().enumerate() {
        let bucket_start = (bucket_idx as i64) * 60 * NS_PER_S;
        if bucket_idx > 0 {
            // Ensure bucket boundary flush and active resting quotes at start of bucket
            events.push(snap_event(sym, bucket_start + 500 * NS_PER_MS, b)?);
        }
        for rt in 0..n_rts {
            let t_base = bucket_start + 2 * NS_PER_S + (rt as i64) * NS_PER_S;
            // Bid fill at 9998.50, Ask fill at 10001.50 -> +3.0 bps gross RT
            events.push(trade_event(sym, t_base, Side::Short, b, 9_998.30, 0.2)?);
            events.push(trade_event(
                sym,
                t_base + 10 * NS_PER_MS,
                Side::Long,
                b,
                10_001.70,
                0.2,
            )?);
            // Reactivate resting bid & ask after 5ms RTT
            events.push(snap_event(sym, t_base + 20 * NS_PER_MS, b)?);
        }
    }

    let mut reader = VecTapeReader::new(events);
    let report = run_mm_falsification(&mut reader, &cfg)?;
    assert_eq!(report.m_hypotheses, 1);
    assert_eq!(report.vip0_cells.len(), 1);
    assert_eq!(report.configs.len(), 1);

    let diag = &report.configs[0];
    assert_eq!(diag.buckets_n, 20);
    assert_eq!(diag.accounting_drift_e8, 0);
    assert_eq!(diag.frontier.len(), MM_FEE_FRONTIER.len());

    // Binding VIP0_Maker loses (-1.0 bps net/RT) and governs GateVerdict::Fail
    let vip0 = &diag.frontier[0];
    assert_eq!(vip0.profile, "VIP0_Maker");
    assert!(vip0.is_binding);
    assert!(vip0.net_pnl_usdt < 0.0);
    assert!(vip0.mean_bucket_bps.ok_or("vip0 mean")? < 0.0);
    assert_eq!(vip0.verdict, GateVerdict::Fail);
    assert_eq!(report.vip0_cells[0].verdict, GateVerdict::Fail);

    // Every non-binding diagnostic profile MUST NEVER produce GateVerdict::Pass even with t_nw >= 3.5 and dsr >= 0.95
    for point in &diag.frontier[1..] {
        assert!(!point.is_binding);
        assert_ne!(
            point.verdict,
            GateVerdict::Pass,
            "non-binding diagnostic {} must never Pass",
            point.profile
        );
        let t_nw = point.t_nw.ok_or("diag t_nw")?;
        let dsr = point.dsr.ok_or("diag dsr")?;
        assert!(
            t_nw >= 3.5,
            "{} expected t_nw >= 3.5, got {t_nw}",
            point.profile
        );
        assert!(
            dsr >= 0.95,
            "{} expected dsr >= 0.95, got {dsr}",
            point.profile
        );
    }
    Ok(())
}

fn build_mm_test_tape(sym: SymbolId) -> Result<Vec<TapeEvent>, DomainError> {
    let mut events = Vec::new();
    let b = book(99.90, 100.10, 5.0, 5.0)?; // mid = 100.00
    for i in 0..25 {
        let ts = (i as i64) * NS_PER_S;
        let drift = if i == 1 { 0.01 } else { 0.0 };
        let cur_b = book(99.90 + drift, 100.10 + drift, 5.0, 5.0)?;
        events.push(snap_event(sym, ts, cur_b)?);
    }
    // Bucket 0 [0, 60s): fills inside [30s, 55s]
    events.push(snap_event(sym, 28 * NS_PER_S, b)?);
    events.push(trade_event(sym, 30 * NS_PER_S, Side::Short, b, 99.50, 2.0)?);
    events.push(snap_event(sym, 31 * NS_PER_S, b)?);
    events.push(trade_event(sym, 35 * NS_PER_S, Side::Long, b, 100.50, 2.0)?);
    events.push(snap_event(sym, 45 * NS_PER_S, b)?);
    events.push(trade_event(sym, 50 * NS_PER_S, Side::Short, b, 99.50, 2.0)?);

    // Bucket 1 [60s, 120s): crosses 60s boundary (closes bucket 0) and cutoff 90s
    events.push(snap_event(sym, 60 * NS_PER_S, b)?);
    events.push(trade_event(sym, 70 * NS_PER_S, Side::Long, b, 100.50, 2.0)?);
    events.push(snap_event(sym, 75 * NS_PER_S, b)?);
    events.push(trade_event(sym, 80 * NS_PER_S, Side::Short, b, 99.50, 2.0)?);
    events.push(snap_event(sym, 90 * NS_PER_S, b)?);

    // Future > cutoff (90s..150s)
    events.push(trade_event(
        sym,
        100 * NS_PER_S,
        Side::Long,
        b,
        100.50,
        2.0,
    )?);
    events.push(snap_event(sym, 120 * NS_PER_S, b)?);
    events.push(trade_event(
        sym,
        130 * NS_PER_S,
        Side::Short,
        b,
        99.50,
        2.0,
    )?);
    events.push(snap_event(sym, 150 * NS_PER_S, b)?);
    Ok(events)
}

#[test]
fn test_mm_zero_lookahead_and_strict_symbol_isolation() -> TestResult {
    // (a) Zero look-ahead end-to-end
    let sym1 = SymbolId(1);
    let base_tape = build_mm_test_tape(sym1)?;
    let cutoff_ns = 90 * NS_PER_S;

    let mut mutated_tape = base_tape.clone();
    for ev in &mut mutated_tape {
        if ev.ts_ns.raw() > cutoff_ns {
            let scaled_bid = PriceE8::new(ev.book.bid().raw().saturating_mul(10))?;
            let scaled_ask = PriceE8::new(ev.book.ask().raw().saturating_mul(10))?;
            ev.book = BookTop::new(scaled_bid, scaled_ask, qty(50.0)?, qty(50.0)?)?;
            ev.side = match ev.side {
                Side::Long => Side::Short,
                Side::Short => Side::Long,
            };
            ev.trade_price = ev
                .trade_price
                .map(|p| PriceE8::new(p.raw().saturating_mul(10)))
                .transpose()?;
            ev.trade_qty = ev
                .trade_qty
                .map(|q| QtyE8::new(q.raw().saturating_mul(100)))
                .transpose()?;
        }
    }

    let cfg = MmFalsifierConfig::default();
    let out_base = run_mm_falsification_detailed(&mut VecTapeReader::new(base_tape), &cfg)?;
    let out_mut = run_mm_falsification_detailed(&mut VecTapeReader::new(mutated_tape), &cfg)?;

    let mut total_pre_cutoff_fills = 0_usize;
    for cfg_idx in 0..cfg.grid.len() {
        let pre_base: Vec<MmFill> = out_base.fills_by_config[cfg_idx]
            .iter()
            .copied()
            .filter(|f| f.fill_ts_ns <= cutoff_ns)
            .collect();
        let pre_mut: Vec<MmFill> = out_mut.fills_by_config[cfg_idx]
            .iter()
            .copied()
            .filter(|f| f.fill_ts_ns <= cutoff_ns)
            .collect();
        assert!(
            !pre_base.is_empty(),
            "expected fills <= cutoff for cfg {cfg_idx}"
        );
        assert_eq!(
            pre_base, pre_mut,
            "fills <= cutoff diverged for config {cfg_idx}"
        );
        total_pre_cutoff_fills += pre_base.len();

        // Bucket 0 [0, 60s) completed at ts=60s <= cutoff_ns -> must be bit-for-bit identical
        let b0_base = out_base.vip0_bucket_returns[cfg_idx][0];
        let b0_mut = out_mut.vip0_bucket_returns[cfg_idx][0];
        assert!(b0_base != 0.0, "expected non-zero bucket 0 return");
        assert_eq!(
            b0_base.to_bits(),
            b0_mut.to_bits(),
            "completed bucket 0 diverged for config {cfg_idx}"
        );
        assert_ne!(
            out_base.vip0_bucket_returns[cfg_idx]
                .last()
                .map(|v| v.to_bits()),
            out_mut.vip0_bucket_returns[cfg_idx]
                .last()
                .map(|v| v.to_bits()),
            "post-cutoff bucket should reflect future mutation for config {cfg_idx}"
        );
    }
    assert!(
        total_pre_cutoff_fills >= cfg.grid.len(),
        "expected fills <= cutoff"
    );

    // (b) Strict SymbolId isolation across MmSignalEngine, MmFifoSimulator, and falsifier_mm
    let sym2 = SymbolId(2);
    let solo_tape = build_mm_test_tape(sym1)?;
    let mut interleaved_tape = Vec::with_capacity(solo_tape.len() * 2);
    for (idx, &ev) in solo_tape.iter().enumerate() {
        interleaved_tape.push(ev);
        let ts2 = ev.ts_ns.raw() + 250 * NS_PER_MS;
        let wild_mid = if idx % 2 == 0 { 5_000.0 } else { 100.0 };
        let wild_b = book(wild_mid - 5.0, wild_mid + 5.0, 200.0, 200.0)?;
        let kind2 = if idx % 3 == 0 {
            TapeEventKind::Liquidation
        } else {
            TapeEventKind::Trade
        };
        let side2 = if idx % 2 == 0 {
            Side::Long
        } else {
            Side::Short
        };
        interleaved_tape.push(TapeEvent::new(
            sym2,
            TimestampNs::new(ts2)?,
            kind2,
            side2,
            wild_b,
            Some(px(wild_mid)?),
            Some(qty(10_000.0)?),
            500_000.0,
            None,
        )?);
    }

    // Engine + FIFO simulator isolation
    let est_cfg = MmEstimatorConfig::default();
    let q_params = default_mm_grid(100.0, 4.0).remove(0);
    let mut eng_solo = MmSignalEngine::new(est_cfg)?;
    let mut eng_inter = MmSignalEngine::new(est_cfg)?;
    let mut sim_solo = MmFifoSimulator::new(MmFifoConfig::default())?;
    let mut sim_inter = MmFifoSimulator::new(MmFifoConfig::default())?;

    let mut solo_features = Vec::new();
    let mut inter_features = Vec::new();
    let mut solo_sim_fills = Vec::new();
    let mut inter_sim_fills = Vec::new();

    for ev in &solo_tape {
        let feat = eng_solo.on_event(ev)?;
        let fills = sim_solo.on_event(ev)?;
        for f in fills {
            solo_sim_fills.push((
                f.symbol_id,
                f.side,
                f.fill_px,
                f.qty,
                f.fill_ts_ns,
                f.mid_at_fill.to_bits(),
                f.spread_capture_bps.to_bits(),
                f.is_adverse_in_flight,
            ));
        }
        let q = eng_solo.quote_for_event(ev, &feat, 0.0, &q_params)?;
        if let Some(target) = q {
            sim_solo.on_quote_decision(
                sym1,
                ev.ts_ns.raw(),
                target
                    .bid_target
                    .map(|p| (p, qty(1.0).unwrap_or(ev.book.bid_qty()))),
                target
                    .ask_target
                    .map(|p| (p, qty(1.0).unwrap_or(ev.book.ask_qty()))),
            )?;
        }
        solo_features.push((feat, q));
    }

    for ev in &interleaved_tape {
        let feat = eng_inter.on_event(ev)?;
        let fills = sim_inter.on_event(ev)?;
        let q = eng_inter.quote_for_event(ev, &feat, 0.0, &q_params)?;
        if let Some(target) = q {
            sim_inter.on_quote_decision(
                ev.symbol_id,
                ev.ts_ns.raw(),
                target
                    .bid_target
                    .map(|p| (p, qty(1.0).unwrap_or(ev.book.bid_qty()))),
                target
                    .ask_target
                    .map(|p| (p, qty(1.0).unwrap_or(ev.book.ask_qty()))),
            )?;
        }
        if ev.symbol_id == sym1 {
            inter_features.push((feat, q));
            for f in fills {
                inter_sim_fills.push((
                    f.symbol_id,
                    f.side,
                    f.fill_px,
                    f.qty,
                    f.fill_ts_ns,
                    f.mid_at_fill.to_bits(),
                    f.spread_capture_bps.to_bits(),
                    f.is_adverse_in_flight,
                ));
            }
        }
    }

    assert!(!solo_sim_fills.is_empty());
    assert_eq!(solo_features, inter_features);
    assert_eq!(solo_sim_fills, inter_sim_fills);
    assert_eq!(sim_solo.order_states(sym1), sim_inter.order_states(sym1));

    // Full falsifier isolation (high global leverage so sym2 50x swings don't trigger portfolio cap)
    let iso_cfg = MmFalsifierConfig {
        max_gross_leverage: 1_000.0,
        ..MmFalsifierConfig::default()
    };
    let res_solo = run_mm_falsification_detailed(&mut VecTapeReader::new(solo_tape), &iso_cfg)?;
    let res_inter =
        run_mm_falsification_detailed(&mut VecTapeReader::new(interleaved_tape), &iso_cfg)?;

    for cfg_idx in 0..iso_cfg.grid.len() {
        assert_eq!(res_inter.report.configs[cfg_idx].accounting_drift_e8, 0);
        let f_solo = &res_solo.fills_by_config[cfg_idx];
        assert!(!f_solo.is_empty());
        let f_inter: Vec<_> = res_inter.fills_by_config[cfg_idx]
            .iter()
            .filter(|f| f.symbol_id == sym1)
            .collect();
        assert_eq!(f_solo.len(), f_inter.len());
        for (a, b) in f_solo.iter().zip(f_inter.iter()) {
            assert_eq!(a.symbol_id, b.symbol_id);
            assert_eq!(a.side, b.side);
            assert_eq!(a.fill_px, b.fill_px);
            assert_eq!(a.qty, b.qty);
            assert_eq!(a.fill_ts_ns, b.fill_ts_ns);
            assert_eq!(a.mid_at_fill.to_bits(), b.mid_at_fill.to_bits());
            assert_eq!(
                a.spread_capture_bps.to_bits(),
                b.spread_capture_bps.to_bits()
            );
            assert_eq!(a.is_adverse_in_flight, b.is_adverse_in_flight);
        }
    }
    Ok(())
}

fn event_to_par2_row(seq: u64, ev: &TapeEvent) -> Par2Row {
    let kind = match ev.kind {
        TapeEventKind::BookSnapshot => 1,
        TapeEventKind::Trade => 2,
        TapeEventKind::TickerMeta => 3,
        TapeEventKind::Liquidation => 4,
    };
    let side = match ev.side {
        Side::Long => 0,
        Side::Short => 1,
    };
    Par2Row {
        symbol_id: ev.symbol_id.0,
        par2_kind: kind,
        side,
        sequence_id: seq,
        ts_exchange_ns: ev.ts_ns.raw(),
        ts_local_ns: ev.ts_ns.raw(),
        price_ticks: ev.trade_price.map_or(0, PriceE8::raw),
        lot_qty: ev.trade_qty.map_or(0, QtyE8::raw),
        best_bid_ticks: ev.book.bid().raw(),
        best_ask_ticks: ev.book.ask().raw(),
        best_bid_qty: ev.book.bid_qty().raw(),
        best_ask_qty: ev.book.ask_qty().raw(),
        ..Par2Row::default()
    }
}

#[test]
fn test_cli_scan_mm_multifile_par2_and_json_report() -> TestResult {
    let dir = TempDirGuard::new("cli_par2")?;
    let par2_dir = dir.path.join("segments");
    fs::create_dir_all(&par2_dir)?;
    let json_out = dir.path.join("mm_report.json");

    let sym = SymbolId(1);
    let b = book(99.90, 100.10, 5.0, 5.0)?;
    let mut events = Vec::new();
    // 22 warmup events + fills spanning 3 60-second buckets
    for i in 0..22 {
        let drift = if i == 1 { 0.01 } else { 0.0 };
        events.push(snap_event(
            sym,
            (i as i64) * NS_PER_S,
            book(99.90 + drift, 100.10 + drift, 5.0, 5.0)?,
        )?);
    }
    events.push(snap_event(sym, 25 * NS_PER_S, b)?);
    events.push(trade_event(sym, 30 * NS_PER_S, Side::Short, b, 99.50, 2.0)?);
    events.push(snap_event(sym, 31 * NS_PER_S, b)?);
    events.push(trade_event(sym, 35 * NS_PER_S, Side::Long, b, 100.50, 2.0)?);
    // Second segment starts at 60s
    let split = events.len();
    events.push(snap_event(sym, 60 * NS_PER_S, b)?);
    events.push(trade_event(sym, 70 * NS_PER_S, Side::Short, b, 99.50, 2.0)?);
    events.push(snap_event(sym, 71 * NS_PER_S, b)?);
    events.push(trade_event(sym, 80 * NS_PER_S, Side::Long, b, 100.50, 2.0)?);
    events.push(snap_event(sym, 120 * NS_PER_S, b)?);
    events.push(trade_event(
        sym,
        130 * NS_PER_S,
        Side::Short,
        b,
        99.50,
        2.0,
    )?);
    events.push(snap_event(sym, 140 * NS_PER_S, b)?);

    let rows1: Vec<Par2Row> = events[..split]
        .iter()
        .enumerate()
        .map(|(i, ev)| event_to_par2_row((i + 1) as u64, ev))
        .collect();
    let rows2: Vec<Par2Row> = events[split..]
        .iter()
        .enumerate()
        .map(|(i, ev)| event_to_par2_row((split + i + 1) as u64, ev))
        .collect();

    fs::write(par2_dir.join("seg_001.parquet"), encode_par2_frame(&rows1)?)?;
    fs::write(par2_dir.join("seg_002.parquet"), encode_par2_frame(&rows2)?)?;

    let ascii = run_cli([
        "vortex-alfa",
        "scan-mm",
        "--format",
        "par2",
        "--input",
        &par2_dir.to_string_lossy(),
        "--capital-usdt",
        "2000",
        "--clip-usdt",
        "100",
        "--max-inv-clips",
        "4",
        "--out",
        &json_out.to_string_lossy(),
    ])?;

    for expected_text in [
        "Track4-MarketMaking",
        "FEE FRONTIER DIAGNOSTICS",
        "VIP0_Maker",
        "AS_Base_Tight",
        "AS_Skew_Deep",
    ] {
        assert!(
            ascii.contains(expected_text),
            "CLI ASCII output missing '{expected_text}'"
        );
    }

    let report: MmFalsificationReport = serde_json::from_str(&fs::read_to_string(&json_out)?)?;
    assert_eq!(report.events_processed, events.len() as u64);
    assert_eq!(report.m_hypotheses, 6);
    assert_eq!(report.configs.len(), 6);
    assert_close(report.capital_usdt, 2000.0, 1e-12, "cli_capital_usdt")?;
    assert_close(report.clip_usdt, 100.0, 1e-12, "cli_clip_usdt")?;
    assert_close(report.max_inv_clips, 4.0, 1e-12, "cli_max_inv_clips")?;
    for cfg in &report.configs {
        assert_eq!(cfg.buckets_n, 3);
        assert!(cfg.fills_total >= 4);
        assert_eq!(cfg.frontier.len(), 5);
        assert_eq!(cfg.accounting_drift_e8, 0);
    }

    // CLI parameter & argument validation rejections
    let par2_str = par2_dir.to_string_lossy().to_string();
    assert!(matches!(
        run_cli([
            "vortex-alfa",
            "scan-mm",
            "--format",
            "par2",
            "--input",
            &par2_str,
            "--capital-usdt",
            "0",
        ]),
        Err(DomainError::Cli(_))
    ));
    assert!(matches!(
        run_cli([
            "vortex-alfa",
            "scan-mm",
            "--format",
            "par2",
            "--input",
            &par2_str,
            "--capital-usdt",
            "-100",
        ]),
        Err(DomainError::Cli(_))
    ));
    assert!(matches!(
        run_cli([
            "vortex-alfa",
            "scan-mm",
            "--format",
            "par2",
            "--input",
            &par2_str,
            "--unknown-flag",
        ]),
        Err(DomainError::Cli(_))
    ));
    assert!(matches!(
        run_cli(["vortex-alfa", "scan-mm", "--input", &par2_str]),
        Err(DomainError::Cli(_))
    ));
    assert!(matches!(
        run_cli(["vortex-alfa", "scan-mm", "--format", "par2"]),
        Err(DomainError::Cli(_))
    ));
    Ok(())
}
