use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::domain::{
    error::DomainError,
    signals::{macro_donchian::*, macro_trend::*, spot_perp_carry::*},
    stats::{newey_west, welford_mean, FalsificationReport, Moments4},
    types::*,
};

const PCT_SCALE: f64 = 100.0;
const MIN_CALMAR_DRAWDOWN_PCT: f64 = 1e-9;

#[derive(Clone, Debug)]
pub struct MacroDailySlice {
    pub ts_ns: TimestampNs,
    pub linear: BTreeMap<SymbolId, Candle>,
    pub spot: BTreeMap<SymbolId, Candle>,
}

#[derive(Clone, Debug)]
pub struct MacroConfig {
    pub trend_vol: TrendVolConfig,
    pub trend_strategies: Vec<MacroTrendParams>,
    pub donchian_strategies: Vec<DonchianParams>,
    pub carry_strategies: Vec<SpotPerpCarryParams>,
    pub slippage_bps: Bps,
    pub funding_semantics: FundingTimestampSemantics,
    pub days_per_year: f64,
    pub benchmark_symbol: SymbolId,
}

impl Default for MacroConfig {
    fn default() -> Self {
        Self {
            trend_vol: TrendVolConfig::default(),
            trend_strategies: default_trend_params().to_vec(),
            donchian_strategies: default_donchian_params().to_vec(),
            carry_strategies: default_carry_params().to_vec(),
            slippage_bps: Bps::new(1.5).unwrap_or(Bps::ZERO),
            funding_semantics: FundingTimestampSemantics::BarOpenSettlement,
            days_per_year: CRYPTO_DAYS_PER_YEAR,
            benchmark_symbol: SymbolId(0),
        }
    }
}

impl MacroConfig {
    pub fn validate(&self) -> Result<(), DomainError> {
        self.trend_vol.validate()?;
        self.trend_strategies
            .iter()
            .try_for_each(MacroTrendParams::validate)?;
        self.donchian_strategies
            .iter()
            .try_for_each(DonchianParams::validate)?;
        self.carry_strategies
            .iter()
            .try_for_each(SpotPerpCarryParams::validate)?;
        let count = self.trend_strategies.len()
            + self.donchian_strategies.len()
            + self.carry_strategies.len();
        if count == 0 || !self.days_per_year.is_finite() || self.days_per_year <= 0.0 {
            return Err(DomainError::InvalidStatParam("macro_config"));
        }
        if !self.slippage_bps.raw().is_finite() || self.slippage_bps.raw() < 0.0 {
            return Err(DomainError::InvalidFeeBps);
        }
        Ok(())
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum MacroFamily {
    VolTargetTrend,
    DonchianBreakout,
    SpotPerpCarry,
    PassiveBaseline,
}

#[derive(Copy, Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EquityCurveMetrics {
    pub final_equity: f64,
    pub cagr_pct: Option<f64>,
    pub ann_sharpe: Option<f64>,
    pub max_drawdown_pct: f64,
    pub calmar: Option<f64>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MacroStrategyRow {
    pub strategy: String,
    pub family: MacroFamily,
    pub days_n: usize,
    pub mean_price_or_basis_bps: Option<f64>,
    pub mean_fund_bps: Option<f64>,
    pub avg_daily_turnover: Option<f64>,
    pub mean_cost_vip0_bps: Option<f64>,
    pub mean_net_vip0_bps: Option<f64>,
    pub mean_net_maker_bps: Option<f64>,
    pub vip0_metrics: EquityCurveMetrics,
    pub maker_metrics: EquityCurveMetrics,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MacroBaselineRow {
    pub name: String,
    pub family: MacroFamily,
    pub days_n: usize,
    pub mean_price_bps: Option<f64>,
    pub avg_daily_turnover: Option<f64>,
    pub mean_net_vip0_bps: Option<f64>,
    pub mean_net_maker_bps: Option<f64>,
    pub t_nw_vip0: Option<f64>,
    pub vip0_metrics: EquityCurveMetrics,
    pub maker_metrics: EquityCurveMetrics,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MacroFalsificationReport {
    pub slippage_bps: f64,
    pub days_per_year: f64,
    pub slices_processed: u64,
    pub linear_candles_processed: u64,
    pub spot_candles_processed: u64,
    pub report: FalsificationReport,
    pub diagnostics: Vec<MacroStrategyRow>,
    pub baselines: Vec<MacroBaselineRow>,
}

pub fn compute_equity_metrics(
    daily_returns_bps: &[f64],
    days_per_year: f64,
) -> Result<EquityCurveMetrics, DomainError> {
    if !days_per_year.is_finite() || days_per_year <= 0.0 {
        return Err(DomainError::InvalidStatParam("days_per_year"));
    }
    let (mut equity, mut peak_equity, mut max_drawdown) = (1.0_f64, 1.0_f64, 0.0_f64);
    for &ret_bps in daily_returns_bps {
        if !ret_bps.is_finite() {
            return Err(DomainError::NonFiniteValue("equity_return_bps"));
        }
        equity = (equity * (1.0 + ret_bps / BPS_PER_UNIT)).max(0.0);
        peak_equity = peak_equity.max(equity);
        max_drawdown = max_drawdown.max((peak_equity - equity) / peak_equity);
    }
    let max_drawdown_pct = max_drawdown * PCT_SCALE;
    let years = (daily_returns_bps.len() as f64) / days_per_year;
    let cagr_pct = (!daily_returns_bps.is_empty() && years > 0.0)
        .then(|| (equity.powf(1.0 / years) - 1.0) * PCT_SCALE)
        .filter(|value| value.is_finite());
    let ann_sharpe = Moments4::compute(daily_returns_bps)?
        .map(|moments| (moments.mean / moments.std_dev) * days_per_year.sqrt())
        .filter(|value| value.is_finite());
    let calmar = cagr_pct
        .filter(|_| max_drawdown_pct > MIN_CALMAR_DRAWDOWN_PCT)
        .map(|cagr| cagr / max_drawdown_pct)
        .filter(|value| value.is_finite());
    Ok(EquityCurveMetrics {
        final_equity: equity,
        cagr_pct,
        ann_sharpe,
        max_drawdown_pct,
        calmar,
    })
}

pub fn compute_passive_baselines(
    slices: &[MacroDailySlice],
    spot_vip0_cost_bps: f64,
    spot_maker_cost_bps: f64,
    days_per_year: f64,
    benchmark_symbol: SymbolId,
) -> Result<Vec<MacroBaselineRow>, DomainError> {
    if !days_per_year.is_finite() || days_per_year <= 0.0 {
        return Err(DomainError::InvalidStatParam("days_per_year"));
    }
    let valid_cost = |value: f64| value.is_finite() && value >= 0.0;
    if !valid_cost(spot_vip0_cost_bps) || !valid_cost(spot_maker_cost_bps) {
        return Err(DomainError::InvalidFeeBps);
    }
    let mut rows = Vec::with_capacity(2);
    for (name, only) in [
        ("Baseline_BuyHold_BTC", Some(benchmark_symbol)),
        ("Baseline_BuyHold_EW", None),
    ] {
        rows.push(run_baseline(
            slices,
            spot_vip0_cost_bps,
            spot_maker_cost_bps,
            days_per_year,
            name,
            only,
        )?);
    }
    Ok(rows)
}

#[derive(Copy, Clone, Debug)]
struct HeldSpotLeg {
    weight: f64,
    open: f64,
    close: f64,
}

struct MarkedSpotStep {
    gross_bps: f64,
    drifted: BTreeMap<SymbolId, f64>,
}

#[derive(Default)]
struct BaselineSeries {
    gross_price_bps: Vec<f64>,
    turnover: Vec<f64>,
    net_vip0_bps: Vec<f64>,
    net_maker_bps: Vec<f64>,
}

fn run_baseline(
    slices: &[MacroDailySlice],
    vip0_cost_bps: f64,
    maker_cost_bps: f64,
    days_per_year: f64,
    name: &str,
    only_symbol: Option<SymbolId>,
) -> Result<MacroBaselineRow, DomainError> {
    let mut series = BaselineSeries::default();
    let mut held: BTreeMap<SymbolId, HeldSpotLeg> = BTreeMap::new();
    let (mut active, mut pending_turnover) = (false, 0.0_f64);
    let mut push_day = |gross_bps: f64, turnover: f64| -> Result<(), DomainError> {
        let net_vip0_bps = gross_bps - turnover * vip0_cost_bps;
        let net_maker_bps = gross_bps - turnover * maker_cost_bps;
        if !gross_bps.is_finite() || !net_vip0_bps.is_finite() || !net_maker_bps.is_finite() {
            return Err(DomainError::NonFiniteValue("baseline_day_return"));
        }
        series.gross_price_bps.push(gross_bps);
        series.turnover.push(turnover);
        series.net_vip0_bps.push(net_vip0_bps);
        series.net_maker_bps.push(net_maker_bps);
        Ok(())
    };
    for (slice_idx, slice) in slices.iter().enumerate() {
        let eligible: BTreeMap<SymbolId, &Candle> = slice
            .spot
            .iter()
            .filter(|(&symbol_id, _)| only_symbol.is_none_or(|bench| bench == symbol_id))
            .map(|(&symbol_id, candle)| (symbol_id, candle))
            .collect();
        if !active && eligible.is_empty() {
            continue;
        }
        let mut drifted = BTreeMap::new();
        if active {
            let marked = mark_spot_step(&held, slice, false);
            drifted = marked.drifted;
            push_day(marked.gross_bps, pending_turnover)?;
        }
        active = true;
        let same = !held.is_empty()
            && held.len() == eligible.len()
            && held.keys().all(|sym| eligible.contains_key(sym));
        let next_weights = if same {
            pending_turnover = 0.0;
            drifted
        } else {
            let equal_weight = 1.0 / (eligible.len().max(1) as f64);
            let target: BTreeMap<SymbolId, f64> =
                eligible.keys().map(|&sym| (sym, equal_weight)).collect();
            pending_turnover = weight_turnover(&drifted, &target);
            target
        };
        held.clear();
        for (symbol_id, weight) in next_weights {
            if let Some(candle) = eligible.get(&symbol_id) {
                let leg = HeldSpotLeg {
                    weight,
                    open: candle.open.as_f64(),
                    close: candle.close.as_f64(),
                };
                held.insert(symbol_id, leg);
            }
        }
        if slice_idx + 1 == slices.len() {
            let eof = mark_spot_step(&held, slice, true);
            let eof_turn: f64 = eof.drifted.values().copied().map(f64::abs).sum();
            push_day(eof.gross_bps, pending_turnover + eof_turn)?;
        }
    }
    Ok(MacroBaselineRow {
        name: name.to_string(),
        family: MacroFamily::PassiveBaseline,
        days_n: series.net_vip0_bps.len(),
        mean_price_bps: welford_mean(&series.gross_price_bps)?,
        avg_daily_turnover: welford_mean(&series.turnover)?,
        mean_net_vip0_bps: welford_mean(&series.net_vip0_bps)?,
        mean_net_maker_bps: welford_mean(&series.net_maker_bps)?,
        t_nw_vip0: newey_west(&series.net_vip0_bps)?.map(|result| result.t_nw),
        vip0_metrics: compute_equity_metrics(&series.net_vip0_bps, days_per_year)?,
        maker_metrics: compute_equity_metrics(&series.net_maker_bps, days_per_year)?,
    })
}

fn mark_spot_step(
    held: &BTreeMap<SymbolId, HeldSpotLeg>,
    slice: &MacroDailySlice,
    is_eof: bool,
) -> MarkedSpotStep {
    let mut gross_return = 0.0_f64;
    let mut drifted = BTreeMap::new();
    for (&symbol_id, leg) in held {
        let exit = match slice.spot.get(&symbol_id) {
            Some(bar) if is_eof => bar.close.as_f64(),
            Some(bar) => bar.open.as_f64(),
            None => leg.close,
        };
        gross_return += leg.weight * (exit - leg.open) / leg.open;
        drifted.insert(symbol_id, leg.weight * (exit / leg.open));
    }
    let denom = 1.0 + gross_return;
    if denom > 0.0 {
        for weight in drifted.values_mut() {
            *weight /= denom;
        }
    } else {
        drifted.clear();
    }
    MarkedSpotStep {
        gross_bps: gross_return * BPS_PER_UNIT,
        drifted,
    }
}

pub fn weight_turnover(previous: &BTreeMap<SymbolId, f64>, next: &BTreeMap<SymbolId, f64>) -> f64 {
    let mut turnover = 0.0_f64;
    for (symbol_id, &next_weight) in next {
        turnover += (next_weight - previous.get(symbol_id).copied().unwrap_or(0.0)).abs();
    }
    for (symbol_id, &prev_weight) in previous {
        if !next.contains_key(symbol_id) {
            turnover += prev_weight.abs();
        }
    }
    turnover
}
