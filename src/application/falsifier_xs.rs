use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::domain::{
    error::DomainError, fees::FeeSchedule, ports::CandleSeriesReader, signals::cross_sectional::*,
    stats::*, types::*,
};

#[derive(Clone, Debug)]
pub struct XsFalsifierConfig {
    pub top_ks: Vec<usize>,
    pub horizons_bars: Vec<usize>,
    pub slippage_bps: Bps,
    pub funding_semantics: FundingTimestampSemantics,
    pub bars_per_year: f64,
    pub signal: XsSignalConfig,
}

impl Default for XsFalsifierConfig {
    fn default() -> Self {
        Self {
            top_ks: vec![3],
            horizons_bars: vec![6, 18, 42],
            slippage_bps: Bps::new(1.5).unwrap_or(Bps::ZERO),
            funding_semantics: FundingTimestampSemantics::BarOpenSettlement,
            bars_per_year: 2_190.0,
            signal: XsSignalConfig::default(),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct XsPeriodRecord {
    pub top_k: usize,
    pub hypothesis: XsHypothesis,
    pub horizon_bars: usize,
    pub decision_ts_ns: i64,
    pub entry_ts_ns: i64,
    pub exit_ts_ns: i64,
    pub is_eof_close: bool,
    pub weights: BTreeMap<SymbolId, f64>,
    pub port_price_bps: f64,
    pub port_fund_bps: f64,
    pub turnover: f64,
    pub cost_vip0_bps: f64,
    pub cost_maker_bps: f64,
    pub net_bps_vip0: f64,
    pub net_bps_maker: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct XsCellDiagnostics {
    pub top_k: usize,
    pub hypothesis: String,
    pub horizon: String,
    pub horizon_bars: usize,
    pub mean_price_bps: Option<f64>,
    pub mean_fund_bps: Option<f64>,
    pub mean_turnover: Option<f64>,
    pub mean_cost_bps: Option<f64>,
    pub ann_sharpe: Option<f64>,
    pub max_drawdown_bps: f64,
    pub ann_sharpe_maker: Option<f64>,
    pub max_drawdown_maker_bps: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct XsFalsificationReport {
    pub top_ks: Vec<usize>,
    pub slippage_bps: f64,
    pub exclude_benchmark_from_trading: bool,
    pub slices_processed: u64,
    pub report: FalsificationReport,
    pub diagnostics: Vec<XsCellDiagnostics>,
}

type Weights = BTreeMap<SymbolId, f64>;
type Legs = BTreeMap<SymbolId, (f64, f64, f64)>;
struct ActivePos(i64, i64, usize, Weights, Legs, f64);
struct Slot(
    usize,
    XsHypothesis,
    usize,
    Option<ActivePos>,
    Option<(i64, Weights)>,
    [Vec<f64>; 6],
);

fn max_dd(ret: &[f64]) -> f64 {
    ret.iter()
        .fold((0.0_f64, 0.0_f64, 0.0_f64), |(e, p, d), &r| {
            let (ne, np) = (e + r, p.max(e + r));
            (ne, np, d.max(np - ne))
        })
        .2
}

fn chk(ok: bool, param: &'static str) -> Result<(), DomainError> {
    ok.then_some(()).ok_or(DomainError::InvalidStatParam(param))
}

fn settle_pos(
    s: &mut Slot,
    pos: ActivePos,
    ex: &CrossSectionSlice,
    eof: bool,
    is_roll: bool,
    costs: (f64, f64),
    recs: &mut Vec<XsPeriodRecord>,
) -> Result<(), DomainError> {
    let (mut px, mut fund) = (0.0_f64, 0.0_f64);
    for (sym, &w) in &pos.3 {
        let Some(&(epx, lc, f)) = pos.4.get(sym) else {
            return Err(DomainError::InvalidStatParam("leg"));
        };
        let xpx = match ex.candles.get(sym) {
            Some(c) if eof => c.close.as_f64(),
            Some(c) => c.open.as_f64(),
            None => lc,
        };
        if epx <= 0.0 || xpx <= 0.0 {
            return Err(DomainError::InvalidCandle("non-positive px".to_string()));
        }
        px += w * ((xpx - epx) / epx) * BPS_PER_UNIT;
        fund -= w * f;
    }
    let mut turn = pos.5;
    if !is_roll {
        turn += pos.3.values().map(|v| v.abs()).sum::<f64>();
    }
    let (c_vip, c_mak) = (turn * costs.0, turn * costs.1);
    let (n_vip, n_mak) = (px + fund - c_vip, px + fund - c_mak);
    if !px.is_finite() || !fund.is_finite() || !n_vip.is_finite() || !n_mak.is_finite() {
        return Err(DomainError::NonFiniteValue("xs_period_return"));
    }
    for (d, v) in s.5.iter_mut().zip([px, fund, turn, c_vip, n_vip, n_mak]) {
        d.push(v);
    }
    recs.push(XsPeriodRecord {
        top_k: s.0,
        hypothesis: s.1,
        horizon_bars: s.2,
        decision_ts_ns: pos.0,
        entry_ts_ns: pos.1,
        exit_ts_ns: ex.ts_ns.raw(),
        is_eof_close: eof,
        weights: pos.3,
        port_price_bps: px,
        port_fund_bps: fund,
        turnover: turn,
        cost_vip0_bps: c_vip,
        cost_maker_bps: c_mak,
        net_bps_vip0: n_vip,
        net_bps_maker: n_mak,
    });
    Ok(())
}

pub fn run_xs_falsification_detailed<R: CandleSeriesReader>(
    reader: &mut R,
    config: &XsFalsifierConfig,
) -> Result<(XsFalsificationReport, Vec<XsPeriodRecord>), DomainError> {
    let (ks, hs) = (&config.top_ks, &config.horizons_bars);
    let (slip, bpy) = (config.slippage_bps.raw(), config.bars_per_year);
    chk(!ks.is_empty() && !ks.contains(&0), "top_ks")?;
    chk(!hs.is_empty() && !hs.contains(&0), "horizons_bars")?;
    if !slip.is_finite() || slip < 0.0 {
        return Err(DomainError::InvalidFeeBps);
    }
    chk(bpy.is_finite() && bpy > 0.0, "bars_per_year")?;
    let mut engine = XsSignalEngine::new(config.signal.clone())?;
    let vip0 = FeeSchedule::bybit_vip0();
    let vip0_taker = FeeSchedule::custom("VIP0-Taker", vip0.maker_bps, vip0.taker_bps, true)?;
    let maker_diag = FeeSchedule::custom("VIP0-MakerDiag", vip0.maker_bps, vip0.taker_bps, false)?;
    let costs = (vip0.taker_bps.raw() + slip, vip0.maker_bps.raw());
    let (mut by_ts, mut events_processed) =
        (BTreeMap::<i64, BTreeMap<SymbolId, Candle>>::new(), 0_u64);
    while let Some(c) = reader.next_candle()? {
        events_processed += 1;
        let (ts, sym) = (c.open_ts_ns.raw(), c.symbol_id);
        if by_ts.entry(ts).or_default().insert(sym, c).is_some() {
            return Err(DomainError::NonMonotonicTimestamp {
                symbol_id: sym.0,
                prev_ns: ts,
                curr_ns: ts,
            });
        }
    }
    let mut slices = Vec::with_capacity(by_ts.len());
    for (ts, candles) in by_ts {
        slices.push(CrossSectionSlice {
            ts_ns: TimestampNs::new(ts)?,
            candles,
        });
    }
    let mut slots = Vec::new();
    for &k in ks {
        for hyp in XsHypothesis::ALL {
            for &h in hs {
                slots.push(Slot(k, hyp, h, None, None, Default::default()));
            }
        }
    }
    let mut recs = Vec::new();
    let is_close = config.funding_semantics == FundingTimestampSemantics::BarCloseSettlement;
    for (idx, slice) in slices.iter().enumerate() {
        let ts = slice.ts_ns.raw();
        let has_all = |w: &Weights| w.keys().all(|k| slice.candles.contains_key(k));
        for s in &mut slots {
            let (mut open_w, mut pw) = (None, BTreeMap::new());
            if let Some(mut pos) = s.3.take() {
                let accrue = pos.2 != idx || !is_close;
                for (sym, (_, lc, f_sum)) in &mut pos.4 {
                    if let Some(c) = slice.candles.get(sym) {
                        *lc = c.close.as_f64();
                        if accrue {
                            *f_sum += c.funding_rate_bps.raw();
                        }
                    }
                }
                if pos.2 == idx {
                    let roll = s.4.take().filter(|(_, w)| has_all(w));
                    if roll.is_some() {
                        pw = pos.3.clone();
                    }
                    settle_pos(s, pos, slice, false, roll.is_some(), costs, &mut recs)?;
                    open_w = roll;
                } else {
                    s.3 = Some(pos);
                }
            } else {
                open_w = s.4.take().filter(|(_, w)| has_all(w));
            }
            if let Some((dt, w)) = open_w {
                let (mut legs, mut ti) = (BTreeMap::new(), 0.0);
                for (&sym, &wt) in &w {
                    let Some(c) = slice.candles.get(&sym) else {
                        return Err(DomainError::InvalidCandle("entry".into()));
                    };
                    let mut f0 = 0.0;
                    if is_close {
                        f0 = c.funding_rate_bps.raw();
                    }
                    legs.insert(sym, (c.open.as_f64(), c.close.as_f64(), f0));
                    ti += (wt - pw.get(&sym).copied().unwrap_or(0.0)).abs();
                }
                for (k, &v) in &pw {
                    if !w.contains_key(k) {
                        ti += v.abs();
                    }
                }
                s.3 = Some(ActivePos(dt, ts, idx + s.2, w, legs, ti));
            }
        }
        let scores = engine.on_slice(slice)?;
        for s in &mut slots {
            s.4 = None;
            if s.3.as_ref().is_none_or(|p| p.2 == idx + 1) && scores.has_benchmark {
                s.4 = build_dollar_neutral_weights(scores.scores_for(s.1), s.0)?.map(|w| (ts, w));
            }
        }
    }
    if let Some(last) = slices.last() {
        for s in &mut slots {
            if let Some(pos) = s.3.take() {
                settle_pos(s, pos, last, true, false, costs, &mut recs)?;
            }
        }
    }
    let (single_k, m_hyp) = (ks.len() == 1, slots.len());
    let (mut vip0_in, mut maker_in, mut diags) = (vec![], vec![], vec![]);
    for s in &mut slots {
        let mut hyp = s.1.as_str().to_string();
        if !single_k {
            hyp = format!("K{}:{hyp}", s.0);
        }
        let hor = format!("{}b", s.2);
        diags.push(XsCellDiagnostics {
            top_k: s.0,
            hypothesis: hyp.clone(),
            horizon: hor.clone(),
            horizon_bars: s.2,
            mean_price_bps: welford_mean(&s.5[0])?,
            mean_fund_bps: welford_mean(&s.5[1])?,
            mean_turnover: welford_mean(&s.5[2])?,
            mean_cost_bps: welford_mean(&s.5[3])?,
            ann_sharpe: None,
            max_drawdown_bps: max_dd(&s.5[4]),
            ann_sharpe_maker: None,
            max_drawdown_maker_bps: max_dd(&s.5[5]),
        });
        let cell = |returns_bps| HypothesisCellInput {
            hypothesis: hyp.clone(),
            horizon: hor.clone(),
            returns_bps,
        };
        vip0_in.push(cell(core::mem::take(&mut s.5[4])));
        maker_in.push(cell(core::mem::take(&mut s.5[5])));
    }
    let v0_res = evaluate_hypothesis_batch(&vip0_in, &vip0_taker)?;
    let mk_res = evaluate_hypothesis_batch(&maker_in, &maker_diag)?;
    let ann = |c: &CellEvaluation, sc: f64| c.sr_hat.map(|sr| sr * sc).filter(|v| v.is_finite());
    for (i, d) in diags.iter_mut().enumerate() {
        let sc = (bpy / (d.horizon_bars as f64)).sqrt();
        d.ann_sharpe = ann(&v0_res.cells[i], sc);
        d.ann_sharpe_maker = ann(&mk_res.cells[i], sc);
    }
    let out = XsFalsificationReport {
        top_ks: ks.clone(),
        slippage_bps: slip,
        exclude_benchmark_from_trading: config.signal.exclude_benchmark_from_trading,
        slices_processed: slices.len() as u64,
        report: FalsificationReport {
            track: "Track3-CrossSectional".to_string(),
            events_processed,
            m_hypotheses: m_hyp,
            t_crit: v0_res.t_crit,
            sr_cross_std_vip0: v0_res.sr_cross_std,
            sr_cross_std_diag: mk_res.sr_cross_std,
            vip0_cells: v0_res.cells,
            diag_zero_cells: mk_res.cells,
        },
        diagnostics: diags,
    };
    Ok((out, recs))
}

pub fn run_xs_falsification<R: CandleSeriesReader>(
    reader: &mut R,
    config: &XsFalsifierConfig,
) -> Result<XsFalsificationReport, DomainError> {
    run_xs_falsification_detailed(reader, config).map(|(r, _)| r)
}
