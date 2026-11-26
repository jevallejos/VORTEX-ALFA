use serde::{Deserialize, Serialize};

use crate::domain::error::DomainError;
use crate::domain::fees::FeeSchedule;
use crate::domain::normal_dist::{normal_cdf, normal_inv_cdf};

#[allow(clippy::approx_constant)]
pub const EULER_MASCHERONI: f64 = 0.577_215_664_901_532_9;
pub const MIN_SAMPLE_SIZE: usize = 5;

#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum GateVerdict {
    Pass,
    Fail,
    InsufficientSample,
}

#[derive(Copy, Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Moments4 {
    pub n: usize,
    pub mean: f64,
    pub variance: f64,
    pub std_dev: f64,
    pub skewness: f64,
    pub kurtosis: f64,
}

pub fn welford_mean(values: &[f64]) -> Result<Option<f64>, DomainError> {
    let mut mean = 0.0_f64;
    for (i, &v) in values.iter().enumerate() {
        if !v.is_finite() {
            return Err(DomainError::NonFiniteValue("welford_mean_input"));
        }
        mean += (v - mean) / ((i + 1) as f64);
    }
    if !mean.is_finite() {
        return Err(DomainError::NonFiniteValue("welford_mean_output"));
    }
    Ok((!values.is_empty()).then_some(mean))
}

fn mean_if_sufficient(returns: &[f64]) -> Result<Option<f64>, DomainError> {
    let mean = welford_mean(returns)?;
    Ok(mean.filter(|_| returns.len() >= MIN_SAMPLE_SIZE))
}

impl Moments4 {
    pub fn compute(returns: &[f64]) -> Result<Option<Moments4>, DomainError> {
        let Some(mean) = mean_if_sufficient(returns)? else {
            return Ok(None);
        };
        let n = returns.len();
        let (s2, s3, s4) = returns.iter().fold((0.0, 0.0, 0.0), |(a, b, c), &r| {
            let d = r - mean;
            let d2 = d * d;
            (a + d2, b + d2 * d, c + d2 * d2)
        });
        let variance = s2 / ((n - 1) as f64);
        if !variance.is_finite() || variance <= 0.0 {
            return Ok(None);
        }
        let (std_dev, n_f) = (variance.sqrt(), n as f64);
        let m2 = s2 / n_f;
        let (skewness, kurtosis) = ((s3 / n_f) / m2.powf(1.5), (s4 / n_f) / (m2 * m2));
        let ok = skewness.is_finite() && kurtosis.is_finite();
        Ok(ok.then_some(Moments4 {
            n,
            mean,
            variance,
            std_dev,
            skewness,
            kurtosis,
        }))
    }
}

#[must_use]
pub fn newey_west_lag(n: usize) -> usize {
    if n <= 1 {
        0
    } else {
        ((4.0 * ((n as f64) / 100.0).powf(2.0 / 9.0)).floor() as usize).min(n - 1)
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct NeweyWestResult {
    pub lag: usize,
    pub lrv: f64,
    pub se_nw: f64,
    pub t_nw: f64,
    pub p_value_two_sided: f64,
}

pub fn newey_west(returns: &[f64]) -> Result<Option<NeweyWestResult>, DomainError> {
    let Some(mu) = mean_if_sufficient(returns)? else {
        return Ok(None);
    };
    let n = returns.len();
    let n_f = n as f64;
    let gamma_0 = returns.iter().map(|&r| (r - mu) * (r - mu)).sum::<f64>() / n_f;
    if !gamma_0.is_finite() || gamma_0 <= 0.0 {
        return Ok(None);
    }
    let lag = newey_west_lag(n);
    let mut lrv = gamma_0;
    for k in 1..=lag {
        let sum_k: f64 = (k..n)
            .map(|t| (returns[t] - mu) * (returns[t - k] - mu))
            .sum();
        lrv += 2.0 * (1.0 - (k as f64) / ((lag + 1) as f64)) * (sum_k / n_f);
    }
    if lrv <= 0.0 {
        lrv = gamma_0;
    }
    let se_nw = (lrv / n_f).sqrt();
    if !se_nw.is_finite() || se_nw <= 0.0 {
        return Ok(None);
    }
    let t_nw = mu / se_nw;
    let p_value_two_sided = (2.0 * normal_cdf(-t_nw.abs())?).clamp(0.0, 1.0);
    Ok(Some(NeweyWestResult {
        lag,
        lrv,
        se_nw,
        t_nw,
        p_value_two_sided,
    }))
}

pub fn t_crit_holm(m_hypotheses: usize) -> Result<f64, DomainError> {
    if m_hypotheses == 0 {
        return Err(DomainError::InvalidStatParam("m_hypotheses"));
    }
    let p = 1.0 - 0.01 / (2.0 * (m_hypotheses as f64));
    Ok(3.50_f64.max(normal_inv_cdf(p)?))
}

pub fn holm_bonferroni_adjust(p_values: &[f64]) -> Result<Vec<f64>, DomainError> {
    if p_values
        .iter()
        .any(|&p| !p.is_finite() || !(0.0..=1.0).contains(&p))
    {
        return Err(DomainError::InvalidStatParam("holm_p_value"));
    }
    let m = p_values.len();
    let mut idx_list: Vec<usize> = (0..m).collect();
    idx_list.sort_by(|&a, &b| p_values[a].total_cmp(&p_values[b]));
    let (mut adjusted, mut rmax) = (vec![0.0_f64; m], 0.0_f64);
    for (rank, &idx) in idx_list.iter().enumerate() {
        rmax = rmax.max((((m - rank) as f64) * p_values[idx]).clamp(0.0, 1.0));
        adjusted[idx] = rmax;
    }
    Ok(adjusted)
}

pub fn deflated_sharpe_ratio(
    moments: &Moments4,
    m_hypotheses: usize,
    sr_cross_std: f64,
) -> Result<Option<f64>, DomainError> {
    if m_hypotheses == 0 || !sr_cross_std.is_finite() || sr_cross_std < 0.0 {
        return Err(DomainError::InvalidStatParam("deflated_sharpe_ratio"));
    }
    if moments.n < MIN_SAMPLE_SIZE || !moments.std_dev.is_finite() || moments.std_dev <= 0.0 {
        return Ok(None);
    }
    let sr_hat = moments.mean / moments.std_dev;
    if !sr_hat.is_finite() || !moments.skewness.is_finite() || !moments.kurtosis.is_finite() {
        return Ok(None);
    }
    let sr_0 = if m_hypotheses == 1 {
        0.0
    } else {
        let m_f = m_hypotheses as f64;
        let q1 = normal_inv_cdf(1.0 - 1.0 / m_f)?;
        let q2 = normal_inv_cdf(1.0 - 1.0 / (m_f * core::f64::consts::E))?;
        sr_cross_std * ((1.0 - EULER_MASCHERONI) * q1 + EULER_MASCHERONI * q2)
    };
    let d_sq = 1.0 - moments.skewness * sr_hat + ((moments.kurtosis - 1.0) / 4.0) * sr_hat * sr_hat;
    if !d_sq.is_finite() || d_sq <= 0.0 {
        return Ok(None);
    }
    let z = ((sr_hat - sr_0) * ((moments.n - 1) as f64).sqrt()) / d_sq.sqrt();
    z.is_finite().then(|| normal_cdf(z)).transpose()
}

#[must_use]
pub fn compute_n_min(mean: f64, std_dev: f64, t_crit: f64) -> Option<u64> {
    let ok = mean.is_finite() && std_dev.is_finite() && t_crit.is_finite();
    if !ok || mean <= 0.0 || std_dev <= 0.0 {
        return None;
    }
    let val = (t_crit * std_dev / mean).powi(2).ceil();
    (val.is_finite() && val <= (u64::MAX as f64)).then_some(val as u64)
}

#[must_use]
pub fn evaluate_gate(
    n: usize,
    mean: f64,
    t_nw: Option<f64>,
    t_crit: f64,
    n_min: Option<u64>,
    dsr: Option<f64>,
    is_binding_for_verdict: bool,
) -> GateVerdict {
    if n < MIN_SAMPLE_SIZE {
        GateVerdict::InsufficientSample
    } else if !mean.is_finite() || mean <= 0.0 || n_min.is_none() {
        GateVerdict::Fail
    } else if n_min.is_some_and(|nm| (n as u64) < nm) {
        GateVerdict::InsufficientSample
    } else if is_binding_for_verdict
        && t_nw.is_some_and(|t| t >= t_crit)
        && dsr.is_some_and(|d| d >= 0.95)
    {
        GateVerdict::Pass
    } else {
        GateVerdict::Fail
    }
}

#[derive(Clone, Debug)]
pub struct HypothesisCellInput {
    pub hypothesis: String,
    pub horizon: String,
    pub returns_bps: Vec<f64>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CellEvaluation {
    pub hypothesis: String,
    pub horizon: String,
    pub profile: String,
    pub is_binding: bool,
    pub n: usize,
    pub n_min: Option<u64>,
    pub mean_bps: Option<f64>,
    pub std_bps: Option<f64>,
    pub sr_hat: Option<f64>,
    pub t_nw: Option<f64>,
    pub t_crit: f64,
    pub p_holm: Option<f64>,
    pub dsr: Option<f64>,
    pub verdict: GateVerdict,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BatchSummary {
    pub sr_cross_std: Option<f64>,
    pub t_crit: f64,
    pub cells: Vec<CellEvaluation>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FalsificationReport {
    pub track: String,
    pub events_processed: u64,
    pub m_hypotheses: usize,
    pub t_crit: f64,
    pub sr_cross_std_vip0: Option<f64>,
    pub sr_cross_std_diag: Option<f64>,
    pub vip0_cells: Vec<CellEvaluation>,
    pub diag_zero_cells: Vec<CellEvaluation>,
}

fn sample_std_positive(values: &[f64]) -> Option<f64> {
    let mean = (values.len() >= 2)
        .then(|| welford_mean(values).ok().flatten())
        .flatten()?;
    let var =
        values.iter().map(|&v| (v - mean) * (v - mean)).sum::<f64>() / ((values.len() - 1) as f64);
    (var.is_finite() && var > 0.0).then(|| var.sqrt())
}

pub fn evaluate_hypothesis_batch(
    cells: &[HypothesisCellInput],
    schedule: &FeeSchedule,
) -> Result<BatchSummary, DomainError> {
    let m_hyp = cells.len().max(1);
    let t_crit = t_crit_holm(m_hyp)?;
    let mut pre = Vec::with_capacity(cells.len());
    for cell in cells {
        pre.push((
            welford_mean(&cell.returns_bps)?,
            Moments4::compute(&cell.returns_bps)?,
            newey_west(&cell.returns_bps)?,
        ));
    }
    let valid_srs: Vec<f64> = pre
        .iter()
        .filter_map(|&(_, m, _)| m.map(|x| x.mean / x.std_dev).filter(|sr| sr.is_finite()))
        .collect();
    let p_entries: Vec<(usize, f64)> = pre
        .iter()
        .enumerate()
        .filter_map(|(i, &(_, _, nw))| nw.map(|r| (i, r.p_value_two_sided)))
        .collect();
    let sr_cross_std = sample_std_positive(&valid_srs);
    let adj_ps = holm_bonferroni_adjust(&p_entries.iter().map(|&(_, p)| p).collect::<Vec<_>>())?;

    let mut out_cells = Vec::with_capacity(cells.len());
    for (idx, cell) in cells.iter().enumerate() {
        let (n, (mean_opt, m_opt, nw)) = (cell.returns_bps.len(), pre[idx]);
        let t_nw = nw.map(|r| r.t_nw);
        let eff_std = sr_cross_std.unwrap_or_else(|| 1.0 / (n.max(1) as f64).sqrt());
        let (std_bps, sr_hat) = (m_opt.map(|m| m.std_dev), m_opt.map(|m| m.mean / m.std_dev));
        let n_min = m_opt.and_then(|m| compute_n_min(m.mean, m.std_dev, t_crit));
        let dsr = m_opt
            .map(|m| deflated_sharpe_ratio(&m, m_hyp, eff_std))
            .transpose()?
            .flatten();
        let verdict = evaluate_gate(
            n,
            mean_opt.unwrap_or(0.0),
            t_nw,
            t_crit,
            n_min,
            dsr,
            schedule.is_binding_for_verdict,
        );
        out_cells.push(CellEvaluation {
            hypothesis: cell.hypothesis.clone(),
            horizon: cell.horizon.clone(),
            profile: schedule.name.to_string(),
            is_binding: schedule.is_binding_for_verdict,
            n,
            n_min,
            mean_bps: mean_opt,
            std_bps,
            sr_hat,
            t_nw,
            t_crit,
            p_holm: None,
            dsr,
            verdict,
        });
    }
    for (&(cidx, _), adj_p) in p_entries.iter().zip(adj_ps) {
        out_cells[cidx].p_holm = Some(adj_p);
    }
    Ok(BatchSummary {
        sr_cross_std,
        t_crit,
        cells: out_cells,
    })
}
