use std::collections::BTreeMap;
use std::error::Error;
use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use vortex_alfa::application::carry_uta_types::{
    default_carry_uta_grid, CarrySelectionMode, CarryUtaConfig, CarryUtaDayRecord, CarryUtaReport,
    CarryUtaStrategySpec,
};
use vortex_alfa::application::falsifier_carry_uta::run_carry_uta_falsification_detailed;
use vortex_alfa::application::falsifier_macro::run_macro_falsification_detailed;
use vortex_alfa::application::macro_baselines::MacroConfig;
use vortex_alfa::domain::error::DomainError;
use vortex_alfa::domain::ports::CandleSeriesReader;
use vortex_alfa::domain::signals::carry_uta::{payback_apr_hurdle_pct, CarryUtaEngine};
use vortex_alfa::domain::signals::spot_perp_carry::default_carry_params;
use vortex_alfa::domain::stats::GateVerdict;
use vortex_alfa::domain::types::{Bps, Candle, PriceE8, SymbolId, TimestampNs, SCALE_E8};
use vortex_alfa::interfaces::cli::run_cli;

type TestResult = Result<(), Box<dyn Error>>;
type DualSliceMap = (BTreeMap<SymbolId, Candle>, BTreeMap<SymbolId, Candle>);

const DAY_NS: i64 = 86_400_000_000_000;
const BASE_TS_NS: i64 = 1_704_067_200_000_000_000;
const DAYS_PER_YEAR: f64 = 365.0;
const VIP0_CARRY_COST_BPS: f64 = 18.50;
const MAKER_DIAG_CARRY_COST_BPS: f64 = 12.00;

static TEMP_SEQ: AtomicU64 = AtomicU64::new(1);

struct TempDirGuard {
    path: PathBuf,
}

impl TempDirGuard {
    fn new(prefix: &str) -> Result<Self, Box<dyn Error>> {
        let id = TEMP_SEQ.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "vortex_alfa_carry_uta_{prefix}_{}_{}",
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

fn daily_bps_for_apr(apr_pct: f64) -> f64 {
    apr_pct * 100.0 / DAYS_PER_YEAR
}

fn make_dual_slice(ts_ns: i64, legs: &[(u16, f64, f64, f64)]) -> Result<DualSliceMap, DomainError> {
    let mut linear = BTreeMap::new();
    let mut spot = BTreeMap::new();
    for &(sym, lin_px, spt_px, fund_bps) in legs {
        let sid = SymbolId(sym);
        linear.insert(
            sid,
            make_candle(
                sym,
                ts_ns,
                (lin_px, lin_px, lin_px, lin_px),
                100.0,
                fund_bps,
            )?,
        );
        spot.insert(
            sid,
            make_candle(sym, ts_ns, (spt_px, spt_px, spt_px, spt_px), 100.0, 0.0)?,
        );
    }
    Ok((linear, spot))
}

#[test]
fn payback_apr_hurdle_and_cost_amortized_rotation() -> TestResult {
    // Independent analytical derivation:
    // Round-trip rotation exits 1 unit and enters 1 unit = 2 * 18.50 bps = 37.00 bps.
    // Annualized over 30-day payback: (37.00 / 30.0) * 365.0 / 100.0 = 4.5016666666666667%.
    let expected_hurdle = (2.0 * VIP0_CARRY_COST_BPS * DAYS_PER_YEAR) / (100.0 * 30.0);
    assert_close(
        expected_hurdle,
        4.501_666_666_666_667,
        1e-15,
        "analytic_hurdle",
    )?;
    assert_close(
        payback_apr_hurdle_pct(VIP0_CARRY_COST_BPS, 30)?,
        expected_hurdle,
        1e-12,
        "payback_apr_hurdle_pct",
    )?;
    assert!(matches!(
        payback_apr_hurdle_pct(-1.0, 30),
        Err(DomainError::InvalidFeeBps)
    ));
    assert!(matches!(
        payback_apr_hurdle_pct(f64::NAN, 30),
        Err(DomainError::InvalidFeeBps)
    ));
    assert!(matches!(
        payback_apr_hurdle_pct(f64::INFINITY, 30),
        Err(DomainError::InvalidFeeBps)
    ));
    assert!(matches!(
        payback_apr_hurdle_pct(VIP0_CARRY_COST_BPS, 0),
        Err(DomainError::InvalidStatParam("payback_days"))
    ));

    let spec = CarryUtaStrategySpec {
        name: "Test_CostAmortized_Top2",
        selection_mode: CarrySelectionMode::CostAmortized,
        coverage_ratio: 0.80,
        recenter_trigger_pct: 0.35,
        window_days: 3,
        entry_apr_pct: 6.0,
        exit_apr_pct: 0.0,
        max_pairs: 2,
        min_diversification: 2,
        check_cadence_days: 7,
        payback_days: 30,
        min_hold_days: 14,
    };
    let (s0, s1, s2) = (SymbolId(0), SymbolId(1), SymbolId(2));

    // Scenario A: Hurdle gate prevents churn when challenger spread (+3.0%) < 4.50167%,
    // checked BOTH at hold day 7 and at hold day 14 (after min_hold_days expires).
    let mut eng_hurdle = CarryUtaEngine::new(spec, VIP0_CARRY_COST_BPS)?;
    assert_close(
        eng_hurdle.payback_hurdle_pct(),
        expected_hurdle,
        1e-12,
        "eng_hurdle_pct",
    )?;
    for day in 0..=16 {
        let ts = BASE_TS_NS + (day as i64) * DAY_NS;
        let apr2 = if day >= 7 { 13.0 } else { 4.0 };
        let (lin, spt) = make_dual_slice(
            ts,
            &[
                (0, 100.0, 100.0, daily_bps_for_apr(10.0)),
                (1, 100.0, 100.0, daily_bps_for_apr(12.0)),
                (2, 100.0, 100.0, daily_bps_for_apr(apr2)),
            ],
        )?;
        let out = eng_hurdle.on_slice(&lin, &spt)?;
        if day < 2 {
            assert_eq!(out, None);
            assert!(!eng_hurdle.is_ready());
        } else {
            let weights = out.ok_or("expected active weights after warmup")?;
            assert!(eng_hurdle.is_ready());
            assert_eq!(weights.len(), 2);
            assert_close(weights[&s0], 0.40, 1e-15, "hurdle_s0_weight")?;
            assert_close(weights[&s1], 0.40, 1e-15, "hurdle_s1_weight")?;
            assert!(!weights.contains_key(&s2));
            if day == 9 || day == 16 {
                let s0_apr = eng_hurdle.current_apr_pct(s0).ok_or("s0 apr")?;
                let s2_apr = eng_hurdle.current_apr_pct(s2).ok_or("s2 apr")?;
                assert_close(s2_apr - s0_apr, 3.0, 1e-10, "spread_below_hurdle")?;
            }
        }
    }

    // Scenario B:
    // - Days 0..2: warmup (None for days 0..1; day 2 enters Sym0 @ 10% and Sym1 @ 12%).
    // - Days 7..9: raise Sym2 to Sym0 + 10% = 20% (>= hurdle); at hold day 7 (< 14), Sym0 protected!
    // - Days 10..16: keep Sym2 at 20%; at hold day 14 (>= 14), Sym2 cleanly replaces Sym0!
    // - Days 21..23: drop Sym2 (hold day 7 < 14) to 0.0% (<= exit 0.0%), raise Sym0 to 7.0% (>= 6.0%).
    //   On check day 23, Sym2 is evicted immediately regardless of min_hold_days, freeing slot for Sym0!
    let mut eng_hold = CarryUtaEngine::new(spec, VIP0_CARRY_COST_BPS)?;
    for day in 0..=23 {
        let ts = BASE_TS_NS + (day as i64) * DAY_NS;
        let (apr0, apr1, apr2) = if day < 7 {
            (10.0, 12.0, 4.0)
        } else if day <= 16 {
            (10.0, 12.0, 20.0)
        } else {
            (7.0, 12.0, 0.0)
        };
        let (lin, spt) = make_dual_slice(
            ts,
            &[
                (0, 100.0, 100.0, daily_bps_for_apr(apr0)),
                (1, 100.0, 100.0, daily_bps_for_apr(apr1)),
                (2, 100.0, 100.0, daily_bps_for_apr(apr2)),
            ],
        )?;
        let out = eng_hold.on_slice(&lin, &spt)?;
        if day < 2 {
            assert_eq!(out, None);
        } else if day < 16 {
            // Day 9 is hold day 7 (< 14): even though Sym2 has 20% (+10% >= hurdle), Sym0 stays!
            let weights = out.ok_or("expected protected holdings before day 16")?;
            assert_eq!(weights.len(), 2);
            assert_close(weights[&s0], 0.40, 1e-15, "protected_s0")?;
            assert_close(weights[&s1], 0.40, 1e-15, "protected_s1")?;
            assert!(!weights.contains_key(&s2));
        } else if day < 23 {
            // Day 16 is hold day 14 (>= 14): Sym2 replaces Sym0; on days 17..22 non-check days hold Sym2.
            let weights = out.ok_or("expected rotated holdings at day 16..22")?;
            assert_eq!(weights.len(), 2);
            assert!(!weights.contains_key(&s0));
            assert_close(weights[&s1], 0.40, 1e-15, "post_swap_s1")?;
            assert_close(weights[&s2], 0.40, 1e-15, "post_swap_s2")?;
        } else {
            // Day 23 check: Sym2 (held 7 days < 14) dropped to 0.0% <= exit_apr_pct, evicted immediately,
            // freeing slot for Sym0 (7.0% >= entry_apr_pct 6.0%).
            let weights = out.ok_or("expected mandatory exit replacement at day 23")?;
            assert_eq!(weights.len(), 2);
            assert_close(weights[&s0], 0.40, 1e-15, "mandatory_reentry_s0")?;
            assert_close(weights[&s1], 0.40, 1e-15, "mandatory_keep_s1")?;
            assert!(!weights.contains_key(&s2));
        }
    }

    Ok(())
}

#[test]
fn uta_cross_margin_borrow_and_recenter_physics() -> TestResult {
    let uta75_spec = CarryUtaStrategySpec {
        name: "Custom_Uta75",
        selection_mode: CarrySelectionMode::BlindPeriodic,
        coverage_ratio: 0.75,
        recenter_trigger_pct: 0.35,
        window_days: 2,
        entry_apr_pct: 8.0,
        exit_apr_pct: -2.0,
        max_pairs: 2,
        min_diversification: 2,
        check_cadence_days: 30,
        payback_days: 30,
        min_hold_days: 0,
    };
    let cash50_spec = CarryUtaStrategySpec {
        name: "Custom_Cash50",
        coverage_ratio: 0.50,
        recenter_trigger_pct: 0.80,
        ..uta75_spec
    };
    let uta85_spec = CarryUtaStrategySpec {
        name: "Custom_Uta85",
        coverage_ratio: 0.85,
        recenter_trigger_pct: 0.20,
        ..uta75_spec
    };

    // 6 slices (days 0..=5):
    // Day 0..=1: warmup at (100, 100), funding = 5.0 bps/day (18.25% APR). Decision on Day 1.
    // Day 2: position opens at (100, 100) for Sym0 & Sym1.
    // Day 3 (Open(k)): Sym0 rallies to 140.0 (+40%), Sym1 rallies to 130.0 (+30%).
    // Day 4 (Open(k+1)) & Day 5: prices stay flat at (140.0, 130.0).
    let mut lin_candles = Vec::new();
    let mut spt_candles = Vec::new();
    for day in 0..=5 {
        let ts = BASE_TS_NS + (day as i64) * DAY_NS;
        let (p0, p1) = if day < 3 {
            (100.0, 100.0)
        } else {
            (140.0, 130.0)
        };
        lin_candles.push(make_candle(0, ts, (p0, p0, p0, p0), 100.0, 5.0)?);
        lin_candles.push(make_candle(1, ts, (p1, p1, p1, p1), 100.0, 5.0)?);
        spt_candles.push(make_candle(0, ts, (p0, p0, p0, p0), 100.0, 0.0)?);
        spt_candles.push(make_candle(1, ts, (p1, p1, p1, p1), 100.0, 0.0)?);
    }

    let cfg_uta75_cash50 = CarryUtaConfig {
        strategies: vec![uta75_spec, cash50_spec],
        slippage_bps: Bps::new(1.5)?,
        borrow_apr_pct: 15.0,
        ..CarryUtaConfig::default()
    };
    let (_, records) = run_carry_uta_falsification_detailed(
        &mut VecCandleReader::new(lin_candles.clone()),
        &mut VecCandleReader::new(spt_candles.clone()),
        &cfg_uta75_cash50,
    )?;

    let ts_2 = BASE_TS_NS + 2 * DAY_NS;
    let ts_3 = BASE_TS_NS + 3 * DAY_NS;
    let ts_4 = BASE_TS_NS + 4 * DAY_NS;

    let uta75_recs: Vec<&CarryUtaDayRecord> = records
        .iter()
        .filter(|r| r.strategy == "Custom_Uta75")
        .collect();
    let cash50_recs: Vec<&CarryUtaDayRecord> = records
        .iter()
        .filter(|r| r.strategy == "Custom_Cash50")
        .collect();
    assert_eq!(uta75_recs.len(), 4);
    assert_eq!(cash50_recs.len(), 4);

    // Day 2 entry verification.
    let u_d2 = uta75_recs[0];
    assert_eq!(u_d2.entry_ts_ns, ts_2);
    assert_close(u_d2.weights[&SymbolId(0)], 0.375, 1e-15, "uta75_l0")?;
    assert_close(u_d2.weights[&SymbolId(1)], 0.375, 1e-15, "uta75_l1")?;

    let c_d2 = cash50_recs[0];
    assert_eq!(c_d2.entry_ts_ns, ts_2);
    assert_close(c_d2.weights[&SymbolId(0)], 0.25, 1e-15, "cash50_l0")?;
    assert_close(c_d2.weights[&SymbolId(1)], 0.25, 1e-15, "cash50_l1")?;

    // Day 3 (Open(k)): Sym0 +40%, Sym1 +30%.
    // Independent analytical expectations for Uta75:
    // total_covered_l = 0.75, usdt_buffer = 0.25
    // unrealized_short_loss = 0.375 * 0.40 + 0.375 * 0.30 = 0.2625
    // borrowed_usdt_frac = 0.2625 - 0.25 = 0.0125
    // borrow_bps = 0.0125 * (15.0 * 100.0 / 365.0)
    // recenter_events = 1 (Sym0 +40% >= 35%), recenter_turn_l = 2 * 0.375 = 0.75
    let expected_borrow_bps = 0.0125 * (15.0 * 100.0 / DAYS_PER_YEAR);
    let u_k = uta75_recs[1];
    assert_eq!(u_k.entry_ts_ns, ts_3);
    assert_eq!(u_k.exit_ts_ns, ts_4);
    assert!(!u_k.is_eof_close);
    assert_close(u_k.total_covered_l, 0.75, 1e-15, "uta75_k_cov")?;
    assert_close(u_k.unrealized_short_loss, 0.2625, 1e-12, "uta75_k_unr")?;
    assert_close(u_k.borrowed_usdt_frac, 0.0125, 1e-12, "uta75_k_borrow_frac")?;
    assert_close(
        u_k.borrow_bps,
        expected_borrow_bps,
        1e-12,
        "uta75_k_borrow_bps",
    )?;
    assert_eq!(u_k.recenter_events, 1);
    assert_close(u_k.recenter_turn_l, 0.75, 1e-15, "uta75_k_rec_turn")?;
    assert_close(u_k.rebalance_turn_l, 0.0, 1e-15, "uta75_k_reb_turn")?;
    assert_close(u_k.total_turn_l, 0.75, 1e-15, "uta75_k_tot_turn")?;
    assert_close(
        u_k.cost_vip0_bps,
        0.75 * VIP0_CARRY_COST_BPS,
        1e-12,
        "uta75_k_cost_vip0",
    )?;

    // Day 4 (Open(k+1)): flat (140, 130). Sym0 entry_perp_px reset to 140 => 0 loss;
    // Sym1 stays at 100 => 0.375 * 0.30 = 0.1125 < 0.25 buffer => 0 borrow & 0 recenter.
    let u_k1 = uta75_recs[2];
    assert_eq!(u_k1.entry_ts_ns, ts_4);
    assert!(!u_k1.is_eof_close);
    assert_close(u_k1.unrealized_short_loss, 0.1125, 1e-12, "uta75_k1_unr")?;
    assert_close(u_k1.borrowed_usdt_frac, 0.0, 1e-15, "uta75_k1_borrow_frac")?;
    assert_close(u_k1.borrow_bps, 0.0, 1e-15, "uta75_k1_borrow_bps")?;
    assert_eq!(u_k1.recenter_events, 0);
    assert_close(u_k1.recenter_turn_l, 0.0, 1e-15, "uta75_k1_rec_turn")?;

    // Cash50 on exact same (+40% / +30%) path:
    // usdt_buffer = 0.50, unrealized_short_loss = 0.25*0.40 + 0.25*0.30 = 0.175 < 0.50.
    let c_k = cash50_recs[1];
    assert_eq!(c_k.entry_ts_ns, ts_3);
    assert_close(c_k.unrealized_short_loss, 0.175, 1e-12, "cash50_k_unr")?;
    assert_close(c_k.borrowed_usdt_frac, 0.0, 1e-15, "cash50_k_borrow_frac")?;
    assert_close(c_k.borrow_bps, 0.0, 1e-15, "cash50_k_borrow_bps")?;
    assert_eq!(c_k.recenter_events, 0);
    assert_close(c_k.recenter_turn_l, 0.0, 1e-15, "cash50_k_rec_turn")?;

    // Uta75 with borrow_apr_pct = 0.0: borrow_bps == 0.0 while recenter_events == 1 & turn == 0.75.
    let cfg_zero_borrow = CarryUtaConfig {
        strategies: vec![uta75_spec],
        slippage_bps: Bps::new(1.5)?,
        borrow_apr_pct: 0.0,
        ..CarryUtaConfig::default()
    };
    let (_, zero_b_recs) = run_carry_uta_falsification_detailed(
        &mut VecCandleReader::new(lin_candles),
        &mut VecCandleReader::new(spt_candles),
        &cfg_zero_borrow,
    )?;
    assert_close(zero_b_recs[1].borrowed_usdt_frac, 0.0125, 1e-12, "zb_frac")?;
    assert_close(zero_b_recs[1].borrow_bps, 0.0, 1e-15, "zb_bps")?;
    assert_eq!(zero_b_recs[1].recenter_events, 1);
    assert_close(zero_b_recs[1].recenter_turn_l, 0.75, 1e-15, "zb_rec_turn")?;

    // Uta85 (recenter_trigger_pct = 0.20) triggers recenter at exact +20% (100.0 -> 120.0).
    let mut u85_lin = Vec::new();
    let mut u85_spt = Vec::new();
    for day in 0..=4 {
        let ts = BASE_TS_NS + (day as i64) * DAY_NS;
        let p0 = if day < 3 { 100.0 } else { 120.0 };
        let p1 = 100.0;
        u85_lin.push(make_candle(0, ts, (p0, p0, p0, p0), 100.0, 5.0)?);
        u85_lin.push(make_candle(1, ts, (p1, p1, p1, p1), 100.0, 5.0)?);
        u85_spt.push(make_candle(0, ts, (p0, p0, p0, p0), 100.0, 0.0)?);
        u85_spt.push(make_candle(1, ts, (p1, p1, p1, p1), 100.0, 0.0)?);
    }
    let cfg_u85 = CarryUtaConfig {
        strategies: vec![uta85_spec],
        slippage_bps: Bps::new(1.5)?,
        borrow_apr_pct: 15.0,
        ..CarryUtaConfig::default()
    };
    let (_, u85_recs) = run_carry_uta_falsification_detailed(
        &mut VecCandleReader::new(u85_lin),
        &mut VecCandleReader::new(u85_spt),
        &cfg_u85,
    )?;
    let u85_k = &u85_recs[1];
    assert_eq!(u85_k.entry_ts_ns, ts_3);
    assert_eq!(u85_k.recenter_events, 1);
    assert_close(u85_k.recenter_turn_l, 0.85, 1e-15, "uta85_rec_turn")?;

    Ok(())
}

#[test]
fn real_basis_bar_open_settlement_eof_and_maker_diag_non_binding() -> TestResult {
    // 3A. Exact deterministic 4-slice dual candles verifying:
    // - basis_bps == sum L_i * (r_spot - r_perp) * 10_000
    // - BarOpenSettlement credits slice[k+1].linear[sym].funding_rate_bps (with +L_i sign)
    // - EOF Close(last) ignores slice[last].funding_rate_bps and charges eof_close_turn_l == total_covered_l
    // - 18.50 bps / turn_L on VIP-0 vs 12.00 bps / turn_L on MakerDiag
    let spec_3a = CarryUtaStrategySpec {
        name: "Exact_Basis_BarOpen_EOF",
        selection_mode: CarrySelectionMode::BlindPeriodic,
        coverage_ratio: 0.80,
        recenter_trigger_pct: 0.50,
        window_days: 1,
        entry_apr_pct: 5.0,
        exit_apr_pct: 0.0,
        max_pairs: 2,
        min_diversification: 2,
        check_cadence_days: 10,
        payback_days: 30,
        min_hold_days: 0,
    };
    let cfg_3a = CarryUtaConfig {
        strategies: vec![spec_3a],
        slippage_bps: Bps::new(1.5)?,
        borrow_apr_pct: 15.0,
        ..CarryUtaConfig::default()
    };
    let ts_0 = BASE_TS_NS;
    let ts_1 = BASE_TS_NS + DAY_NS;
    let ts_2 = BASE_TS_NS + 2 * DAY_NS;
    let ts_3 = BASE_TS_NS + 3 * DAY_NS;

    let lin_3a = vec![
        // t=0: decision slice (window_days=1 fills => {Sym0: 0.40, Sym1: 0.40})
        make_candle(0, ts_0, (100.0, 100.0, 100.0, 100.0), 100.0, 5.0)?,
        make_candle(1, ts_0, (200.0, 200.0, 200.0, 200.0), 100.0, 6.0)?,
        // t=1: entry Open(1); poison funding on t=1 MUST NOT be credited for t=1..2
        make_candle(0, ts_1, (100.0, 100.0, 100.0, 100.0), 100.0, 999.0)?,
        make_candle(1, ts_1, (200.0, 200.0, 200.0, 200.0), 100.0, -888.0)?,
        // t=2: exit Open(2); Sym0 perp +2%, Sym1 perp +5%, known funding 12.0 & 8.0 bps
        make_candle(0, ts_2, (102.0, 102.0, 102.0, 102.0), 100.0, 12.0)?,
        make_candle(1, ts_2, (210.0, 210.0, 210.0, 210.0), 100.0, 8.0)?,
        // t=3 (last): Open(3) settles t=2->3 with funding 100.0 & 50.0;
        // Close(3) settles EOF t=3 Open->Close (perp +2% & +1%) and MUST ignore t=3 funding!
        make_candle(0, ts_3, (102.0, 104.04, 102.0, 104.04), 100.0, 100.0)?,
        make_candle(1, ts_3, (210.0, 212.10, 210.0, 212.10), 100.0, 50.0)?,
    ];
    let spt_3a = vec![
        make_candle(0, ts_0, (100.0, 100.0, 100.0, 100.0), 100.0, 0.0)?,
        make_candle(1, ts_0, (200.0, 200.0, 200.0, 200.0), 100.0, 0.0)?,
        make_candle(0, ts_1, (100.0, 100.0, 100.0, 100.0), 100.0, 0.0)?,
        make_candle(1, ts_1, (200.0, 200.0, 200.0, 200.0), 100.0, 0.0)?,
        // t=2: Sym0 spot +5% (vs perp +2% => +300 bps), Sym1 spot +3% (vs perp +5% => -200 bps)
        make_candle(0, ts_2, (105.0, 105.0, 105.0, 105.0), 100.0, 0.0)?,
        make_candle(1, ts_2, (206.0, 206.0, 206.0, 206.0), 100.0, 0.0)?,
        // t=3 (last): Open(3) unchanged; Close(3) spot +3% & +4%
        make_candle(0, ts_3, (105.0, 108.15, 105.0, 108.15), 100.0, 0.0)?,
        make_candle(1, ts_3, (206.0, 214.24, 206.0, 214.24), 100.0, 0.0)?,
    ];

    let (_, recs_3a) = run_carry_uta_falsification_detailed(
        &mut VecCandleReader::new(lin_3a),
        &mut VecCandleReader::new(spt_3a),
        &cfg_3a,
    )?;
    assert_eq!(recs_3a.len(), 3);

    // Record 0: t=1 Open -> t=2 Open
    // basis = 0.40*(0.05-0.02)*10_000 + 0.40*(0.03-0.05)*10_000 = 120 - 80 = 40.0 bps
    // fund = 0.40*12.0 + 0.40*8.0 = 8.0 bps
    // turn = 0.80 (entry rebalance) => vip0 cost = 0.80*18.50 = 14.80, diag cost = 0.80*12.00 = 9.60
    let r0 = &recs_3a[0];
    assert_eq!(r0.decision_ts_ns, ts_0);
    assert_eq!(r0.entry_ts_ns, ts_1);
    assert_eq!(r0.exit_ts_ns, ts_2);
    assert!(!r0.is_eof_close);
    assert_close(r0.basis_bps, 40.0, 1e-10, "r0_basis")?;
    assert_close(r0.fund_bps, 8.0, 1e-12, "r0_fund")?;
    assert_close(r0.total_turn_l, 0.80, 1e-15, "r0_turn")?;
    assert_close(
        r0.cost_vip0_bps,
        0.80 * VIP0_CARRY_COST_BPS,
        1e-12,
        "r0_cost_vip0",
    )?;
    assert_close(
        r0.cost_maker_diag_bps,
        0.80 * MAKER_DIAG_CARRY_COST_BPS,
        1e-12,
        "r0_cost_diag",
    )?;
    assert_close(r0.net_vip0_bps, 40.0 + 8.0 - 14.80, 1e-10, "r0_net_vip0")?;
    assert_close(
        r0.net_maker_diag_bps,
        40.0 + 8.0 - 9.60,
        1e-10,
        "r0_net_diag",
    )?;

    // Record 1: t=2 Open -> t=3 Open (credits t=3 funding: 0.40*100 + 0.40*50 = 60.0 bps, basis = 0)
    let r1 = &recs_3a[1];
    assert!(!r1.is_eof_close);
    assert_close(r1.basis_bps, 0.0, 1e-12, "r1_basis")?;
    assert_close(r1.fund_bps, 60.0, 1e-12, "r1_fund")?;
    assert_close(r1.total_turn_l, 0.0, 1e-15, "r1_turn")?;

    // Record 2: t=3 Open -> t=3 Close (EOF)
    // basis = 0.40*(0.03-0.02)*10_000 + 0.40*(0.04-0.01)*10_000 = 40 + 120 = 160.0 bps
    // fund = 0.0 (slice[last] funding ignored on EOF)
    // eof_close_turn_l == total_covered_l == 0.80
    let r2 = &recs_3a[2];
    assert!(r2.is_eof_close);
    assert_eq!(r2.entry_ts_ns, ts_3);
    assert_eq!(r2.exit_ts_ns, ts_3);
    assert_close(r2.basis_bps, 160.0, 1e-10, "r2_eof_basis")?;
    assert_close(r2.fund_bps, 0.0, 1e-15, "r2_eof_fund_ignored")?;
    assert_close(
        r2.eof_close_turn_l,
        r2.total_covered_l,
        1e-15,
        "r2_eof_turn",
    )?;
    assert_close(r2.total_turn_l, 0.80, 1e-15, "r2_total_turn")?;
    assert_close(r2.cost_vip0_bps, 14.80, 1e-12, "r2_cost_vip0")?;
    assert_close(r2.cost_maker_diag_bps, 9.60, 1e-12, "r2_cost_diag")?;
    assert_close(r2.net_vip0_bps, 160.0 - 14.80, 1e-10, "r2_net_vip0")?;
    assert_close(r2.net_maker_diag_bps, 160.0 - 9.60, 1e-10, "r2_net_diag")?;

    // 3B. High-Sharpe GateVerdict checks:
    // Case 1: Strong positive Carry where BOTH VIP-0 and MakerDiag have t_nw >= 3.50 & DSR >= 0.95;
    // VIP-0 passes, while diag_zero_cells[0] MUST remain non-binding and strictly GateVerdict::Fail!
    let spec_3b = CarryUtaStrategySpec {
        name: "HighSharpe_SinglePair",
        selection_mode: CarrySelectionMode::BlindPeriodic,
        coverage_ratio: 1.0,
        recenter_trigger_pct: 0.50,
        window_days: 1,
        entry_apr_pct: 5.0,
        exit_apr_pct: 0.0,
        max_pairs: 1,
        min_diversification: 1,
        check_cadence_days: 1,
        payback_days: 30,
        min_hold_days: 0,
    };
    let mut pass_lin = Vec::with_capacity(40);
    let mut pass_spt = Vec::with_capacity(40);
    for day in 0..40 {
        let ts = BASE_TS_NS + (day as i64) * DAY_NS;
        let fund = if day % 2 == 0 { 42.0 } else { 48.0 };
        pass_lin.push(make_candle(
            0,
            ts,
            (100.0, 100.0, 100.0, 100.0),
            100.0,
            fund,
        )?);
        pass_spt.push(make_candle(
            0,
            ts,
            (100.0, 100.0, 100.0, 100.0),
            100.0,
            0.0,
        )?);
    }
    let cfg_3b = CarryUtaConfig {
        strategies: vec![spec_3b],
        slippage_bps: Bps::new(1.5)?,
        borrow_apr_pct: 15.0,
        ..CarryUtaConfig::default()
    };
    let (pass_rep, _) = run_carry_uta_falsification_detailed(
        &mut VecCandleReader::new(pass_lin),
        &mut VecCandleReader::new(pass_spt),
        &cfg_3b,
    )?;
    let p_vip = &pass_rep.report.vip0_cells[0];
    let p_diag = &pass_rep.report.diag_zero_cells[0];
    assert!(p_vip.is_binding);
    assert!(p_vip.t_nw.ok_or("pass vip t_nw")? >= 3.50);
    assert!(p_vip.dsr.ok_or("pass vip dsr")? >= 0.95);
    assert_eq!(p_vip.verdict, GateVerdict::Pass);
    assert!(!p_diag.is_binding);
    assert!(p_diag.t_nw.ok_or("pass diag t_nw")? >= 3.50);
    assert!(p_diag.dsr.ok_or("pass diag dsr")? >= 0.95);
    assert_eq!(p_diag.verdict, GateVerdict::Fail);

    // Case 2: Borderline switching Carry where VIP-0 fails (18.50 bps/turn * 2.0 = 37.0 > 32.0),
    // while MakerDiag (12.00 bps/turn * 2.0 = 24.0 < 32.0) achieves t_nw >= 3.50, DSR >= 0.95, n >= n_min,
    // YET diag_zero_cells[0].is_binding == false and verdict == GateVerdict::Fail!
    let mut border_lin = Vec::with_capacity(160);
    let mut border_spt = Vec::with_capacity(160);
    for day in 0..80 {
        let ts = BASE_TS_NS + (day as i64) * DAY_NS;
        let (f0, f1) = if day % 2 == 0 {
            let bump = if (day / 2) % 2 == 0 { 31.0 } else { 33.0 };
            (bump, -5.0)
        } else {
            let bump = if ((day - 1) / 2) % 2 == 0 { 31.0 } else { 33.0 };
            (-5.0, bump)
        };
        border_lin.push(make_candle(0, ts, (100.0, 100.0, 100.0, 100.0), 100.0, f0)?);
        border_lin.push(make_candle(1, ts, (100.0, 100.0, 100.0, 100.0), 100.0, f1)?);
        border_spt.push(make_candle(
            0,
            ts,
            (100.0, 100.0, 100.0, 100.0),
            100.0,
            0.0,
        )?);
        border_spt.push(make_candle(
            1,
            ts,
            (100.0, 100.0, 100.0, 100.0),
            100.0,
            0.0,
        )?);
    }
    let (border_rep, _) = run_carry_uta_falsification_detailed(
        &mut VecCandleReader::new(border_lin),
        &mut VecCandleReader::new(border_spt),
        &cfg_3b,
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

    Ok(())
}

#[test]
fn zero_lookahead_bit_exact_and_late_spot_listing() -> TestResult {
    // 4A. 120-day, 5-symbol dual dataset. Cutoff = day 80.
    // Includes pre-cutoff funding rotation and +40% price jump around day 45 so recenter & borrow
    // trigger BEFORE cutoff; mutating all bars ts > cutoff (10x prices, -500 bps funding) MUST preserve
    // every CarryUtaDayRecord with exit_ts_ns <= cutoff bit-for-bit across all 9 default strategies.
    let total_days = 120usize;
    let cutoff = 80usize;
    let cutoff_ts_ns = BASE_TS_NS + (cutoff as i64) * DAY_NS;

    let mut base_lin = Vec::with_capacity(total_days * 5);
    let mut base_spt = Vec::with_capacity(total_days * 5);
    for day in 0..total_days {
        let ts = BASE_TS_NS + (day as i64) * DAY_NS;
        for sym in 0_u16..5 {
            let base_px = match sym {
                0 => {
                    if day < 45 {
                        100.0 + (day as f64) * 0.2
                    } else {
                        152.0 + ((day - 45) as f64) * 0.1
                    }
                }
                1 => {
                    if day < 50 {
                        100.0
                    } else {
                        136.0 - ((day - 50) as f64) * 0.15
                    }
                }
                _ => 100.0 + (sym as f64) * 20.0 + ((day % 7) as f64) * 0.5,
            };
            let spt_px = base_px * (1.0 + 0.0004 * ((sym as f64) - 2.0));
            let fund = match sym {
                0 => {
                    if day < 55 {
                        5.5
                    } else {
                        1.5
                    }
                }
                1 => 4.8,
                2 => {
                    if day < 40 {
                        2.5
                    } else {
                        8.5
                    }
                }
                3 => 3.6,
                _ => 2.2,
            };
            base_lin.push(make_candle(
                sym,
                ts,
                (base_px, base_px * 1.01, base_px * 0.99, base_px * 1.001),
                100.0,
                fund,
            )?);
            base_spt.push(make_candle(
                sym,
                ts,
                (spt_px, spt_px * 1.01, spt_px * 0.99, spt_px * 1.0005),
                100.0,
                0.0,
            )?);
        }
    }

    let mut mut_lin = base_lin.clone();
    let mut mut_spt = base_spt.clone();
    for bar in &mut mut_lin {
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
    for bar in &mut mut_spt {
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

    let cfg = CarryUtaConfig::default();
    let (base_rep, base_recs) = run_carry_uta_falsification_detailed(
        &mut VecCandleReader::new(base_lin),
        &mut VecCandleReader::new(base_spt),
        &cfg,
    )?;
    let (mut_rep, mut_recs) = run_carry_uta_falsification_detailed(
        &mut VecCandleReader::new(mut_lin),
        &mut VecCandleReader::new(mut_spt),
        &cfg,
    )?;
    assert_eq!(base_rep.diagnostics.len(), 9);
    assert_eq!(mut_rep.diagnostics.len(), 9);

    for spec in default_carry_uta_grid() {
        let name = spec.name;
        let base_pre: Vec<&CarryUtaDayRecord> = base_recs
            .iter()
            .filter(|r| r.strategy == name && r.exit_ts_ns <= cutoff_ts_ns)
            .collect();
        let mut_pre: Vec<&CarryUtaDayRecord> = mut_recs
            .iter()
            .filter(|r| r.strategy == name && r.exit_ts_ns <= cutoff_ts_ns)
            .collect();
        assert!(!base_pre.is_empty(), "{name} must have pre-cutoff records");
        assert_eq!(base_pre.len(), mut_pre.len(), "{name} pre-cutoff len");
        assert!(
            base_pre.iter().any(|r| !r.weights.is_empty()),
            "{name} must hold non-empty weights before cutoff"
        );
        for (b, m) in base_pre.iter().zip(mut_pre.iter()) {
            assert_eq!(b.decision_ts_ns, m.decision_ts_ns);
            assert_eq!(b.entry_ts_ns, m.entry_ts_ns);
            assert_eq!(b.exit_ts_ns, m.exit_ts_ns);
            assert_eq!(b.is_eof_close, m.is_eof_close);
            assert_eq!(b.recenter_events, m.recenter_events);
            assert_eq!(b.weights.len(), m.weights.len());
            for ((bk, bv), (mk, mv)) in b.weights.iter().zip(m.weights.iter()) {
                assert_eq!(bk, mk);
                assert_eq!(bv.to_bits(), mv.to_bits(), "{name} weight bits");
            }
            assert_eq!(b.basis_bps.to_bits(), m.basis_bps.to_bits());
            assert_eq!(b.fund_bps.to_bits(), m.fund_bps.to_bits());
            assert_eq!(b.borrow_bps.to_bits(), m.borrow_bps.to_bits());
            assert_eq!(b.total_turn_l.to_bits(), m.total_turn_l.to_bits());
            assert_eq!(b.cost_vip0_bps.to_bits(), m.cost_vip0_bps.to_bits());
            assert_eq!(b.net_vip0_bps.to_bits(), m.net_vip0_bps.to_bits());
            assert_eq!(
                b.net_maker_diag_bps.to_bits(),
                m.net_maker_diag_bps.to_bits()
            );
        }
    }

    // 4B. Late Spot listing: Sym3 has linear candles from day 0 with +40% APR funding,
    // and Spot candles starting ONLY at day 60.
    // Sym3 MUST have zero allocation until after day 60 (decision >= day 60, entry >= day 61),
    // and MUST participate across all 9 strategies once dual-listed.
    let mut late_lin = Vec::new();
    let mut late_spt = Vec::new();
    let sym3 = SymbolId(3);
    for day in 0..120 {
        let ts = BASE_TS_NS + (day as i64) * DAY_NS;
        for sym in 0_u16..3 {
            let p = 100.0 + (sym as f64) * 10.0;
            let apr = 9.0 + (sym as f64);
            late_lin.push(make_candle(
                sym,
                ts,
                (p, p, p, p),
                100.0,
                daily_bps_for_apr(apr),
            )?);
            late_spt.push(make_candle(sym, ts, (p, p, p, p), 100.0, 0.0)?);
        }
        late_lin.push(make_candle(
            3,
            ts,
            (150.0, 150.0, 150.0, 150.0),
            100.0,
            daily_bps_for_apr(40.0),
        )?);
        if day >= 60 {
            late_spt.push(make_candle(
                3,
                ts,
                (150.0, 150.0, 150.0, 150.0),
                100.0,
                0.0,
            )?);
        }
    }
    let (_, late_recs) = run_carry_uta_falsification_detailed(
        &mut VecCandleReader::new(late_lin),
        &mut VecCandleReader::new(late_spt),
        &cfg,
    )?;
    let ts_60 = BASE_TS_NS + 60 * DAY_NS;
    let ts_61 = BASE_TS_NS + 61 * DAY_NS;

    for spec in default_carry_uta_grid() {
        let name = spec.name;
        assert!(
            late_recs
                .iter()
                .filter(|r| r.strategy == name && r.decision_ts_ns < ts_60)
                .all(|r| !r.weights.contains_key(&sym3)),
            "{name} leaked Sym3 before spot listing day 60"
        );
        assert!(
            late_recs
                .iter()
                .filter(|r| r.strategy == name && r.entry_ts_ns < ts_61)
                .all(|r| !r.weights.contains_key(&sym3)),
            "{name} entered Sym3 before day 61"
        );
        assert!(
            late_recs.iter().any(|r| {
                r.strategy == name
                    && r.decision_ts_ns >= ts_60
                    && r.entry_ts_ns >= ts_61
                    && r.weights.get(&sym3).is_some_and(|&w| w > 0.0)
            }),
            "{name} failed to allocate to Sym3 after dual listing"
        );
    }

    Ok(())
}

#[test]
fn cli_scan_carry_uta_end_to_end_and_rejections() -> TestResult {
    let tmp = TempDirGuard::new("cli_scan_carry_uta")?;
    let linear_dir = tmp.path.join("linear");
    let spot_dir = tmp.path.join("spot");
    let out_json = tmp.path.join("carry_uta_report.json");
    fs::create_dir_all(&linear_dir)?;
    fs::create_dir_all(&spot_dir)?;

    let days = 120usize;
    for sym in 0_u16..4 {
        let mut lin = Vec::with_capacity(days);
        let mut spt = Vec::with_capacity(days);
        for day in 0..days {
            let ts = BASE_TS_NS + (day as i64) * DAY_NS;
            let base = if sym == 0 && day >= 45 {
                142.0 + ((day - 45) as f64) * 0.2
            } else {
                100.0 + (sym as f64) * 15.0 + ((day % 13) as f64) * 0.4
            };
            let s_px = base * 1.0005;
            let fund = 3.0 + (sym as f64) * 0.8 + if day % 2 == 0 { 0.3 } else { -0.2 };
            lin.push(make_candle(
                sym,
                ts,
                (base, base * 1.01, base * 0.99, base * 1.002),
                100.0,
                fund,
            )?);
            spt.push(make_candle(
                sym,
                ts,
                (s_px, s_px * 1.01, s_px * 0.99, s_px * 1.001),
                100.0,
                0.0,
            )?);
        }
        write_csv_lf(&linear_dir.join(format!("sym{sym}.csv")), &lin)?;
        write_csv_lf(&spot_dir.join(format!("sym{sym}.csv")), &spt)?;
    }

    let lin_arg = linear_dir.to_str().ok_or("linear dir utf8")?;
    let spt_arg = spot_dir.to_str().ok_or("spot dir utf8")?;
    let out_arg = out_json.to_str().ok_or("out json utf8")?;

    let ascii = run_cli([
        "vortex-alfa",
        "scan-carry-uta",
        "--linear-dir",
        lin_arg,
        "--spot-dir",
        spt_arg,
        "--slippage-bps",
        "1.5",
        "--borrow-apr-pct",
        "15.0",
        "--out",
        out_arg,
    ])?;

    assert!(ascii.contains("Track6-CarryUTA"));
    for col in ["BorrowBp", "AvgUtil%", "RecCnt"] {
        assert!(ascii.contains(col), "ASCII missing column {col}");
    }
    for spec in default_carry_uta_grid() {
        assert!(
            ascii.contains(spec.name),
            "ASCII missing strategy {}",
            spec.name
        );
    }

    let json_text = fs::read_to_string(&out_json)?;
    let parsed: CarryUtaReport = serde_json::from_str(&json_text)?;
    assert_eq!(parsed.report.track, "Track6-CarryUTA");
    assert_eq!(parsed.report.m_hypotheses, 9);
    assert_close(parsed.borrow_apr_pct, 15.0, 1e-15, "json_borrow_apr")?;
    assert_close(parsed.slippage_bps, 1.5, 1e-15, "json_slippage_bps")?;
    assert_close(parsed.days_per_year, 365.0, 1e-15, "json_days_per_year")?;
    assert_eq!(parsed.slices_processed, 120);
    assert_eq!(parsed.linear_candles_processed, 480);
    assert_eq!(parsed.spot_candles_processed, 480);
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
        run_cli(["vortex-alfa", "scan-carry-uta", "--spot-dir", spt_arg]),
        Err(DomainError::Cli(_))
    ));
    assert!(matches!(
        run_cli(["vortex-alfa", "scan-carry-uta", "--linear-dir", lin_arg]),
        Err(DomainError::Cli(_))
    ));
    assert!(matches!(
        run_cli([
            "vortex-alfa",
            "scan-carry-uta",
            "--linear-dir",
            lin_arg,
            "--spot-dir",
            spt_arg,
            "--slippage-bps",
            "-0.5",
        ]),
        Err(DomainError::Cli(_))
    ));
    assert!(matches!(
        run_cli([
            "vortex-alfa",
            "scan-carry-uta",
            "--linear-dir",
            lin_arg,
            "--spot-dir",
            spt_arg,
            "--borrow-apr-pct",
            "-1.0",
        ]),
        Err(DomainError::Cli(_))
    ));
    assert!(matches!(
        run_cli([
            "vortex-alfa",
            "scan-carry-uta",
            "--linear-dir",
            lin_arg,
            "--spot-dir",
            spt_arg,
            "--borrow-apr-pct",
            "NaN",
        ]),
        Err(DomainError::Cli(_))
    ));
    assert!(matches!(
        run_cli(["vortex-alfa", "scan-carry-uta", "--bogus"]),
        Err(DomainError::Cli(_))
    ));

    Ok(())
}

#[test]
fn cash50_blind30d_replicates_phase4_topcarry30d() -> TestResult {
    // 100-day smooth 4-symbol dual dataset with prices bounded within +/-15% around 100.0
    // so +80% recenter never fires and UTA borrow is always 0.0.
    // Includes funding shifts across 30d checkpoints to exercise rebalance & exit parity too.
    let mut lin_candles = Vec::with_capacity(400);
    let mut spt_candles = Vec::with_capacity(400);
    for day in 0..100 {
        let ts = BASE_TS_NS + (day as i64) * DAY_NS;
        for sym in 0_u16..4 {
            let phase = (day as f64) * 0.15 + (sym as f64) * 1.1;
            let p_lin = 100.0 + 10.0 * phase.sin();
            let p_spt = p_lin * (1.0 + 0.0005 * (((day + sym as usize) % 3) as f64 - 1.0));
            let fund = match sym {
                0 => {
                    if day < 45 {
                        4.5
                    } else {
                        -2.0
                    }
                }
                1 => 3.5,
                2 => {
                    if day < 40 {
                        2.5
                    } else {
                        5.2
                    }
                }
                _ => 1.8,
            };
            lin_candles.push(make_candle(
                sym,
                ts,
                (p_lin, p_lin * 1.01, p_lin * 0.99, p_lin * 1.001),
                100.0,
                fund,
            )?);
            spt_candles.push(make_candle(
                sym,
                ts,
                (p_spt, p_spt * 1.01, p_spt * 0.99, p_spt * 1.0005),
                100.0,
                0.0,
            )?);
        }
    }

    let uta_cfg = CarryUtaConfig {
        strategies: vec![default_carry_uta_grid()[0]],
        slippage_bps: Bps::new(1.5)?,
        ..CarryUtaConfig::default()
    };
    let macro_cfg = MacroConfig {
        trend_strategies: vec![],
        donchian_strategies: vec![],
        carry_strategies: vec![default_carry_params()[2]],
        slippage_bps: Bps::new(1.5)?,
        ..MacroConfig::default()
    };

    let (uta_rep, uta_recs) = run_carry_uta_falsification_detailed(
        &mut VecCandleReader::new(lin_candles.clone()),
        &mut VecCandleReader::new(spt_candles.clone()),
        &uta_cfg,
    )?;
    let (macro_rep, macro_recs) = run_macro_falsification_detailed(
        &mut VecCandleReader::new(lin_candles),
        &mut VecCandleReader::new(spt_candles),
        &macro_cfg,
    )?;

    assert_eq!(uta_rep.diagnostics[0].strategy, "Cash50_Blind30d");
    assert_eq!(
        macro_rep.diagnostics[0].strategy,
        "Carry_SpotPerp_TopCarry30d"
    );
    assert_eq!(uta_recs.len(), macro_recs.len());
    assert!(uta_recs.len() >= 65);

    for (u, m) in uta_recs.iter().zip(macro_recs.iter()) {
        assert_eq!(u.decision_ts_ns, m.decision_ts_ns);
        assert_eq!(u.entry_ts_ns, m.entry_ts_ns);
        assert_eq!(u.exit_ts_ns, m.exit_ts_ns);
        assert_eq!(u.is_eof_close, m.is_eof_close);
        assert_eq!(u.recenter_events, 0);
        assert_close(u.borrow_bps, 0.0, 1e-15, "cash50_zero_borrow")?;
        assert_eq!(u.weights.len(), m.weights.len());
        for ((uk, uv), (mk, mv)) in u.weights.iter().zip(m.weights.iter()) {
            assert_eq!(uk, mk);
            assert_close(*uv, *mv, 1e-15, "cash50_weight")?;
        }
        assert_close(u.basis_bps, m.price_or_basis_bps, 1e-10, "cash50_basis")?;
        assert_close(u.fund_bps, m.fund_bps, 1e-10, "cash50_fund")?;
        assert_close(u.total_turn_l, m.turnover, 1e-10, "cash50_turnover")?;
        assert_close(u.cost_vip0_bps, m.cost_vip0_bps, 1e-10, "cash50_cost_vip0")?;
        assert_close(u.net_vip0_bps, m.net_vip0_bps, 1e-10, "cash50_net_vip0")?;
        assert_close(
            u.net_maker_diag_bps,
            m.net_maker_bps,
            1e-10,
            "cash50_net_maker",
        )?;
    }

    Ok(())
}
