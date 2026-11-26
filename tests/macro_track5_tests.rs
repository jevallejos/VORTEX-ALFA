use std::collections::BTreeMap;
use std::error::Error;
use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use vortex_alfa::application::falsifier_macro::{
    run_macro_falsification, run_macro_falsification_detailed,
};
use vortex_alfa::application::macro_accounting::{
    CostPerTurnover, MacroDayRecord, MacroFeeProfiles, SPOT_VIP0_FEE_BPS,
};
use vortex_alfa::application::macro_baselines::{
    compute_equity_metrics, compute_passive_baselines, weight_turnover, EquityCurveMetrics,
    MacroBaselineRow, MacroConfig, MacroDailySlice, MacroFalsificationReport, MacroFamily,
    MacroStrategyRow,
};
use vortex_alfa::domain::error::DomainError;
use vortex_alfa::domain::ports::CandleSeriesReader;
use vortex_alfa::domain::signals::macro_donchian::{
    default_donchian_params, prior_donchian_channel, DonchianBreakoutStrategy, DonchianChannels,
    DonchianParams,
};
use vortex_alfa::domain::signals::macro_trend::{
    default_trend_params, MacroTrendFeatureEngine, MacroTrendParams, SymbolTrendSnapshot,
    VolTargetTrendStrategy, TREND_HORIZONS,
};
use vortex_alfa::domain::signals::macro_vol::{
    CausalEwmaVol, TrendVolConfig, CRYPTO_DAYS_PER_YEAR, EWMA_VOL_SPAN_DAYS, MAX_SIGMA_ANN,
    MIN_SIGMA_ANN,
};
use vortex_alfa::domain::signals::spot_perp_carry::{
    default_carry_params, SpotPerpCarryEngine, SpotPerpCarryParams,
};
use vortex_alfa::domain::stats::GateVerdict;
use vortex_alfa::domain::types::{
    Bps, Candle, FundingTimestampSemantics, PriceE8, SymbolId, TimestampNs, SCALE_E8,
};
use vortex_alfa::interfaces::cli::run_cli;

type TestResult = Result<(), Box<dyn Error>>;

const DAY_NS: i64 = 86_400_000_000_000;
const BASE_TS_NS: i64 = 1_704_067_200_000_000_000;

static TEMP_SEQ: AtomicU64 = AtomicU64::new(1);

struct TempDirGuard {
    path: PathBuf,
}

impl TempDirGuard {
    fn new(prefix: &str) -> Result<Self, Box<dyn Error>> {
        let id = TEMP_SEQ.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "vortex_alfa_macro_{prefix}_{}_{}",
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

fn assert_opt_close(
    actual: Option<f64>,
    expected: Option<f64>,
    tol: f64,
    label: &str,
) -> TestResult {
    match (actual, expected) {
        (None, None) => Ok(()),
        (Some(a), Some(e)) => assert_close(a, e, tol, label),
        _ => {
            Err(format!("{label}: option mismatch actual={actual:?}, expected={expected:?}").into())
        }
    }
}

fn make_slice(
    ts_ns: i64,
    linear_list: Vec<Candle>,
    spot_list: Vec<Candle>,
) -> Result<MacroDailySlice, DomainError> {
    let mut linear = BTreeMap::new();
    for candle in linear_list {
        linear.insert(candle.symbol_id, candle);
    }
    let mut spot = BTreeMap::new();
    for candle in spot_list {
        spot.insert(candle.symbol_id, candle);
    }
    Ok(MacroDailySlice {
        ts_ns: TimestampNs::new(ts_ns)?,
        linear,
        spot,
    })
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

fn independent_ewma_sigma_series(closes: &[f64]) -> Vec<Option<f64>> {
    let alpha = 2.0 / ((EWMA_VOL_SPAN_DAYS as f64) + 1.0);
    let mut init_rets = Vec::new();
    let mut ewma_var: Option<f64> = None;
    let mut out = Vec::with_capacity(closes.len());
    for (idx, &close) in closes.iter().enumerate() {
        if idx > 0 {
            let ret = (close / closes[idx - 1]).ln();
            if let Some(prev) = ewma_var {
                ewma_var = Some((1.0 - alpha) * prev + alpha * ret * ret);
            } else {
                init_rets.push(ret);
                if init_rets.len() == EWMA_VOL_SPAN_DAYS {
                    let n = EWMA_VOL_SPAN_DAYS as f64;
                    let mean = init_rets.iter().sum::<f64>() / n;
                    let ss: f64 = init_rets.iter().map(|r| (r - mean) * (r - mean)).sum();
                    ewma_var = Some(ss / (n - 1.0));
                }
            }
        }
        out.push(ewma_var.map(|v| {
            (v * CRYPTO_DAYS_PER_YEAR)
                .sqrt()
                .clamp(MIN_SIGMA_ANN, MAX_SIGMA_ANN)
        }));
    }
    out
}

#[test]
fn test_macro_trend_and_donchian_closed_form_vectors() -> TestResult {
    // 1A. CausalEwmaVol direct analytical verification.
    let vol_cfg = TrendVolConfig::default();
    assert_eq!(vol_cfg.ewma_span, 20);
    assert_close(vol_cfg.days_per_year, 365.0, 1e-15, "days_per_year")?;
    assert_close(vol_cfg.min_sigma_ann, 0.15, 1e-15, "min_sigma_ann")?;
    assert_close(vol_cfg.max_sigma_ann, 3.00, 1e-15, "max_sigma_ann")?;

    let mut vol = CausalEwmaVol::new(vol_cfg)?;
    for idx in 0..19 {
        let ret = if idx % 2 == 0 { 0.01 } else { -0.01 };
        assert_eq!(vol.update(ret)?, None);
        assert_eq!(vol.current_sigma_ann(), None);
        assert_eq!(vol.current_variance(), None);
    }
    // 20th return: 10 values of +0.01 and 10 values of -0.01 => mean = 0.0, var = 0.0020 / 19.
    let expected_var_20 = 0.0020_f64 / 19.0;
    let expected_sigma_20 = (expected_var_20 * 365.0).sqrt();
    let sigma_20 = vol
        .update(-0.01)?
        .ok_or("expected initialized sigma at return 20")?;
    assert_close(
        vol.current_variance().ok_or("missing var_20")?,
        expected_var_20,
        1e-15,
        "ewma_var_20",
    )?;
    assert_close(sigma_20, expected_sigma_20, 1e-14, "ewma_sigma_20")?;

    // 21st return: alpha = 2 / 21, var_21 = (19/21)*(0.0020/19) + (2/21)*0.03^2 = 0.0038 / 21.
    let expected_var_21 = 0.0038_f64 / 21.0;
    let expected_sigma_21 = (expected_var_21 * 365.0).sqrt();
    let sigma_21 = vol
        .update(0.03)?
        .ok_or("expected updated sigma at return 21")?;
    assert_close(
        vol.current_variance().ok_or("missing var_21")?,
        expected_var_21,
        1e-15,
        "ewma_var_21",
    )?;
    assert_close(sigma_21, expected_sigma_21, 1e-14, "ewma_sigma_21")?;

    // Low and high sigma_ann clamping to [0.15, 3.00].
    let mut low_vol = CausalEwmaVol::new(vol_cfg)?;
    let mut high_vol = CausalEwmaVol::new(vol_cfg)?;
    for idx in 0..20 {
        let sign = if idx % 2 == 0 { 1.0 } else { -1.0 };
        let _ = low_vol.update(sign * 0.001)?;
        let _ = high_vol.update(sign * 0.30)?;
    }
    assert_close(
        low_vol.current_sigma_ann().ok_or("low sigma")?,
        MIN_SIGMA_ANN,
        1e-15,
        "low_sigma_clamped",
    )?;
    assert_close(
        high_vol.current_sigma_ann().ok_or("high sigma")?,
        MAX_SIGMA_ANN,
        1e-15,
        "high_sigma_clamped",
    )?;

    // 1B. MacroTrendFeatureEngine + VolTargetTrendStrategy piecewise closed-form checks.
    assert_eq!(TREND_HORIZONS, [20, 60, 120]);
    let defaults = default_trend_params();
    assert_eq!(defaults.len(), 3);
    assert_eq!(defaults[0].name, "MacroTrend_LS_Vol15");
    assert_eq!(defaults[1].name, "MacroTrend_LS_Vol30");
    assert_eq!(defaults[2].name, "MacroTrend_LO_Vol20");

    // Build 123 daily bars (indices 0..=122) for Symbol(0):
    // Bars 0..=59: 100.0, Bars 60..=99: 110.0, Bars 100..=119: 120.0,
    // Bar 120: 115.0, Bar 121: 115.0, Bar 122: 95.0.
    let mut closes = Vec::with_capacity(123);
    for t in 0..123 {
        let c = if t < 60 {
            100.0
        } else if t < 100 {
            110.0
        } else if t < 120 {
            120.0
        } else if t <= 121 {
            115.0
        } else {
            95.0
        };
        closes.push(c);
    }
    let expected_sigmas = independent_ewma_sigma_series(&closes);

    let sym0 = SymbolId(0);
    let mut trend_engine = MacroTrendFeatureEngine::new(vol_cfg)?;
    let mut strat_ls15 = VolTargetTrendStrategy::new(defaults[0])?;
    let mut strat_lo20 = VolTargetTrendStrategy::new(defaults[2])?;
    let mut weight_120_ls15 = 0.0_f64;

    for (t, &c) in closes.iter().enumerate() {
        let mut slice_map = BTreeMap::new();
        slice_map.insert(
            sym0,
            make_candle(
                0,
                BASE_TS_NS + (t as i64) * DAY_NS,
                (c, c + 1.0, c - 1.0, c),
                100.0,
                0.0,
            )?,
        );
        let active = trend_engine.on_slice(&slice_map)?;
        let snap: SymbolTrendSnapshot = trend_engine.snapshot(sym0).ok_or("missing snapshot")?;
        assert_opt_close(
            snap.sigma_ann,
            expected_sigmas[t],
            1e-13,
            "engine_sigma_ann",
        )?;

        let ls15_out = strat_ls15.evaluate(&active, &trend_engine)?;
        let lo20_out = strat_lo20.evaluate(&active, &trend_engine)?;

        if t < 120 {
            assert_eq!(snap.trend_signal_ls, None);
            assert_eq!(ls15_out, None);
            assert_eq!(lo20_out, None);
        } else if t == 120 {
            // s_20 = sign(ln(115/120)) = -1, s_60 = sign(ln(115/110)) = +1, s_120 = sign(ln(115/100)) = +1
            let expected_sig = 1.0 / 3.0;
            assert_opt_close(snap.trend_signal_ls, Some(expected_sig), 1e-15, "sig_120")?;
            let sig120 = expected_sigmas[120].ok_or("sigma 120")?;
            let exp_w_ls = expected_sig * (0.15 / sig120);
            let exp_w_lo = expected_sig * (0.20 / sig120);
            let w_ls = ls15_out.ok_or("ls15_120")?;
            let w_lo = lo20_out.ok_or("lo20_120")?;
            assert_close(w_ls[&sym0], exp_w_ls, 1e-13, "ls15_w_120")?;
            assert_close(w_lo[&sym0], exp_w_lo, 1e-13, "lo20_w_120")?;
            weight_120_ls15 = w_ls[&sym0];
        } else if t == 121 {
            // Return = 0.0 => sigma decreases by sqrt(19/21), delta weight < 0.02 rebalance band!
            let sig121 = expected_sigmas[121].ok_or("sigma 121")?;
            let target_w_121 = (1.0 / 3.0) * (0.15 / sig121);
            assert!((target_w_121 - weight_120_ls15).abs() < 0.02);
            let w_ls = ls15_out.ok_or("ls15_121")?;
            // Anti-churn retains exact previous bit pattern.
            assert_eq!(w_ls[&sym0].to_bits(), weight_120_ls15.to_bits());
        } else if t == 122 {
            // close=95 vs close[2]=100, close[62]=110, close[102]=120 => all 3 horizons negative (-1.0).
            assert_opt_close(snap.trend_signal_ls, Some(-1.0), 1e-15, "sig_122")?;
            let sig122 = expected_sigmas[122].ok_or("sigma 122")?;
            let exp_w_ls = -(0.15 / sig122);
            let w_ls = ls15_out.ok_or("ls15_122")?;
            let w_lo = lo20_out.ok_or("lo20_122")?;
            assert_close(w_ls[&sym0], exp_w_ls, 1e-13, "ls15_w_122")?;
            // LongOnly clips negative signal to 0.0 (removed from map).
            assert!(w_lo.is_empty());
        }
    }

    // Verify max_gross_leverage rescaling with 2 symbols on flat vol (sigma_ann = 0.15).
    let mut cap_engine = MacroTrendFeatureEngine::new(vol_cfg)?;
    let mut cap_strat = VolTargetTrendStrategy::new(MacroTrendParams {
        name: "CapTest",
        long_only: false,
        target_vol_ann: 0.60,
        max_gross_leverage: 0.50,
        rebalance_band: 0.0,
    })?;
    let sym1 = SymbolId(1);
    for t in 0..121 {
        let c0 = if t == 120 { 101.0 } else { 100.0 };
        let c1 = if t == 120 { 99.0 } else { 100.0 };
        let mut m = BTreeMap::new();
        m.insert(
            sym0,
            make_candle(0, BASE_TS_NS + t * DAY_NS, (c0, c0, c0, c0), 10.0, 0.0)?,
        );
        m.insert(
            sym1,
            make_candle(1, BASE_TS_NS + t * DAY_NS, (c1, c1, c1, c1), 10.0, 0.0)?,
        );
        let active = cap_engine.on_slice(&m)?;
        let out = cap_strat.evaluate(&active, &cap_engine)?;
        if t == 120 {
            let w = out.ok_or("expected capped weights")?;
            let gross: f64 = w.values().map(|v| v.abs()).sum();
            assert_close(gross, 0.50, 1e-14, "capped_gross_leverage")?;
            assert_close(w[&sym0], 0.25, 1e-14, "capped_sym0")?;
            assert_close(w[&sym1], -0.25, 1e-14, "capped_sym1")?;
        }
    }

    // 1C. DonchianBreakoutStrategy + prior_donchian_channel lookback isolation & transitions.
    let don_defaults = default_donchian_params();
    assert_eq!(don_defaults.len(), 3);
    assert_eq!(don_defaults[0].name, "Donchian_20_10_LS");
    assert_eq!(don_defaults[1].name, "Donchian_55_20_LS");
    assert_eq!(don_defaults[2].name, "Donchian_55_20_LO");

    let mut don_engine = MacroTrendFeatureEngine::new(vol_cfg)?;
    let don_ls_params = DonchianParams {
        name: "Don_5_3_LS",
        entry_n: 5,
        exit_n: 3,
        long_only: false,
        target_vol_ann: 0.20,
        max_gross_leverage: 1.0,
    };
    let don_lo_params = DonchianParams {
        name: "Don_5_3_LO",
        entry_n: 5,
        exit_n: 3,
        long_only: true,
        target_vol_ann: 0.20,
        max_gross_leverage: 1.0,
    };
    let mut don_ls = DonchianBreakoutStrategy::new(don_ls_params)?;
    let mut don_lo = DonchianBreakoutStrategy::new(don_lo_params)?;

    // Bars 0..=4: range H=104, L=96, C=100; Bar 5 has huge wick H=500, L=10, C=100.
    // Prior channel at Bar 5 MUST exclude Bar 5's own H=500 and L=10!
    for t in 0..=5 {
        let (h, l) = if t == 5 {
            (500.0, 10.0)
        } else {
            (104.0 - (t as f64) * 0.2, 96.0 + (t as f64) * 0.2)
        };
        let mut m = BTreeMap::new();
        m.insert(
            sym0,
            make_candle(0, BASE_TS_NS + t * DAY_NS, (100.0, h, l, 100.0), 10.0, 0.0)?,
        );
        let active = don_engine.on_slice(&m)?;
        assert_eq!(don_ls.evaluate(&active, &don_engine)?, None);
        if t == 5 {
            let ch: DonchianChannels =
                prior_donchian_channel(&don_engine, sym0, 5, 3).ok_or("missing ch at t=5")?;
            assert_close(ch.upper_in, 104.0, 1e-12, "prior_upper_in_excludes_t5")?;
            assert_close(ch.lower_in, 96.0, 1e-12, "prior_lower_in_excludes_t5")?;
            assert_close(ch.upper_out, 103.6, 1e-12, "prior_upper_out_excludes_t5")?;
            assert_close(ch.lower_out, 96.4, 1e-12, "prior_lower_out_excludes_t5")?;
        }
    }

    // Continue bars 6..=20 with flat H=104, L=96, C=100 so sigma_ann becomes Some(0.15) at t=20.
    // Then drive exact Donchian transitions at t=21..=25:
    // t=20: C=100 -> pos=0
    // t=21: C=106 (H=106, L=100) > upper_in(104) -> 0 -> +1 (both LS and LO)
    // t=22: C=94  (H=102, L=94)  < lower_in(96)  -> LS: +1 -> -1, LO: +1 -> 0 (never -1)
    // t=23: C=98  (H=101, L=97)  inside (lower_out=94, upper_out=106) -> LS holds -1, LO holds 0
    // t=24: C=107 (H=107, L=98)  > upper_out(106) AND > upper_in(106) wait: make t=21 H=110 so upper_in=110 while upper_out(last 3: 21,22,23 -> 22,23 have H<=102, wait make t=21 H=120, t=22..24 H<=102 so upper_in(last 5)=120, upper_out(last 3)=102, then t=25 C=105 exits short to 0 without entering long!)
    let script: [(f64, f64, f64); 6] = [
        (104.0, 96.0, 100.0),  // t=20: warmup completes, close=100 -> pos 0
        (120.0, 100.0, 105.0), // t=21: C=105 > upper_in=104 -> +1 (upper_in for next 5 bars = 120)
        (102.0, 94.0, 95.0),   // t=22: C=95 < lower_in=96 -> LS=-1, LO=0
        (101.0, 94.0, 98.0),   // t=23: inside (lower_out=94, upper_out=120) -> LS=-1, LO=0
        (100.0, 94.0, 97.0),   // t=24: still inside (upper_out(21..23)=120) -> LS=-1, LO=0
        (105.0, 97.0, 105.0), // t=25: upper_in(20..24)=120, upper_out(22..24)=102 -> 105 > 102 && <= 120 => LS=-1 -> 0!
    ];
    let mut all_don_closes = vec![100.0; 20];
    for t in 6..=25 {
        let (h, l, c) = if t < 20 {
            (104.0, 96.0, 100.0)
        } else {
            script[t - 20]
        };
        if t >= 20 {
            all_don_closes.push(c);
        }
        let mut m = BTreeMap::new();
        m.insert(
            sym0,
            make_candle(0, BASE_TS_NS + (t as i64) * DAY_NS, (c, h, l, c), 10.0, 0.0)?,
        );
        let active = don_engine.on_slice(&m)?;
        let out_ls = don_ls.evaluate(&active, &don_engine)?;
        let out_lo = don_lo.evaluate(&active, &don_engine)?;
        if t >= 20 {
            let sig_map = independent_ewma_sigma_series(&all_don_closes);
            let exp_sigma = sig_map[t].ok_or("expected don sigma")?;
            let w_ls = out_ls.ok_or("expected ls out")?;
            let w_lo = out_lo.ok_or("expected lo out")?;
            match t {
                20 => {
                    assert_eq!(don_ls.position(sym0), 0);
                    assert_eq!(don_lo.position(sym0), 0);
                    assert!(w_ls.is_empty());
                    assert!(w_lo.is_empty());
                }
                21 => {
                    assert_eq!(don_ls.position(sym0), 1);
                    assert_eq!(don_lo.position(sym0), 1);
                    let exp_w = 0.20 / exp_sigma;
                    assert_close(w_ls[&sym0], exp_w, 1e-13, "don_ls_long_w")?;
                    assert_close(w_lo[&sym0], exp_w, 1e-13, "don_lo_long_w")?;
                }
                22..=24 => {
                    assert_eq!(don_ls.position(sym0), -1);
                    assert_eq!(don_lo.position(sym0), 0);
                    let exp_w = -0.20 / exp_sigma;
                    assert_close(w_ls[&sym0], exp_w, 1e-13, "don_ls_short_w")?;
                    assert!(w_lo.is_empty());
                }
                25 => {
                    assert_eq!(don_ls.position(sym0), 0);
                    assert_eq!(don_lo.position(sym0), 0);
                    assert!(w_ls.is_empty());
                    assert!(w_lo.is_empty());
                }
                _ => {}
            }
        }
    }

    Ok(())
}

#[test]
fn test_spot_perp_carry_real_basis_vip0_fees_and_eof() -> TestResult {
    // 2A. Exact daily basis, funding, VIP-0 (18.50 bps), MakerDiag (12.00 bps), and EOF under BarOpenSettlement.
    let fees = MacroFeeProfiles::new(Bps::new(1.5)?)?;
    assert_close(SPOT_VIP0_FEE_BPS, 10.0, 1e-15, "spot_vip0_fee")?;
    let carry_costs: CostPerTurnover = fees.carry_costs();
    assert_close(carry_costs.vip0_bps, 18.50, 1e-15, "carry_vip0_cost")?;
    assert_close(carry_costs.maker_diag_bps, 12.00, 1e-15, "carry_maker_cost")?;

    let defaults = default_carry_params();
    assert_eq!(defaults[0].name, "Carry_SpotPerp_10Apr");
    assert_eq!(defaults[1].name, "Carry_SpotPerp_15Apr");
    assert_eq!(defaults[2].name, "Carry_SpotPerp_TopCarry30d");

    let carry_10 = defaults[0];
    let cfg = MacroConfig {
        trend_strategies: Vec::new(),
        donchian_strategies: Vec::new(),
        carry_strategies: vec![carry_10],
        slippage_bps: Bps::new(1.5)?,
        funding_semantics: FundingTimestampSemantics::BarOpenSettlement,
        ..MacroConfig::default()
    };

    // Direct hysteresis band verification (exit=0%, entry=10%):
    // Inactive stays inactive inside (0%, 10%); Active stays active inside (0%, 10%); exits <= 0%.
    let mut hyst = SpotPerpCarryEngine::new(carry_10)?;
    for day in 0..11 {
        let ts = BASE_TS_NS + (day as i64) * DAY_NS;
        // Days 0..=6: +1.5/day => 5.475% APR inside (0%, 10%) -> stays inactive.
        // Day 7: +20.0 => sum=29.0 (15.12% >= 10%) -> activates!
        // Day 8: -10.0 => sum=17.5 (9.125% in (0%, 10%)) -> stays active!
        // Day 9: -20.0 => sum=-4.0 (-2.09% <= 0%) -> exits!
        // Day 10: +12.0 => sum=6.5 (3.39% in (0%, 10%)) -> stays inactive!
        let f = match day {
            0..=6 => 1.5,
            7 => 20.0,
            8 => -10.0,
            9 => -20.0,
            _ => 12.0,
        };
        let mut lin_m = BTreeMap::new();
        let mut spt_m = BTreeMap::new();
        lin_m.insert(
            SymbolId(0),
            make_candle(0, ts, (100.0, 100.0, 100.0, 100.0), 10.0, f)?,
        );
        spt_m.insert(
            SymbolId(0),
            make_candle(0, ts, (100.0, 100.0, 100.0, 100.0), 10.0, 0.0)?,
        );
        let out = hyst.on_slice(&lin_m, &spt_m)?;
        if day < 6 {
            assert_eq!(out, None);
        } else if day == 6 || day >= 9 {
            assert!(out.ok_or("hyst inactive")?.is_empty());
        } else {
            let w = out.ok_or("hyst active")?;
            assert_close(w[&SymbolId(0)], 0.25, 1e-15, "hyst_active_band")?;
        }
    }

    // 12-day dual series (indices 0..=11) for Symbol(0):
    // Days 0..=6: fund = +4.0 bps (APR = 14.6% >= 10%) => decides L=0.25 at ts[6], opens at Open(7).
    // Day 7: Spot Open=100, Perp Open=100, fund=+4.0
    // Day 8: Spot Open=102 (+2%), Perp Open=101 (+1%), fund=+3.0 => APR=(24+3)/7*3.65=14.078% (stays active!).
    // Day 9: Spot Open=102, Perp Open=101, fund=-28.0 => APR=(20+3-28)/7*3.65 = -2.607% <= 0% => decides exit at ts[9], closes at Open(10)!
    // Day 10: Spot Open=104.04 (+2%), Perp Open=102.01 (+1%), fund=+30.0 => APR=(16+3-28+30)/7*3.65=10.95% >= 10% => re-enters at ts[10], opens at Open(11)!
    // Day 11 (EOF): Spot Open=105, Close=106.05 (+1%), Perp Open=103, Close=105.06 (+2%), fund=+999.0 (must be ignored at EOF!).
    let linear_spec: [(f64, f64, f64); 12] = [
        (100.0, 100.0, 4.0),
        (100.0, 100.0, 4.0),
        (100.0, 100.0, 4.0),
        (100.0, 100.0, 4.0),
        (100.0, 100.0, 4.0),
        (100.0, 100.0, 4.0),
        (100.0, 100.0, 4.0),
        (100.0, 100.0, 4.0),
        (101.0, 101.0, 3.0),
        (101.0, 101.0, -28.0),
        (102.01, 102.01, 30.0),
        (103.0, 105.06, 999.0),
    ];
    let spot_spec: [(f64, f64); 12] = [
        (100.0, 100.0),
        (100.0, 100.0),
        (100.0, 100.0),
        (100.0, 100.0),
        (100.0, 100.0),
        (100.0, 100.0),
        (100.0, 100.0),
        (100.0, 100.0),
        (102.0, 102.0),
        (102.0, 102.0),
        (104.04, 104.04),
        (105.0, 106.05),
    ];

    let mut lin_candles = Vec::with_capacity(12);
    let mut spt_candles = Vec::with_capacity(12);
    for (t, (&(lo, lc, lf), &(so, sc))) in linear_spec.iter().zip(spot_spec.iter()).enumerate() {
        let ts = BASE_TS_NS + (t as i64) * DAY_NS;
        lin_candles.push(make_candle(
            0,
            ts,
            (lo, lo.max(lc), lo.min(lc), lc),
            10.0,
            lf,
        )?);
        spt_candles.push(make_candle(
            0,
            ts,
            (so, so.max(sc), so.min(sc), sc),
            10.0,
            0.0,
        )?);
    }

    let mut lin_r = VecCandleReader::new(lin_candles);
    let mut spt_r = VecCandleReader::new(spt_candles);
    let (rep, records) = run_macro_falsification_detailed(&mut lin_r, &mut spt_r, &cfg)?;
    assert_eq!(rep.slices_processed, 12);
    // Records:
    // 0: Open(7)->Open(8), decision ts[6], L=0.25, turn=0.25, basis=0.25*(0.02-0.01)*10000=+25, fund=0.25*3=+0.75
    // 1: Open(8)->Open(9), decision ts[7], L=0.25, turn=0.0,  basis=0, fund=0.25*(-28)=-7.0
    // 2: Open(9)->Open(10), decision ts[8], L=0.25, turn=0.0, basis=0.25*(0.02-0.01)*10000=+25, fund=0.25*30=+7.5
    // 3: Open(10)->Open(11), decision ts[9] (exit {}!), L=0, turn=0.25, basis=0, fund=0
    // 4: Open(11)->Close(11) EOF, decision ts[10] (re-enter L=0.25!), turn=0.25+0.25=0.50,
    //    basis=0.25*((106.05-105)/105 - (105.06-103)/103)*10000 = 0.25*(0.01-0.02)*10000=-25, fund=0
    assert_eq!(records.len(), 5);
    let expected_days: [(bool, f64, f64, f64); 5] = [
        (false, 25.0, 0.75, 0.25),
        (false, 0.0, -7.0, 0.0),
        (false, 25.0, 7.5, 0.0),
        (false, 0.0, 0.0, 0.25),
        (true, -25.0, 0.0, 0.50),
    ];
    for (idx, (rec, &(eof, exp_bas, exp_fnd, exp_trn))) in
        records.iter().zip(expected_days.iter()).enumerate()
    {
        assert_eq!(rec.is_eof_close, eof, "eof flag day {idx}");
        assert_close(rec.price_or_basis_bps, exp_bas, 1e-11, "basis_bps")?;
        assert_close(rec.fund_bps, exp_fnd, 1e-12, "fund_bps")?;
        assert_close(rec.turnover, exp_trn, 1e-14, "turnover")?;
        let exp_vip0_cost = exp_trn * 18.50;
        let exp_mkr_cost = exp_trn * 12.00;
        assert_close(rec.cost_vip0_bps, exp_vip0_cost, 1e-12, "cost_vip0")?;
        assert_close(rec.cost_maker_bps, exp_mkr_cost, 1e-12, "cost_maker")?;
        assert_close(
            rec.net_vip0_bps,
            exp_bas + exp_fnd - exp_vip0_cost,
            1e-11,
            "net_vip0",
        )?;
        assert_close(
            rec.net_maker_bps,
            exp_bas + exp_fnd - exp_mkr_cost,
            1e-11,
            "net_maker",
        )?;
    }

    // 2B. TopCarry30d 30-day cadence (days 1..29 hold, day 30 rebalances) + SymbolId tie-breaking.
    let top30_params = SpotPerpCarryParams {
        name: "TopCarry30d_Tie",
        window_days: 30,
        entry_apr_pct: 8.0,
        exit_apr_pct: -2.0,
        max_pairs: 2,
        min_diversification: 2,
        min_rebalance_days: 30,
    };
    let mut top30 = SpotPerpCarryEngine::new(top30_params)?;
    let (s0, s1, s2) = (SymbolId(0), SymbolId(1), SymbolId(2));
    for day in 0..=59 {
        let ts = BASE_TS_NS + (day as i64) * DAY_NS;
        // Days 0..=29: all 3 symbols have identical +4.0 bps/day (14.6% APR) -> tie-break picks Sym(0), Sym(1).
        // Days 30..=58: Sym(0) plunges to -20 bps/day (rolling APR goes <= -2%), Sym(2) surges to +50 bps/day.
        // Because min_rebalance_days=30, days 30..=58 MUST hold {Sym(0): 0.25, Sym(1): 0.25} without rebalancing!
        // On day 59 (30 days after day 29), rebalance runs: Sym(0) exits, Sym(2) and Sym(1) are selected!
        let (f0, f1, f2) = if day < 30 {
            (4.0, 4.0, 4.0)
        } else {
            (-20.0, 4.0, 50.0)
        };
        let mut lin_map = BTreeMap::new();
        let mut spt_map = BTreeMap::new();
        for (sym, f) in [(s0, f0), (s1, f1), (s2, f2)] {
            lin_map.insert(
                sym,
                make_candle(sym.0, ts, (100.0, 100.0, 100.0, 100.0), 10.0, f)?,
            );
            spt_map.insert(
                sym,
                make_candle(sym.0, ts, (100.0, 100.0, 100.0, 100.0), 10.0, 0.0)?,
            );
        }
        let out = top30.on_slice(&lin_map, &spt_map)?;
        if day < 29 {
            assert_eq!(out, None);
        } else if day < 59 {
            let w = out.ok_or("expected top30 weights")?;
            assert_eq!(w.len(), 2);
            assert_close(w[&s0], 0.25, 1e-15, "top30_tie_sym0")?;
            assert_close(w[&s1], 0.25, 1e-15, "top30_tie_sym1")?;
            assert!(!w.contains_key(&s2));
        } else {
            let w = out.ok_or("expected rebalanced weights at day 59")?;
            assert_eq!(w.len(), 2);
            assert!(!w.contains_key(&s0));
            assert_close(w[&s1], 0.25, 1e-15, "top30_rebal_sym1")?;
            assert_close(w[&s2], 0.25, 1e-15, "top30_rebal_sym2")?;
        }
    }

    // 2C. GateVerdict checks:
    // Case 1: Strong positive Carry passes VIP0-Taker (t_nw >= 3.50, DSR >= 0.95, N >= n_min),
    // while diag_zero_cells STILL NEVER returns Pass.
    let mut pass_lin = Vec::new();
    let mut pass_spt = Vec::new();
    for t in 0..70 {
        let ts = BASE_TS_NS + t * DAY_NS;
        let fund = if t % 2 == 0 { 42.0 } else { 48.0 };
        pass_lin.push(make_candle(
            0,
            ts,
            (100.0, 100.0, 100.0, 100.0),
            10.0,
            fund,
        )?);
        pass_spt.push(make_candle(0, ts, (100.0, 100.0, 100.0, 100.0), 10.0, 0.0)?);
    }
    let pass_rep = run_macro_falsification(
        &mut VecCandleReader::new(pass_lin),
        &mut VecCandleReader::new(pass_spt),
        &cfg,
    )?;
    let pass_vip = &pass_rep.report.vip0_cells[0];
    let pass_diag = &pass_rep.report.diag_zero_cells[0];
    assert!(pass_vip.t_nw.ok_or("missing t_nw")? >= 3.50);
    assert!(pass_vip.dsr.ok_or("missing dsr")? >= 0.95);
    assert!((pass_vip.n as u64) >= pass_vip.n_min.ok_or("missing n_min")?);
    assert_eq!(pass_vip.verdict, GateVerdict::Pass);
    assert!(!pass_diag.is_binding);
    assert_ne!(pass_diag.verdict, GateVerdict::Pass);

    // Case 2: Borderline Carry where VIP0-Taker fails (negative mean due to 18.50 bps * turn)
    // while MakerDiag (12.00 bps * turn) has positive mean, t_nw >= 3.50, DSR >= 0.95, N >= n_min,
    // YET diag_zero_cells MUST STILL NEVER return GateVerdict::Pass!
    let border_cfg = MacroConfig {
        carry_strategies: vec![SpotPerpCarryParams {
            name: "Carry_Borderline_Switch",
            window_days: 1,
            entry_apr_pct: 10.0,
            exit_apr_pct: 0.0,
            max_pairs: 1,
            min_diversification: 1,
            min_rebalance_days: 1,
        }],
        ..cfg.clone()
    };
    let mut b_lin = Vec::new();
    let mut b_spt = Vec::new();
    for t in 0..80 {
        let ts = BASE_TS_NS + t * DAY_NS;
        // Decision at even t picks Sym(0) (held t+1 -> t+2, settles funding at even t+2!).
        // Decision at odd t picks Sym(1) (held t+1 -> t+2, settles funding at odd t+2!).
        let (f0, f1) = if t % 2 == 0 {
            let bump = if (t / 2) % 2 == 0 { 31.0 } else { 33.0 };
            (bump, -5.0)
        } else {
            let bump = if ((t - 1) / 2) % 2 == 0 { 31.0 } else { 33.0 };
            (-5.0, bump)
        };
        b_lin.push(make_candle(0, ts, (100.0, 100.0, 100.0, 100.0), 10.0, f0)?);
        b_lin.push(make_candle(1, ts, (100.0, 100.0, 100.0, 100.0), 10.0, f1)?);
        b_spt.push(make_candle(0, ts, (100.0, 100.0, 100.0, 100.0), 10.0, 0.0)?);
        b_spt.push(make_candle(1, ts, (100.0, 100.0, 100.0, 100.0), 10.0, 0.0)?);
    }
    let border_rep = run_macro_falsification(
        &mut VecCandleReader::new(b_lin),
        &mut VecCandleReader::new(b_spt),
        &border_cfg,
    )?;
    let b_vip = &border_rep.report.vip0_cells[0];
    let b_diag = &border_rep.report.diag_zero_cells[0];
    assert!(b_vip.mean_bps.ok_or("vip mean")? < 0.0);
    assert_eq!(b_vip.verdict, GateVerdict::Fail);
    assert!(b_diag.mean_bps.ok_or("diag mean")? > 0.0);
    assert!(b_diag.t_nw.ok_or("diag t_nw")? >= b_diag.t_crit);
    assert!(b_diag.dsr.ok_or("diag dsr")? >= 0.95);
    assert!((b_diag.n as u64) >= b_diag.n_min.ok_or("diag n_min")?);
    assert!(!b_diag.is_binding);
    assert!(matches!(
        b_diag.verdict,
        GateVerdict::Fail | GateVerdict::InsufficientSample
    ));

    Ok(())
}

#[test]
fn test_equity_metrics_and_passive_baselines_closed_form() -> TestResult {
    // 3A. compute_equity_metrics exact closed-form & ruin checks.
    let empty: EquityCurveMetrics = compute_equity_metrics(&[], 365.0)?;
    assert_close(empty.final_equity, 1.0, 1e-15, "empty_final_eq")?;
    assert_eq!(empty.cagr_pct, None);
    assert_eq!(empty.ann_sharpe, None);
    assert_close(empty.max_drawdown_pct, 0.0, 1e-15, "empty_dd")?;
    assert_eq!(empty.calmar, None);

    // 3-day curve: +10%, -20%, +5% => 1.1 * 0.8 * 1.05 = 0.924, max DD = (1.1 - 0.88)/1.1 = 20%.
    let m3 = compute_equity_metrics(&[1000.0, -2000.0, 500.0], 365.0)?;
    let exp_cagr3 = (0.924_f64.powf(365.0 / 3.0) - 1.0) * 100.0;
    assert_close(m3.final_equity, 0.924, 1e-14, "m3_final_eq")?;
    assert_close(m3.max_drawdown_pct, 20.0, 1e-13, "m3_dd")?;
    assert_opt_close(m3.cagr_pct, Some(exp_cagr3), 1e-10, "m3_cagr")?;
    assert_eq!(m3.ann_sharpe, None);
    assert_opt_close(m3.calmar, Some(exp_cagr3 / 20.0), 1e-11, "m3_calmar")?;

    // 6-day curve with non-zero variance so ann_sharpe is defined.
    let rets6 = [200.0, 400.0, -500.0, 100.0, 300.0, -100.0];
    let m6 = compute_equity_metrics(&rets6, 365.0)?;
    let exp_eq6: f64 = 1.02 * 1.04 * 0.95 * 1.01 * 1.03 * 0.99;
    let peak6 = 1.02 * 1.04;
    let trough6 = peak6 * 0.95;
    let exp_dd6 = ((peak6 - trough6) / peak6) * 100.0; // exact 5%
    let exp_cagr6 = (exp_eq6.powf(365.0 / 6.0) - 1.0) * 100.0;
    let mean6 = rets6.iter().sum::<f64>() / 6.0;
    let var6 = rets6.iter().map(|r| (r - mean6) * (r - mean6)).sum::<f64>() / 5.0;
    let exp_sr6 = (mean6 / var6.sqrt()) * 365.0_f64.sqrt();
    assert_close(m6.final_equity, exp_eq6, 1e-14, "m6_eq")?;
    assert_close(m6.max_drawdown_pct, exp_dd6, 1e-12, "m6_dd")?;
    assert_opt_close(m6.cagr_pct, Some(exp_cagr6), 1e-9, "m6_cagr")?;
    assert_opt_close(m6.ann_sharpe, Some(exp_sr6), 1e-12, "m6_sharpe")?;
    assert_opt_close(m6.calmar, Some(exp_cagr6 / exp_dd6), 1e-10, "m6_calmar")?;

    // Full ruin: -10_000 bps clamps equity to 0.0 and cagr_pct to -100.0%.
    let ruin = compute_equity_metrics(&[100.0, -10_000.0, 500.0], 365.0)?;
    assert_eq!(ruin.final_equity.to_bits(), 0.0_f64.to_bits());
    assert_close(ruin.max_drawdown_pct, 100.0, 1e-14, "ruin_dd")?;
    assert_opt_close(ruin.cagr_pct, Some(-100.0), 1e-14, "ruin_cagr")?;
    assert_opt_close(ruin.calmar, Some(-1.0), 1e-14, "ruin_calmar")?;

    // 3B. compute_passive_baselines with 2 Spot pairs where Symbol(1) lists late (day 2).
    // Day 0: Sym(0) Open=100
    // Day 1: Sym(0) Open=110 (+10%)
    // Day 2: Sym(0) Open=132 (+20%), Sym(1) Open=50 (late listing!)
    // Day 3 (EOF): Sym(0) Open=145.2 (+10%), Close=145.2 (0%); Sym(1) Open=60 (+20%), Close=69 (+15%)
    let slices = vec![
        make_slice(
            BASE_TS_NS,
            vec![],
            vec![make_candle(
                0,
                BASE_TS_NS,
                (100.0, 105.0, 99.0, 105.0),
                10.0,
                0.0,
            )?],
        )?,
        make_slice(
            BASE_TS_NS + DAY_NS,
            vec![],
            vec![make_candle(
                0,
                BASE_TS_NS + DAY_NS,
                (110.0, 115.0, 108.0, 115.0),
                10.0,
                0.0,
            )?],
        )?,
        make_slice(
            BASE_TS_NS + 2 * DAY_NS,
            vec![],
            vec![
                make_candle(
                    0,
                    BASE_TS_NS + 2 * DAY_NS,
                    (132.0, 140.0, 130.0, 140.0),
                    10.0,
                    0.0,
                )?,
                make_candle(
                    1,
                    BASE_TS_NS + 2 * DAY_NS,
                    (50.0, 55.0, 49.0, 55.0),
                    10.0,
                    0.0,
                )?,
            ],
        )?,
        make_slice(
            BASE_TS_NS + 3 * DAY_NS,
            vec![],
            vec![
                make_candle(
                    0,
                    BASE_TS_NS + 3 * DAY_NS,
                    (145.2, 145.2, 145.2, 145.2),
                    10.0,
                    0.0,
                )?,
                make_candle(
                    1,
                    BASE_TS_NS + 3 * DAY_NS,
                    (60.0, 69.0, 60.0, 69.0),
                    10.0,
                    0.0,
                )?,
            ],
        )?,
    ];

    let rows: Vec<MacroBaselineRow> =
        compute_passive_baselines(&slices, 11.50, 10.00, 365.0, SymbolId(0))?;
    assert_eq!(rows.len(), 2);

    // Baseline_BuyHold_BTC: holds only Sym(0), turn = [1.0, 0.0, 0.0, 1.0].
    let btc = &rows[0];
    assert_eq!(btc.name, "Baseline_BuyHold_BTC");
    assert_eq!(btc.family, MacroFamily::PassiveBaseline);
    assert_eq!(btc.days_n, 4);
    let btc_gross = [1000.0, 2000.0, 1000.0, 0.0];
    let btc_turn = [1.0, 0.0, 0.0, 1.0];
    let btc_net_vip0: Vec<f64> = btc_gross
        .iter()
        .zip(btc_turn.iter())
        .map(|(g, t)| g - t * 11.50)
        .collect();
    let btc_net_mkr: Vec<f64> = btc_gross
        .iter()
        .zip(btc_turn.iter())
        .map(|(g, t)| g - t * 10.00)
        .collect();
    assert_opt_close(btc.mean_price_bps, Some(1000.0), 1e-11, "btc_mean_gross")?;
    assert_opt_close(btc.avg_daily_turnover, Some(0.5), 1e-14, "btc_avg_turn")?;
    assert_opt_close(
        btc.mean_net_vip0_bps,
        Some(btc_net_vip0.iter().sum::<f64>() / 4.0),
        1e-11,
        "btc_net_vip0",
    )?;
    assert_opt_close(
        btc.mean_net_maker_bps,
        Some(btc_net_mkr.iter().sum::<f64>() / 4.0),
        1e-11,
        "btc_net_mkr",
    )?;
    let exp_btc_eq = compute_equity_metrics(&btc_net_vip0, 365.0)?;
    assert_close(
        btc.vip0_metrics.final_equity,
        exp_btc_eq.final_equity,
        1e-13,
        "btc_eq",
    )?;

    // Baseline_BuyHold_EW:
    // Day 0->1: Sym(0) +1000 bps, turn=1.0
    // Day 1->2: Sym(0) +2000 bps, turn=0.0 (unchanged universe!)
    // Day 2->3: rebalances 1.0 -> 0.50/0.50 => turn = |0.5 - 1.0| + |0.5 - 0.0| = 1.0,
    //           gross = 0.5*1000 + 0.5*2000 = 1500 bps, drifted = (11/23, 12/23)
    // Day 3 EOF: drifted weights (11/23, 12/23) earn (11/23)*0 + (12/23)*1500 = 18000/23 bps,
    //           turn = 0.0 + 1.0 = 1.0!
    let ew = &rows[1];
    assert_eq!(ew.name, "Baseline_BuyHold_EW");
    assert_eq!(ew.days_n, 4);
    let ew_eof_gross = (12.0 / 23.0) * 1500.0;
    let ew_gross = [1000.0, 2000.0, 1500.0, ew_eof_gross];
    let ew_turn = [1.0, 0.0, 1.0, 1.0];
    let ew_net_vip0: Vec<f64> = ew_gross
        .iter()
        .zip(ew_turn.iter())
        .map(|(g, t)| g - t * 11.50)
        .collect();
    assert_opt_close(
        ew.mean_price_bps,
        Some(ew_gross.iter().sum::<f64>() / 4.0),
        1e-10,
        "ew_mean_gross",
    )?;
    assert_opt_close(ew.avg_daily_turnover, Some(0.75), 1e-14, "ew_avg_turn")?;
    assert_opt_close(
        ew.mean_net_vip0_bps,
        Some(ew_net_vip0.iter().sum::<f64>() / 4.0),
        1e-10,
        "ew_net_vip0",
    )?;
    let exp_ew_eq = compute_equity_metrics(&ew_net_vip0, 365.0)?;
    assert_close(
        ew.vip0_metrics.final_equity,
        exp_ew_eq.final_equity,
        1e-13,
        "ew_eq",
    )?;

    // 3C. Additional non-trivial drifted turnover verification with 3 pairs where Sym(2) lists late:
    // Sym(0), Sym(1) start 0.5/0.5, drift on day 0->1 (+10% and +20%) to (11/23, 12/23),
    // hold with 0.0 turnover on unchanged day 1->2, then Sym(2) lists on day 2 so target = (1/3, 1/3, 1/3)!
    let mut prev_drifted = BTreeMap::new();
    prev_drifted.insert(SymbolId(0), 11.0 / 23.0);
    prev_drifted.insert(SymbolId(1), 12.0 / 23.0);
    let mut target3 = BTreeMap::new();
    target3.insert(SymbolId(0), 1.0 / 3.0);
    target3.insert(SymbolId(1), 1.0 / 3.0);
    target3.insert(SymbolId(2), 1.0 / 3.0);
    let expected_turn3: f64 =
        (1.0_f64 / 3.0 - 11.0 / 23.0).abs() + (1.0_f64 / 3.0 - 12.0 / 23.0).abs() + (1.0 / 3.0);
    assert_close(
        weight_turnover(&prev_drifted, &target3),
        expected_turn3,
        1e-15,
        "weight_turnover_drifted",
    )?;

    Ok(())
}

#[test]
fn test_zero_lookahead_bit_for_bit_and_symbol_isolation() -> TestResult {
    // 4A. 160-day multi-symbol dataset (linear + spot). Mutating all bars idx > 135 (10x px, -500 bps funding)
    // must preserve EVERY MacroDayRecord with exit_ts_ns <= slices[135].ts_ns bit-for-bit across all 9 strategies!
    let total_days = 160usize;
    let cutoff = 135usize;
    let mut base_linear = Vec::with_capacity(total_days * 2);
    let mut base_spot = Vec::with_capacity(total_days * 2);

    for t in 0..total_days {
        let ts = BASE_TS_NS + (t as i64) * DAY_NS;
        // Sym(0): piecewise strong moves to trigger Donchian long/short and Trend signals before day 135.
        let p0 = if t < 30 {
            100.0 + (t as f64) * 0.4
        } else if t < 70 {
            112.0 - ((t - 30) as f64) * 0.8
        } else if t < 110 {
            80.0 + ((t - 70) as f64) * 1.5
        } else {
            140.0 + ((t - 110) as f64) * 0.6
        };
        let p1 = if t < 40 {
            200.0 - (t as f64) * 0.5
        } else if t < 90 {
            180.0 + ((t - 40) as f64) * 1.2
        } else if t < 125 {
            240.0 - ((t - 90) as f64) * 1.3
        } else {
            194.5 + ((t - 125) as f64) * 0.9
        };

        let s0 = p0 * 1.001;
        let s1 = p1 * 0.999;

        base_linear.push(make_candle(
            0,
            ts,
            (p0, p0 * 1.01, p0 * 0.99, p0 * 1.002),
            100.0,
            5.0,
        )?);
        base_linear.push(make_candle(
            1,
            ts,
            (p1, p1 * 1.01, p1 * 0.99, p1 * 0.998),
            100.0,
            3.2,
        )?);
        base_spot.push(make_candle(
            0,
            ts,
            (s0, s0 * 1.01, s0 * 0.99, s0 * 1.001),
            100.0,
            0.0,
        )?);
        base_spot.push(make_candle(
            1,
            ts,
            (s1, s1 * 1.01, s1 * 0.99, s1 * 0.999),
            100.0,
            0.0,
        )?);
    }

    let cutoff_ts_ns = BASE_TS_NS + (cutoff as i64) * DAY_NS;
    let mut mut_linear = base_linear.clone();
    let mut mut_spot = base_spot.clone();
    for candle in &mut mut_linear {
        if candle.open_ts_ns.raw() > cutoff_ts_ns {
            *candle = make_candle(
                candle.symbol_id.0,
                candle.open_ts_ns.raw(),
                (
                    candle.open.as_f64() * 10.0,
                    candle.high.as_f64() * 10.0,
                    candle.low.as_f64() * 10.0,
                    candle.close.as_f64() * 10.0,
                ),
                candle.volume * 5.0,
                -500.0,
            )?;
        }
    }
    for candle in &mut mut_spot {
        if candle.open_ts_ns.raw() > cutoff_ts_ns {
            *candle = make_candle(
                candle.symbol_id.0,
                candle.open_ts_ns.raw(),
                (
                    candle.open.as_f64() * 10.0,
                    candle.high.as_f64() * 10.0,
                    candle.low.as_f64() * 10.0,
                    candle.close.as_f64() * 10.0,
                ),
                candle.volume * 5.0,
                -500.0,
            )?;
        }
    }

    let cfg = MacroConfig::default();
    let (base_rep, base_records) = run_macro_falsification_detailed(
        &mut VecCandleReader::new(base_linear),
        &mut VecCandleReader::new(base_spot),
        &cfg,
    )?;
    let (mut_rep, mut_records) = run_macro_falsification_detailed(
        &mut VecCandleReader::new(mut_linear),
        &mut VecCandleReader::new(mut_spot),
        &cfg,
    )?;
    assert_eq!(base_rep.diagnostics.len(), 9);
    assert_eq!(mut_rep.diagnostics.len(), 9);

    let all_names: Vec<&str> = base_rep
        .diagnostics
        .iter()
        .map(|row: &MacroStrategyRow| row.strategy.as_str())
        .collect();
    assert_eq!(all_names.len(), 9);

    for name in all_names {
        let base_pre: Vec<&MacroDayRecord> = base_records
            .iter()
            .filter(|r| r.strategy == name && r.exit_ts_ns <= cutoff_ts_ns)
            .collect();
        let mut_pre: Vec<&MacroDayRecord> = mut_records
            .iter()
            .filter(|r| r.strategy == name && r.exit_ts_ns <= cutoff_ts_ns)
            .collect();
        assert!(!base_pre.is_empty(), "{name} should have records <= cutoff");
        assert_eq!(base_pre.len(), mut_pre.len(), "{name} pre-cutoff count");
        assert!(
            base_pre.iter().any(|r| !r.weights.is_empty()),
            "{name} must have active weights before cutoff"
        );
        for (b, m) in base_pre.iter().zip(mut_pre.iter()) {
            assert_eq!(b.family, m.family);
            assert_eq!(b.decision_ts_ns, m.decision_ts_ns);
            assert_eq!(b.entry_ts_ns, m.entry_ts_ns);
            assert_eq!(b.exit_ts_ns, m.exit_ts_ns);
            assert_eq!(b.is_eof_close, m.is_eof_close);
            assert_eq!(b.weights.len(), m.weights.len());
            for ((sk, sv), (mk, mv)) in b.weights.iter().zip(m.weights.iter()) {
                assert_eq!(sk, mk);
                assert_eq!(sv.to_bits(), mv.to_bits(), "{name} weight bits");
            }
            assert_eq!(
                b.price_or_basis_bps.to_bits(),
                m.price_or_basis_bps.to_bits()
            );
            assert_eq!(b.fund_bps.to_bits(), m.fund_bps.to_bits());
            assert_eq!(b.turnover.to_bits(), m.turnover.to_bits());
            assert_eq!(b.cost_vip0_bps.to_bits(), m.cost_vip0_bps.to_bits());
            assert_eq!(b.cost_maker_bps.to_bits(), m.cost_maker_bps.to_bits());
            assert_eq!(b.net_vip0_bps.to_bits(), m.net_vip0_bps.to_bits());
            assert_eq!(b.net_maker_bps.to_bits(), m.net_maker_bps.to_bits());
        }
    }

    // 4B. SymbolId isolation for MacroTrendFeatureEngine and SpotPerpCarryEngine.
    let mut trend_a = MacroTrendFeatureEngine::new(TrendVolConfig::default())?;
    let mut trend_b = MacroTrendFeatureEngine::new(TrendVolConfig::default())?;
    let carry_params = default_carry_params()[0];
    let mut carry_a = SpotPerpCarryEngine::new(carry_params)?;
    let mut carry_b = SpotPerpCarryEngine::new(carry_params)?;
    let mut carry_c = SpotPerpCarryEngine::new(carry_params)?;
    let sym0 = SymbolId(0);
    let sym99 = SymbolId(99);

    for t in 0..130 {
        let ts = BASE_TS_NS + t * DAY_NS;
        let p0 = 100.0 + ((t % 17) as f64) * 0.5 + (t as f64) * 0.2;
        let p99 = if t % 2 == 0 { 50.0 } else { 5000.0 };
        let c0 = make_candle(0, ts, (p0, p0 + 1.0, p0 - 1.0, p0 + 0.3), 100.0, 4.0)?;
        let c99 = make_candle(99, ts, (p99, p99 * 1.2, p99 * 0.8, p99), 9999.0, 5000.0)?;
        let c99_neg = make_candle(99, ts, (p99, p99 * 1.2, p99 * 0.8, p99), 9999.0, -5000.0)?;
        let s0 = make_candle(0, ts, (p0, p0 + 1.0, p0 - 1.0, p0 + 0.3), 100.0, 0.0)?;
        let s99 = make_candle(99, ts, (p99, p99, p99, p99), 9999.0, 0.0)?;

        let mut map_a = BTreeMap::new();
        map_a.insert(sym0, c0);
        let mut map_b = BTreeMap::new();
        map_b.insert(sym0, c0);
        map_b.insert(sym99, c99);

        let _ = trend_a.on_slice(&map_a)?;
        let _ = trend_b.on_slice(&map_b)?;

        let sa = trend_a.snapshot(sym0).ok_or("snap a")?;
        let sb = trend_b.snapshot(sym0).ok_or("snap b")?;
        assert_eq!(sa.close.to_bits(), sb.close.to_bits());
        assert_eq!(sa.gk_vol_daily.to_bits(), sb.gk_vol_daily.to_bits());
        assert_eq!(
            sa.sigma_ann.map(f64::to_bits),
            sb.sigma_ann.map(f64::to_bits)
        );
        assert_eq!(
            sa.trend_signal_ls.map(f64::to_bits),
            sb.trend_signal_ls.map(f64::to_bits)
        );
        assert_eq!(
            prior_donchian_channel(&trend_a, sym0, 20, 10),
            prior_donchian_channel(&trend_b, sym0, 20, 10)
        );

        let mut spot_a = BTreeMap::new();
        spot_a.insert(sym0, s0);
        let mut spot_b = BTreeMap::new();
        spot_b.insert(sym0, s0);
        spot_b.insert(sym99, s99);
        let mut map_c = BTreeMap::new();
        map_c.insert(sym0, c0);
        map_c.insert(sym99, c99_neg);

        let wa = carry_a.on_slice(&map_a, &spot_a)?;
        let _wb = carry_b.on_slice(&map_b, &spot_b)?;
        let wc = carry_c.on_slice(&map_c, &spot_b)?;

        assert_eq!(
            carry_a.current_apr_pct(sym0).map(f64::to_bits),
            carry_b.current_apr_pct(sym0).map(f64::to_bits)
        );
        assert_eq!(
            carry_a.current_apr_pct(sym0).map(f64::to_bits),
            carry_c.current_apr_pct(sym0).map(f64::to_bits)
        );
        assert_eq!(wa, wc);
    }

    Ok(())
}

#[test]
fn test_edge_cases_late_spot_flat_candles_and_invalid_inputs() -> TestResult {
    // 5A. Late-listed Spot symbol:
    // Sym(0) has linear + spot from day 0. Sym(1) has linear from day 0 and spot ONLY from day 130.
    // Trend/Donchian use Sym(1) from linear once warmed up, while Carry ignores Sym(1) until day 130.
    let mut late_lin = Vec::new();
    let mut late_spt = Vec::new();
    let sym0 = SymbolId(0);
    let sym1 = SymbolId(1);
    for t in 0..135 {
        let ts = BASE_TS_NS + t * DAY_NS;
        let p0 = 100.0 + (t as f64) * 2.0;
        let p1 = 200.0 + (t as f64) * 2.0;
        late_lin.push(make_candle(0, ts, (p0, p0 + 1.0, p0 - 1.0, p0), 10.0, 5.0)?);
        late_lin.push(make_candle(1, ts, (p1, p1 + 1.0, p1 - 1.0, p1), 10.0, 5.0)?);
        late_spt.push(make_candle(0, ts, (p0, p0 + 1.0, p0 - 1.0, p0), 10.0, 0.0)?);
        if t >= 130 {
            late_spt.push(make_candle(1, ts, (p1, p1 + 1.0, p1 - 1.0, p1), 10.0, 0.0)?);
        }
    }
    let (_, late_records) = run_macro_falsification_detailed(
        &mut VecCandleReader::new(late_lin),
        &mut VecCandleReader::new(late_spt),
        &MacroConfig::default(),
    )?;
    let ts_130 = BASE_TS_NS + 130 * DAY_NS;
    let ts_131 = BASE_TS_NS + 131 * DAY_NS;
    assert!(late_records.iter().any(|r| {
        r.family == MacroFamily::DonchianBreakout
            && r.decision_ts_ns < ts_130
            && r.weights.contains_key(&sym1)
    }));
    assert!(late_records.iter().any(|r| {
        r.family == MacroFamily::VolTargetTrend
            && r.decision_ts_ns < ts_130
            && r.weights.contains_key(&sym1)
    }));
    assert!(late_records
        .iter()
        .filter(|r| r.family == MacroFamily::SpotPerpCarry && r.decision_ts_ns < ts_130)
        .all(|r| !r.weights.contains_key(&sym1) && r.weights.contains_key(&sym0)));
    assert!(late_records.iter().any(|r| {
        r.family == MacroFamily::SpotPerpCarry
            && r.entry_ts_ns == ts_131
            && r.weights.contains_key(&sym1)
    }));

    // 5B. Flat candles (O == H == L == C) -> GK=0, sigma_ann clamped to MIN_SIGMA_ANN (0.15), no NaN.
    let mut flat_engine = MacroTrendFeatureEngine::new(TrendVolConfig::default())?;
    let mut flat_don = DonchianBreakoutStrategy::new(default_donchian_params()[0])?;
    let mut flat_lin = Vec::with_capacity(130);
    let mut flat_spt = Vec::with_capacity(130);
    for t in 0..130 {
        let ts = BASE_TS_NS + t * DAY_NS;
        let c = make_candle(0, ts, (100.0, 100.0, 100.0, 100.0), 0.0, 0.0)?;
        flat_lin.push(c);
        flat_spt.push(c);
        let mut m = BTreeMap::new();
        m.insert(sym0, c);
        let active = flat_engine.on_slice(&m)?;
        let snap = flat_engine.snapshot(sym0).ok_or("flat snap")?;
        assert_eq!(snap.gk_vol_daily.to_bits(), 0.0_f64.to_bits());
        if t >= 20 {
            assert_opt_close(snap.sigma_ann, Some(MIN_SIGMA_ANN), 1e-15, "flat_sigma")?;
            let w_don = flat_don
                .evaluate(&active, &flat_engine)?
                .ok_or("flat don")?;
            assert!(w_don.is_empty());
        }
        if t >= 120 {
            assert_opt_close(snap.trend_signal_ls, Some(0.0), 1e-15, "flat_trend_sig")?;
        }
    }
    let (flat_rep, flat_records) = run_macro_falsification_detailed(
        &mut VecCandleReader::new(flat_lin),
        &mut VecCandleReader::new(flat_spt),
        &MacroConfig::default(),
    )?;
    assert_eq!(flat_rep.slices_processed, 130);
    assert!(flat_records.iter().all(|r| {
        r.price_or_basis_bps == 0.0
            && r.fund_bps == 0.0
            && r.turnover == 0.0
            && r.net_vip0_bps.is_finite()
    }));

    // 5C. Invalid configs & non-finite parameters.
    assert!(matches!(
        CausalEwmaVol::new(TrendVolConfig {
            ewma_span: 1,
            ..TrendVolConfig::default()
        }),
        Err(DomainError::InvalidStatParam("trend_vol_config"))
    ));
    assert!(matches!(
        CausalEwmaVol::new(TrendVolConfig {
            min_sigma_ann: f64::NAN,
            ..TrendVolConfig::default()
        }),
        Err(DomainError::InvalidStatParam("trend_vol_config"))
    ));
    assert!(matches!(
        CausalEwmaVol::new(TrendVolConfig {
            min_sigma_ann: 1.0,
            max_sigma_ann: 0.5,
            ..TrendVolConfig::default()
        }),
        Err(DomainError::InvalidStatParam("trend_vol_config"))
    ));
    assert!(matches!(
        VolTargetTrendStrategy::new(MacroTrendParams {
            rebalance_band: -0.01,
            ..default_trend_params()[0]
        }),
        Err(DomainError::InvalidStatParam("macro_trend_params"))
    ));
    assert!(matches!(
        DonchianBreakoutStrategy::new(DonchianParams {
            entry_n: 10,
            exit_n: 20,
            ..default_donchian_params()[0]
        }),
        Err(DomainError::InvalidStatParam("donchian_params"))
    ));
    assert!(matches!(
        SpotPerpCarryEngine::new(SpotPerpCarryParams {
            entry_apr_pct: 5.0,
            exit_apr_pct: 5.0,
            ..default_carry_params()[0]
        }),
        Err(DomainError::InvalidStatParam("carry_hysteresis"))
    ));
    assert!(matches!(
        SpotPerpCarryEngine::new(SpotPerpCarryParams {
            entry_apr_pct: 2.0,
            exit_apr_pct: 10.0,
            ..default_carry_params()[0]
        }),
        Err(DomainError::InvalidStatParam("carry_hysteresis"))
    ));
    assert!(matches!(Bps::new(f64::NAN), Err(DomainError::NonFiniteBps)));
    assert!(matches!(
        MacroFeeProfiles::new(Bps::new(-0.5)?),
        Err(DomainError::InvalidFeeBps)
    ));
    assert!(matches!(
        compute_equity_metrics(&[f64::NAN], 365.0),
        Err(DomainError::NonFiniteValue("equity_return_bps"))
    ));
    assert!(matches!(
        compute_passive_baselines(&[], -1.0, 10.0, 365.0, sym0),
        Err(DomainError::InvalidFeeBps)
    ));
    let mut bad_cfg = MacroConfig {
        slippage_bps: Bps::new(-1.0)?,
        ..MacroConfig::default()
    };
    assert!(matches!(
        run_macro_falsification(
            &mut VecCandleReader::new(vec![]),
            &mut VecCandleReader::new(vec![]),
            &bad_cfg,
        ),
        Err(DomainError::InvalidFeeBps)
    ));
    bad_cfg.slippage_bps = Bps::ZERO;
    bad_cfg.trend_strategies.clear();
    bad_cfg.donchian_strategies.clear();
    bad_cfg.carry_strategies.clear();
    assert!(matches!(
        bad_cfg.validate(),
        Err(DomainError::InvalidStatParam("macro_config"))
    ));

    // 5D. Duplicate (ts, sym) rejected with DomainError::NonMonotonicTimestamp across engines & falsifier.
    let c0 = make_candle(0, BASE_TS_NS, (100.0, 101.0, 99.0, 100.0), 10.0, 1.0)?;
    let mut dup_trend = MacroTrendFeatureEngine::new(TrendVolConfig::default())?;
    let mut one = BTreeMap::new();
    one.insert(sym0, c0);
    let _ = dup_trend.on_slice(&one)?;
    assert!(matches!(
        dup_trend.on_slice(&one),
        Err(DomainError::NonMonotonicTimestamp {
            symbol_id: 0,
            prev_ns: BASE_TS_NS,
            curr_ns: BASE_TS_NS,
        })
    ));

    let mut dup_carry = SpotPerpCarryEngine::new(default_carry_params()[0])?;
    let _ = dup_carry.on_slice(&one, &one)?;
    assert!(matches!(
        dup_carry.on_slice(&one, &one),
        Err(DomainError::NonMonotonicTimestamp {
            symbol_id: 0,
            prev_ns: BASE_TS_NS,
            curr_ns: BASE_TS_NS,
        })
    ));

    assert!(matches!(
        run_macro_falsification_detailed(
            &mut VecCandleReader::new(vec![c0, c0]),
            &mut VecCandleReader::new(vec![c0]),
            &MacroConfig::default(),
        ),
        Err(DomainError::NonMonotonicTimestamp {
            symbol_id: 0,
            prev_ns: BASE_TS_NS,
            curr_ns: BASE_TS_NS,
        })
    ));
    assert!(matches!(
        run_macro_falsification_detailed(
            &mut VecCandleReader::new(vec![c0]),
            &mut VecCandleReader::new(vec![c0, c0]),
            &MacroConfig::default(),
        ),
        Err(DomainError::NonMonotonicTimestamp {
            symbol_id: 0,
            prev_ns: BASE_TS_NS,
            curr_ns: BASE_TS_NS,
        })
    ));

    Ok(())
}

#[test]
fn test_cli_scan_macro_end_to_end_and_json_roundtrip() -> TestResult {
    let tmp = TempDirGuard::new("cli_scan_macro")?;
    let linear_dir = tmp.path.join("linear");
    let spot_dir = tmp.path.join("spot");
    let out_json = tmp.path.join("macro_report.json");
    fs::create_dir_all(&linear_dir)?;
    fs::create_dir_all(&spot_dir)?;

    let days = 150usize;
    let mut lin0 = Vec::with_capacity(days);
    let mut lin1 = Vec::with_capacity(days);
    let mut spt0 = Vec::with_capacity(days);
    let mut spt1 = Vec::with_capacity(days);
    for t in 0..days {
        let ts = BASE_TS_NS + (t as i64) * DAY_NS;
        let p0 = 100.0 + (t as f64) * 0.7 + ((t % 9) as f64) * 0.25;
        let p1 = 200.0 - (t as f64) * 0.4 + ((t % 11) as f64) * 0.5;
        let s0 = p0 * 1.0005;
        let s1 = p1 * 0.9995;
        lin0.push(make_candle(
            0,
            ts,
            (p0, p0 * 1.01, p0 * 0.99, p0 * 1.003),
            100.0,
            4.5,
        )?);
        lin1.push(make_candle(
            1,
            ts,
            (p1, p1 * 1.01, p1 * 0.99, p1 * 0.997),
            100.0,
            3.2,
        )?);
        spt0.push(make_candle(
            0,
            ts,
            (s0, s0 * 1.01, s0 * 0.99, s0 * 1.002),
            100.0,
            0.0,
        )?);
        spt1.push(make_candle(
            1,
            ts,
            (s1, s1 * 1.01, s1 * 0.99, s1 * 0.998),
            100.0,
            0.0,
        )?);
    }

    write_csv_lf(&linear_dir.join("sym0.csv"), &lin0)?;
    write_csv_lf(&linear_dir.join("sym1.csv"), &lin1)?;
    write_csv_lf(&spot_dir.join("sym0.csv"), &spt0)?;
    write_csv_lf(&spot_dir.join("sym1.csv"), &spt1)?;

    let lin_arg = linear_dir.to_str().ok_or("linear path utf8")?;
    let spt_arg = spot_dir.to_str().ok_or("spot path utf8")?;
    let out_arg = out_json.to_str().ok_or("out path utf8")?;

    let ascii = run_cli([
        "vortex-alfa",
        "scan-macro",
        "--linear-dir",
        lin_arg,
        "--spot-dir",
        spt_arg,
        "--slippage-bps",
        "1.5",
        "--out",
        out_arg,
    ])?;

    assert!(ascii.contains("Track5-Macro1D"));
    assert!(ascii.contains("M=9"));
    for strat_name in [
        "MacroTrend_LS_Vol15",
        "MacroTrend_LS_Vol30",
        "MacroTrend_LO_Vol20",
        "Donchian_20_10_LS",
        "Donchian_55_20_LS",
        "Donchian_55_20_LO",
        "Carry_SpotPerp_10Apr",
        "Carry_SpotPerp_15Apr",
        "Carry_SpotPerp_TopCarry30d",
        "Baseline_BuyHold_BTC",
        "Baseline_BuyHold_EW",
    ] {
        assert!(ascii.contains(strat_name), "ASCII missing {strat_name}");
    }

    let json_text = fs::read_to_string(&out_json)?;
    let parsed: MacroFalsificationReport = serde_json::from_str(&json_text)?;
    assert_close(parsed.slippage_bps, 1.5, 1e-15, "cli_slippage_bps")?;
    assert_close(parsed.days_per_year, 365.0, 1e-15, "cli_days_per_year")?;
    assert_eq!(parsed.slices_processed, 150);
    assert_eq!(parsed.linear_candles_processed, 300);
    assert_eq!(parsed.spot_candles_processed, 300);
    assert_eq!(parsed.report.track, "Track5-Macro1D");
    assert_eq!(parsed.report.events_processed, 600);
    assert_eq!(parsed.report.m_hypotheses, 9);
    assert_close(parsed.report.t_crit, 3.5, 1e-15, "cli_t_crit")?;
    assert_eq!(parsed.diagnostics.len(), 9);
    assert_eq!(parsed.report.vip0_cells.len(), 9);
    assert_eq!(parsed.report.diag_zero_cells.len(), 9);
    assert_eq!(parsed.baselines.len(), 2);
    assert_eq!(parsed.baselines[0].name, "Baseline_BuyHold_BTC");
    assert_eq!(parsed.baselines[1].name, "Baseline_BuyHold_EW");
    assert!(parsed
        .report
        .diag_zero_cells
        .iter()
        .all(|c| !c.is_binding && c.verdict != GateVerdict::Pass));

    // Verify CLI validation rejects negative slippage or missing dirs.
    assert!(matches!(
        run_cli([
            "vortex-alfa",
            "scan-macro",
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
        run_cli(["vortex-alfa", "scan-macro", "--linear-dir", lin_arg]),
        Err(DomainError::Cli(_))
    ));
    assert!(matches!(
        run_cli(["vortex-alfa", "scan-macro", "--bogus"]),
        Err(DomainError::Cli(_))
    ));

    Ok(())
}
