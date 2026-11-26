use std::collections::{BTreeMap, BTreeSet, VecDeque};

use serde::{Deserialize, Serialize};

use crate::domain::{
    error::DomainError,
    signals::meso_energy::garman_klass_vol,
    types::{Candle, SymbolId, TimestampNs},
};

#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum XsHypothesis {
    XsFundingCarry,
    XsResidualReversion,
    XsMomentumVolAdj,
}

impl XsHypothesis {
    pub const ALL: [Self; 3] = [
        Self::XsFundingCarry,
        Self::XsResidualReversion,
        Self::XsMomentumVolAdj,
    ];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::XsFundingCarry => "XsFundingCarry",
            Self::XsResidualReversion => "XsResidualReversion",
            Self::XsMomentumVolAdj => "XsMomentumVolAdj",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct XsSignalConfig {
    pub benchmark_symbol: SymbolId,
    pub exclude_benchmark_from_trading: bool,
    pub funding_window_bars: usize,
    pub beta_window_bars: usize,
    pub resid_window_bars: usize,
    pub beta_min: f64,
    pub beta_max: f64,
    pub min_beta_var: f64,
    pub mom_lookback_bars: usize,
    pub mom_skip_bars: usize,
    pub vol_window_bars: usize,
    pub min_gk_vol: f64,
}

impl Default for XsSignalConfig {
    fn default() -> Self {
        Self {
            benchmark_symbol: SymbolId(0),
            exclude_benchmark_from_trading: true,
            funding_window_bars: 18,
            beta_window_bars: 84,
            resid_window_bars: 18,
            beta_min: -3.0,
            beta_max: 5.0,
            min_beta_var: 1e-12,
            mom_lookback_bars: 84,
            mom_skip_bars: 6,
            vol_window_bars: 84,
            min_gk_vol: 1e-9,
        }
    }
}

#[derive(Clone, Debug)]
pub struct CrossSectionSlice {
    pub ts_ns: TimestampNs,
    pub candles: BTreeMap<SymbolId, Candle>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct XsSliceScores {
    pub ts_ns: TimestampNs,
    pub has_benchmark: bool,
    pub scores: [Vec<(SymbolId, f64)>; 3],
}

impl XsSliceScores {
    #[must_use]
    pub fn scores_for(&self, hypothesis: XsHypothesis) -> &[(SymbolId, f64)] {
        match hypothesis {
            XsHypothesis::XsFundingCarry => &self.scores[0],
            XsHypothesis::XsResidualReversion => &self.scores[1],
            XsHypothesis::XsMomentumVolAdj => &self.scores[2],
        }
    }
}

#[derive(Clone, Debug, Default)]
struct SymbolXsState {
    prev_ts_ns: Option<i64>,
    prev_close: Option<f64>,
    funding_window: VecDeque<f64>,
    gk_history: VecDeque<f64>,
    close_history: VecDeque<f64>,
    beta_pairs: VecDeque<(f64, f64)>,
}

fn push_bounded<T>(buf: &mut VecDeque<T>, val: T, max_len: usize) {
    if buf.len() == max_len {
        buf.pop_front();
    }
    buf.push_back(val);
}

fn chk(ok: bool, param: &'static str) -> Result<(), DomainError> {
    ok.then_some(()).ok_or(DomainError::InvalidStatParam(param))
}

#[derive(Clone, Debug)]
pub struct XsSignalEngine {
    config: XsSignalConfig,
    prev_slice_ts_ns: Option<i64>,
    symbols: BTreeMap<SymbolId, SymbolXsState>,
}

impl XsSignalEngine {
    pub fn new(config: XsSignalConfig) -> Result<Self, DomainError> {
        let c = &config;
        let beta_ok = c.beta_min.is_finite() && c.beta_max.is_finite() && c.beta_min <= c.beta_max;
        chk(c.funding_window_bars >= 1, "funding_window_bars")?;
        chk(c.beta_window_bars >= 2, "beta_window_bars")?;
        chk(c.resid_window_bars >= 1, "resid_window_bars")?;
        chk(beta_ok, "beta_bounds")?;
        chk(
            c.min_beta_var.is_finite() && c.min_beta_var > 0.0,
            "min_beta_var",
        )?;
        chk(c.mom_lookback_bars > c.mom_skip_bars, "mom_lookback_bars")?;
        chk(c.vol_window_bars >= 1, "vol_window_bars")?;
        chk(c.min_gk_vol.is_finite() && c.min_gk_vol > 0.0, "min_gk_vol")?;
        Ok(Self {
            config,
            prev_slice_ts_ns: None,
            symbols: BTreeMap::new(),
        })
    }

    #[must_use]
    pub const fn config(&self) -> &XsSignalConfig {
        &self.config
    }

    pub fn on_slice(&mut self, slice: &CrossSectionSlice) -> Result<XsSliceScores, DomainError> {
        let slice_ts = slice.ts_ns.raw();
        if let Some(prev_ns) = self.prev_slice_ts_ns.filter(|&p| slice_ts <= p) {
            return Err(DomainError::NonMonotonicTimestamp {
                symbol_id: self.config.benchmark_symbol.0,
                prev_ns,
                curr_ns: slice_ts,
            });
        }
        let mut validated = Vec::with_capacity(slice.candles.len());
        for (&sym_id, c) in &slice.candles {
            let bad = c.symbol_id != sym_id
                || c.open_ts_ns != slice.ts_ns
                || !c.volume.is_finite()
                || c.volume < 0.0
                || c.close.as_f64() <= 0.0;
            if bad {
                return Err(DomainError::InvalidCandle(format!("sym {}", sym_id.0)));
            }
            if !c.funding_rate_bps.raw().is_finite() {
                return Err(DomainError::NonFiniteBps);
            }
            validated.push((sym_id, c, garman_klass_vol(c)?));
        }

        let cfg = &self.config;
        let bench = cfg.benchmark_symbol;
        let has_benchmark = slice.candles.contains_key(&bench);
        let btc_ret_pair = match (slice.candles.get(&bench), self.symbols.get(&bench)) {
            (Some(bc), Some(bs)) => match (bs.prev_ts_ns, bs.prev_close) {
                (Some(pts), Some(pc)) if pc > 0.0 => Some((pts, (bc.close.as_f64() / pc).ln())),
                _ => None,
            },
            _ => None,
        };
        if btc_ret_pair.is_some_and(|(_, r)| !r.is_finite()) {
            return Err(DomainError::NonFiniteValue("r_btc"));
        }

        let pair_cap = cfg.beta_window_bars.max(cfg.resid_window_bars);
        let mut aligned = BTreeSet::new();
        for (sym_id, c, gk_vol) in validated {
            let st = self.symbols.entry(sym_id).or_default();
            let (close_f, fund) = (c.close.as_f64(), c.funding_rate_bps.raw());
            push_bounded(&mut st.funding_window, fund, cfg.funding_window_bars);
            push_bounded(&mut st.gk_history, gk_vol, cfg.vol_window_bars);
            push_bounded(&mut st.close_history, close_f, cfg.mom_lookback_bars + 1);
            if let (Some((bpts, r_btc)), Some(pc)) = (btc_ret_pair, st.prev_close) {
                if st.prev_ts_ns == Some(bpts) && pc > 0.0 {
                    let r_i = (close_f / pc).ln();
                    if !r_i.is_finite() {
                        return Err(DomainError::NonFiniteValue("r_i"));
                    }
                    push_bounded(&mut st.beta_pairs, (r_i, r_btc), pair_cap);
                    aligned.insert(sym_id);
                }
            }
            st.prev_ts_ns = Some(slice_ts);
            st.prev_close = Some(close_f);
        }
        self.prev_slice_ts_ns = Some(slice_ts);

        let mut scores: [Vec<(SymbolId, f64)>; 3] = [Vec::new(), Vec::new(), Vec::new()];
        if has_benchmark {
            for &sym_id in slice.candles.keys() {
                if cfg.exclude_benchmark_from_trading && sym_id == bench {
                    continue;
                }
                let Some(st) = self.symbols.get(&sym_id) else {
                    continue;
                };
                if st.funding_window.len() == cfg.funding_window_bars {
                    let s = -st.funding_window.iter().sum::<f64>();
                    if s.is_finite() {
                        scores[0].push((sym_id, s));
                    }
                }
                if aligned.contains(&sym_id) && st.beta_pairs.len() >= pair_cap {
                    if let Some(s) = residual_reversion_score(&st.beta_pairs, cfg) {
                        scores[1].push((sym_id, s));
                    }
                }
                if st.close_history.len() == cfg.mom_lookback_bars + 1
                    && st.gk_history.len() == cfg.vol_window_bars
                {
                    let gk = st.gk_history.iter().sum::<f64>() / (cfg.vol_window_bars as f64);
                    let c_old = st.close_history[0];
                    let c_rec = st.close_history[cfg.mom_lookback_bars - cfg.mom_skip_bars];
                    if gk.is_finite() && gk > cfg.min_gk_vol && c_old > 0.0 && c_rec > 0.0 {
                        let s = (c_rec / c_old).ln() / gk;
                        if s.is_finite() {
                            scores[2].push((sym_id, s));
                        }
                    }
                }
            }
        }
        Ok(XsSliceScores {
            ts_ns: slice.ts_ns,
            has_benchmark,
            scores,
        })
    }
}

fn residual_reversion_score(pairs: &VecDeque<(f64, f64)>, cfg: &XsSignalConfig) -> Option<f64> {
    let w = cfg.beta_window_bars;
    let (mut si, mut sb) = (0.0, 0.0);
    for &(ri, rb) in pairs.iter().skip(pairs.len() - w) {
        si += ri;
        sb += rb;
    }
    let (mi, mb) = (si / (w as f64), sb / (w as f64));
    let (mut s_xx, mut s_xy) = (0.0, 0.0);
    for &(ri, rb) in pairs.iter().skip(pairs.len() - w) {
        let db = rb - mb;
        s_xx += db * db;
        s_xy += (ri - mi) * db;
    }
    let var_btc = s_xx / ((w - 1) as f64);
    if !var_btc.is_finite() || var_btc <= cfg.min_beta_var {
        return None;
    }
    let raw_beta = s_xy / s_xx;
    if !raw_beta.is_finite() {
        return None;
    }
    let beta_hat = raw_beta.clamp(cfg.beta_min, cfg.beta_max);
    let mut e_i = 0.0;
    for &(ri, rb) in pairs.iter().skip(pairs.len() - cfg.resid_window_bars) {
        e_i += ri - beta_hat * rb;
    }
    (-e_i).is_finite().then_some(-e_i)
}

pub fn build_dollar_neutral_weights(
    scores: &[(SymbolId, f64)],
    top_k: usize,
) -> Result<Option<BTreeMap<SymbolId, f64>>, DomainError> {
    chk(top_k > 0, "top_k")?;
    if scores.iter().any(|&(_, s)| !s.is_finite()) {
        return Err(DomainError::NonFiniteValue("xs_score"));
    }
    if scores.len() / 2 < top_k {
        return Ok(None);
    }
    let mut sorted = scores.to_vec();
    sorted.sort_by(|a, b| b.1.total_cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    let (w, n) = (1.0 / (2.0 * (top_k as f64)), sorted.len());
    let mut weights = BTreeMap::new();
    for &(sym, _) in &sorted[..top_k] {
        weights.insert(sym, w);
    }
    for &(sym, _) in &sorted[(n - top_k)..] {
        weights.insert(sym, -w);
    }
    Ok(Some(weights))
}
