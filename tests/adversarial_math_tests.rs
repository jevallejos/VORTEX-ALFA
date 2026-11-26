use std::error::Error;

use vortex_alfa::application::falsifier_meso::{
    run_meso_falsification_detailed, MesoFalsifierConfig,
};
use vortex_alfa::domain::error::DomainError;
use vortex_alfa::domain::fees::{book_roundtrip_net_bps, leg_fee_e8, FeeSchedule};
use vortex_alfa::domain::normal_dist::{normal_cdf, normal_inv_cdf};
use vortex_alfa::domain::ports::CandleSeriesReader;
use vortex_alfa::domain::signals::meso_energy::{garman_klass_vol, MesoConfig};
use vortex_alfa::domain::signals::micro_detectors::HawkesProcess;
use vortex_alfa::domain::stats::{
    evaluate_hypothesis_batch, holm_bonferroni_adjust, newey_west, GateVerdict,
    HypothesisCellInput, Moments4,
};
use vortex_alfa::domain::types::{
    BookTop, Bps, Candle, FundingTimestampSemantics, NotionalE8, PriceE8, QtyE8, Side, SymbolId,
    TimestampNs, SCALE_E8,
};

type TestResult = Result<(), Box<dyn Error>>;

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

#[test]
fn test_garman_klass_closed_form_and_flat_bar() -> TestResult {
    let sym = SymbolId(1);
    let ts = TimestampNs::new(1_000_000_000)?;
    let candle = Candle::new(
        sym,
        ts,
        PriceE8::new(100 * (SCALE_E8 as i64))?,
        PriceE8::new(110 * (SCALE_E8 as i64))?,
        PriceE8::new(95 * (SCALE_E8 as i64))?,
        PriceE8::new(105 * (SCALE_E8 as i64))?,
        10.0,
        Bps::ZERO,
    )?;
    let gk = garman_klass_vol(&candle)?;
    // Independent analytical derivation:
    // sqrt(0.5 * ln(110/95)^2 - (2*ln(2) - 1) * ln(105/100)^2)
    // = 0.09912983040222308
    assert_close(gk, 0.099_129_830_402_223_08, 1e-13, "garman_klass_vol")?;

    let flat = Candle::new(
        sym,
        ts,
        PriceE8::new(100 * (SCALE_E8 as i64))?,
        PriceE8::new(100 * (SCALE_E8 as i64))?,
        PriceE8::new(100 * (SCALE_E8 as i64))?,
        PriceE8::new(100 * (SCALE_E8 as i64))?,
        0.0,
        Bps::ZERO,
    )?;
    let gk_flat = garman_klass_vol(&flat)?;
    assert_eq!(gk_flat.to_bits(), 0.0_f64.to_bits());
    Ok(())
}

#[test]
fn test_moments4_closed_form_exact() -> TestResult {
    // Data: [1, 2, 3, 4, 10], N = 5
    // Mean = 20 / 5 = 4.0
    // Deviations d = [-3, -2, -1, 0, 6]
    // sum(d^2) = 9 + 4 + 1 + 0 + 36 = 50 -> variance = 50/4 = 12.5, m2 = 50/5 = 10.0
    // sum(d^3) = -27 - 8 - 1 + 0 + 216 = 180 -> m3 = 180/5 = 36.0 -> skew = 36 / 10^1.5
    // sum(d^4) = 81 + 16 + 1 + 0 + 1296 = 1394 -> m4 = 1394/5 = 278.8 -> kurt = 278.8 / 100 = 2.788
    let data = [1.0, 2.0, 3.0, 4.0, 10.0];
    let m = Moments4::compute(&data)?.ok_or("expected Some(Moments4)")?;
    assert_eq!(m.n, 5);
    assert_close(m.mean, 4.0, 1e-14, "mean")?;
    assert_close(m.variance, 12.5, 1e-14, "variance")?;
    assert_close(m.std_dev, 3.535_533_905_932_737_8, 1e-14, "std_dev")?;
    assert_close(m.skewness, 1.138_419_957_660_616_5, 1e-13, "skewness")?;
    assert_close(m.kurtosis, 2.788, 1e-14, "kurtosis")?;
    Ok(())
}

#[test]
fn test_newey_west_closed_form_n8() -> TestResult {
    // N = 8: [1, 3, 2, 4, 3, 5, 4, 6], mean = 3.5, L = floor(4*(8/100)^(2/9)) = 2
    // gamma_0 = 2.25, gamma_1 = 0.28125, gamma_2 = 1.0625
    // LRV = 2.25 + 2*(2/3)*0.28125 + 2*(1/3)*1.0625 = 10/3 = 3.3333333333333335
    // SE_NW = sqrt((10/3)/8) = sqrt(5/12) = 0.6454972243679028
    // t_NW = 3.5 / sqrt(5/12) = 5.422176684690384
    let data = [1.0, 3.0, 2.0, 4.0, 3.0, 5.0, 4.0, 6.0];
    let res = newey_west(&data)?.ok_or("expected Some(NeweyWestResult)")?;
    assert_eq!(res.lag, 2);
    assert_close(res.lrv, 3.333_333_333_333_333_5, 1e-14, "lrv")?;
    assert_close(res.se_nw, 0.645_497_224_367_902_8, 1e-14, "se_nw")?;
    assert_close(res.t_nw, 5.422_176_684_690_384, 1e-13, "t_nw")?;
    assert!(res.p_value_two_sided > 0.0 && res.p_value_two_sided < 1e-7);
    Ok(())
}

#[test]
fn test_acklam_normal_inv_cdf_accuracy() -> TestResult {
    let cases = [
        (0.5_f64, 0.0_f64),
        (0.975, 1.959_963_984_540_054),
        (0.999, 3.090_232_306_167_813_6),
        (1.0 - 0.01 / 2.0, 2.575_829_303_548_900_4),
        (1e-6, -4.753_424_308_822_899),
        (1.0 - 1e-8, 5.612_001_244_174_789),
    ];
    for &(p, expected_z) in &cases {
        let z = normal_inv_cdf(p)?;
        assert_close(z, expected_z, 1e-9, &format!("normal_inv_cdf({p})"))?;
        let roundtrip_p = normal_cdf(z)?;
        assert_close(roundtrip_p, p, 1e-12, &format!("normal_cdf(inv({p}))"))?;
    }
    Ok(())
}

#[test]
fn test_holm_bonferroni_order_and_monotonicity() -> TestResult {
    // Sorted p-values for [0.001, 0.01, 0.04, 0.03]:
    // rank 0 (idx 0): 4 * 0.001 = 0.004
    // rank 1 (idx 1): 3 * 0.01  = 0.030
    // rank 2 (idx 3): 2 * 0.03  = 0.060
    // rank 3 (idx 2): max(0.060, 1 * 0.04) = 0.060 (monotonicity clamp)
    let adj_monotone = holm_bonferroni_adjust(&[0.001, 0.01, 0.04, 0.03])?;
    assert_eq!(adj_monotone.len(), 4);
    assert_close(adj_monotone[0], 0.004, 1e-15, "holm[0]")?;
    assert_close(adj_monotone[1], 0.03, 1e-15, "holm[1]")?;
    assert_close(adj_monotone[2], 0.06, 1e-15, "holm[2]")?;
    assert_close(adj_monotone[3], 0.06, 1e-15, "holm[3]")?;
    assert!(adj_monotone[2] >= adj_monotone[3]);

    // Also verify [0.001, 0.01, 0.04, 0.02] -> [0.004, 0.03, 0.04, 0.04]
    let adj_exact = holm_bonferroni_adjust(&[0.001, 0.01, 0.04, 0.02])?;
    assert_close(adj_exact[0], 0.004, 1e-15, "holm2[0]")?;
    assert_close(adj_exact[1], 0.03, 1e-15, "holm2[1]")?;
    assert_close(adj_exact[2], 0.04, 1e-15, "holm2[2]")?;
    assert_close(adj_exact[3], 0.04, 1e-15, "holm2[3]")?;
    Ok(())
}

#[test]
fn test_dsr_cross_sectional_std_and_diag_never_passes() -> TestResult {
    // Cell 0: positive SR
    // [1, 3, 2, 4, 3, 5, 4, 6] -> mean = 3.5, std = sqrt(18/7) = 1.6035674514745464
    // sr_0 = 3.5 / sqrt(18/7) = 2.1826334756181326
    let r0 = vec![1.0, 3.0, 2.0, 4.0, 3.0, 5.0, 4.0, 6.0];
    // Cell 1: negative SR
    // [-2, -4, -1, -5, -3, -6] -> mean = -3.5, var = (1.5^2+0.5^2+2.5^2+1.5^2+0.5^2+2.5^2)/5 = 17.5/5 = 3.5
    // std = sqrt(3.5) = 1.8708286933869707, sr_1 = -3.5 / sqrt(3.5) = -1.8708286933869707
    let r1 = vec![-2.0, -4.0, -1.0, -5.0, -3.0, -6.0];
    // Cell 2: huge N and strong positive SR so t_nw >> t_crit and DSR > 0.99
    // Repeat [4.0, 6.0] 60 times (N = 120): mean = 5.0, sample_var = 120/119, std = sqrt(120/119)
    // sr_2 = 5.0 / sqrt(120/119) = 4.979123082096552
    let mut r2 = Vec::with_capacity(120);
    for _ in 0..60 {
        r2.push(4.0);
        r2.push(6.0);
    }

    let cells = vec![
        HypothesisCellInput {
            hypothesis: "H0".to_string(),
            horizon: "15s".to_string(),
            returns_bps: r0,
        },
        HypothesisCellInput {
            hypothesis: "H1".to_string(),
            horizon: "15s".to_string(),
            returns_bps: r1,
        },
        HypothesisCellInput {
            hypothesis: "H2".to_string(),
            horizon: "15s".to_string(),
            returns_bps: r2,
        },
    ];

    // Independent cross-sectional sample std of [sr_0, sr_1, sr_2]:
    let srs = [
        2.182_633_475_618_132_6_f64,
        -1.870_828_693_386_970_7_f64,
        4.979_123_082_096_552_f64,
    ];
    let sr_mean = (srs[0] + srs[1] + srs[2]) / 3.0;
    let expected_sr_cross_std =
        (((srs[0] - sr_mean).powi(2) + (srs[1] - sr_mean).powi(2) + (srs[2] - sr_mean).powi(2))
            / 2.0)
            .sqrt();

    let vip0 = evaluate_hypothesis_batch(&cells, &FeeSchedule::bybit_vip0())?;
    let actual_std = vip0.sr_cross_std.ok_or("expected Some(sr_cross_std)")?;
    assert_close(actual_std, expected_sr_cross_std, 1e-12, "sr_cross_std")?;
    // Ensure it is NOT the naive 1/sqrt(N) fallback
    assert!((actual_std - 1.0 / (120.0_f64).sqrt()).abs() > 1.0);
    assert!(vip0.cells[2].t_nw.unwrap_or(0.0) > vip0.t_crit);
    assert!(vip0.cells[2].dsr.unwrap_or(0.0) > 0.99);
    assert_eq!(vip0.cells[2].verdict, GateVerdict::Pass);

    // DiagZero NEVER yields GateVerdict::Pass even with huge positive t_nw and DSR > 0.99
    let diag = evaluate_hypothesis_batch(&cells, &FeeSchedule::diag_zero_maker())?;
    for c in &diag.cells {
        assert!(!c.is_binding);
        assert_ne!(c.verdict, GateVerdict::Pass);
    }
    assert!(diag.cells[2].t_nw.unwrap_or(0.0) > diag.t_crit);
    assert!(diag.cells[2].dsr.unwrap_or(0.0) > 0.99);
    assert_eq!(diag.cells[2].verdict, GateVerdict::Fail);
    Ok(())
}

#[test]
fn test_hawkes_o1_three_step_analytical() -> TestResult {
    let mut hp = HawkesProcess::new(1.0, 0.65, 2.0)?;
    let sym = SymbolId(7);

    let (r1, l1) = hp.on_event(0, sym)?;
    assert_eq!(r1, 0.0);
    assert_close(l1, 2.0, 1e-15, "lambda_1")?;

    let (r2, l2) = hp.on_event(500_000_000, sym)?;
    // exp(-0.5) = 0.6065306597126334
    assert_close(r2, 0.606_530_659_712_633_4, 1e-14, "R_2")?;
    assert_close(l2, 2.394_244_928_813_211_7, 1e-14, "lambda_2")?;

    let (r3, l3) = hp.on_event(1_500_000_000, sym)?;
    // exp(-1.0) * (1 + exp(-0.5)) = 0.59100960131987212
    assert_close(r3, 0.591_009_601_319_872_1, 1e-14, "R_3")?;
    assert_close(l3, 2.384_156_240_857_916_7, 1e-14, "lambda_3")?;
    Ok(())
}

#[test]
fn test_vip0_ceil_fee_e8_and_book_roundtrip_asymmetry() -> TestResult {
    let vip0 = FeeSchedule::bybit_vip0();
    // 100_000_001 * 55_000 = 5_500_000_055_000; ceil(/ 100_000_000) = 55_001
    let fee_odd = leg_fee_e8(NotionalE8::new(100_000_001)?, vip0.taker_bps)?;
    assert_eq!(fee_odd, 55_001);

    // Exact multiple: 100_000_000 * 55_000 / 100_000_000 = 55_000
    let fee_exact = leg_fee_e8(NotionalE8::new(100_000_000)?, vip0.taker_bps)?;
    assert_eq!(fee_exact, 55_000);

    // 1 satoshi at 2.0 bps -> ceil(20_000 / 1e8) = 1
    let fee_tiny = leg_fee_e8(NotionalE8::new(1)?, vip0.maker_bps)?;
    assert_eq!(fee_tiny, 1);

    let entry_book = BookTop::new(
        PriceE8::new(100 * (SCALE_E8 as i64))?,
        PriceE8::new(100_1000_0000)?, // 100.10
        QtyE8::new(SCALE_E8 as i64)?,
        QtyE8::new(SCALE_E8 as i64)?,
    )?;
    let exit_book = BookTop::new(
        PriceE8::new(101 * (SCALE_E8 as i64))?,
        PriceE8::new(101_2000_0000)?, // 101.20
        QtyE8::new(SCALE_E8 as i64)?,
        QtyE8::new(SCALE_E8 as i64)?,
    )?;

    // Long enters at entry.ask (100.10), exits at exit.bid (101.00)
    // ((101.0 - 100.1) / 100.1) * 10_000 - 11.0 = 78.91008991008991
    let long_net = book_roundtrip_net_bps(
        Side::Long,
        entry_book,
        exit_book,
        &vip0,
        true,
        true,
        Bps::ZERO,
    )?;
    assert_close(
        long_net.raw(),
        78.910_089_910_089_91,
        1e-11,
        "long_book_net",
    )?;

    // Short enters at entry.bid (100.00), exits at exit.ask (101.20)
    // -((101.2 - 100.0) / 100.0) * 10_000 - 11.0 = -131.0
    let short_net = book_roundtrip_net_bps(
        Side::Short,
        entry_book,
        exit_book,
        &vip0,
        true,
        true,
        Bps::ZERO,
    )?;
    assert_close(short_net.raw(), -131.0, 1e-12, "short_book_net")?;
    Ok(())
}

#[test]
fn test_exact_funding_boundary_bar_open_vs_bar_close() -> TestResult {
    // Construct deterministic candles for both Long and Short signal directions.
    // Bar 0: volatile bar (O=100, H=120, L=80, C=100) to seed slow GK EMA.
    // Bar 1 (t): compressed breakout bar (O=100, H=100.6, L=100, C=100.6 for Long;
    //            or O=100, H=100, L=99.4, C=99.4 for Short) -> triggers signal at bar 1.
    // Bar 2 (t+1, entry_bar): funding = +100.0 bps
    // Bar 3 (t+2):            funding = +25.0 bps
    // Bar 4 (t+3):            funding = -10.0 bps
    let sym = SymbolId(1);
    let step = 300_000_000_000_i64;
    let p = |v: f64| -> Result<PriceE8, DomainError> { PriceE8::new((v * 1e8).round() as i64) };

    for &(is_long, expected_sign) in &[(true, 1.0_f64), (false, -1.0_f64)] {
        let b1 = if is_long {
            Candle::new(
                sym,
                TimestampNs::new(step)?,
                p(100.0)?,
                p(100.6)?,
                p(100.0)?,
                p(100.6)?,
                100.0,
                Bps::ZERO,
            )?
        } else {
            Candle::new(
                sym,
                TimestampNs::new(step)?,
                p(100.0)?,
                p(100.0)?,
                p(99.4)?,
                p(99.4)?,
                100.0,
                Bps::ZERO,
            )?
        };

        let candles = vec![
            Candle::new(
                sym,
                TimestampNs::new(0)?,
                p(100.0)?,
                p(120.0)?,
                p(80.0)?,
                p(100.0)?,
                100.0,
                Bps::ZERO,
            )?,
            b1,
            Candle::new(
                sym,
                TimestampNs::new(2 * step)?,
                p(100.6)?,
                p(100.6)?,
                p(100.6)?,
                p(100.6)?,
                100.0,
                Bps::new(100.0)?,
            )?,
            Candle::new(
                sym,
                TimestampNs::new(3 * step)?,
                p(100.6)?,
                p(100.6)?,
                p(100.6)?,
                p(100.6)?,
                100.0,
                Bps::new(25.0)?,
            )?,
            Candle::new(
                sym,
                TimestampNs::new(4 * step)?,
                p(100.6)?,
                p(100.6)?,
                p(100.6)?,
                p(100.6)?,
                100.0,
                Bps::new(-10.0)?,
            )?,
        ];

        let det_cfg = MesoConfig {
            ema_fast_span: 1,
            ema_slow_span: 10,
            r_comp_threshold: 0.70,
            tsmom_window: 1,
            vol_ema_span: 24,
            vol_ratio_cap: 5.0,
            tsmom_threshold: 0.005,
            funding_warmup: 100,
            funding_z_threshold: 2.0,
        };

        let mut cfg_open = MesoFalsifierConfig {
            horizons_bars: vec![1, 3],
            slippage_bps: Bps::ZERO,
            funding_semantics: FundingTimestampSemantics::BarOpenSettlement,
            detector: det_cfg.clone(),
            entry_is_taker: true,
            exit_is_taker: true,
        };
        let mut r_open = VecCandleReader {
            candles: candles.clone(),
            idx: 0,
        };
        let (_, trades_open) = run_meso_falsification_detailed(&mut r_open, &cfg_open)?;
        let t_open_h1 = trades_open
            .iter()
            .find(|t| t.signal_ts_ns == step && t.horizon_bars == 1)
            .ok_or("missing BarOpen H=1 trade")?;
        let t_open_h3 = trades_open
            .iter()
            .find(|t| t.signal_ts_ns == step && t.horizon_bars == 3)
            .ok_or("missing BarOpen H=3 trade")?;
        assert_eq!(
            t_open_h1.side,
            if is_long { Side::Long } else { Side::Short }
        );
        assert_close(
            t_open_h1.funding_cost_bps,
            0.0,
            1e-12,
            "BarOpen H=1 funding",
        )?;
        assert_close(
            t_open_h3.funding_cost_bps,
            expected_sign * 15.0,
            1e-12,
            "BarOpen H=3 funding",
        )?;

        cfg_open.funding_semantics = FundingTimestampSemantics::BarCloseSettlement;
        let mut r_close = VecCandleReader { candles, idx: 0 };
        let (_, trades_close) = run_meso_falsification_detailed(&mut r_close, &cfg_open)?;
        let t_close_h1 = trades_close
            .iter()
            .find(|t| t.signal_ts_ns == step && t.horizon_bars == 1)
            .ok_or("missing BarClose H=1 trade")?;
        let t_close_h3 = trades_close
            .iter()
            .find(|t| t.signal_ts_ns == step && t.horizon_bars == 3)
            .ok_or("missing BarClose H=3 trade")?;
        assert_close(
            t_close_h1.funding_cost_bps,
            expected_sign * 100.0,
            1e-12,
            "BarClose H=1 funding",
        )?;
        assert_close(
            t_close_h3.funding_cost_bps,
            expected_sign * 115.0,
            1e-12,
            "BarClose H=3 funding",
        )?;
    }
    Ok(())
}
