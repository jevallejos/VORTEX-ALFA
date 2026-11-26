use std::collections::BTreeMap;

use crate::domain::error::DomainError;
use crate::domain::types::{
    BookTop, PriceE8, Side, SymbolId, TapeEvent, TapeEventKind, TimestampNs, BPS_PER_UNIT, SCALE_E8,
};

fn chk(ok: bool, p: &'static str) -> Result<(), DomainError> {
    ok.then_some(()).ok_or(DomainError::InvalidStatParam(p))
}
fn pos(v: f64, p: &'static str) -> Result<(), DomainError> {
    chk(v.is_finite() && v > 0.0, p)
}
fn fin(ok: bool, f: &'static str) -> Result<(), DomainError> {
    ok.then_some(()).ok_or(DomainError::NonFiniteValue(f))
}
fn gcd_pos(acc: &mut Option<i64>, d: i64) {
    if d > 0 {
        let (mut x, mut y) = (acc.unwrap_or(d).unsigned_abs(), d.unsigned_abs());
        while y != 0 {
            (x, y) = (y, x % y);
        }
        *acc = Some((x.min(i64::MAX as u64) as i64).max(1));
    }
}
fn l1_delta(up: bool, dn: bool, q: f64, pq: f64) -> f64 {
    let d = if dn { -pq } else { q - pq };
    if up {
        return q;
    }
    d
}
fn round_px(px: f64, tick: i64, is_ceil: bool) -> i64 {
    let u = (px * (SCALE_E8 as f64)) / (tick as f64) + if is_ceil { -1e-9 } else { 1e-9 };
    (if is_ceil { u.ceil() } else { u.floor() } as i64).saturating_mul(tick)
}
type FlowAcc = (Option<i64>, f64, f64);
fn flow_step(f: &mut FlowAcc, ts: i64, tau_s: f64, delta: Option<(f64, f64)>) -> f64 {
    if let Some((nd, dd)) = delta {
        let dt = f.0.map_or(0.0, |p| ((ts - p) as f64 * 1e-9).max(0.0)); // [s]
        let k = (-dt / tau_s).exp();
        (f.1, f.2, f.0) = (f.1 * k + nd, f.2 * k + dd, Some(ts));
    }
    if f.2 > 1e-12 {
        return (f.1 / f.2).clamp(-1.0, 1.0);
    }
    0.0
}

/// Causal market-making estimator configuration.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct MmEstimatorConfig {
    pub tau_ofi_s: f64,    // [s] OFI exponential decay constant
    pub tau_tfi_s: f64,    // [s] TFI exponential decay constant
    pub tau_vol_s: f64,    // [s] EWMA variance decay constant
    pub min_sigma: f64,    // [frac / sqrt(s)] lower volatility floor
    pub max_sigma: f64,    // [frac / sqrt(s)] upper volatility cap
    pub max_inst_var: f64, // [frac^2 / s] per-update variance cap
    pub min_book_updates: usize,
}
impl Default for MmEstimatorConfig {
    fn default() -> Self {
        Self {
            tau_ofi_s: 2.0,
            tau_tfi_s: 5.0,
            tau_vol_s: 30.0,
            min_sigma: 1e-6,
            max_sigma: 0.05,
            max_inst_var: 0.05 * 0.05,
            min_book_updates: 20,
        }
    }
}
impl MmEstimatorConfig {
    pub fn validate(&self) -> Result<(), DomainError> {
        pos(self.tau_ofi_s, "tau_ofi_s")?;
        pos(self.tau_tfi_s, "tau_tfi_s")?;
        pos(self.tau_vol_s, "tau_vol_s")?;
        pos(self.min_sigma, "min_sigma")?;
        pos(self.max_sigma, "max_sigma")?;
        chk(self.max_sigma >= self.min_sigma, "max_sigma")?;
        pos(self.max_inst_var, "max_inst_var")?;
        chk(self.min_book_updates >= 1, "min_book_updates")
    }
}

/// Avellaneda-Stoikov quoting parameters in homogeneous [bps] units.
#[derive(Clone, Debug, PartialEq)]
pub struct MmQuoteParams {
    pub name: String,
    pub gamma: f64,               // [bps^-1] inventory risk aversion
    pub theta_skew_bps: f64,      // [bps] alpha skew weight
    pub min_half_spread_bps: f64, // [bps] minimum half-spread floor
    pub horizon_s: f64,           // [s] inventory horizon
    pub k_arrival: f64,           // [bps^-1] order arrival decay intensity
    pub clip_usdt: f64,           // [USDT] notional size per unit clip
    pub max_inv_clips: f64,       // [clips] hard inventory bound
    pub tick_e8_override: Option<i64>,
}
impl MmQuoteParams {
    pub fn validate(&self) -> Result<(), DomainError> {
        let (t, m) = (self.theta_skew_bps, self.min_half_spread_bps);
        chk(!self.name.trim().is_empty(), "mm_name")?;
        pos(self.gamma, "gamma")?;
        chk(t.is_finite() && t >= 0.0, "theta_skew_bps")?;
        chk(m.is_finite() && m >= 0.0, "min_half_spread_bps")?;
        pos(self.horizon_s, "horizon_s")?;
        pos(self.k_arrival, "k_arrival")?;
        pos(self.clip_usdt, "clip_usdt")?;
        pos(self.max_inv_clips, "max_inv_clips")?;
        chk(self.tick_e8_override.unwrap_or(1) > 0, "tick_e8_override")
    }
}
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct MmMarketFeatures {
    pub symbol_id: SymbolId,
    pub ts_ns: TimestampNs,
    pub s_mid: f64,            // [USDT] L1 mid price
    pub s_micro: f64,          // [USDT] Stoikov L1 microprice
    pub ofi_norm: f64,         // [1] normalized causal OFI in [-1, 1]
    pub tfi_norm: f64,         // [1] normalized causal TFI in [-1, 1]
    pub alpha_skew: f64,       // [1] combined skew in [-1, 1]
    pub sigma_per_sqrt_s: f64, // [frac / sqrt(s)] EWMA volatility
    pub tick_e8: i64,
    pub book_updates: usize,
    pub is_warm: bool,
}
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct MmQuoteTarget {
    pub reservation_px: f64,  // [USDT] reservation price
    pub delta_half_bps: f64,  // [bps] optimal half-spread
    pub delta_half_frac: f64, // [frac] optimal fractional half-spread
    pub bid_target: Option<PriceE8>,
    pub ask_target: Option<PriceE8>,
    pub tick_e8: i64,
}

/// Stoikov L1 imbalance-weighted microprice: (ask * bq + bid * aq) / (bq + aq).
#[must_use]
pub fn stoikov_microprice(book: BookTop) -> f64 {
    let (bid, ask) = (book.bid().as_f64(), book.ask().as_f64());
    let (bq, aq) = (book.bid_qty().as_f64(), book.ask_qty().as_f64());
    let d = bq + aq;
    if d > 0.0 && d.is_finite() {
        return (ask * bq + bid * aq) / d;
    }
    book.mid_f64()
}
#[derive(Copy, Clone, Debug, Default)]
struct MmSymbolState {
    prev_ts_ns: Option<i64>,
    prev_book: Option<BookTop>,
    ofi: FlowAcc,
    tfi: FlowAcc,
    prev_mid: Option<(f64, i64)>,
    ewma_var: f64, // [frac^2 / s]
    vol_init: bool,
    tick_e8: Option<i64>,
    book_updates: usize,
}

/// Stateful per-symbol causal market-making feature engine.
#[derive(Clone, Debug)]
pub struct MmSignalEngine {
    config: MmEstimatorConfig,
    states: BTreeMap<SymbolId, MmSymbolState>,
}
impl MmSignalEngine {
    pub fn new(config: MmEstimatorConfig) -> Result<Self, DomainError> {
        config.validate()?;
        Ok(Self {
            config,
            states: BTreeMap::new(),
        })
    }
    pub fn on_event(&mut self, event: &TapeEvent) -> Result<MmMarketFeatures, DomainError> {
        let (b, ts) = (event.book, event.ts_ns.raw());
        let prev = self.states.get(&event.symbol_id).and_then(|s| s.prev_ts_ns);
        if let Some(prev_ns) = prev.filter(|&p| ts < p) {
            return Err(DomainError::NonMonotonicTimestamp {
                symbol_id: event.symbol_id.0,
                prev_ns,
                curr_ns: ts,
            });
        }
        let (s_mid, s_micro) = (b.mid_f64(), stoikov_microprice(b));
        let ok_px = s_mid.is_finite() && s_mid > 0.0 && s_micro.is_finite() && s_micro > 0.0;
        fin(ok_px, "mm_book_px")?;
        let c = &self.config;
        let st = self.states.entry(event.symbol_id).or_default();
        st.prev_ts_ns = Some(ts);
        let (be, ae) = (b.bid().raw(), b.ask().raw());
        // Causal tick inference & OFI strictly from event.book (zero external fields).
        let mut ofi_d = None;
        if st.prev_book != Some(b) || event.kind == TapeEventKind::BookSnapshot {
            st.book_updates += 1;
            gcd_pos(&mut st.tick_e8, ae.saturating_sub(be));
            ofi_d = Some((0.0, 0.0));
            if let Some(p) = st.prev_book {
                let (pb, pa) = (p.bid().raw(), p.ask().raw());
                gcd_pos(&mut st.tick_e8, be.abs_diff(pb) as i64);
                gcd_pos(&mut st.tick_e8, ae.abs_diff(pa) as i64);
                let (bq, aq) = (b.bid_qty().as_f64(), b.ask_qty().as_f64());
                let (pbq, paq) = (p.bid_qty().as_f64(), p.ask_qty().as_f64());
                let eb = l1_delta(be > pb, be < pb, bq, pbq);
                let ea = -l1_delta(ae < pa, ae > pa, aq, paq);
                ofi_d = Some((eb + ea, eb.abs() + ea.abs()));
            }
            st.prev_book = Some(b);
        }
        let tick_e8 = st.tick_e8.unwrap_or(1).max(1);
        let ofi_norm = flow_step(&mut st.ofi, ts, c.tau_ofi_s, ofi_d);
        // Causal TFI strictly from TapeEventKind::Trade events.
        let mut tfi_d = None;
        if let (TapeEventKind::Trade, Some(q)) = (event.kind, event.trade_qty) {
            let (qty, side): (f64, Side) = (q.as_f64(), event.side);
            tfi_d = Some((side.sign_f64() * qty, qty));
        }
        let tfi_norm = flow_step(&mut st.tfi, ts, c.tau_tfi_s, tfi_d);
        let alpha_skew = (0.5 * ofi_norm + 0.5 * tfi_norm).clamp(-1.0, 1.0);
        // Causal EWMA volatility [frac / sqrt(s)] over mid-price displacements.
        let (pm, pts) = st.prev_mid.unwrap_or((s_mid, ts));
        if (s_mid - pm).abs() > 0.0 && ts > pts {
            let (dt, r) = (((ts - pts) as f64) * 1e-9, (s_mid / pm).ln()); // [s]
            let iv = ((r * r) / dt.max(1e-3)).min(c.max_inst_var);
            let k = (-dt / c.tau_vol_s).exp();
            let w = if st.vol_init { 1.0 - k } else { 1.0 };
            st.ewma_var = (1.0 - w) * st.ewma_var + w * iv;
            st.vol_init = true;
        }
        if st.prev_mid.is_none() || (s_mid - pm).abs() > 0.0 {
            st.prev_mid = Some((s_mid, ts));
        }
        let cl = st.ewma_var.sqrt().clamp(c.min_sigma, c.max_sigma);
        let sig = if st.vol_init { cl } else { c.min_sigma };
        let ok = (ofi_norm + tfi_norm + alpha_skew + sig).is_finite();
        fin(ok, "mm_features")?;
        Ok(MmMarketFeatures {
            symbol_id: event.symbol_id,
            ts_ns: event.ts_ns,
            s_mid,
            s_micro,
            ofi_norm,
            tfi_norm,
            alpha_skew,
            sigma_per_sqrt_s: sig,
            tick_e8,
            book_updates: st.book_updates,
            is_warm: st.book_updates >= c.min_book_updates,
        })
    }
    pub fn quote_for_event(
        &self,
        event: &TapeEvent,
        features: &MmMarketFeatures,
        inventory_notional_usdt: f64,
        params: &MmQuoteParams,
    ) -> Result<Option<MmQuoteTarget>, DomainError> {
        let (b, f, inv) = (event.book, features, inventory_notional_usdt);
        let r = f.is_warm.then(|| compute_as_quote(b, f, inv, params));
        r.transpose()
    }
}

/// Avellaneda-Stoikov optimal quote computation in homogeneous [bps] units with PostOnly guard.
pub fn compute_as_quote(
    book: BookTop,
    features: &MmMarketFeatures,
    inventory_notional_usdt: f64,
    params: &MmQuoteParams,
) -> Result<MmQuoteTarget, DomainError> {
    params.validate()?;
    let (p, inv, f) = (params, inventory_notional_usdt, features);
    let (s_micro, sig, g) = (f.s_micro, f.sigma_per_sqrt_s, p.gamma);
    fin(inv.is_finite(), "inventory_notional_usdt")?;
    fin(s_micro.is_finite() && s_micro > 0.0, "s_micro")?;
    fin(f.alpha_skew.is_finite(), "alpha_skew")?;
    fin(sig.is_finite() && sig > 0.0, "sigma_per_sqrt_s")?;
    let raw_q_clips = inv / p.clip_usdt; // [USDT] / [USDT/clip] = [clips]
    let q_clips = raw_q_clips.clamp(-p.max_inv_clips, p.max_inv_clips); // [clips]
    let sigma_bps = sig * BPS_PER_UNIT; // [bps / sqrt(s)]
    let inv_var_bps = g * sigma_bps * sigma_bps * p.horizon_s; // [bps^-1]*[bps^2/s]*[s] = [bps]
    let shift_bps = p.theta_skew_bps * f.alpha_skew - q_clips * inv_var_bps; // [bps]
    let res = s_micro * (1.0 + (shift_bps / BPS_PER_UNIT).clamp(-0.5, 0.5));
    let as_h = 0.5 * inv_var_bps + (1.0 + g / p.k_arrival).ln() / g; // [bps]
    let dh_bps = p.min_half_spread_bps.max(as_h); // [bps]
    fin((res + dh_bps).is_finite(), "mm_quote_math")?;
    let df = (dh_bps / BPS_PER_UNIT).min(0.5);
    let tick_e8 = p.tick_e8_override.unwrap_or(f.tick_e8).max(1);
    let (bb, ba) = (book.bid().raw(), book.ask().raw());
    let raw_b = round_px(res * (1.0 - df), tick_e8, false).min(ba.saturating_sub(tick_e8));
    let raw_a = round_px(res * (1.0 + df), tick_e8, true).max(bb.saturating_add(tick_e8));
    let mut bid_e8 = (raw_q_clips < p.max_inv_clips && raw_b > 0).then_some(raw_b);
    let mut ask_e8 = (raw_q_clips > -p.max_inv_clips && raw_a > 0).then_some(raw_a);
    if let (Some(b), Some(a)) = (bid_e8, ask_e8) {
        if b >= a {
            let (b2, a2) = (b.saturating_sub(tick_e8), a.saturating_add(tick_e8));
            let ok = b2 < ba && a2 > bb && b2 < a2;
            (bid_e8, ask_e8) = (ok.then_some(b2).filter(|&v| v > 0), ok.then_some(a2));
        }
    }
    Ok(MmQuoteTarget {
        reservation_px: res,
        delta_half_bps: dh_bps,
        delta_half_frac: df,
        bid_target: bid_e8.map(PriceE8::new).transpose()?,
        ask_target: ask_e8.map(PriceE8::new).transpose()?,
        tick_e8,
    })
}
