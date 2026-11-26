use std::collections::BTreeMap;
use std::error::Error;
use std::fs::{self, File};
use std::io::Write;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use vortex_alfa::application::falsifier_xs::{
    run_xs_falsification_detailed, XsFalsificationReport, XsFalsifierConfig, XsPeriodRecord,
};
use vortex_alfa::domain::error::DomainError;
use vortex_alfa::domain::ports::CandleSeriesReader;
use vortex_alfa::domain::signals::cross_sectional::{
    build_dollar_neutral_weights, CrossSectionSlice, XsHypothesis, XsSignalConfig, XsSignalEngine,
    XsSliceScores,
};
use vortex_alfa::domain::stats::GateVerdict;
use vortex_alfa::domain::types::{
    Bps, Candle, FundingTimestampSemantics, PriceE8, SymbolId, TimestampNs, SCALE_E8,
};
use vortex_alfa::interfaces::cli::run_cli;

type TestResult = Result<(), Box<dyn Error>>;

const BAR_4H_NS: i64 = 14_400_000_000_000;
const BAR_4H_MS: i64 = 14_400_000;
const BPS_UNIT: f64 = 10_000.0;

static TEMP_SEQ: AtomicU64 = AtomicU64::new(1);

struct TempDirGuard {
    path: PathBuf,
}

impl TempDirGuard {
    fn new(prefix: &str) -> Result<Self, Box<dyn Error>> {
        let id = TEMP_SEQ.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "vortex_alfa_xs_{prefix}_{}_{}",
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

fn make_slice(ts_ns: i64, candles: Vec<Candle>) -> Result<CrossSectionSlice, DomainError> {
    let mut map = BTreeMap::new();
    for candle in candles {
        map.insert(candle.symbol_id, candle);
    }
    Ok(CrossSectionSlice {
        ts_ns: TimestampNs::new(ts_ns)?,
        candles: map,
    })
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

fn scores_to_map(scores: &XsSliceScores, hyp: XsHypothesis) -> BTreeMap<SymbolId, f64> {
    scores.scores_for(hyp).iter().copied().collect()
}

fn independent_gk_vol(ohlc: (f64, f64, f64, f64)) -> f64 {
    let (o, h, l, c) = ohlc;
    if o.to_bits() == h.to_bits() && h.to_bits() == l.to_bits() && l.to_bits() == c.to_bits() {
        return 0.0;
    }
    let log_hl = (h / l).ln();
    let log_co = (c / o).ln();
    let var =
        (0.5 * log_hl * log_hl - (2.0 * std::f64::consts::LN_2 - 1.0) * log_co * log_co).max(0.0);
    var.sqrt()
}

fn build_synthetic_xs_panel(num_bars: usize) -> Result<Vec<CrossSectionSlice>, DomainError> {
    let mut closes = [100.0_f64; 9];
    closes[0] = 30_000.0;
    let mut slices = Vec::with_capacity(num_bars);
    let base_ts = 1_700_000_000_000_000_000_i64;
    for t in 0..num_bars {
        let ts = base_ts + (t as i64) * BAR_4H_NS;
        let tf = t as f64;
        let r_btc = 0.008 * (0.27 * tf).sin() + 0.004 * (0.11 * tf).cos() + 0.0005;
        let mut candles = BTreeMap::new();
        for sym in 0_u16..9 {
            let sf = f64::from(sym);
            let r_i = if sym == 0 {
                r_btc
            } else {
                let beta = 0.6 + 0.25 * sf;
                let idio = 0.004 * (0.37 * tf + 0.9 * sf).sin()
                    + 0.002 * (0.19 * tf - 0.5 * sf).cos()
                    + 0.0002 * (sf - 4.5);
                beta * r_btc + idio
            };
            let o = closes[usize::from(sym)];
            let c = o * r_i.exp();
            let pad = (0.004 + 0.0008 * sf) * o.max(c);
            let h = o.max(c) + pad;
            let l = (o.min(c) - pad).max(0.01);
            let fund = if sym == 0 {
                1.0
            } else {
                (sf - 4.5) * 1.4 + 0.8 * (0.21 * tf + sf).sin()
            };
            let vol = 1_000.0 + 150.0 * sf + 50.0 * (0.13 * tf).cos();
            let candle = make_candle(sym, ts, (o, h, l, c), vol, fund)?;
            closes[usize::from(sym)] = candle.close.as_f64();
            candles.insert(SymbolId(sym), candle);
        }
        slices.push(CrossSectionSlice {
            ts_ns: TimestampNs::new(ts)?,
            candles,
        });
    }
    Ok(slices)
}

#[test]
fn test_xs_funding_carry_closed_form_and_ranking() -> TestResult {
    assert!(XsSignalConfig::default().exclude_benchmark_from_trading);
    let cfg = XsSignalConfig {
        benchmark_symbol: SymbolId(0),
        exclude_benchmark_from_trading: true,
        funding_window_bars: 3,
        ..XsSignalConfig::default()
    };
    let mut engine = XsSignalEngine::new(cfg.clone())?;

    // Funding rates per bar for symbols 0..=4 across 3 slices:
    // Sym0 (BTC): [10.0, 20.0, 30.0] => sum = +60.0 => score = -60.0 (excluded by default)
    // Sym1:       [-3.0, -2.0, -1.0] => sum =  -6.0 => score =  +6.0
    // Sym2:       [-1.0,  0.0, -0.5] => sum =  -1.5 => score =  +1.5
    // Sym3:       [ 1.0,  2.0,  1.5] => sum =  +4.5 => score =  -4.5
    // Sym4:       [ 4.0,  3.0,  5.0] => sum = +12.0 => score = -12.0
    let fundings_by_bar: [[f64; 5]; 3] = [
        [10.0, -3.0, -1.0, 1.0, 4.0],
        [20.0, -2.0, 0.0, 2.0, 3.0],
        [30.0, -1.0, -0.5, 1.5, 5.0],
    ];

    let mut slices = Vec::with_capacity(3);
    for (bar_idx, bar_funds) in fundings_by_bar.iter().enumerate() {
        let ts = 1_000_000_000_000_i64 + (bar_idx as i64) * BAR_4H_NS;
        let mut candles = Vec::with_capacity(5);
        for (sym, &fund) in (0_u16..5).zip(bar_funds.iter()) {
            candles.push(make_candle(
                sym,
                ts,
                (100.0, 102.0, 98.0, 101.0),
                500.0,
                fund,
            )?);
        }
        slices.push(make_slice(ts, candles)?);
    }

    let s0 = engine.on_slice(&slices[0])?;
    assert!(s0.has_benchmark);
    assert!(s0.scores_for(XsHypothesis::XsFundingCarry).is_empty());

    let s1 = engine.on_slice(&slices[1])?;
    assert!(s1.has_benchmark);
    assert!(s1.scores_for(XsHypothesis::XsFundingCarry).is_empty());

    let s2 = engine.on_slice(&slices[2])?;
    assert!(s2.has_benchmark);
    let carry_scores = s2.scores_for(XsHypothesis::XsFundingCarry);
    assert_eq!(carry_scores.len(), 4);

    let score_map = scores_to_map(&s2, XsHypothesis::XsFundingCarry);
    assert!(!score_map.contains_key(&SymbolId(0)));
    assert_close(score_map[&SymbolId(1)], 6.0, 1e-12, "carry sym1")?;
    assert_close(score_map[&SymbolId(2)], 1.5, 1e-12, "carry sym2")?;
    assert_close(score_map[&SymbolId(3)], -4.5, 1e-12, "carry sym3")?;
    assert_close(score_map[&SymbolId(4)], -12.0, 1e-12, "carry sym4")?;

    let w_k1 = build_dollar_neutral_weights(carry_scores, 1)?.ok_or("expected weights K=1")?;
    assert_eq!(w_k1.len(), 2);
    assert_close(w_k1[&SymbolId(1)], 0.5, 1e-15, "w_k1 sym1")?;
    assert_close(w_k1[&SymbolId(4)], -0.5, 1e-15, "w_k1 sym4")?;

    let w_k2 = build_dollar_neutral_weights(carry_scores, 2)?.ok_or("expected weights K=2")?;
    assert_eq!(w_k2.len(), 4);
    assert_close(w_k2[&SymbolId(1)], 0.25, 1e-15, "w_k2 sym1")?;
    assert_close(w_k2[&SymbolId(2)], 0.25, 1e-15, "w_k2 sym2")?;
    assert_close(w_k2[&SymbolId(3)], -0.25, 1e-15, "w_k2 sym3")?;
    assert_close(w_k2[&SymbolId(4)], -0.25, 1e-15, "w_k2 sym4")?;

    // Toggle exclude_benchmark_from_trading = false => SymbolId(0) must be included.
    let cfg_incl = XsSignalConfig {
        exclude_benchmark_from_trading: false,
        ..cfg
    };
    let mut engine_incl = XsSignalEngine::new(cfg_incl)?;
    for slice in &slices[..2] {
        let warmup = engine_incl.on_slice(slice)?;
        assert!(warmup.scores_for(XsHypothesis::XsFundingCarry).is_empty());
    }
    let s2_incl = engine_incl.on_slice(&slices[2])?;
    let carry_incl = s2_incl.scores_for(XsHypothesis::XsFundingCarry);
    assert_eq!(carry_incl.len(), 5);
    let incl_map = scores_to_map(&s2_incl, XsHypothesis::XsFundingCarry);
    assert_close(incl_map[&SymbolId(0)], -60.0, 1e-12, "carry sym0 incl")?;
    assert_close(incl_map[&SymbolId(1)], 6.0, 1e-12, "carry sym1 incl")?;

    let w_incl_k1 =
        build_dollar_neutral_weights(carry_incl, 1)?.ok_or("expected weights incl K=1")?;
    assert_eq!(w_incl_k1.len(), 2);
    assert_close(w_incl_k1[&SymbolId(1)], 0.5, 1e-15, "w_incl sym1")?;
    assert_close(w_incl_k1[&SymbolId(0)], -0.5, 1e-15, "w_incl sym0")?;
    Ok(())
}

#[test]
fn test_xs_residual_reversion_ols_beta_and_clamp_analytical() -> TestResult {
    let cfg = XsSignalConfig {
        benchmark_symbol: SymbolId(0),
        exclude_benchmark_from_trading: true,
        beta_window_bars: 5,
        resid_window_bars: 2,
        beta_min: -3.0,
        beta_max: 5.0,
        ..XsSignalConfig::default()
    };
    let mut engine = XsSignalEngine::new(cfg)?;

    // 5 return bars (across 6 slices 0..=5):
    // r_btc = [-0.03, -0.01, 0.00, 0.01, 0.03], mean = 0, b4 + b5 = 0.04 != 0
    // Alt1: r_1 = 2.0 * r_btc + [0, 0, 0, +0.003, -0.001] -> Cov/Var == 2.0 exactly,
    //       because 0.003*0.01 + (-0.001)*0.03 == 0; e_1 = +0.002 => -e_1 = -0.002.
    // Alt2: r_2 = 8.0 * r_btc -> raw beta = 8.0, clamped to 5.0 => e_2 = 3.0 * 0.04 = 0.12 => -0.12.
    // Alt3: r_3 = -5.0 * r_btc -> raw beta = -5.0, clamped to -3.0 => e_3 = -2.0 * 0.04 = -0.08 => +0.08.
    let r_btc = [-0.03, -0.01, 0.00, 0.01, 0.03];
    let shocks_alt1 = [0.0, 0.0, 0.0, 0.003, -0.001];

    let mut close_prev = [10_000.0_f64; 4];
    let mut actual_closes: [Vec<f64>; 4] = Default::default();
    let mut slices = Vec::with_capacity(6);

    for slice_idx in 0..6 {
        let ts = 1_000_000_000_000_i64 + (slice_idx as i64) * BAR_4H_NS;
        let mut candles = Vec::with_capacity(4);
        for sym in 0_u16..4 {
            let c_val = if slice_idx == 0 {
                close_prev[usize::from(sym)]
            } else {
                let j = slice_idx - 1;
                let rb = r_btc[j];
                let r: f64 = match sym {
                    0 => rb,
                    1 => 2.0 * rb + shocks_alt1[j],
                    2 => 8.0 * rb,
                    _ => -5.0 * rb,
                };
                close_prev[usize::from(sym)] * r.exp()
            };
            let o_val = close_prev[usize::from(sym)];
            let h_val = o_val.max(c_val) * 1.01;
            let l_val = o_val.min(c_val) * 0.99;
            let candle = make_candle(sym, ts, (o_val, h_val, l_val, c_val), 1_000.0, 0.0)?;
            let quantized_close = candle.close.as_f64();
            close_prev[usize::from(sym)] = quantized_close;
            actual_closes[usize::from(sym)].push(quantized_close);
            candles.push(candle);
        }
        slices.push(make_slice(ts, candles)?);
    }

    for slice in &slices[..5] {
        let out = engine.on_slice(slice)?;
        assert!(out.scores_for(XsHypothesis::XsResidualReversion).is_empty());
    }
    let out5 = engine.on_slice(&slices[5])?;
    let rev_map = scores_to_map(&out5, XsHypothesis::XsResidualReversion);
    assert_eq!(rev_map.len(), 3);

    // Independent analytical OLS Cov/Var, clamp, and 2-bar residual accumulation from actual closes.
    let btc_rets: Vec<f64> = actual_closes[0]
        .windows(2)
        .map(|w| (w[1] / w[0]).ln())
        .collect();
    let mean_b = btc_rets.iter().sum::<f64>() / 5.0;
    let var_b = btc_rets
        .iter()
        .map(|&rb| (rb - mean_b).powi(2))
        .sum::<f64>()
        / 4.0;

    let expected_raw_betas = [2.0_f64, 8.0, -5.0];
    let expected_clamped = [2.0_f64, 5.0, -3.0];
    let expected_scores = [-0.002_f64, -0.12, 0.08];

    for sym in 1_u16..=3 {
        let sym_idx = usize::from(sym);
        let alt_rets: Vec<f64> = actual_closes[sym_idx]
            .windows(2)
            .map(|w| (w[1] / w[0]).ln())
            .collect();
        let mean_i = alt_rets.iter().sum::<f64>() / 5.0;
        let cov_ib = alt_rets
            .iter()
            .zip(btc_rets.iter())
            .map(|(&ri, &rb)| (ri - mean_i) * (rb - mean_b))
            .sum::<f64>()
            / 4.0;
        let raw_beta = cov_ib / var_b;
        assert_close(
            raw_beta,
            expected_raw_betas[sym_idx - 1],
            1e-9,
            &format!("raw_beta sym{sym}"),
        )?;

        let beta_hat = raw_beta.clamp(-3.0, 5.0);
        assert_close(
            beta_hat,
            expected_clamped[sym_idx - 1],
            1e-9,
            &format!("clamped_beta sym{sym}"),
        )?;

        let e_i: f64 = alt_rets[3..]
            .iter()
            .zip(btc_rets[3..].iter())
            .map(|(&ri, &rb)| ri - beta_hat * rb)
            .sum();
        let indep_score = -e_i;
        assert_close(
            indep_score,
            expected_scores[sym_idx - 1],
            1e-9,
            &format!("theoretical score sym{sym}"),
        )?;
        assert_close(
            rev_map[&SymbolId(sym)],
            indep_score,
            1e-11,
            &format!("engine vs independent resid sym{sym}"),
        )?;
    }
    Ok(())
}

#[test]
fn test_xs_momentum_vol_adj_skip_and_gk_analytical() -> TestResult {
    let cfg = XsSignalConfig {
        benchmark_symbol: SymbolId(0),
        exclude_benchmark_from_trading: true,
        mom_lookback_bars: 6,
        mom_skip_bars: 2,
        vol_window_bars: 4,
        ..XsSignalConfig::default()
    };
    let mut engine_base = XsSignalEngine::new(cfg.clone())?;
    let mut engine_spike = XsSignalEngine::new(cfg)?;

    // 7 slices (indices 0..=6):
    // Bars 0..=4 are identical across baseline and spiked variants:
    // Sym1: C = [100, 102, 105, 108, 120] -> ln(C(4)/C(0)) = ln(1.2)
    // Sym2: C = [100, 101, 102, 103, 125] -> ln(C(4)/C(0)) = ln(1.25)
    // On bars 5..=6, engine_spike gets huge close spikes (e.g., Sym1 plunges 60 -> 300, Sym2 -> 30).
    let shared_ohlc_sym1 = [
        (100.0, 102.0, 99.0, 100.0),
        (100.0, 104.0, 99.5, 102.0),
        (102.0, 106.0, 101.0, 105.0),
        (105.0, 112.0, 102.0, 108.0),
        (108.0, 124.0, 106.0, 120.0),
    ];
    let shared_ohlc_sym2 = [
        (100.0, 102.0, 98.0, 100.0),
        (100.0, 103.0, 99.0, 101.0),
        (101.0, 104.0, 100.0, 102.0),
        (102.0, 106.0, 101.0, 103.0),
        (103.0, 127.0, 102.0, 125.0),
    ];

    let tail_base_sym1 = [(120.0, 125.0, 118.0, 121.0), (121.0, 126.0, 119.0, 122.0)];
    let tail_base_sym2 = [(125.0, 129.0, 123.0, 126.0), (126.0, 130.0, 124.0, 128.0)];

    // Huge close spikes on bar 5 (t-1) and bar 6 (t):
    let tail_spike_sym1 = [(120.0, 122.0, 58.0, 60.0), (60.0, 310.0, 59.0, 300.0)];
    let tail_spike_sym2 = [(125.0, 127.0, 55.0, 58.0), (58.0, 62.0, 28.0, 30.0)];

    let mut base_slices = Vec::with_capacity(7);
    let mut spike_slices = Vec::with_capacity(7);

    for idx in 0..7 {
        let ts = 1_000_000_000_000_i64 + (idx as i64) * BAR_4H_NS;
        let btc_c = make_candle(0, ts, (1000.0, 1015.0, 985.0, 1005.0), 100.0, 0.0)?;
        let (ohlc1_b, ohlc2_b, ohlc1_s, ohlc2_s) = if idx < 5 {
            (
                shared_ohlc_sym1[idx],
                shared_ohlc_sym2[idx],
                shared_ohlc_sym1[idx],
                shared_ohlc_sym2[idx],
            )
        } else {
            (
                tail_base_sym1[idx - 5],
                tail_base_sym2[idx - 5],
                tail_spike_sym1[idx - 5],
                tail_spike_sym2[idx - 5],
            )
        };
        base_slices.push(make_slice(
            ts,
            vec![
                btc_c,
                make_candle(1, ts, ohlc1_b, 50.0, 0.0)?,
                make_candle(2, ts, ohlc2_b, 50.0, 0.0)?,
            ],
        )?);
        spike_slices.push(make_slice(
            ts,
            vec![
                btc_c,
                make_candle(1, ts, ohlc1_s, 50.0, 0.0)?,
                make_candle(2, ts, ohlc2_s, 50.0, 0.0)?,
            ],
        )?);
    }

    for (b_slice, s_slice) in base_slices[..6].iter().zip(&spike_slices[..6]) {
        let ob = engine_base.on_slice(b_slice)?;
        let os = engine_spike.on_slice(s_slice)?;
        assert!(ob.scores_for(XsHypothesis::XsMomentumVolAdj).is_empty());
        assert!(os.scores_for(XsHypothesis::XsMomentumVolAdj).is_empty());
    }

    let out_base = engine_base.on_slice(&base_slices[6])?;
    let out_spike = engine_spike.on_slice(&spike_slices[6])?;
    let map_base = scores_to_map(&out_base, XsHypothesis::XsMomentumVolAdj);
    let map_spike = scores_to_map(&out_spike, XsHypothesis::XsMomentumVolAdj);
    assert_eq!(map_base.len(), 2);
    assert_eq!(map_spike.len(), 2);

    // Independent analytical GK vol over bars 3..=6 and skipped momentum numerator ln(C(4)/C(0)).
    let gk_base_1 = (shared_ohlc_sym1[3..5]
        .iter()
        .chain(tail_base_sym1.iter())
        .map(|&o| independent_gk_vol(o))
        .sum::<f64>())
        / 4.0;
    let gk_base_2 = (shared_ohlc_sym2[3..5]
        .iter()
        .chain(tail_base_sym2.iter())
        .map(|&o| independent_gk_vol(o))
        .sum::<f64>())
        / 4.0;
    let gk_spike_1 = (shared_ohlc_sym1[3..5]
        .iter()
        .chain(tail_spike_sym1.iter())
        .map(|&o| independent_gk_vol(o))
        .sum::<f64>())
        / 4.0;
    let gk_spike_2 = (shared_ohlc_sym2[3..5]
        .iter()
        .chain(tail_spike_sym2.iter())
        .map(|&o| independent_gk_vol(o))
        .sum::<f64>())
        / 4.0;

    let num_1 = (120.0_f64 / 100.0_f64).ln();
    let num_2 = (125.0_f64 / 100.0_f64).ln();

    assert_close(
        map_base[&SymbolId(1)],
        num_1 / gk_base_1,
        1e-11,
        "base sym1",
    )?;
    assert_close(
        map_base[&SymbolId(2)],
        num_2 / gk_base_2,
        1e-11,
        "base sym2",
    )?;
    assert_close(
        map_spike[&SymbolId(1)],
        num_1 / gk_spike_1,
        1e-11,
        "spike sym1",
    )?;
    assert_close(
        map_spike[&SymbolId(2)],
        num_2 / gk_spike_2,
        1e-11,
        "spike sym2",
    )?;

    // Prove numerator is strictly uncontaminated by bars 5..=6 (C(5)=58, C(6)=30 drops < C(0)=100
    // would turn an unskipped return negative, yet score * mean_gk equals exact ln(C(4)/C(0))).
    assert!(map_spike[&SymbolId(2)] > 0.0);
    assert_close(
        map_spike[&SymbolId(1)] * gk_spike_1,
        num_1,
        1e-12,
        "uncontaminated numerator sym1",
    )?;
    assert_close(
        map_spike[&SymbolId(2)] * gk_spike_2,
        num_2,
        1e-12,
        "uncontaminated numerator sym2",
    )?;
    Ok(())
}

fn find_carry_records(recs: &[XsPeriodRecord]) -> Vec<&XsPeriodRecord> {
    recs.iter()
        .filter(|r| r.hypothesis == XsHypothesis::XsFundingCarry)
        .collect()
}

fn make_strong_alpha_candles(
    num_slices: usize,
    carry_scale: f64,
) -> Result<Vec<Candle>, DomainError> {
    let base_ts = 1_700_000_000_000_000_000_i64;
    let target_px_bps = [1.8_f64, 2.6, 4.2, 3.4];
    let mut candles = Vec::with_capacity(num_slices * 5);

    // Precompute per-slice open and close prices so each 2-bar period captures exact target_px_bps.
    // Period p (even) holds Long Sym1, Short Sym2 across [2 + 2p ..= 4 + 2p].
    // Period p (odd)  holds Long Sym3, Short Sym4 across [2 + 2p ..= 4 + 2p].
    let mut opens = vec![[100.0_f64; 5]; num_slices];
    let mut closes = vec![[100.0_f64; 5]; num_slices];

    let max_period = num_slices / 2;
    for p in 0..max_period {
        let exit = 4 + 2 * p;
        if exit < num_slices {
            let active_sym = if p.is_multiple_of(2) { 1 } else { 3 };
            // For K=1, w = +0.5 on long leg, so leg return = 2 * target_px_bps[p % 4].
            let leg_ret = 2.0 * target_px_bps[p % 4] / BPS_UNIT;
            let bump_px = 100.0 * (1.0 + leg_ret);
            opens[exit][active_sym] = bump_px;
            closes[exit][active_sym] = bump_px;
            if exit + 1 < num_slices {
                opens[exit + 1][active_sym] = bump_px;
                closes[exit + 1][active_sym] = bump_px;
            }
        }
    }
    // Compensate truncated EOF close (single bar funding + extra liquidation turnover) cleanly.
    let last_p = (num_slices - 3) / 2;
    let last_active_sym = if last_p.is_multiple_of(2) { 1 } else { 3 };
    let last_idx = num_slices - 1;
    let eof_port_px_bps = 5.0 * carry_scale + 5.0;
    closes[last_idx][last_active_sym] =
        opens[last_idx - 1][last_active_sym] * (1.0 + 2.0 * eof_port_px_bps / BPS_UNIT);

    for idx in 0..num_slices {
        let ts = base_ts + (idx as i64) * BAR_4H_NS;
        let block_even = (idx / 2).is_multiple_of(2);
        let fundings = if block_even {
            [
                0.0,
                -5.0 * carry_scale,
                5.0 * carry_scale,
                -2.0 * carry_scale,
                2.0 * carry_scale,
            ]
        } else {
            [
                0.0,
                -2.0 * carry_scale,
                2.0 * carry_scale,
                -5.0 * carry_scale,
                5.0 * carry_scale,
            ]
        };
        for sym in 0_u16..5 {
            let s = usize::from(sym);
            let o = opens[idx][s];
            let c = closes[idx][s];
            let h = o.max(c) + 1.0;
            let l = o.min(c) - 1.0;
            candles.push(make_candle(sym, ts, (o, h, l, c), 500.0, fundings[s])?);
        }
    }
    Ok(candles)
}

#[test]
fn test_exact_turnover_funding_bar_open_eof_and_maker_never_passes() -> TestResult {
    // Part A: Exact 3-period hand-constructed sequence across 8 slices (idx = 0..=7), K=2, H=2.
    let cfg = XsFalsifierConfig {
        top_ks: vec![2],
        horizons_bars: vec![2],
        slippage_bps: Bps::new(1.5)?,
        funding_semantics: FundingTimestampSemantics::BarOpenSettlement,
        bars_per_year: 2_190.0,
        signal: XsSignalConfig {
            benchmark_symbol: SymbolId(0),
            exclude_benchmark_from_trading: true,
            funding_window_bars: 2,
            beta_window_bars: 100,
            resid_window_bars: 2,
            mom_lookback_bars: 100,
            mom_skip_bars: 2,
            vol_window_bars: 100,
            ..XsSignalConfig::default()
        },
    };

    // Opens and closes for Sym0..=Sym5 across idx 0..=7:
    let opens: [[f64; 8]; 6] = [
        [1000.0; 8],
        [100.0, 100.0, 100.0, 102.0, 110.0, 112.0, 121.0, 121.0],
        [100.0, 100.0, 100.0, 101.0, 104.0, 104.0, 104.0, 104.0],
        [100.0, 100.0, 100.0, 100.0, 100.0, 98.0, 96.0, 96.0],
        [100.0, 100.0, 100.0, 99.0, 98.0, 99.0, 100.0, 100.0],
        [100.0, 100.0, 100.0, 98.0, 96.0, 96.0, 96.0, 96.0],
    ];
    let closes: [[f64; 8]; 6] = [
        [1000.0; 8],
        [100.0, 100.0, 102.0, 104.0, 112.0, 115.0, 120.0, 133.1],
        [100.0, 100.0, 101.0, 102.0, 104.0, 104.0, 104.0, 104.0],
        [100.0, 100.0, 100.0, 100.0, 98.0, 97.0, 96.0, 91.2],
        [100.0, 100.0, 99.0, 98.0, 99.0, 100.0, 100.0, 104.0],
        [100.0, 100.0, 98.0, 97.0, 96.0, 96.0, 96.0, 91.2],
    ];
    // Funding per slice: huge on idx=2 (excluded by Period 1) and idx=6 (excluded by Period 3).
    let fundings: [[f64; 8]; 6] = [
        [0.0; 8],
        [-5.0, -5.0, -5000.0, -10.0, -8.0, 2.0, 200.0, -12.0],
        [-3.0, -3.0, -4000.0, -4.0, 2.0, -4.0, -100.0, 4.0],
        [0.0, 0.0, 4000.0, 12.0, 4.0, 6.0, 300.0, 8.0],
        [3.0, 3.0, 0.0, 2.0, -6.0, -10.0, -400.0, -4.0],
        [5.0, 5.0, 5000.0, 6.0, 10.0, 12.0, 500.0, 16.0],
    ];

    let base_ts = 1_000_000_000_000_i64;
    let mut candles = Vec::with_capacity(48);
    for idx in 0..8 {
        let ts = base_ts + (idx as i64) * BAR_4H_NS;
        for sym in 0_u16..6 {
            let s = usize::from(sym);
            let o = opens[s][idx];
            let c = closes[s][idx];
            let h = o.max(c) + 10.0;
            let l = o.min(c) - 10.0;
            candles.push(make_candle(
                sym,
                ts,
                (o, h, l, c),
                1_000.0,
                fundings[s][idx],
            )?);
        }
    }

    let mut reader = VecCandleReader::new(candles);
    let (_rep, recs) = run_xs_falsification_detailed(&mut reader, &cfg)?;
    let carry_recs = find_carry_records(&recs);
    assert_eq!(carry_recs.len(), 3);

    // Period 1: entry=2 -> exit=4 (Open->Open), Long {1,2}, Short {4,5}, turnover=1.0
    let p1 = carry_recs[0];
    assert!(!p1.is_eof_close);
    assert_eq!(p1.decision_ts_ns, base_ts + BAR_4H_NS);
    assert_eq!(p1.entry_ts_ns, base_ts + 2 * BAR_4H_NS);
    assert_eq!(p1.exit_ts_ns, base_ts + 4 * BAR_4H_NS);
    assert_close(p1.weights[&SymbolId(1)], 0.25, 1e-15, "p1 w1")?;
    assert_close(p1.weights[&SymbolId(2)], 0.25, 1e-15, "p1 w2")?;
    assert_close(p1.weights[&SymbolId(4)], -0.25, 1e-15, "p1 w4")?;
    assert_close(p1.weights[&SymbolId(5)], -0.25, 1e-15, "p1 w5")?;
    assert_close(p1.port_price_bps, 500.0, 1e-11, "p1 price")?;
    assert_close(p1.port_fund_bps, 8.0, 1e-11, "p1 fund")?;
    assert_close(p1.turnover, 1.0, 1e-15, "p1 turnover")?;
    assert_close(p1.cost_vip0_bps, 7.0, 1e-12, "p1 cost_vip0")?;
    assert_close(p1.cost_maker_bps, 2.0, 1e-12, "p1 cost_maker")?;
    assert_close(p1.net_bps_vip0, 501.0, 1e-11, "p1 net_vip0")?;
    assert_close(p1.net_bps_maker, 506.0, 1e-11, "p1 net_maker")?;

    // Period 2: entry=4 -> exit=6 (Open->Open), hold {1,2,-5}, swap 4->3, turnover=0.5
    let p2 = carry_recs[1];
    assert!(!p2.is_eof_close);
    assert_eq!(p2.decision_ts_ns, base_ts + 3 * BAR_4H_NS);
    assert_eq!(p2.entry_ts_ns, base_ts + 4 * BAR_4H_NS);
    assert_eq!(p2.exit_ts_ns, base_ts + 6 * BAR_4H_NS);
    assert_close(p2.weights[&SymbolId(1)], 0.25, 1e-15, "p2 w1")?;
    assert_close(p2.weights[&SymbolId(2)], 0.25, 1e-15, "p2 w2")?;
    assert_close(p2.weights[&SymbolId(3)], -0.25, 1e-15, "p2 w3")?;
    assert_close(p2.weights[&SymbolId(5)], -0.25, 1e-15, "p2 w5")?;
    assert_close(p2.port_price_bps, 350.0, 1e-11, "p2 price")?;
    assert_close(p2.port_fund_bps, 180.0, 1e-11, "p2 fund")?;
    assert_close(p2.turnover, 0.5, 1e-15, "p2 turnover")?;
    assert_close(p2.cost_vip0_bps, 3.5, 1e-12, "p2 cost_vip0")?;
    assert_close(p2.cost_maker_bps, 1.0, 1e-12, "p2 cost_maker")?;
    assert_close(p2.net_bps_vip0, 526.5, 1e-11, "p2 net_vip0")?;
    assert_close(p2.net_bps_maker, 529.0, 1e-11, "p2 net_maker")?;

    // Period 3: entry=6 -> exit=7 (EOF Close(7)), hold {1,-3,-5}, swap 2->4, turnover=0.5+1.0=1.5
    let p3 = carry_recs[2];
    assert!(p3.is_eof_close);
    assert_eq!(p3.decision_ts_ns, base_ts + 5 * BAR_4H_NS);
    assert_eq!(p3.entry_ts_ns, base_ts + 6 * BAR_4H_NS);
    assert_eq!(p3.exit_ts_ns, base_ts + 7 * BAR_4H_NS);
    assert_close(p3.weights[&SymbolId(1)], 0.25, 1e-15, "p3 w1")?;
    assert_close(p3.weights[&SymbolId(4)], 0.25, 1e-15, "p3 w4")?;
    assert_close(p3.weights[&SymbolId(3)], -0.25, 1e-15, "p3 w3")?;
    assert_close(p3.weights[&SymbolId(5)], -0.25, 1e-15, "p3 w5")?;
    assert_close(p3.port_price_bps, 600.0, 1e-11, "p3 price")?;
    assert_close(p3.port_fund_bps, 10.0, 1e-11, "p3 fund")?;
    assert_close(p3.turnover, 1.5, 1e-15, "p3 turnover")?;
    assert_close(p3.cost_vip0_bps, 10.5, 1e-12, "p3 cost_vip0")?;
    assert_close(p3.cost_maker_bps, 3.0, 1e-12, "p3 cost_maker")?;
    assert_close(p3.net_bps_vip0, 599.5, 1e-11, "p3 net_vip0")?;
    assert_close(p3.net_bps_maker, 607.0, 1e-11, "p3 net_maker")?;

    // Part B: Strong-alpha synthetic dataset (> 20 periods) where MakerDiag achieves t_nw > 4.0
    // and DSR > 0.99, while VIP0-Taker fails; verify MakerDiag is non-binding and NEVER passes.
    let cfg_alpha = XsFalsifierConfig {
        top_ks: vec![1],
        horizons_bars: vec![2],
        slippage_bps: Bps::new(1.5)?,
        funding_semantics: FundingTimestampSemantics::BarOpenSettlement,
        bars_per_year: 2_190.0,
        signal: XsSignalConfig {
            funding_window_bars: 2,
            beta_window_bars: 500,
            resid_window_bars: 2,
            mom_lookback_bars: 500,
            mom_skip_bars: 2,
            vol_window_bars: 500,
            ..XsSignalConfig::default()
        },
    };
    let candles_mid = make_strong_alpha_candles(66, 1.0)?;
    let mut reader_mid = VecCandleReader::new(candles_mid);
    let (rep_mid, _) = run_xs_falsification_detailed(&mut reader_mid, &cfg_alpha)?;
    let vip0_mid = &rep_mid.report.vip0_cells[0];
    let maker_mid = &rep_mid.report.diag_zero_cells[0];
    assert!(vip0_mid.n > 20);
    assert_eq!(vip0_mid.verdict, GateVerdict::Fail);
    assert!(maker_mid.t_nw.unwrap_or(0.0) > 4.0);
    assert!(maker_mid.dsr.unwrap_or(0.0) > 0.99);
    assert!(!maker_mid.is_binding);
    assert_eq!(maker_mid.verdict, GateVerdict::Fail);
    for cell in &rep_mid.report.diag_zero_cells {
        assert!(!cell.is_binding);
        assert_ne!(cell.verdict, GateVerdict::Pass);
    }

    // Also verify high carry (3.0x) where VIP0-Taker passes, yet MakerDiag STILL never passes.
    let candles_high = make_strong_alpha_candles(66, 3.0)?;
    let mut reader_high = VecCandleReader::new(candles_high);
    let (rep_high, _) = run_xs_falsification_detailed(&mut reader_high, &cfg_alpha)?;
    let vip0_high = &rep_high.report.vip0_cells[0];
    let maker_high = &rep_high.report.diag_zero_cells[0];
    assert_eq!(vip0_high.verdict, GateVerdict::Pass);
    assert!(maker_high.t_nw.unwrap_or(0.0) > 4.0);
    assert!(maker_high.dsr.unwrap_or(0.0) > 0.99);
    for cell in &rep_high.report.diag_zero_cells {
        assert!(!cell.is_binding);
        assert_ne!(cell.verdict, GateVerdict::Pass);
    }
    Ok(())
}

#[test]
fn test_zero_lookahead_end_to_end_track3_xs() -> TestResult {
    let orig_slices = build_synthetic_xs_panel(160)?;
    let k = 120_usize;
    let cutoff_ts = orig_slices[k].ts_ns.raw();

    let mut mut_slices = orig_slices.clone();
    for slice in &mut mut_slices[(k + 1)..] {
        for candle in slice.candles.values_mut() {
            let o = candle.open.as_f64() * 10.0;
            let c = candle.close.as_f64() * 10.0;
            let h = o.max(c) * 1.45;
            let l = (o.min(c) * 0.55).max(0.01);
            *candle = make_candle(
                candle.symbol_id.0,
                candle.open_ts_ns.raw(),
                (o, h, l, c),
                candle.volume * 50.0,
                -500.0,
            )?;
        }
    }

    let flatten = |slices: &[CrossSectionSlice]| -> Vec<Candle> {
        slices
            .iter()
            .flat_map(|s| s.candles.values().copied())
            .collect()
    };

    let cfg = XsFalsifierConfig {
        top_ks: vec![1, 2, 3],
        horizons_bars: vec![6, 18, 42],
        slippage_bps: Bps::new(1.5)?,
        funding_semantics: FundingTimestampSemantics::BarOpenSettlement,
        bars_per_year: 2_190.0,
        signal: XsSignalConfig::default(),
    };

    let mut orig_reader = VecCandleReader::new(flatten(&orig_slices));
    let mut mut_reader = VecCandleReader::new(flatten(&mut_slices));
    let (_orig_rep, orig_recs) = run_xs_falsification_detailed(&mut orig_reader, &cfg)?;
    let (_mut_rep, mut_recs) = run_xs_falsification_detailed(&mut mut_reader, &cfg)?;

    let orig_pre: Vec<&XsPeriodRecord> = orig_recs
        .iter()
        .filter(|r| r.exit_ts_ns <= cutoff_ts)
        .collect();
    let mut_pre: Vec<&XsPeriodRecord> = mut_recs
        .iter()
        .filter(|r| r.exit_ts_ns <= cutoff_ts)
        .collect();

    assert_eq!(orig_pre.len(), mut_pre.len());
    for &top_k in &[1_usize, 2, 3] {
        for hyp in XsHypothesis::ALL {
            let count = orig_pre
                .iter()
                .filter(|r| r.top_k == top_k && r.hypothesis == hyp)
                .count();
            assert!(
                count > 0,
                "expected non-empty pre-cutoff records for K={top_k}, hyp={}",
                hyp.as_str()
            );
        }
    }

    for (a, b) in orig_pre.iter().zip(mut_pre.iter()) {
        assert_eq!(a.top_k, b.top_k);
        assert_eq!(a.hypothesis, b.hypothesis);
        assert_eq!(a.horizon_bars, b.horizon_bars);
        assert_eq!(a.decision_ts_ns, b.decision_ts_ns);
        assert_eq!(a.entry_ts_ns, b.entry_ts_ns);
        assert_eq!(a.exit_ts_ns, b.exit_ts_ns);
        assert_eq!(a.is_eof_close, b.is_eof_close);
        assert_eq!(a.weights.len(), b.weights.len());
        for (sym, &wa) in &a.weights {
            let wb = b.weights.get(sym).copied().ok_or("missing weight symbol")?;
            assert_eq!(wa.to_bits(), wb.to_bits());
        }
        assert_eq!(a.port_price_bps.to_bits(), b.port_price_bps.to_bits());
        assert_eq!(a.port_fund_bps.to_bits(), b.port_fund_bps.to_bits());
        assert_eq!(a.turnover.to_bits(), b.turnover.to_bits());
        assert_eq!(a.cost_vip0_bps.to_bits(), b.cost_vip0_bps.to_bits());
        assert_eq!(a.cost_maker_bps.to_bits(), b.cost_maker_bps.to_bits());
        assert_eq!(a.net_bps_vip0.to_bits(), b.net_bps_vip0.to_bits());
        assert_eq!(a.net_bps_maker.to_bits(), b.net_bps_maker.to_bits());
    }

    // Confirm post-cutoff mutations actually changed post-cutoff records.
    let orig_post: Vec<&XsPeriodRecord> = orig_recs
        .iter()
        .filter(|r| r.exit_ts_ns > cutoff_ts)
        .collect();
    let mut_post: Vec<&XsPeriodRecord> = mut_recs
        .iter()
        .filter(|r| r.exit_ts_ns > cutoff_ts)
        .collect();
    assert!(!orig_post.is_empty());
    assert!(orig_post
        .iter()
        .zip(mut_post.iter())
        .any(|(a, b)| a.net_bps_vip0.to_bits() != b.net_bps_vip0.to_bits()));
    Ok(())
}

#[test]
fn test_xs_edge_cases_zero_var_flat_bar_ties_insufficient_symbols_and_non_finite() -> TestResult {
    // 1. Zero BTC variance (Var(r_btc) == 0) => XsResidualReversion omits all symbols cleanly.
    let cfg_var = XsSignalConfig {
        beta_window_bars: 4,
        resid_window_bars: 2,
        ..XsSignalConfig::default()
    };
    let mut eng_var = XsSignalEngine::new(cfg_var)?;
    for idx in 0..5 {
        let ts = 1_000_000_000_i64 + (idx as i64) * BAR_4H_NS;
        let alt1_c = 100.0 + (idx as f64) * 3.0;
        let alt2_c = 100.0 - (idx as f64) * 2.0;
        let slice = make_slice(
            ts,
            vec![
                make_candle(0, ts, (1000.0, 1005.0, 995.0, 1000.0), 100.0, 0.0)?,
                make_candle(1, ts, (100.0, alt1_c + 2.0, 99.0, alt1_c), 100.0, 0.0)?,
                make_candle(2, ts, (100.0, 101.0, alt2_c - 2.0, alt2_c), 100.0, 0.0)?,
            ],
        )?;
        let scores = eng_var.on_slice(&slice)?;
        assert!(scores
            .scores_for(XsHypothesis::XsResidualReversion)
            .is_empty());
    }

    // 2. Flat altcoin candles (O == H == L == C, gk_vol == 0) => XsMomentumVolAdj omits flat alt.
    let cfg_flat = XsSignalConfig {
        mom_lookback_bars: 4,
        mom_skip_bars: 1,
        vol_window_bars: 3,
        ..XsSignalConfig::default()
    };
    let mut eng_flat = XsSignalEngine::new(cfg_flat)?;
    let mut last_flat_scores = None;
    for idx in 0..5 {
        let ts = 2_000_000_000_i64 + (idx as i64) * BAR_4H_NS;
        let c1 = 100.0 + (idx as f64) * 2.0;
        let slice = make_slice(
            ts,
            vec![
                make_candle(0, ts, (1000.0, 1010.0, 990.0, 1002.0), 100.0, 0.0)?,
                make_candle(1, ts, (c1 - 1.0, c1 + 2.0, c1 - 2.0, c1), 100.0, 0.0)?,
                make_candle(2, ts, (100.0, 100.0, 100.0, 100.0), 0.0, 0.0)?,
            ],
        )?;
        last_flat_scores = Some(eng_flat.on_slice(&slice)?);
    }
    let flat_scores = last_flat_scores.ok_or("expected last slice scores")?;
    let mom_map = scores_to_map(&flat_scores, XsHypothesis::XsMomentumVolAdj);
    assert_eq!(mom_map.len(), 1);
    assert!(mom_map.contains_key(&SymbolId(1)));
    assert!(!mom_map.contains_key(&SymbolId(2)));
    assert!(mom_map[&SymbolId(1)].is_finite());

    // 3. Exact score ties across 4 altcoins => deterministic ascending SymbolId tie-break.
    let tied = [
        (SymbolId(40), 1.0),
        (SymbolId(10), 1.0),
        (SymbolId(30), 1.0),
        (SymbolId(20), 1.0),
    ];
    let w_tie_1 = build_dollar_neutral_weights(&tied, 1)?.ok_or("expected tie weights K=1")?;
    assert_eq!(w_tie_1.len(), 2);
    assert_close(w_tie_1[&SymbolId(10)], 0.5, 1e-15, "tie k1 long")?;
    assert_close(w_tie_1[&SymbolId(40)], -0.5, 1e-15, "tie k1 short")?;

    let w_tie_2 = build_dollar_neutral_weights(&tied, 2)?.ok_or("expected tie weights K=2")?;
    assert_eq!(w_tie_2.len(), 4);
    assert_close(w_tie_2[&SymbolId(10)], 0.25, 1e-15, "tie k2 long1")?;
    assert_close(w_tie_2[&SymbolId(20)], 0.25, 1e-15, "tie k2 long2")?;
    assert_close(w_tie_2[&SymbolId(30)], -0.25, 1e-15, "tie k2 short1")?;
    assert_close(w_tie_2[&SymbolId(40)], -0.25, 1e-15, "tie k2 short2")?;

    // 4. Fewer than 2*K valid altcoins => Ok(None).
    let three_syms = [(SymbolId(1), 3.0), (SymbolId(2), 2.0), (SymbolId(3), 1.0)];
    assert_eq!(build_dollar_neutral_weights(&three_syms, 2)?, None);
    assert_eq!(build_dollar_neutral_weights(&[], 1)?, None);
    assert!(matches!(
        build_dollar_neutral_weights(&three_syms, 0),
        Err(DomainError::InvalidStatParam("top_k"))
    ));

    // 5. Missing BTC in slice => has_benchmark == false and all 3 score vectors empty.
    let cfg_bench = XsSignalConfig {
        funding_window_bars: 1,
        ..XsSignalConfig::default()
    };
    let mut eng_bench = XsSignalEngine::new(cfg_bench)?;
    let ts_b0 = 3_000_000_000_i64;
    let with_btc = make_slice(
        ts_b0,
        vec![
            make_candle(0, ts_b0, (1000.0, 1010.0, 990.0, 1000.0), 10.0, 1.0)?,
            make_candle(1, ts_b0, (100.0, 102.0, 98.0, 101.0), 10.0, 2.0)?,
        ],
    )?;
    assert!(eng_bench.on_slice(&with_btc)?.has_benchmark);
    let ts_b1 = ts_b0 + BAR_4H_NS;
    let without_btc = make_slice(
        ts_b1,
        vec![
            make_candle(1, ts_b1, (101.0, 103.0, 99.0, 102.0), 10.0, -2.0)?,
            make_candle(2, ts_b1, (100.0, 102.0, 98.0, 99.0), 10.0, 3.0)?,
        ],
    )?;
    let missing_out = eng_bench.on_slice(&without_btc)?;
    assert!(!missing_out.has_benchmark);
    for hyp in XsHypothesis::ALL {
        assert!(missing_out.scores_for(hyp).is_empty());
    }

    // 6. Non-finite scores and invalid/non-finite volumes rejected.
    for &bad in &[f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        let bad_scores = [(SymbolId(1), bad), (SymbolId(2), 1.0)];
        assert!(matches!(
            build_dollar_neutral_weights(&bad_scores, 1),
            Err(DomainError::NonFiniteValue(_))
        ));
        assert!(matches!(
            make_candle(1, 1000, (100.0, 102.0, 98.0, 101.0), bad, 0.0),
            Err(DomainError::InvalidCandle(_))
        ));
        assert!(matches!(
            make_candle(1, 1000, (100.0, 102.0, 98.0, 101.0), 10.0, bad),
            Err(DomainError::NonFiniteBps)
        ));

        let mut eng_bad = XsSignalEngine::new(XsSignalConfig::default())?;
        let mut raw_bad = make_candle(1, 1000, (100.0, 102.0, 98.0, 101.0), 10.0, 0.0)?;
        raw_bad.volume = bad;
        let bad_slice = make_slice(1000, vec![raw_bad])?;
        assert!(matches!(
            eng_bad.on_slice(&bad_slice),
            Err(DomainError::InvalidCandle(_))
        ));
    }
    Ok(())
}

#[test]
fn test_cli_scan_xs_csv_and_global27_report() -> TestResult {
    let dir = TempDirGuard::new("cli_global27")?;
    let slices = build_synthetic_xs_panel(160)?;

    let mut files: Vec<File> = Vec::with_capacity(9);
    for sym in 0_u16..9 {
        let csv_path = dir.path.join(format!("sym_{sym}.csv"));
        let mut file = File::create(&csv_path)?;
        writeln!(
            file,
            "symbol_id,ts_ms,open,high,low,close,volume,funding_rate_bps"
        )?;
        files.push(file);
    }

    let base_ms = 1_700_000_000_000_i64;
    for (idx, slice) in slices.iter().enumerate() {
        let ts_ms = base_ms + (idx as i64) * BAR_4H_MS;
        for sym in 0_u16..9 {
            let c = slice
                .candles
                .get(&SymbolId(sym))
                .ok_or("missing symbol in panel")?;
            writeln!(
                files[usize::from(sym)],
                "{sym},{ts_ms},{:.8},{:.8},{:.8},{:.8},{:.6},{:.6}",
                c.open.as_f64(),
                c.high.as_f64(),
                c.low.as_f64(),
                c.close.as_f64(),
                c.volume,
                c.funding_rate_bps.raw()
            )?;
        }
    }
    drop(files);

    let input_str = dir.path.to_str().ok_or("non-utf8 temp dir")?;

    // 1. --top-k 2 => M = 9, t_crit = 3.5, exclude_benchmark_from_trading = true.
    let out9_path = dir.path.join("report_m9.json");
    let out9_str = out9_path.to_str().ok_or("non-utf8 out9 path")?;
    let ascii9 = run_cli([
        "vortex-alfa",
        "scan-xs",
        "--format",
        "csv",
        "--input",
        input_str,
        "--top-k",
        "2",
        "--out",
        out9_str,
    ])?;
    assert!(ascii9.contains("Track3-CrossSectional"));
    assert!(ascii9.contains("M=9"));

    let rep9: XsFalsificationReport = serde_json::from_str(&fs::read_to_string(&out9_path)?)?;
    assert_eq!(rep9.top_ks, vec![2]);
    assert!(rep9.exclude_benchmark_from_trading);
    assert_eq!(rep9.slices_processed, 160);
    assert_eq!(rep9.report.events_processed, 1_440);
    assert_eq!(rep9.report.m_hypotheses, 9);
    assert_close(rep9.report.t_crit, 3.5, 1e-12, "t_crit M=9")?;
    assert_eq!(rep9.diagnostics.len(), 9);

    // 2. --top-k 1,2,3 --slippage-bps 1.5 => M = 27, t_crit ~ 3.56034323, 27 diagnostics.
    let out27_path = dir.path.join("report_m27.json");
    let out27_str = out27_path.to_str().ok_or("non-utf8 out27 path")?;
    let ascii27 = run_cli([
        "vortex-alfa",
        "scan-xs",
        "--format",
        "csv",
        "--input",
        input_str,
        "--top-k",
        "1,2,3",
        "--slippage-bps",
        "1.5",
        "--out",
        out27_str,
    ])?;
    assert!(ascii27.contains("M=27"));

    let rep27: XsFalsificationReport = serde_json::from_str(&fs::read_to_string(&out27_path)?)?;
    assert_eq!(rep27.top_ks, vec![1, 2, 3]);
    assert_close(rep27.slippage_bps, 1.5, 1e-12, "slippage_bps")?;
    assert!(rep27.exclude_benchmark_from_trading);
    assert_eq!(rep27.report.m_hypotheses, 27);
    assert_close(rep27.report.t_crit, 3.560_343_23, 1e-4, "t_crit M=27")?;
    assert_eq!(rep27.diagnostics.len(), 27);
    assert_eq!(rep27.report.vip0_cells.len(), 27);
    assert_eq!(rep27.report.diag_zero_cells.len(), 27);
    for cell in &rep27.report.vip0_cells {
        assert!(cell.is_binding);
    }
    for cell in &rep27.report.diag_zero_cells {
        assert!(!cell.is_binding);
        assert_ne!(cell.verdict, GateVerdict::Pass);
    }

    // 3. --include-btc => exclude_benchmark_from_trading = false.
    let out_btc_path = dir.path.join("report_incl_btc.json");
    let out_btc_str = out_btc_path.to_str().ok_or("non-utf8 out_btc path")?;
    let _ascii_btc = run_cli([
        "vortex-alfa",
        "scan-xs",
        "--format",
        "csv",
        "--input",
        input_str,
        "--top-k",
        "2",
        "--include-btc",
        "--out",
        out_btc_str,
    ])?;
    let rep_btc: XsFalsificationReport = serde_json::from_str(&fs::read_to_string(&out_btc_path)?)?;
    assert!(!rep_btc.exclude_benchmark_from_trading);
    assert_eq!(rep_btc.report.m_hypotheses, 9);
    Ok(())
}
