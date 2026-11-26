use std::collections::BTreeMap;
use std::error::Error;
use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use vortex_alfa::application::falsifier_xvenue_carry::run_xvenue_carry_falsification_detailed;
use vortex_alfa::application::xvenue_carry_types::{
    default_xvenue_grid, XVenueBarRecord, XVenueCarryConfig, XVenueCarryReport, XVenueFeeProfiles,
    XVenueStrategySpec, BARS_4H_PER_YEAR, CUMULATIVE_HYPOTHESES_M, DEFAULT_VENUE_IMBALANCE_TRIGGER,
    DEFAULT_XVENUE_BRIDGE_FEE_USDT, DEFAULT_XVENUE_CAPITAL_USDT, DEFAULT_XVENUE_SLIPPAGE_BPS,
};
use vortex_alfa::domain::error::DomainError;
use vortex_alfa::domain::ports::CandleSeriesReader;
use vortex_alfa::domain::signals::carry_uta::payback_apr_hurdle_pct;
use vortex_alfa::domain::signals::xvenue_carry::XVenueCarryEngine;
use vortex_alfa::domain::stats::{t_crit_holm, GateVerdict};
use vortex_alfa::domain::types::{Bps, Candle, PriceE8, SymbolId, TimestampNs, SCALE_E8};
use vortex_alfa::interfaces::cli::run_cli;

type TestResult = Result<(), Box<dyn Error>>;
type DualSliceMap = (BTreeMap<SymbolId, Candle>, BTreeMap<SymbolId, Candle>);

const BAR_4H_NS: i64 = 4 * 3_600 * 1_000_000_000;
const BASE_TS_NS: i64 = 1_704_067_200_000_000_000;
const DAYS_PER_YEAR: f64 = 365.0;
const VIP0_XVENUE_COST_BPS: f64 = 13.00;
const MAKER_DIAG_XVENUE_COST_BPS: f64 = 3.50;

static TEMP_SEQ: AtomicU64 = AtomicU64::new(1);

struct TempDirGuard {
    path: PathBuf,
}

impl TempDirGuard {
    fn new(prefix: &str) -> Result<Self, Box<dyn Error>> {
        let id = TEMP_SEQ.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "vortex_alfa_xvenue_carry_{prefix}_{}_{}",
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

struct VecCandleReader {
    candles: Vec<Candle>,
    idx: usize,
}

impl VecCandleReader {
    const fn new(candles: Vec<Candle>) -> Self {
        Self { candles, idx: 0 }
    }
}

impl CandleSeriesReader for VecCandleReader {
    fn next_candle(&mut self) -> Result<Option<Candle>, DomainError> {
        if self.idx >= self.candles.len() {
            return Ok(None);
        }
        let candle = self.candles[self.idx];
        self.idx += 1;
        Ok(Some(candle))
    }
}

fn px(val: f64) -> Result<PriceE8, DomainError> {
    PriceE8::new((val * (SCALE_E8 as f64)).round() as i64)
}

fn make_candle(
    sym_id: u16,
    ts_ns: i64,
    ohlc: (f64, f64, f64, f64),
    volume: f64,
    funding_rate_bps: f64,
) -> Result<Candle, DomainError> {
    Candle::new(
        SymbolId(sym_id),
        TimestampNs::new(ts_ns)?,
        px(ohlc.0)?,
        px(ohlc.1)?,
        px(ohlc.2)?,
        px(ohlc.3)?,
        volume,
        Bps::new(funding_rate_bps)?,
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

fn write_csv_lf(path: &Path, candles: &[Candle]) -> TestResult {
    let mut file = File::create(path)?;
    file.write_all(b"symbol_id,ts_ns,open,high,low,close,volume,funding_rate_bps\n")?;
    for candle in candles {
        let line = format!(
            "{},{},{:.8},{:.8},{:.8},{:.8},{:.6},{:.8}\n",
            candle.symbol_id.0,
            candle.open_ts_ns.raw(),
            candle.open.as_f64(),
            candle.high.as_f64(),
            candle.low.as_f64(),
            candle.close.as_f64(),
            candle.volume,
            candle.funding_rate_bps.raw(),
        );
        file.write_all(line.as_bytes())?;
    }
    Ok(())
}

fn bar_4h_bps_for_apr(apr_pct: f64) -> f64 {
    apr_pct * 100.0 / BARS_4H_PER_YEAR
}

fn make_dual_slice(
    ts_ns: i64,
    legs: &[(u16, f64, f64, f64, f64)],
) -> Result<DualSliceMap, DomainError> {
    let mut bybit = BTreeMap::new();
    let mut hl = BTreeMap::new();
    for &(sym, byb_px, hl_px, byb_fund_bps, hl_fund_bps) in legs {
        let sid = SymbolId(sym);
        bybit.insert(
            sid,
            make_candle(
                sym,
                ts_ns,
                (byb_px, byb_px, byb_px, byb_px),
                100.0,
                byb_fund_bps,
            )?,
        );
        hl.insert(
            sid,
            make_candle(sym, ts_ns, (hl_px, hl_px, hl_px, hl_px), 100.0, hl_fund_bps)?,
        );
    }
    Ok((bybit, hl))
}

#[test]
fn test_xvenue_bidirectional_basis_funding_and_sign_flip_turnover() -> TestResult {
    // 1A. Closed-form fee schedule & payback APR hurdles for 7d / 14d / 30d at 13.0 bps/turn_L.
    let fee_profiles = XVenueFeeProfiles::new(Bps::new(DEFAULT_XVENUE_SLIPPAGE_BPS)?)?;
    let costs = fee_profiles.xvenue_costs();
    assert_close(costs.vip0_bps, VIP0_XVENUE_COST_BPS, 1e-12, "vip0_cost")?;
    assert_close(
        costs.maker_diag_bps,
        MAKER_DIAG_XVENUE_COST_BPS,
        1e-12,
        "maker_diag_cost",
    )?;

    let expected_7d = (2.0 * VIP0_XVENUE_COST_BPS * DAYS_PER_YEAR) / (100.0 * 7.0);
    let expected_14d = (2.0 * VIP0_XVENUE_COST_BPS * DAYS_PER_YEAR) / (100.0 * 14.0);
    let expected_30d = (2.0 * VIP0_XVENUE_COST_BPS * DAYS_PER_YEAR) / (100.0 * 30.0);
    assert_close(
        payback_apr_hurdle_pct(VIP0_XVENUE_COST_BPS, 7)?,
        expected_7d,
        1e-12,
        "hurdle_7d",
    )?;
    assert_close(
        expected_7d,
        13.557_142_857_142_857,
        1e-9,
        "hurdle_7d_numeric",
    )?;
    assert_close(
        payback_apr_hurdle_pct(VIP0_XVENUE_COST_BPS, 14)?,
        expected_14d,
        1e-12,
        "hurdle_14d",
    )?;
    assert_close(
        expected_14d,
        6.778_571_428_571_428,
        1e-9,
        "hurdle_14d_numeric",
    )?;
    assert_close(
        payback_apr_hurdle_pct(VIP0_XVENUE_COST_BPS, 30)?,
        expected_30d,
        1e-12,
        "hurdle_30d",
    )?;
    assert_close(
        expected_30d,
        3.163_333_333_333_333,
        1e-9,
        "hurdle_30d_numeric",
    )?;

    // 1B. Amortized sign-flip guard in XVenueCarryEngine:
    // Case 1: held_bars < min_hold_bars blocks immediate flip even when -held_apr >= hurdle,
    // causing clean exit (w = 0, empty map) without immediately opening the opposite side.
    let spec_guard = XVenueStrategySpec {
        name: "SignFlip_Guard_Test",
        total_notional_l: 0.50,
        recenter_trigger_pct: 1.0,
        window_bars: 1,
        payback_days: 7,
        check_cadence_bars: 1,
        min_hold_bars: 3,
        max_pairs: 1,
        min_diversification: 1,
        entry_apr_pct: 10.0,
        exit_apr_pct: 0.0,
    };
    let s0 = SymbolId(0);

    let mut eng_hold_guard = XVenueCarryEngine::new(spec_guard, VIP0_XVENUE_COST_BPS)?;
    assert_close(
        eng_hold_guard.payback_hurdle_pct(),
        expected_7d,
        1e-12,
        "engine_hurdle_7d",
    )?;
    // Bar 0: +21.9% APR -> enter +0.50 (held_bars = 0).
    let (b0, h0) = make_dual_slice(BASE_TS_NS, &[(0, 100.0, 100.0, 1.0, 2.0)])?;
    let w0 = eng_hold_guard
        .on_slice(&b0, &h0)?
        .ok_or("expected entry on bar 0")?;
    assert_close(w0[&s0], 0.50, 1e-15, "hold_guard_entry")?;
    // Bar 1: held_bars = 1 (< 3), spread flips to -21.9% APR (-held_apr = 21.9% >= 13.557%).
    // Guard MUST cleanly exit (empty map / w = 0) instead of immediately flipping to -0.50!
    let (b1, h1) = make_dual_slice(BASE_TS_NS + BAR_4H_NS, &[(0, 100.0, 100.0, 2.0, 1.0)])?;
    let w1 = eng_hold_guard
        .on_slice(&b1, &h1)?
        .ok_or("expected weights on bar 1")?;
    assert!(
        w1.is_empty(),
        "expected clean exit w=0 when held_bars < min_hold_bars, got {w1:?}"
    );
    // Bar 2: from flat, same -21.9% APR can now enter -0.50 cleanly.
    let (b2, h2) = make_dual_slice(BASE_TS_NS + 2 * BAR_4H_NS, &[(0, 100.0, 100.0, 2.0, 1.0)])?;
    let w2 = eng_hold_guard
        .on_slice(&b2, &h2)?
        .ok_or("expected short entry from flat on bar 2")?;
    assert_close(w2[&s0], -0.50, 1e-15, "entry_after_clean_exit")?;

    // Case 2a: held_bars >= min_hold_bars, payback hurdle (13.557%) > entry_apr_pct (10.0%).
    // When -held_apr = 12.045% (above entry 10.0% but below hurdle 13.557%), flip is blocked -> w = 0.
    let mut eng_hurdle_guard = XVenueCarryEngine::new(spec_guard, VIP0_XVENUE_COST_BPS)?;
    for bar in 0..=3 {
        let ts = BASE_TS_NS + (bar as i64) * BAR_4H_NS;
        let (f_byb, f_hl) = if bar < 3 { (0.0, 1.0) } else { (0.55, 0.0) };
        let (byb, hl) = make_dual_slice(ts, &[(0, 100.0, 100.0, f_byb, f_hl)])?;
        let out = eng_hurdle_guard
            .on_slice(&byb, &hl)?
            .ok_or("expected slice weights")?;
        if bar < 3 {
            assert_close(out[&s0], 0.50, 1e-15, "pre_hurdle_guard_long")?;
        } else {
            assert!(
                out.is_empty(),
                "expected clean exit w=0 when -held_apr < payback_hurdle, got {out:?}"
            );
        }
    }

    // Case 2b: held_bars >= min_hold_bars, entry_apr_pct (20.0%) > payback hurdle (13.557%).
    // When -held_apr = 16.425% (above hurdle 13.557% but below entry 20.0%), flip is blocked -> w = 0.
    let spec_entry_max = XVenueStrategySpec {
        entry_apr_pct: 20.0,
        ..spec_guard
    };
    let mut eng_entry_guard = XVenueCarryEngine::new(spec_entry_max, VIP0_XVENUE_COST_BPS)?;
    for bar in 0..=3 {
        let ts = BASE_TS_NS + (bar as i64) * BAR_4H_NS;
        let (f_byb, f_hl) = if bar < 3 { (0.0, 1.5) } else { (0.75, 0.0) };
        let (byb, hl) = make_dual_slice(ts, &[(0, 100.0, 100.0, f_byb, f_hl)])?;
        let out = eng_entry_guard
            .on_slice(&byb, &hl)?
            .ok_or("expected slice weights")?;
        if bar < 3 {
            assert_close(out[&s0], 0.50, 1e-15, "pre_entry_max_long")?;
        } else {
            assert!(
                out.is_empty(),
                "expected clean exit w=0 when -held_apr < entry_apr_pct, got {out:?}"
            );
        }
    }

    // 1C. Closed-form bidirectional d = +1 & d = -1 basis/funding and direct flip (+L -> -L)
    // once held_bars >= min_hold_bars AND -held_apr >= max(entry_apr_pct, payback_hurdle_pct),
    // verifying exact rebalance_turn_l == 2.0 * L.
    let mut byb_candles = Vec::new();
    let mut hl_candles = Vec::new();
    // Slice 0 (decision): f_hl(2.0) > f_byb(1.0) => +21.9% APR => d = +1 (w = +0.50).
    // Slice 1 (entry d=+1): Open = (100, 100), funding stays +21.9% (held_bars=1 < 3).
    // Slice 2 (settle d=+1): Bybit=102 (+2%), HL=99 (-1%), f_byb=1.0, f_hl=4.0 => w=+0.50,
    //   basis = +0.50*(0.02 - (-0.01))*10_000 = +150.0 bps, fund_spread = +0.50*(4.0-1.0) = +1.5 bps.
    // Slice 3 (settle d=+1, decision flip!): Bybit=102, HL=99, f_byb=3.0, f_hl=1.0 (diff=-2.0 => -43.8%).
    //   held_bars = 3 >= 3 and |-43.8%| >= 13.557% => direct flip +0.50 -> -0.50!
    // Slice 4 (entry d=-1): Open = (100, 100), f_byb=3.0, f_hl=1.0.
    // Slice 5 (settle d=-1 positive basis): Bybit=98 (-2%), HL=103 (+3%), f_byb=4.0, f_hl=1.0 =>
    //   w=-0.50, rebalance_turn_l=1.00, basis=(-0.50)*(-0.02-0.03)*10_000 = +250.0 bps,
    //   fund_spread=(-0.50)*(1.0-4.0) = +1.5 bps.
    // Slice 6 (settle d=-1 negative basis & negative fund spread):
    //   Bybit 98 -> 99.96 (+2%), HL 103 -> 103 (0%), f_byb=1.0, f_hl=3.0 =>
    //   basis=(-0.50)*(0.02 - 0.0)*10_000 = -100.0 bps, fund_spread=(-0.50)*(3.0 - 1.0) = -1.0 bps.
    let timeline = [
        (0, 100.0, 100.0, 1.0, 2.0),
        (1, 100.0, 100.0, 1.0, 2.0),
        (2, 102.0, 99.0, 1.0, 4.0),
        (3, 102.0, 99.0, 3.0, 1.0),
        (4, 100.0, 100.0, 3.0, 1.0),
        (5, 98.0, 103.0, 4.0, 1.0),
        (6, 99.96, 103.0, 1.0, 3.0),
    ];
    for &(bar, byb_px, hl_px, f_byb, f_hl) in &timeline {
        let ts = BASE_TS_NS + (bar as i64) * BAR_4H_NS;
        byb_candles.push(make_candle(
            0,
            ts,
            (byb_px, byb_px, byb_px, byb_px),
            100.0,
            f_byb,
        )?);
        hl_candles.push(make_candle(
            0,
            ts,
            (hl_px, hl_px, hl_px, hl_px),
            100.0,
            f_hl,
        )?);
    }
    let cfg = XVenueCarryConfig {
        strategies: vec![spec_guard],
        slippage_bps: Bps::new(DEFAULT_XVENUE_SLIPPAGE_BPS)?,
        ..XVenueCarryConfig::default()
    };
    let (_, recs) = run_xvenue_carry_falsification_detailed(
        &mut VecCandleReader::new(byb_candles),
        &mut VecCandleReader::new(hl_candles),
        &cfg,
    )?;
    // Records settle bars: 1->2, 2->3, 3->4, 4->5, 5->6, 6 EOF => 6 records.
    assert_eq!(recs.len(), 6);

    // Record 0 (t1 -> t2): d = +1
    let r_pos = &recs[0];
    assert_close(r_pos.weights[&s0], 0.50, 1e-15, "r_pos_w")?;
    assert_close(r_pos.rebalance_turn_l, 0.50, 1e-15, "r_pos_entry_turn")?;
    assert_close(r_pos.basis_bps, 150.0, 1e-10, "r_pos_basis")?;
    assert_close(r_pos.fund_spread_bps, 1.5, 1e-12, "r_pos_fund")?;

    // Record 2 (t3 -> t4): still d = +1 (opened at t3 from decision at t2); decision at t3 triggers flip for t4!
    assert_close(recs[2].weights[&s0], 0.50, 1e-15, "r_pre_flip_w")?;

    // Record 3 (t4 -> t5): direct flip +0.50 -> -0.50 (d = -1), exact rebalance_turn_l == 2.0 * 0.50 == 1.00!
    let r_flip = &recs[3];
    assert_close(r_flip.weights[&s0], -0.50, 1e-15, "r_flip_w")?;
    assert_close(r_flip.rebalance_turn_l, 1.00, 1e-15, "r_flip_turn_2L")?;
    assert_close(r_flip.recenter_turn_l, 0.0, 1e-15, "r_flip_no_recenter")?;
    assert_close(
        r_flip.cost_vip0_bps,
        1.00 * VIP0_XVENUE_COST_BPS,
        1e-12,
        "r_flip_cost_vip0",
    )?;
    assert_close(r_flip.basis_bps, 250.0, 1e-10, "r_flip_pos_basis")?;
    assert_close(r_flip.fund_spread_bps, 1.5, 1e-12, "r_flip_pos_fund")?;

    // Record 4 (t5 -> t6): d = -1 with Bybit outperforming HL and f_hl > f_bybit => both negative.
    let r_neg = &recs[4];
    assert_close(r_neg.weights[&s0], -0.50, 1e-15, "r_neg_w")?;
    assert_close(r_neg.rebalance_turn_l, 0.0, 1e-15, "r_neg_turn")?;
    assert_close(r_neg.basis_bps, -100.0, 1e-10, "r_neg_basis")?;
    assert_close(r_neg.fund_spread_bps, -1.0, 1e-12, "r_neg_fund")?;

    Ok(())
}

#[test]
fn test_xvenue_split_margin_recenter_and_bridge_toll() -> TestResult {
    // 2A. Bilateral split-margin recentering across 0.60, 0.30, 0.18 tiers,
    // triggered independently by Bybit move (+60% at bar 2) and HL moves (+18% bar 4, +30% bar 5, +60% bar 6),
    // verifying recenter_events == 1, recenter_turn_l == 2.0 * |w_i|, VIP-0 cost delta 2 * |w_i| * 13.0,
    // and entry price reset so subsequent flat bars do not re-trigger.
    let make_rec_spec = |name: &'static str, l: f64, trig: f64| XVenueStrategySpec {
        name,
        total_notional_l: l,
        recenter_trigger_pct: trig,
        window_bars: 1,
        payback_days: 7,
        check_cadence_bars: 100,
        min_hold_bars: 0,
        max_pairs: 1,
        min_diversification: 1,
        entry_apr_pct: 5.0,
        exit_apr_pct: 0.0,
    };
    let spec_60 = make_rec_spec("Tier_60", 0.50, 0.60);
    let spec_30 = make_rec_spec("Tier_30", 1.00, 0.30);
    let spec_18 = make_rec_spec("Tier_18", 1.50, 0.18);

    // Bars 0..=8:
    // 0,1: (100, 100) entry
    // 2:   (160, 100) -> Bybit +60% triggers ALL 3 tiers; resets entry_bybit_px=160, entry_hl_px=100
    // 3:   (160, 100) -> flat after Bybit recenter => 0 recenter for all 3
    // 4:   (160, 118) -> HL +18% triggers ONLY Tier_18; resets entry_hl_px=118 for Tier_18
    // 5:   (160, 130) -> HL +30% from 100 triggers Tier_30 (118->130 is +10.17% < 0.18, so Tier_18 stays 0)
    // 6:   (160, 160) -> HL +60% from 100 triggers Tier_60 (118->160 is +35.6% >= 0.18, so Tier_18 too)
    // 7,8: (160, 160) -> flat after HL recenter => 0 recenter for all 3
    let path_rec = [
        (0, 100.0, 100.0),
        (1, 100.0, 100.0),
        (2, 160.0, 100.0),
        (3, 160.0, 100.0),
        (4, 160.0, 118.0),
        (5, 160.0, 130.0),
        (6, 160.0, 160.0),
        (7, 160.0, 160.0),
        (8, 160.0, 160.0),
    ];
    let mut rec_byb = Vec::with_capacity(path_rec.len());
    let mut rec_hl = Vec::with_capacity(path_rec.len());
    for &(bar, b_px, h_px) in &path_rec {
        let ts = BASE_TS_NS + (bar as i64) * BAR_4H_NS;
        rec_byb.push(make_candle(0, ts, (b_px, b_px, b_px, b_px), 100.0, 0.0)?);
        rec_hl.push(make_candle(0, ts, (h_px, h_px, h_px, h_px), 100.0, 5.0)?);
    }
    let cfg_rec = XVenueCarryConfig {
        strategies: vec![spec_60, spec_30, spec_18],
        slippage_bps: Bps::new(DEFAULT_XVENUE_SLIPPAGE_BPS)?,
        ..XVenueCarryConfig::default()
    };
    let (rep_rec, recs_rec) = run_xvenue_carry_falsification_detailed(
        &mut VecCandleReader::new(rec_byb),
        &mut VecCandleReader::new(rec_hl),
        &cfg_rec,
    )?;

    let by_strat = |name: &str| -> Vec<&XVenueBarRecord> {
        recs_rec.iter().filter(|r| r.strategy == name).collect()
    };
    let r60 = by_strat("Tier_60");
    let r30 = by_strat("Tier_30");
    let r18 = by_strat("Tier_18");
    assert_eq!(r60.len(), 8);
    assert_eq!(r30.len(), 8);
    assert_eq!(r18.len(), 8);

    // Record index 1 opens at bar 2 (Bybit +60%), Record index 2 opens at bar 3 (flat),
    // Record index 3 opens at bar 4 (HL +18%), Record index 4 opens at bar 5 (HL +30%),
    // Record index 5 opens at bar 6 (HL +60%), Record index 6 opens at bar 7 (flat).
    let check_recenter_bar = |rec: &XVenueBarRecord, w_abs: f64, label: &str| -> TestResult {
        assert_eq!(rec.recenter_events, 1, "{label} events");
        assert_close(rec.recenter_turn_l, 2.0 * w_abs, 1e-15, label)?;
        assert_close(rec.rebalance_turn_l, 0.0, 1e-15, label)?;
        assert_close(
            rec.cost_vip0_bps,
            2.0 * w_abs * VIP0_XVENUE_COST_BPS,
            1e-12,
            label,
        )?;
        Ok(())
    };
    let check_no_recenter = |rec: &XVenueBarRecord, label: &str| -> TestResult {
        assert_eq!(rec.recenter_events, 0, "{label} events");
        assert_close(rec.recenter_turn_l, 0.0, 1e-15, label)?;
        Ok(())
    };

    // Bybit +60% triggers all three; flat bar 3 triggers none.
    check_recenter_bar(r60[1], 0.50, "t60_byb")?;
    check_recenter_bar(r30[1], 1.00, "t30_byb")?;
    check_recenter_bar(r18[1], 1.50, "t18_byb")?;
    check_no_recenter(r60[2], "t60_flat_after_byb")?;
    check_no_recenter(r30[2], "t30_flat_after_byb")?;
    check_no_recenter(r18[2], "t18_flat_after_byb")?;

    // HL +18% triggers only Tier_18.
    check_no_recenter(r60[3], "t60_hl18")?;
    check_no_recenter(r30[3], "t30_hl18")?;
    check_recenter_bar(r18[3], 1.50, "t18_hl18")?;

    // HL +30% triggers Tier_30 only.
    check_no_recenter(r60[4], "t60_hl30")?;
    check_recenter_bar(r30[4], 1.00, "t30_hl30")?;
    check_no_recenter(r18[4], "t18_hl30")?;

    // HL +60% triggers Tier_60 (and Tier_18); flat bar 7 triggers none.
    check_recenter_bar(r60[5], 0.50, "t60_hl60")?;
    check_no_recenter(r30[5], "t30_hl60")?;
    check_recenter_bar(r18[5], 1.50, "t18_hl60")?;
    check_no_recenter(r60[6], "t60_flat_after_hl")?;
    check_no_recenter(r30[6], "t30_flat_after_hl")?;
    check_no_recenter(r18[6], "t18_flat_after_hl")?;

    assert_eq!(rep_rec.diagnostics[0].recenter_count, 2);
    assert_eq!(rep_rec.diagnostics[1].recenter_count, 2);
    assert_eq!(rep_rec.diagnostics[2].recenter_count, 3);

    // Also verify negative price move (-30% on HL) triggers bilateral recentering cleanly.
    let mut neg_byb = Vec::new();
    let mut neg_hl = Vec::new();
    for (bar, h_px) in [(0, 100.0), (1, 100.0), (2, 70.0), (3, 70.0), (4, 70.0)] {
        let ts = BASE_TS_NS + (bar as i64) * BAR_4H_NS;
        neg_byb.push(make_candle(
            0,
            ts,
            (100.0, 100.0, 100.0, 100.0),
            100.0,
            0.0,
        )?);
        neg_hl.push(make_candle(0, ts, (h_px, h_px, h_px, h_px), 100.0, 5.0)?);
    }
    let cfg_neg = XVenueCarryConfig {
        strategies: vec![spec_30],
        ..cfg_rec
    };
    let (_, neg_recs) = run_xvenue_carry_falsification_detailed(
        &mut VecCandleReader::new(neg_byb),
        &mut VecCandleReader::new(neg_hl),
        &cfg_neg,
    )?;
    check_recenter_bar(&neg_recs[1], 1.00, "t30_hl_neg30")?;
    check_no_recenter(&neg_recs[2], "t30_flat_after_hl_neg30")?;

    // 2B. Closed-form venue_imbalance_frac accumulation & bridge toll:
    // w_i * (r_bybit - f_bybit / 10_000) across both positive (>= +0.15) and negative (<= -0.15)
    // crossings, verifying $2,000 -> 5.0 bps and $1,000 -> 10.0 bps, bridge_count, and reset to 0.0.
    let spec_bridge = make_rec_spec("Bridge_Verify", 1.00, 5.0);
    let bridge_path = [
        // 0: decision (+1.0), 1: entry at (100, 100)
        (0, 100.0, 100.0, 0.0, 5.0),
        (1, 100.0, 100.0, 0.0, 5.0),
        // 2 (settles r0): Bybit +12%, f_byb = +200 bps => pnl_frac = 1*(0.12 - 0.02) = +0.10 (< 0.15)
        (2, 112.0, 100.0, 200.0, 200.0),
        // 3 (settles r1): Bybit flat (0%), f_byb = -600 bps => pnl_frac = 1*(0 - (-0.06)) = +0.06
        //   cumulative imbalance = +0.16 >= +0.15 => FIRES bridge #1, resets to 0.0!
        (3, 112.0, 100.0, -600.0, 0.0),
        // 4 (settles r2): Bybit 112 -> 100.8 (-10%), f_byb = 0 => pnl_frac = -0.10 (> -0.15)
        (4, 100.8, 100.0, 0.0, 0.0),
        // 5 (settles r3): Bybit flat (0%), f_byb = +600 bps => pnl_frac = -0.06
        //   cumulative imbalance = -0.16 <= -0.15 => FIRES bridge #2, resets to 0.0!
        (5, 100.8, 100.0, 600.0, 0.0),
    ];
    let mut b_byb = Vec::with_capacity(bridge_path.len());
    let mut b_hl = Vec::with_capacity(bridge_path.len());
    for &(bar, byb_px, hl_px, f_byb, f_hl) in &bridge_path {
        let ts = BASE_TS_NS + (bar as i64) * BAR_4H_NS;
        b_byb.push(make_candle(
            0,
            ts,
            (byb_px, byb_px, byb_px, byb_px),
            100.0,
            f_byb,
        )?);
        b_hl.push(make_candle(
            0,
            ts,
            (hl_px, hl_px, hl_px, hl_px),
            100.0,
            f_hl,
        )?);
    }

    let cfg_2k = XVenueCarryConfig {
        strategies: vec![spec_bridge],
        capital_usdt: DEFAULT_XVENUE_CAPITAL_USDT,
        bridge_fee_usdt: DEFAULT_XVENUE_BRIDGE_FEE_USDT,
        venue_imbalance_trigger: DEFAULT_VENUE_IMBALANCE_TRIGGER,
        ..XVenueCarryConfig::default()
    };
    let (rep_2k, recs_2k) = run_xvenue_carry_falsification_detailed(
        &mut VecCandleReader::new(b_byb.clone()),
        &mut VecCandleReader::new(b_hl.clone()),
        &cfg_2k,
    )?;
    assert_eq!(recs_2k.len(), 5);
    assert_eq!(rep_2k.diagnostics[0].bridge_count, 2);

    // r0: imbalance = +0.10 (< 0.15) -> no bridge
    assert_eq!(recs_2k[0].bridge_events, 0);
    assert_close(recs_2k[0].bridge_bps, 0.0, 1e-15, "r0_no_bridge")?;
    assert_close(recs_2k[0].venue_imbalance_after, 0.10, 1e-12, "r0_imb_010")?;

    // r1: imbalance hits +0.16 (>= 0.15) -> fires bridge (5.0 bps at $2k), resets to 0.0
    assert_eq!(recs_2k[1].bridge_events, 1);
    assert_close(recs_2k[1].bridge_bps, 5.0, 1e-12, "r1_bridge_5bps_2k")?;
    assert_close(recs_2k[1].venue_imbalance_after, 0.0, 1e-15, "r1_imb_reset")?;

    // r2: imbalance = -0.10 (> -0.15) -> no bridge
    assert_eq!(recs_2k[2].bridge_events, 0);
    assert_close(recs_2k[2].bridge_bps, 0.0, 1e-15, "r2_no_bridge")?;
    assert_close(
        recs_2k[2].venue_imbalance_after,
        -0.10,
        1e-12,
        "r2_imb_neg010",
    )?;

    // r3: imbalance hits -0.16 (<= -0.15) -> fires bridge (5.0 bps at $2k), resets to 0.0
    assert_eq!(recs_2k[3].bridge_events, 1);
    assert_close(recs_2k[3].bridge_bps, 5.0, 1e-12, "r3_bridge_5bps_2k")?;
    assert_close(recs_2k[3].venue_imbalance_after, 0.0, 1e-15, "r3_imb_reset")?;

    // Re-run at capital_usdt = $1,000 ($1 / $1k = 10.0 bps).
    let cfg_1k = XVenueCarryConfig {
        capital_usdt: 1_000.0,
        ..cfg_2k
    };
    let (rep_1k, recs_1k) = run_xvenue_carry_falsification_detailed(
        &mut VecCandleReader::new(b_byb),
        &mut VecCandleReader::new(b_hl),
        &cfg_1k,
    )?;
    assert_eq!(rep_1k.diagnostics[0].bridge_count, 2);
    assert_close(recs_1k[1].bridge_bps, 10.0, 1e-12, "r1_bridge_10bps_1k")?;
    assert_close(recs_1k[3].bridge_bps, 10.0, 1e-12, "r3_bridge_10bps_1k")?;
    assert_close(
        recs_2k[1].net_vip0_bps - recs_1k[1].net_vip0_bps,
        5.0,
        1e-10,
        "net_delta_2k_vs_1k_r1",
    )?;
    assert_close(
        recs_2k[3].net_vip0_bps - recs_1k[3].net_vip0_bps,
        5.0,
        1e-10,
        "net_delta_2k_vs_1k_r3",
    )?;

    Ok(())
}

#[test]
fn test_xvenue_bar_open_settlement_eof_maker_diag_and_cum116() -> TestResult {
    // 3A. Exact BarOpenSettlement & EOF Close(last) accounting:
    // - Regular settle credits slice[k+1] funding and ignores entry slice[k] funding
    // - EOF Close(last) ignores slice[last] funding (0.0) and charges eof_close_turn_l == sum |w_i|
    //   at 13.0 bps/turn_L (VIP0) and 3.50 bps/turn_L (MakerDiag).
    let spec_3a = XVenueStrategySpec {
        name: "Exact_BarOpen_EOF",
        total_notional_l: 0.80,
        recenter_trigger_pct: 1.0,
        window_bars: 1,
        payback_days: 7,
        check_cadence_bars: 10,
        min_hold_bars: 0,
        max_pairs: 2,
        min_diversification: 2,
        entry_apr_pct: 5.0,
        exit_apr_pct: 0.0,
    };
    let ts_0 = BASE_TS_NS;
    let ts_1 = BASE_TS_NS + BAR_4H_NS;
    let ts_2 = BASE_TS_NS + 2 * BAR_4H_NS;
    let ts_3 = BASE_TS_NS + 3 * BAR_4H_NS;

    let byb_3a = vec![
        // t0: decision slice -> w0 = +0.40, w1 = +0.40
        make_candle(0, ts_0, (100.0, 100.0, 100.0, 100.0), 100.0, 0.0)?,
        make_candle(1, ts_0, (200.0, 200.0, 200.0, 200.0), 100.0, 0.0)?,
        // t1: entry Open(1); poison funding MUST NOT affect t1 -> t2
        make_candle(0, ts_1, (100.0, 100.0, 100.0, 100.0), 100.0, 999.0)?,
        make_candle(1, ts_1, (200.0, 200.0, 200.0, 200.0), 100.0, -888.0)?,
        // t2: exit Open(2): Sym0 +5% (105), Sym1 +3% (206); Bybit funding = 2.0, 1.0
        make_candle(0, ts_2, (105.0, 105.0, 105.0, 105.0), 100.0, 2.0)?,
        make_candle(1, ts_2, (206.0, 206.0, 206.0, 206.0), 100.0, 1.0)?,
        // t3 (last): Open(3) settles t2->t3; Close(3) settles EOF Open(3)->Close(3) (+3% & +4%)
        // and MUST ignore t3 funding!
        make_candle(0, ts_3, (105.0, 108.15, 105.0, 108.15), 100.0, 10.0)?,
        make_candle(1, ts_3, (206.0, 214.24, 206.0, 214.24), 100.0, 5.0)?,
    ];
    let hl_3a = vec![
        make_candle(0, ts_0, (100.0, 100.0, 100.0, 100.0), 100.0, 5.0)?,
        make_candle(1, ts_0, (200.0, 200.0, 200.0, 200.0), 100.0, 6.0)?,
        make_candle(0, ts_1, (100.0, 100.0, 100.0, 100.0), 100.0, -777.0)?,
        make_candle(1, ts_1, (200.0, 200.0, 200.0, 200.0), 100.0, 666.0)?,
        // t2: Sym0 +2% (102), Sym1 +5% (210); HL funding = 14.0, 9.0
        make_candle(0, ts_2, (102.0, 102.0, 102.0, 102.0), 100.0, 14.0)?,
        make_candle(1, ts_2, (210.0, 210.0, 210.0, 210.0), 100.0, 9.0)?,
        // t3 (last): Open(3) flat, Close(3) +2% & +1%; HL funding = 110.0, 55.0
        make_candle(0, ts_3, (102.0, 104.04, 102.0, 104.04), 100.0, 110.0)?,
        make_candle(1, ts_3, (210.0, 212.10, 210.0, 212.10), 100.0, 55.0)?,
    ];
    let cfg_3a = XVenueCarryConfig {
        strategies: vec![spec_3a],
        ..XVenueCarryConfig::default()
    };
    let (_, recs_3a) = run_xvenue_carry_falsification_detailed(
        &mut VecCandleReader::new(byb_3a),
        &mut VecCandleReader::new(hl_3a),
        &cfg_3a,
    )?;
    assert_eq!(recs_3a.len(), 3);

    // Record 0 (t1 -> t2):
    // basis = 0.40*(0.05-0.02)*10_000 + 0.40*(0.03-0.05)*10_000 = 120 - 80 = 40.0 bps
    // fund_spread = 0.40*(14.0-2.0) + 0.40*(9.0-1.0) = 4.8 + 3.2 = 8.0 bps
    // turn = 0.80 => vip0 = 0.80*13.0 = 10.40, diag = 0.80*3.50 = 2.80
    let r0 = &recs_3a[0];
    assert!(!r0.is_eof_close);
    assert_close(r0.basis_bps, 40.0, 1e-10, "3a_r0_basis")?;
    assert_close(r0.fund_spread_bps, 8.0, 1e-12, "3a_r0_fund")?;
    assert_close(r0.total_turn_l, 0.80, 1e-15, "3a_r0_turn")?;
    assert_close(r0.cost_vip0_bps, 10.40, 1e-12, "3a_r0_cost_vip0")?;
    assert_close(r0.cost_maker_diag_bps, 2.80, 1e-12, "3a_r0_cost_diag")?;
    assert_close(r0.net_vip0_bps, 40.0 + 8.0 - 10.40, 1e-10, "3a_r0_net_vip0")?;
    assert_close(
        r0.net_maker_diag_bps,
        40.0 + 8.0 - 2.80,
        1e-10,
        "3a_r0_net_diag",
    )?;

    // Record 1 (t2 -> t3): Open(3) credits t3 funding: 0.40*(110-10) + 0.40*(55-5) = 60.0 bps.
    let r1 = &recs_3a[1];
    assert!(!r1.is_eof_close);
    assert_close(r1.basis_bps, 0.0, 1e-12, "3a_r1_basis")?;
    assert_close(r1.fund_spread_bps, 60.0, 1e-12, "3a_r1_fund")?;
    assert_close(r1.total_turn_l, 0.0, 1e-15, "3a_r1_turn")?;

    // Record 2 (t3 Open -> t3 Close, EOF):
    // basis = 0.40*(0.03-0.02)*10_000 + 0.40*(0.04-0.01)*10_000 = 40 + 120 = 160.0 bps
    // fund_spread = 0.0 (t3 funding ignored on EOF)
    // eof_close_turn_l == sum |w_i| == 0.80
    let r2 = &recs_3a[2];
    assert!(r2.is_eof_close);
    assert_eq!(r2.entry_ts_ns, ts_3);
    assert_eq!(r2.exit_ts_ns, ts_3);
    assert_close(r2.basis_bps, 160.0, 1e-10, "3a_r2_eof_basis")?;
    assert_close(r2.fund_spread_bps, 0.0, 1e-15, "3a_r2_eof_fund_zero")?;
    assert_close(r2.eof_close_turn_l, 0.80, 1e-15, "3a_r2_eof_turn")?;
    assert_close(r2.total_turn_l, 0.80, 1e-15, "3a_r2_total_turn")?;
    assert_close(r2.cost_vip0_bps, 10.40, 1e-12, "3a_r2_eof_cost_vip0")?;
    assert_close(r2.cost_maker_diag_bps, 2.80, 1e-12, "3a_r2_eof_cost_diag")?;
    assert_close(r2.net_vip0_bps, 160.0 - 10.40, 1e-10, "3a_r2_eof_net_vip0")?;
    assert_close(
        r2.net_maker_diag_bps,
        160.0 - 2.80,
        1e-10,
        "3a_r2_eof_net_diag",
    )?;

    // 3B. MakerDiag non-binding check even on strong positive returns & borderline switching:
    let spec_switch = XVenueStrategySpec {
        name: "MakerDiag_NonBinding",
        total_notional_l: 1.00,
        recenter_trigger_pct: 5.0,
        window_bars: 1,
        payback_days: 7,
        check_cadence_bars: 1,
        min_hold_bars: 0,
        max_pairs: 1,
        min_diversification: 1,
        entry_apr_pct: 5.0,
        exit_apr_pct: 0.0,
    };
    // Switching every bar (turn = 2.0/bar => VIP0 cost 26.0 bps vs MakerDiag cost 7.0 bps):
    // Spread alternates around 21..23 bps, so VIP0 is negative (< 0, Fails),
    // while MakerDiag has +14..+16 bps net, t_nw >= 3.50, DSR >= 0.95, YET is_binding=false => Fail!
    let mut border_byb = Vec::with_capacity(160);
    let mut border_hl = Vec::with_capacity(160);
    for bar in 0..80 {
        let ts = BASE_TS_NS + (bar as i64) * BAR_4H_NS;
        let (f0, f1) = if bar % 2 == 0 {
            let bump = if (bar / 2) % 2 == 0 { 21.0 } else { 23.0 };
            (bump, -5.0)
        } else {
            let bump = if ((bar - 1) / 2) % 2 == 0 { 21.0 } else { 23.0 };
            (-5.0, bump)
        };
        border_byb.push(make_candle(
            0,
            ts,
            (100.0, 100.0, 100.0, 100.0),
            100.0,
            0.0,
        )?);
        border_byb.push(make_candle(
            1,
            ts,
            (100.0, 100.0, 100.0, 100.0),
            100.0,
            0.0,
        )?);
        border_hl.push(make_candle(0, ts, (100.0, 100.0, 100.0, 100.0), 100.0, f0)?);
        border_hl.push(make_candle(1, ts, (100.0, 100.0, 100.0, 100.0), 100.0, f1)?);
    }
    let cfg_border = XVenueCarryConfig {
        strategies: vec![spec_switch],
        ..XVenueCarryConfig::default()
    };
    let (border_rep, _) = run_xvenue_carry_falsification_detailed(
        &mut VecCandleReader::new(border_byb),
        &mut VecCandleReader::new(border_hl),
        &cfg_border,
    )?;
    let b_vip = &border_rep.report.vip0_cells[0];
    let b_diag = &border_rep.report.diag_zero_cells[0];
    assert!(b_vip.mean_bps.ok_or("border vip mean")? < 0.0);
    assert_eq!(b_vip.verdict, GateVerdict::Fail);
    assert!(b_diag.mean_bps.ok_or("border diag mean")? > 0.0);
    assert!(b_diag.t_nw.ok_or("border diag t_nw")? >= 3.50);
    assert!(b_diag.dsr.ok_or("border diag dsr")? >= 0.95);
    assert!((b_diag.n as u64) >= b_diag.n_min.ok_or("border diag n_min")?);
    assert!(!b_diag.is_binding);
    assert_eq!(b_diag.verdict, GateVerdict::Fail);

    // 3C. Cum116 divergence:
    // Construct a 100-bar block-wave net VIP-0 return stream (10 bars of 16.8 bps, 10 bars of -3.2 bps,
    // repeated 5x: mean=6.8, std~10.05, n=100 >= n_min_cum116=34, t_nw~3.645 in (3.50, 3.9265)),
    // so local report.vip0_cells[0].verdict == Pass while diagnostics[0].verdict_cum116 == Fail,
    // AND diag_zero_cells[0].verdict == Fail despite strong positive MakerDiag returns!
    let spec_cum = XVenueStrategySpec {
        name: "Cum116_Divergence",
        total_notional_l: 1.00,
        recenter_trigger_pct: 5.0,
        window_bars: 1,
        payback_days: 7,
        check_cadence_bars: 1000,
        min_hold_bars: 0,
        max_pairs: 1,
        min_diversification: 1,
        entry_apr_pct: 5.0,
        exit_apr_pct: 0.0,
    };
    // 101 slices (0..=100) produce 100 settled records (0..100): 99 regular + 1 EOF.
    let mut cum_byb = Vec::with_capacity(101);
    let mut cum_hl = Vec::with_capacity(101);
    let target_ret = |rec_idx: usize| -> f64 {
        if (rec_idx / 10).is_multiple_of(2) {
            16.8
        } else {
            -3.2
        }
    };
    for slice_idx in 0..101 {
        let ts = BASE_TS_NS + (slice_idx as i64) * BAR_4H_NS;
        let (byb_close, byb_high) = if slice_idx == 100 {
            // EOF bar (rec_idx = 99): target -3.2 bps net after 13.0 bps EOF cost => basis = +9.8 bps.
            (100.098, 100.098)
        } else {
            (100.0, 100.0)
        };
        let f_hl = match slice_idx {
            0 | 1 => 5.0,
            2 => target_ret(0) + VIP0_XVENUE_COST_BPS,
            3..=100 => target_ret(slice_idx - 2),
            _ => 0.0,
        };
        cum_byb.push(make_candle(
            0,
            ts,
            (100.0, byb_high, 100.0, byb_close),
            100.0,
            0.0,
        )?);
        cum_hl.push(make_candle(
            0,
            ts,
            (100.0, 100.0, 100.0, 100.0),
            100.0,
            f_hl,
        )?);
    }
    let cfg_cum = XVenueCarryConfig {
        strategies: vec![spec_cum],
        ..XVenueCarryConfig::default()
    };
    let (cum_rep, _) = run_xvenue_carry_falsification_detailed(
        &mut VecCandleReader::new(cum_byb),
        &mut VecCandleReader::new(cum_hl),
        &cfg_cum,
    )?;
    let expected_t_crit_116 = t_crit_holm(CUMULATIVE_HYPOTHESES_M)?;
    assert_eq!(cum_rep.m_cumulative, 116);
    assert_close(
        cum_rep.t_crit_cum116,
        expected_t_crit_116,
        1e-12,
        "t_crit_cum116",
    )?;
    assert!(
        cum_rep.t_crit_cum116 > 3.92 && cum_rep.t_crit_cum116 < 3.93,
        "unexpected t_crit_cum116: {}",
        cum_rep.t_crit_cum116
    );

    let c_vip = &cum_rep.report.vip0_cells[0];
    let c_maker = &cum_rep.report.diag_zero_cells[0];
    let c_diag = &cum_rep.diagnostics[0];
    let t_nw = c_vip.t_nw.ok_or("cum116 t_nw")?;
    let n_min_116 = c_diag.n_min_cum116.ok_or("cum116 n_min")?;
    assert_eq!(c_diag.bars_n, 100);
    assert!(
        t_nw >= 3.50 && t_nw < cum_rep.t_crit_cum116,
        "expected t_nw in [3.50, {}), got {t_nw}",
        cum_rep.t_crit_cum116
    );
    assert!(
        (c_diag.bars_n as u64) >= n_min_116,
        "expected n={} >= n_min_cum116={n_min_116}",
        c_diag.bars_n
    );
    assert_eq!(c_vip.verdict, GateVerdict::Pass);
    assert_eq!(c_diag.verdict_cum116, GateVerdict::Fail);
    assert!(!c_maker.is_binding);
    assert!(c_maker.t_nw.ok_or("cum maker t_nw")? >= 3.50);
    assert!(c_maker.dsr.ok_or("cum maker dsr")? >= 0.95);
    assert_eq!(c_maker.verdict, GateVerdict::Fail);

    Ok(())
}

#[test]
fn test_xvenue_zero_lookahead_bit_for_bit_and_late_hl_listing() -> TestResult {
    // 280-bar (>= 240) dual stream across all 9 default_xvenue_grid() configs:
    // - Symbols 0..4 listed on both Bybit and HL from bar 0.
    // - Symbol 4 (late HL listing) listed on Bybit from bar 0 with +76.65% APR differential,
    //   and listed on HL ONLY starting at bar 60.
    // - At bar 195 (pre-cutoff = 230, after Slow30d W=180 has entered), Sym0 jumps +130% on Bybit
    //   so ALL 9 strategies trigger recenter & bridge events BEFORE cutoff!
    // - Mutate all bars strictly after cutoff (bar > 230) with 10x prices and -500 bps funding on
    //   both venues and verify bit-for-bit equality (to_bits()) for all records exit_ts_ns <= cutoff.
    let total_bars = 280usize;
    let cutoff_bar = 230usize;
    let cutoff_ts_ns = BASE_TS_NS + (cutoff_bar as i64) * BAR_4H_NS;
    let late_hl_bar = 60usize;
    let late_sym = SymbolId(4);

    let mut base_byb = Vec::with_capacity(total_bars * 5);
    let mut base_hl = Vec::with_capacity(total_bars * 5);

    for bar in 0..total_bars {
        let ts = BASE_TS_NS + (bar as i64) * BAR_4H_NS;
        for sym in 0_u16..4 {
            let b_px = match sym {
                0 => {
                    if bar < 195 {
                        100.0 + (bar as f64) * 0.05
                    } else {
                        252.425 + ((bar - 195) as f64) * 0.05
                    }
                }
                _ => 100.0 + (sym as f64) * 20.0 + ((bar % 7) as f64) * 0.3,
            };
            let h_px = b_px * (1.0 + 0.0004 * ((sym as f64) - 1.5));
            let f_byb = 0.5;
            let f_hl = match sym {
                0 => 2.1,
                1 => 1.9,
                2 => 1.5,
                _ => 1.3,
            };
            base_byb.push(make_candle(
                sym,
                ts,
                (b_px, b_px * 1.01, b_px * 0.99, b_px * 1.001),
                100.0,
                f_byb,
            )?);
            base_hl.push(make_candle(
                sym,
                ts,
                (h_px, h_px * 1.01, h_px * 0.99, h_px * 1.0005),
                100.0,
                f_hl,
            )?);
        }
        // Sym4: Bybit present from bar 0, HL absent for first 60 bars (bar < 60).
        let late_px = 150.0 + ((bar % 11) as f64) * 0.2;
        base_byb.push(make_candle(
            4,
            ts,
            (late_px, late_px * 1.01, late_px * 0.99, late_px * 1.001),
            100.0,
            0.5,
        )?);
        if bar >= late_hl_bar {
            base_hl.push(make_candle(
                4,
                ts,
                (
                    late_px * 1.0002,
                    late_px * 1.01,
                    late_px * 0.99,
                    late_px * 1.0005,
                ),
                100.0,
                4.0,
            )?);
        }
    }

    let mut mut_byb = base_byb.clone();
    let mut mut_hl = base_hl.clone();
    for bar in &mut mut_byb {
        if bar.open_ts_ns.raw() > cutoff_ts_ns {
            *bar = make_candle(
                bar.symbol_id.0,
                bar.open_ts_ns.raw(),
                (
                    bar.open.as_f64() * 10.0,
                    bar.high.as_f64() * 10.0,
                    bar.low.as_f64() * 10.0,
                    bar.close.as_f64() * 10.0,
                ),
                bar.volume * 5.0,
                -500.0,
            )?;
        }
    }
    for bar in &mut mut_hl {
        if bar.open_ts_ns.raw() > cutoff_ts_ns {
            *bar = make_candle(
                bar.symbol_id.0,
                bar.open_ts_ns.raw(),
                (
                    bar.open.as_f64() * 10.0,
                    bar.high.as_f64() * 10.0,
                    bar.low.as_f64() * 10.0,
                    bar.close.as_f64() * 10.0,
                ),
                bar.volume * 5.0,
                -500.0,
            )?;
        }
    }

    let cfg = XVenueCarryConfig::default();
    let (base_rep, base_recs) = run_xvenue_carry_falsification_detailed(
        &mut VecCandleReader::new(base_byb),
        &mut VecCandleReader::new(base_hl),
        &cfg,
    )?;
    let (mut_rep, mut_recs) = run_xvenue_carry_falsification_detailed(
        &mut VecCandleReader::new(mut_byb),
        &mut VecCandleReader::new(mut_hl),
        &cfg,
    )?;
    assert_eq!(base_rep.diagnostics.len(), 9);
    assert_eq!(mut_rep.diagnostics.len(), 9);

    for spec in default_xvenue_grid() {
        let name = spec.name;
        let base_pre: Vec<&XVenueBarRecord> = base_recs
            .iter()
            .filter(|r| r.strategy == name && r.exit_ts_ns <= cutoff_ts_ns)
            .collect();
        let mut_pre: Vec<&XVenueBarRecord> = mut_recs
            .iter()
            .filter(|r| r.strategy == name && r.exit_ts_ns <= cutoff_ts_ns)
            .collect();
        assert!(!base_pre.is_empty(), "{name} must have pre-cutoff records");
        assert_eq!(base_pre.len(), mut_pre.len(), "{name} pre-cutoff len");
        assert!(
            base_pre.iter().any(|r| !r.weights.is_empty()),
            "{name} must hold active weights pre-cutoff"
        );
        assert!(
            base_pre.iter().any(|r| r.recenter_events > 0),
            "{name} must trigger pre-cutoff recenter"
        );
        assert!(
            base_pre.iter().any(|r| r.bridge_events > 0),
            "{name} must trigger pre-cutoff bridge"
        );

        for (b, m) in base_pre.iter().zip(mut_pre.iter()) {
            assert_eq!(b.decision_ts_ns, m.decision_ts_ns);
            assert_eq!(b.entry_ts_ns, m.entry_ts_ns);
            assert_eq!(b.exit_ts_ns, m.exit_ts_ns);
            assert_eq!(b.is_eof_close, m.is_eof_close);
            assert_eq!(b.recenter_events, m.recenter_events);
            assert_eq!(b.bridge_events, m.bridge_events);
            assert_eq!(b.weights.len(), m.weights.len());
            for ((bk, bv), (mk, mv)) in b.weights.iter().zip(m.weights.iter()) {
                assert_eq!(bk, mk);
                assert_eq!(bv.to_bits(), mv.to_bits(), "{name} weight bits");
            }
            assert_eq!(b.total_gross_l.to_bits(), m.total_gross_l.to_bits());
            assert_eq!(b.basis_bps.to_bits(), m.basis_bps.to_bits());
            assert_eq!(b.fund_spread_bps.to_bits(), m.fund_spread_bps.to_bits());
            assert_eq!(b.bridge_bps.to_bits(), m.bridge_bps.to_bits());
            assert_eq!(b.rebalance_turn_l.to_bits(), m.rebalance_turn_l.to_bits());
            assert_eq!(b.recenter_turn_l.to_bits(), m.recenter_turn_l.to_bits());
            assert_eq!(b.eof_close_turn_l.to_bits(), m.eof_close_turn_l.to_bits());
            assert_eq!(b.total_turn_l.to_bits(), m.total_turn_l.to_bits());
            assert_eq!(
                b.venue_imbalance_after.to_bits(),
                m.venue_imbalance_after.to_bits()
            );
            assert_eq!(b.cost_vip0_bps.to_bits(), m.cost_vip0_bps.to_bits());
            assert_eq!(
                b.cost_maker_diag_bps.to_bits(),
                m.cost_maker_diag_bps.to_bits()
            );
            assert_eq!(b.net_vip0_bps.to_bits(), m.net_vip0_bps.to_bits());
            assert_eq!(
                b.net_maker_diag_bps.to_bits(),
                m.net_maker_diag_bps.to_bits()
            );
        }

        // Late HL listing causal guard: Sym4 must NOT appear before W dual bars complete
        // (earliest decision at bar 60 + W - 1, earliest entry at bar 60 + W),
        // and MUST join once W dual bars and rotation cadence complete.
        let min_dec_ts = BASE_TS_NS + ((late_hl_bar + spec.window_bars - 1) as i64) * BAR_4H_NS;
        let min_entry_ts = BASE_TS_NS + ((late_hl_bar + spec.window_bars) as i64) * BAR_4H_NS;
        let ts_60 = BASE_TS_NS + (late_hl_bar as i64) * BAR_4H_NS;
        assert!(
            base_recs
                .iter()
                .filter(|r| r.strategy == name && r.decision_ts_ns < ts_60)
                .all(|r| !r.weights.contains_key(&late_sym)),
            "{name} leaked Sym4 before HL listing"
        );
        assert!(
            base_recs
                .iter()
                .filter(|r| r.strategy == name && r.decision_ts_ns < min_dec_ts)
                .all(|r| !r.weights.contains_key(&late_sym)),
            "{name} decided Sym4 before W dual bars"
        );
        assert!(
            base_recs
                .iter()
                .filter(|r| r.strategy == name && r.entry_ts_ns < min_entry_ts)
                .all(|r| !r.weights.contains_key(&late_sym)),
            "{name} entered Sym4 before W+1 dual bars"
        );
        assert!(
            base_recs.iter().any(|r| {
                r.strategy == name
                    && r.decision_ts_ns >= min_dec_ts
                    && r.entry_ts_ns >= min_entry_ts
                    && r.weights.get(&late_sym).is_some_and(|&w| w > 0.0)
            }),
            "{name} failed to allocate to Sym4 after W dual bars"
        );
    }

    Ok(())
}

#[test]
fn test_xvenue_cli_end_to_end_json_roundtrip_and_rejections() -> TestResult {
    let tmp = TempDirGuard::new("cli_scan_xvenue")?;
    let bybit_dir = tmp.path.join("bybit");
    let hl_dir = tmp.path.join("hyperliquid");
    let out_json = tmp.path.join("xvenue_report.json");
    fs::create_dir_all(&bybit_dir)?;
    fs::create_dir_all(&hl_dir)?;

    let bars = 260usize;
    for sym in 0_u16..4 {
        let mut byb = Vec::with_capacity(bars);
        let mut hl = Vec::with_capacity(bars);
        for bar in 0..bars {
            let ts = BASE_TS_NS + (bar as i64) * BAR_4H_NS;
            let base = if sym == 0 && bar >= 195 {
                235.0 + ((bar - 195) as f64) * 0.1
            } else {
                100.0 + (sym as f64) * 15.0 + ((bar % 13) as f64) * 0.3
            };
            let h_px = base * 1.0005;
            let f_byb = 0.5;
            let f_hl = bar_4h_bps_for_apr(12.0 + (sym as f64) * 4.0)
                + if bar % 2 == 0 { 0.25 } else { -0.15 };
            byb.push(make_candle(
                sym,
                ts,
                (base, base * 1.01, base * 0.99, base * 1.002),
                100.0,
                f_byb,
            )?);
            hl.push(make_candle(
                sym,
                ts,
                (h_px, h_px * 1.01, h_px * 0.99, h_px * 1.001),
                100.0,
                f_hl,
            )?);
        }
        write_csv_lf(&bybit_dir.join(format!("sym{sym}.csv")), &byb)?;
        write_csv_lf(&hl_dir.join(format!("sym{sym}.csv")), &hl)?;
    }

    let byb_arg = bybit_dir.to_str().ok_or("bybit dir utf8")?;
    let hl_arg = hl_dir.to_str().ok_or("hl dir utf8")?;
    let out_arg = out_json.to_str().ok_or("out json utf8")?;

    let ascii = run_cli([
        "vortex-alfa",
        "scan-xvenue",
        "--bybit-dir",
        byb_arg,
        "--hl-dir",
        hl_arg,
        "--slippage-bps",
        "1.5",
        "--capital-usdt",
        "2000",
        "--bridge-fee-usdt",
        "1.0",
        "--out",
        out_arg,
    ])?;

    assert!(ascii.contains("Track7-XVenueCarry"));
    for col in [
        "FundSpd", "Basis", "Bridge", "CostVIP0", "RecCnt", "BrdgCnt", "AvgUtil%", "NetVIP0",
        "CAGR%", "AnnSR", "MaxDD%", "t_NW", "DSR", "Verdict", "Cum116",
    ] {
        assert!(ascii.contains(col), "ASCII missing column {col}");
    }
    for spec in default_xvenue_grid() {
        assert!(
            ascii.contains(spec.name),
            "ASCII missing strategy {}",
            spec.name
        );
    }

    let json_text = fs::read_to_string(&out_json)?;
    let parsed: XVenueCarryReport = serde_json::from_str(&json_text)?;
    let re_json = serde_json::to_string(&parsed)?;
    let roundtrip: XVenueCarryReport = serde_json::from_str(&re_json)?;
    assert_eq!(parsed, roundtrip);

    assert_eq!(parsed.report.track, "Track7-XVenueCarry");
    assert_eq!(parsed.report.m_hypotheses, 9);
    assert_eq!(parsed.m_cumulative, CUMULATIVE_HYPOTHESES_M);
    assert_close(
        parsed.t_crit_cum116,
        t_crit_holm(CUMULATIVE_HYPOTHESES_M)?,
        1e-12,
        "json_t_crit_cum116",
    )?;
    assert_close(parsed.slippage_bps, 1.5, 1e-15, "json_slippage_bps")?;
    assert_close(parsed.capital_usdt, 2_000.0, 1e-15, "json_capital_usdt")?;
    assert_close(parsed.bridge_fee_usdt, 1.0, 1e-15, "json_bridge_fee_usdt")?;
    assert_close(
        parsed.venue_imbalance_trigger,
        DEFAULT_VENUE_IMBALANCE_TRIGGER,
        1e-15,
        "json_imbalance_trigger",
    )?;
    assert_close(
        parsed.bars_per_year,
        BARS_4H_PER_YEAR,
        1e-15,
        "json_bars_per_year",
    )?;
    assert_eq!(parsed.slices_processed, bars as u64);
    assert_eq!(parsed.bybit_candles_processed, (bars * 4) as u64);
    assert_eq!(parsed.hl_candles_processed, (bars * 4) as u64);
    assert_eq!(parsed.diagnostics.len(), 9);
    assert_eq!(parsed.report.vip0_cells.len(), 9);
    assert_eq!(parsed.report.diag_zero_cells.len(), 9);
    assert!(parsed
        .report
        .diag_zero_cells
        .iter()
        .all(|c| !c.is_binding && c.verdict != GateVerdict::Pass));

    // CLI rejection checks:
    assert!(matches!(
        run_cli(["vortex-alfa", "scan-xvenue", "--hl-dir", hl_arg]),
        Err(DomainError::Cli(_))
    ));
    assert!(matches!(
        run_cli(["vortex-alfa", "scan-xvenue", "--bybit-dir", byb_arg]),
        Err(DomainError::Cli(_))
    ));
    assert!(matches!(
        run_cli([
            "vortex-alfa",
            "scan-xvenue",
            "--bybit-dir",
            byb_arg,
            "--hl-dir",
            hl_arg,
            "--slippage-bps",
            "-0.1",
        ]),
        Err(DomainError::Cli(_))
    ));
    assert!(matches!(
        run_cli([
            "vortex-alfa",
            "scan-xvenue",
            "--bybit-dir",
            byb_arg,
            "--hl-dir",
            hl_arg,
            "--capital-usdt",
            "0",
        ]),
        Err(DomainError::Cli(_))
    ));
    assert!(matches!(
        run_cli([
            "vortex-alfa",
            "scan-xvenue",
            "--bybit-dir",
            byb_arg,
            "--hl-dir",
            hl_arg,
            "--bridge-fee-usdt",
            "-1",
        ]),
        Err(DomainError::Cli(_))
    ));
    assert!(matches!(
        run_cli(["vortex-alfa", "scan-xvenue", "--unknown"]),
        Err(DomainError::Cli(_))
    ));

    Ok(())
}
