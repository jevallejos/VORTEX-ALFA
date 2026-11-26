#![allow(clippy::excessive_precision)]

use crate::domain::error::DomainError;

const SQRT_2PI: f64 = 2.506_628_274_631_000_7;

// Cody (1969) rational Chebyshev coefficients for erf/erfc.
const THRESH: f64 = 0.46875;
const XBIG: f64 = 27.3;

const A: [f64; 5] = [
    3.161_123_743_870_565_6,
    1.138_641_541_510_501_6e2,
    3.774_852_376_853_020_2e2,
    3.209_377_589_138_469_5e3,
    1.857_777_061_846_031_5e-1,
];

const B: [f64; 4] = [
    2.360_129_095_234_412_1e1,
    2.440_246_379_344_441_7e2,
    1.282_616_526_077_372_3e3,
    2.844_236_833_439_170_6e3,
];

const C: [f64; 9] = [
    5.641_884_969_886_700_9e-1,
    8.883_149_794_388_375_9,
    6.611_919_063_714_163_0e1,
    2.986_351_381_974_001_3e2,
    8.819_522_212_417_690_9e2,
    1.712_047_612_634_070_6e3,
    2.051_078_377_826_071_5e3,
    1.230_339_354_797_997_3e3,
    2.153_115_354_744_038_5e-8,
];

const D: [f64; 8] = [
    1.574_492_611_070_983_5e1,
    1.176_939_508_913_125_0e2,
    5.371_811_018_620_098_6e2,
    1.621_389_574_566_690_2e3,
    3.290_799_235_733_459_6e3,
    4.362_619_090_143_247_2e3,
    3.439_367_674_143_721_6e3,
    1.230_339_354_803_749_4e3,
];

const P: [f64; 6] = [
    3.053_266_349_612_323_4e-1,
    3.603_448_999_498_044_4e-1,
    1.257_817_261_112_292_5e-1,
    1.608_378_514_874_227_7e-2,
    6.587_491_615_298_378_0e-4,
    1.631_538_713_730_209_8e-2,
];

const Q: [f64; 5] = [
    2.568_520_192_289_822_3,
    1.872_952_849_923_460_5,
    5.279_051_029_514_284_1e-1,
    6.051_834_131_244_131_9e-2,
    2.335_204_976_268_691_9e-3,
];

// Acklam inverse normal rational approximation coefficients.
const ACK_A: [f64; 6] = [
    -3.969_683_028_665_376e1,
    2.209_460_984_245_205e2,
    -2.759_285_104_469_687e2,
    1.383_577_518_672_690e2,
    -3.066_479_806_614_716e1,
    2.506_628_277_459_239,
];

const ACK_B: [f64; 5] = [
    -5.447_609_879_822_406e1,
    1.615_858_368_580_409e2,
    -1.556_989_798_598_866e2,
    6.680_131_188_771_972e1,
    -1.328_068_155_288_572e1,
];

const ACK_C: [f64; 6] = [
    -7.784_894_002_430_293e-3,
    -3.223_964_580_411_365e-1,
    -2.400_758_277_161_838,
    -2.549_732_539_343_734,
    4.374_664_141_464_968,
    2.938_163_982_698_783,
];

const ACK_D: [f64; 4] = [
    7.784_695_709_041_462e-3,
    3.224_671_290_700_398e-1,
    2.445_134_137_142_996,
    3.754_408_661_907_416,
];

/// Complementary error function `erfc(x)` using Cody's (1969) rational Chebyshev
/// approximation (< 1e-15 relative error across all IEEE-754 double precision regions).
#[must_use]
pub fn erfc_exact(x: f64) -> f64 {
    if x.is_nan() {
        return f64::NAN;
    }
    if x == f64::INFINITY {
        return 0.0;
    }
    if x == f64::NEG_INFINITY {
        return 2.0;
    }

    let y = x.abs();
    if y <= THRESH {
        let ysq = if y > 1.11e-16 { y * y } else { 0.0 };
        let mut xnum = A[4] * ysq;
        let mut xden = ysq;
        for i in 0..3 {
            xnum = (xnum + A[i]) * ysq;
            xden = (xden + B[i]) * ysq;
        }
        let erf_val = x * (xnum + A[3]) / (xden + B[3]);
        return 1.0 - erf_val;
    }

    let mut result = if y <= 4.0 {
        let mut xnum = C[8] * y;
        let mut xden = y;
        for i in 0..7 {
            xnum = (xnum + C[i]) * y;
            xden = (xden + D[i]) * y;
        }
        let ratio = (xnum + C[7]) / (xden + D[7]);
        let ysq = (y * 16.0).trunc() / 16.0;
        let del = (y - ysq) * (y + ysq);
        (-ysq * ysq).exp() * (-del).exp() * ratio
    } else if y >= XBIG {
        0.0
    } else {
        let ysq = 1.0 / (y * y);
        let mut xnum = P[5] * ysq;
        let mut xden = ysq;
        for i in 0..4 {
            xnum = (xnum + P[i]) * ysq;
            xden = (xden + Q[i]) * ysq;
        }
        let ratio = ysq * (xnum + P[4]) / (xden + Q[4]);
        let frac = (0.5 * core::f64::consts::FRAC_2_SQRT_PI - ratio) / y;
        let ysq_i = (y * 16.0).trunc() / 16.0;
        let del = (y - ysq_i) * (y + ysq_i);
        (-ysq_i * ysq_i).exp() * (-del).exp() * frac
    };

    if x < 0.0 {
        result = 2.0 - result;
    }
    result
}

/// Standard normal cumulative distribution function `Phi(x)`.
/// Rejects NaN; allows `+/-inf` returning `1.0`/`0.0`.
pub fn normal_cdf(x: f64) -> Result<f64, DomainError> {
    if x.is_nan() {
        return Err(DomainError::NonFiniteValue("normal_cdf_input"));
    }
    if x == f64::INFINITY {
        return Ok(1.0);
    }
    if x == f64::NEG_INFINITY {
        return Ok(0.0);
    }
    let cdf = 0.5 * erfc_exact(-x / core::f64::consts::SQRT_2);
    Ok(cdf.clamp(0.0, 1.0))
}

fn inv_lower_tail(q: f64) -> Result<f64, DomainError> {
    const P_LOW: f64 = 0.02425;
    let mut x = if q < P_LOW {
        let r = (-2.0 * q.ln()).sqrt();
        (((((ACK_C[0] * r + ACK_C[1]) * r + ACK_C[2]) * r + ACK_C[3]) * r + ACK_C[4]) * r
            + ACK_C[5])
            / ((((ACK_D[0] * r + ACK_D[1]) * r + ACK_D[2]) * r + ACK_D[3]) * r + 1.0)
    } else {
        let r = q - 0.5;
        let s = r * r;
        (((((ACK_A[0] * s + ACK_A[1]) * s + ACK_A[2]) * s + ACK_A[3]) * s + ACK_A[4]) * s
            + ACK_A[5])
            * r
            / (((((ACK_B[0] * s + ACK_B[1]) * s + ACK_B[2]) * s + ACK_B[3]) * s + ACK_B[4]) * s
                + 1.0)
    };

    // One Halley refinement step on the lower tail (x <= 0 avoids 1 - Phi cancellation).
    let exp_half_x2 = (0.5 * x * x).exp();
    if exp_half_x2.is_finite() {
        let u = (normal_cdf(x)? - q) * SQRT_2PI * exp_half_x2;
        let denom = 1.0 + 0.5 * x * u;
        if u.is_finite() && denom.is_finite() && denom != 0.0 {
            x -= u / denom;
        }
    }
    Ok(x)
}

/// Inverse standard normal CDF `Phi^{-1}(p)` using Acklam's rational approximation
/// refined by one Halley step with tail symmetry (`p > 0.5 => -inv_lower(1.0 - p)`).
pub fn normal_inv_cdf(p: f64) -> Result<f64, DomainError> {
    if !p.is_finite() || p <= 0.0 || p >= 1.0 {
        return Err(DomainError::InvalidStatParam("normal_inv_cdf_probability"));
    }
    if p > 0.5 {
        Ok(-inv_lower_tail(1.0 - p)?)
    } else {
        inv_lower_tail(p)
    }
}
