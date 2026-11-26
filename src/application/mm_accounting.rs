use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::application::mm_fifo_sim::MmFifoConfig;
use crate::domain::error::DomainError;
use crate::domain::fees::{leg_fee_e8, FEE_BPS_SCALE, FEE_DENOM};
use crate::domain::ledger::EquityLedgerE8;
use crate::domain::signals::market_making::{MmEstimatorConfig, MmQuoteParams};
use crate::domain::stats::{CellEvaluation, GateVerdict};
use crate::domain::types::{
    BookTop, Bps, NotionalE8, PriceE8, QtyE8, Side, SymbolId, BPS_PER_UNIT, SCALE_E8,
};

pub const FRONTIER_COUNT: usize = 5;
const SCALE_E8_F64: f64 = SCALE_E8 as f64;

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct MmFeeProfileSpec {
    pub name: &'static str,
    pub maker_bps: f64,
    pub is_binding: bool,
}

impl MmFeeProfileSpec {
    const fn new(name: &'static str, maker_bps: f64, is_binding: bool) -> Self {
        Self {
            name,
            maker_bps,
            is_binding,
        }
    }
}

/// Maker fee frontier profiles: only `VIP0_Maker` (+2.00 bps) can bind `GateVerdict::Pass`.
pub const MM_FEE_FRONTIER: [MmFeeProfileSpec; FRONTIER_COUNT] = [
    MmFeeProfileSpec::new("VIP0_Maker", 2.00, true),
    MmFeeProfileSpec::new("Venue_1bp", 1.00, false),
    MmFeeProfileSpec::new("Venue_0bp", 0.00, false),
    MmFeeProfileSpec::new("Rebate_05", -0.50, false),
    MmFeeProfileSpec::new("Rebate_10", -1.00, false),
];

#[derive(Clone, Debug)]
pub struct MmFalsifierConfig {
    pub capital_usdt: f64,
    pub clip_usdt: f64,
    pub max_inv_clips: f64,
    pub max_gross_leverage: f64,
    pub bucket_ns: i64,
    pub estimator: MmEstimatorConfig,
    pub fifo: MmFifoConfig,
    pub grid: Vec<MmQuoteParams>,
}

impl MmFalsifierConfig {
    pub fn validate(&self) -> Result<(), DomainError> {
        for (label, value) in [
            ("capital_usdt", self.capital_usdt),
            ("clip_usdt", self.clip_usdt),
            ("max_inv_clips", self.max_inv_clips),
            ("max_gross_leverage", self.max_gross_leverage),
        ] {
            if !value.is_finite() || value <= 0.0 {
                return Err(DomainError::InvalidStatParam(label));
            }
        }
        let equity_e8 = self.capital_usdt * SCALE_E8_F64;
        let max_gross_usdt = self.capital_usdt * self.max_gross_leverage;
        if !max_gross_usdt.is_finite() || !equity_e8.is_finite() || equity_e8 > 1e24 {
            return Err(DomainError::ArithmeticOverflow("initial_equity_e8"));
        }
        if self.bucket_ns <= 0 || self.grid.is_empty() {
            return Err(DomainError::InvalidStatParam("mm_config"));
        }
        self.estimator.validate()?;
        self.fifo.validate()?;
        self.grid.iter().try_for_each(MmQuoteParams::validate)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MmFeeFrontierPoint {
    pub profile: String,
    pub maker_fee_bps: f64,
    pub is_binding: bool,
    pub net_pnl_usdt: f64,
    pub mean_bucket_bps: Option<f64>,
    pub std_bucket_bps: Option<f64>,
    pub t_nw: Option<f64>,
    pub t_crit: f64,
    pub p_holm: Option<f64>,
    pub dsr: Option<f64>,
    pub n_min: Option<u64>,
    pub max_drawdown_bps: f64,
    pub verdict: GateVerdict,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MmConfigDiagnostics {
    pub config_name: String,
    pub gamma: f64,
    pub theta_skew_bps: f64,
    pub min_half_spread_bps: f64,
    pub horizon_s: f64,
    pub k_arrival: f64,
    pub buckets_n: usize,
    pub fills_total: u64,
    pub post_only_rejects: u64,
    pub adverse_in_flight_fills: u64,
    pub total_maker_volume_usdt: f64,
    pub gross_pnl_usdt: f64,
    pub net_pnl_vip0_usdt: f64,
    pub mean_spread_capture_bps: Option<f64>,
    pub mean_markout_1s_bps: Option<f64>,
    pub mean_markout_5s_bps: Option<f64>,
    pub mean_markout_15s_bps: Option<f64>,
    pub mean_lvr_5s_bps: Option<f64>,
    pub peak_gross_inv_usdt: f64,
    pub eof_liq_cost_usdt: f64,
    pub eof_liq_fee_usdt: f64,
    pub max_drawdown_bps: f64,
    pub breakeven_maker_bps: Option<f64>,
    pub accounting_drift_e8: i128,
    pub frontier: Vec<MmFeeFrontierPoint>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MmFalsificationReport {
    pub track: String,
    pub events_processed: u64,
    pub capital_usdt: f64,
    pub clip_usdt: f64,
    pub max_inv_clips: f64,
    pub max_gross_leverage: f64,
    pub bucket_s: f64,
    pub rtt_ms: f64,
    pub m_hypotheses: usize,
    pub t_crit: f64,
    pub sr_cross_std_vip0: Option<f64>,
    pub vip0_cells: Vec<CellEvaluation>,
    pub configs: Vec<MmConfigDiagnostics>,
}

#[derive(Copy, Clone, Debug, Default)]
struct SymbolPositionState {
    qty_e8: i128,
    open_cost_e8: i128,
    last_book: Option<BookTop>,
}

/// Exact `i128` fixed-point position, fee frontier, and bucket accounting ledger (`Drift == 0`).
#[derive(Clone, Debug)]
pub struct MmAccountLedger {
    pub(crate) initial_equity_e8: i128,
    pub(crate) ledger: EquityLedgerE8,
    cash_ex_fees_delta_e8: i128,
    pub(crate) total_maker_volume_e8: i128,
    cumulative_maker_fees_e8: [i128; FRONTIER_COUNT],
    pub(crate) eof_taker_fee_e8: i128,
    pub(crate) eof_spread_cost_e8: i128,
    pub(crate) prev_bucket_net_e8: [i128; FRONTIER_COUNT],
    pub(crate) bucket_returns_bps: [Vec<f64>; FRONTIER_COUNT],
    pub(crate) peak_gross_inv_usdt: f64,
    positions: BTreeMap<SymbolId, SymbolPositionState>,
}

impl MmAccountLedger {
    pub fn new(initial_equity_e8: i128) -> Result<Self, DomainError> {
        Ok(Self {
            initial_equity_e8,
            ledger: EquityLedgerE8::new(initial_equity_e8)?,
            cash_ex_fees_delta_e8: 0,
            total_maker_volume_e8: 0,
            cumulative_maker_fees_e8: [0; FRONTIER_COUNT],
            eof_taker_fee_e8: 0,
            eof_spread_cost_e8: 0,
            prev_bucket_net_e8: [0; FRONTIER_COUNT],
            bucket_returns_bps: Default::default(),
            peak_gross_inv_usdt: 0.0,
            positions: BTreeMap::new(),
        })
    }

    #[must_use]
    pub fn symbol_qty_e8(&self, symbol_id: SymbolId) -> i128 {
        self.positions.get(&symbol_id).map_or(0, |pos| pos.qty_e8)
    }

    pub fn update_book_and_gross(&mut self, symbol_id: SymbolId, book: BookTop) -> f64 {
        self.positions.entry(symbol_id).or_default().last_book = Some(book);
        let gross_inv_usdt: f64 = self
            .positions
            .values()
            .map(|pos| {
                ((pos.qty_e8.abs() as f64) / SCALE_E8_F64)
                    * pos.last_book.map_or(0.0, BookTop::mid_f64)
            })
            .sum();
        self.peak_gross_inv_usdt = self.peak_gross_inv_usdt.max(gross_inv_usdt);
        gross_inv_usdt
    }

    pub fn record_fill(
        &mut self,
        symbol_id: SymbolId,
        side: Side,
        fill_px: PriceE8,
        qty: QtyE8,
        taker_fee_bps: Option<Bps>,
    ) -> Result<(), DomainError> {
        let qty_i128 = i128::from(qty.raw());
        let dir = side.sign_i128();
        let px_i128 = i128::from(fill_px.raw());
        let pos = self.positions.entry(symbol_id).or_default();

        let (close_qty_e8, released_cost_e8) = if pos.qty_e8 != 0 && pos.qty_e8.signum() != dir {
            let pos_abs = pos.qty_e8.abs();
            let close_qty = pos_abs.min(qty_i128);
            let released = if close_qty == pos_abs {
                pos.open_cost_e8
            } else {
                ovf(pos.open_cost_e8.checked_mul(close_qty))? / pos_abs
            };
            (close_qty, released)
        } else {
            (0, 0)
        };

        let rem_qty_e8 = qty_i128 - close_qty_e8;
        let close_notional_e8 = ovf(px_i128.checked_mul(close_qty_e8))? / SCALE_E8;
        let rem_notional_e8 = ovf(px_i128.checked_mul(rem_qty_e8))? / SCALE_E8;
        let fill_notional_e8 = ovf(close_notional_e8.checked_add(rem_notional_e8))?;
        let gross_e8 =
            ovf(ovf((-dir).checked_mul(close_notional_e8))?.checked_sub(released_cost_e8))?;

        pos.qty_e8 = ovf(pos.qty_e8.checked_add(dir * qty_i128))?;
        let kept_cost = ovf(pos.open_cost_e8.checked_sub(released_cost_e8))?;
        pos.open_cost_e8 = ovf(kept_cost.checked_add(dir * rem_notional_e8))?;
        let cash_delta = &mut self.cash_ex_fees_delta_e8;
        *cash_delta = ovf(cash_delta.checked_sub(dir * fill_notional_e8))?;

        let fill_notional = NotionalE8::new(fill_notional_e8)?;
        let vip0_fee_e8 = if let Some(taker_bps) = taker_fee_bps {
            let fee_e8 = leg_fee_e8(fill_notional, taker_bps)?;
            self.eof_taker_fee_e8 = ovf(self.eof_taker_fee_e8.checked_add(fee_e8))?;
            fee_e8
        } else {
            let maker_vol = &mut self.total_maker_volume_e8;
            *maker_vol = ovf(maker_vol.checked_add(fill_notional_e8))?;
            for (idx, spec) in MM_FEE_FRONTIER.iter().enumerate() {
                let fee_e8 = signed_leg_fee_e8(fill_notional, spec.maker_bps)?;
                let acc = &mut self.cumulative_maker_fees_e8[idx];
                *acc = ovf(acc.checked_add(fee_e8))?;
            }
            signed_leg_fee_e8(fill_notional, MM_FEE_FRONTIER[0].maker_bps)?
        };
        let ledger = &mut self.ledger;
        let _ = ledger.record_trade(gross_e8, vip0_fee_e8, 0, 0)?;
        Ok(())
    }

    pub fn flush_bucket(&mut self) -> Result<(), DomainError> {
        let mut mtm_delta_e8 = self.cash_ex_fees_delta_e8;
        for pos in self.positions.values().filter(|pos| pos.qty_e8 != 0) {
            let book = pos
                .last_book
                .ok_or(DomainError::InvalidStatParam("mtm_missing_book"))?;
            let mark_e8 = ovf(pos.qty_e8.checked_mul(book_mid_e8(book)))? / SCALE_E8;
            mtm_delta_e8 = ovf(mtm_delta_e8.checked_add(mark_e8))?;
        }
        let init_f64 = self.initial_equity_e8 as f64;
        for idx in 0..FRONTIER_COUNT {
            let net_ex_eof = ovf(mtm_delta_e8.checked_sub(self.cumulative_maker_fees_e8[idx]))?;
            let net_delta_e8 = ovf(net_ex_eof.checked_sub(self.eof_taker_fee_e8))?;
            let bucket_delta_e8 = ovf(net_delta_e8.checked_sub(self.prev_bucket_net_e8[idx]))?;
            self.prev_bucket_net_e8[idx] = net_delta_e8;
            self.bucket_returns_bps[idx].push(((bucket_delta_e8 as f64) / init_f64) * BPS_PER_UNIT);
        }
        Ok(())
    }

    pub fn liquidate_eof(&mut self, taker_bps: Bps) -> Result<(), DomainError> {
        let open_positions: Vec<(SymbolId, i128, Option<BookTop>)> = self
            .positions
            .iter()
            .filter(|(_, pos)| pos.qty_e8 != 0)
            .map(|(&symbol_id, pos)| (symbol_id, pos.qty_e8, pos.last_book))
            .collect();
        for (symbol_id, pos_qty_e8, book_opt) in open_positions {
            let book = book_opt.ok_or(DomainError::InvalidStatParam("eof_missing_book"))?;
            let (liq_side, liq_px) = if pos_qty_e8 > 0 {
                (Side::Short, book.bid())
            } else {
                (Side::Long, book.ask())
            };
            let qty_raw = i64::try_from(pos_qty_e8.abs())
                .map_err(|_| DomainError::ArithmeticOverflow("eof_qty"))?;
            let qty = QtyE8::new(qty_raw)?;
            let spread_diff = (book_mid_e8(book) - i128::from(liq_px.raw())).abs();
            let spread_cost_e8 = ovf(pos_qty_e8.abs().checked_mul(spread_diff))? / SCALE_E8;
            self.eof_spread_cost_e8 = ovf(self.eof_spread_cost_e8.checked_add(spread_cost_e8))?;
            self.record_fill(symbol_id, liq_side, liq_px, qty, Some(taker_bps))?;
        }
        self.flush_bucket()
    }

    pub fn accounting_drift_e8(&self) -> Result<i128, DomainError> {
        let total_fees_e8 =
            ovf(self.cumulative_maker_fees_e8[0].checked_add(self.eof_taker_fee_e8))?;
        let net_cash_delta = ovf(self.cash_ex_fees_delta_e8.checked_sub(total_fees_e8))?;
        let cash_vip0_e8 = ovf(self.initial_equity_e8.checked_add(net_cash_delta))?;
        let open_cost_sum_e8: i128 = self.positions.values().map(|pos| pos.open_cost_e8).sum();
        Ok(self.ledger.drift_e8()?.abs()
            + (cash_vip0_e8 + open_cost_sum_e8 - self.ledger.current_equity_e8()).abs())
    }

    #[must_use]
    pub fn breakeven_maker_bps(&self) -> Option<f64> {
        (self.total_maker_volume_e8 > 0).then(|| {
            let gross_after_eof = self.ledger.cumulative_gross_pnl_e8() - self.eof_taker_fee_e8;
            ((gross_after_eof as f64) / (self.total_maker_volume_e8 as f64)) * BPS_PER_UNIT
        })
    }
}

pub fn signed_leg_fee_e8(notional: NotionalE8, fee_bps: f64) -> Result<i128, DomainError> {
    let bps = Bps::new(fee_bps)?;
    if fee_bps >= 0.0 {
        return leg_fee_e8(notional, bps);
    }
    let scaled_rebate_f = (-bps.raw() * (FEE_BPS_SCALE as f64)).round();
    if !scaled_rebate_f.is_finite() || scaled_rebate_f > (i64::MAX as f64) {
        return Err(DomainError::ArithmeticOverflow("maker_rebate_scale"));
    }
    Ok(-(ovf(notional.raw().checked_mul(scaled_rebate_f as i128))? / FEE_DENOM))
}

fn book_mid_e8(book: BookTop) -> i128 {
    (i128::from(book.bid().raw()) + i128::from(book.ask().raw())) / 2
}

fn ovf(val: Option<i128>) -> Result<i128, DomainError> {
    val.ok_or(DomainError::ArithmeticOverflow("mm_accounting"))
}
