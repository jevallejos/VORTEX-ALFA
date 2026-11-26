use serde::{Deserialize, Serialize};

use crate::domain::error::DomainError;
use crate::domain::types::{BookTop, Bps, NotionalE8, PriceE8, Side, BPS_PER_UNIT};

/// Sub-bps integer scale: 1 bps = `FEE_BPS_SCALE` units (0.0001 bps resolution).
pub const FEE_BPS_SCALE: i128 = 10_000;

/// Exact denominator for converting `(notional_e8 * fee_bps_scaled)` back to 1e8 units.
/// Since `1 bps = 1e-4` and `fee_bps_scaled = fee_bps * 10_000`, `FEE_DENOM = 10_000 * 10_000 = 1e8`.
pub const FEE_DENOM: i128 = 10_000 * FEE_BPS_SCALE;

#[derive(Copy, Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FeeSchedule {
    pub name: &'static str,
    pub maker_bps: Bps,
    pub taker_bps: Bps,
    pub is_binding_for_verdict: bool,
}

impl FeeSchedule {
    /// Binding production schedule: Bybit VIP-0 (Maker 2.00 bps, Taker 5.50 bps per leg).
    #[must_use]
    pub fn bybit_vip0() -> Self {
        Self {
            name: "VIP-0",
            maker_bps: Bps::new(2.00).unwrap_or(Bps::ZERO),
            taker_bps: Bps::new(5.50).unwrap_or(Bps::ZERO),
            is_binding_for_verdict: true,
        }
    }

    /// Non-binding diagnostic schedule: Maker 0.00 bps, Taker 2.00 bps per leg.
    /// Can NEVER produce `GateVerdict::Pass`.
    #[must_use]
    pub fn diag_zero_maker() -> Self {
        Self {
            name: "DiagZero",
            maker_bps: Bps::ZERO,
            taker_bps: Bps::new(2.00).unwrap_or(Bps::ZERO),
            is_binding_for_verdict: false,
        }
    }

    pub fn custom(
        name: &'static str,
        maker_bps: Bps,
        taker_bps: Bps,
        is_binding_for_verdict: bool,
    ) -> Result<Self, DomainError> {
        if maker_bps.raw() < 0.0 || taker_bps.raw() < 0.0 {
            return Err(DomainError::InvalidFeeBps);
        }
        Ok(Self {
            name,
            maker_bps,
            taker_bps,
            is_binding_for_verdict,
        })
    }

    #[must_use]
    pub const fn leg_bps(&self, is_taker: bool) -> Bps {
        if is_taker {
            self.taker_bps
        } else {
            self.maker_bps
        }
    }
}

/// Conservative integer fee debit per leg using `ceil` rounding in `i128`:
/// `ceil((notional_e8 * round(fee_bps * 10_000)) / 100_000_000)`.
pub fn leg_fee_e8(notional: NotionalE8, fee_bps: Bps) -> Result<i128, DomainError> {
    let raw_bps = fee_bps.raw();
    if !raw_bps.is_finite() || raw_bps < 0.0 {
        return Err(DomainError::InvalidFeeBps);
    }
    let scaled_f = (raw_bps * (FEE_BPS_SCALE as f64)).round();
    if !scaled_f.is_finite() || scaled_f > (i64::MAX as f64) {
        return Err(DomainError::ArithmeticOverflow("fee_bps_scale"));
    }
    let fee_bps_scaled = scaled_f as i128;
    if fee_bps_scaled == 0 || notional.raw() == 0 {
        return Ok(0);
    }
    let numer = notional
        .raw()
        .checked_mul(fee_bps_scaled)
        .ok_or(DomainError::ArithmeticOverflow("leg_fee_mul"))?;
    let rounded = numer
        .checked_add(FEE_DENOM - 1)
        .ok_or(DomainError::ArithmeticOverflow("leg_fee_ceil"))?;
    Ok(rounded / FEE_DENOM)
}

#[must_use]
pub const fn executable_entry_price(side: Side, book: BookTop) -> PriceE8 {
    match side {
        Side::Long => book.ask(),
        Side::Short => book.bid(),
    }
}

#[must_use]
pub const fn executable_exit_price(side: Side, book: BookTop) -> PriceE8 {
    match side {
        Side::Long => book.bid(),
        Side::Short => book.ask(),
    }
}

/// Canonical net return in bps:
/// `d * ((exit_px - entry_px) / entry_px) * 10_000 - fee_entry_bps - fee_exit_bps - funding_cost_bps`.
pub fn net_return_bps(
    side: Side,
    entry_px: PriceE8,
    exit_px: PriceE8,
    fee_entry_bps: Bps,
    fee_exit_bps: Bps,
    funding_cost_bps: Bps,
) -> Result<Bps, DomainError> {
    if fee_entry_bps.raw() < 0.0 || fee_exit_bps.raw() < 0.0 {
        return Err(DomainError::InvalidFeeBps);
    }
    let entry_f = entry_px.as_f64();
    let exit_f = exit_px.as_f64();
    let gross_bps = side.sign_f64() * ((exit_f - entry_f) / entry_f) * BPS_PER_UNIT;
    let net_bps = gross_bps - fee_entry_bps.raw() - fee_exit_bps.raw() - funding_cost_bps.raw();
    Bps::new(net_bps)
}

pub fn book_roundtrip_net_bps(
    side: Side,
    entry_book: BookTop,
    exit_book: BookTop,
    schedule: &FeeSchedule,
    entry_is_taker: bool,
    exit_is_taker: bool,
    funding_cost_bps: Bps,
) -> Result<Bps, DomainError> {
    let entry_px = executable_entry_price(side, entry_book);
    let exit_px = executable_exit_price(side, exit_book);
    net_return_bps(
        side,
        entry_px,
        exit_px,
        schedule.leg_bps(entry_is_taker),
        schedule.leg_bps(exit_is_taker),
        funding_cost_bps,
    )
}

#[allow(clippy::too_many_arguments)]
pub fn candle_roundtrip_net_bps(
    side: Side,
    entry_open: PriceE8,
    exit_close: PriceE8,
    slippage_per_leg_bps: Bps,
    schedule: &FeeSchedule,
    entry_is_taker: bool,
    exit_is_taker: bool,
    funding_cost_bps: Bps,
) -> Result<Bps, DomainError> {
    if slippage_per_leg_bps.raw() < 0.0 {
        return Err(DomainError::InvalidFeeBps);
    }
    let s = slippage_per_leg_bps.raw() / BPS_PER_UNIT;
    let d = side.sign_f64();
    let eff_entry = entry_open.as_f64() * (1.0 + d * s);
    let eff_exit = exit_close.as_f64() * (1.0 - d * s);
    if !eff_entry.is_finite() || !eff_exit.is_finite() || eff_entry <= 0.0 || eff_exit <= 0.0 {
        return Err(DomainError::NonFiniteValue("effective_candle_fill_price"));
    }
    let gross_bps = d * ((eff_exit - eff_entry) / eff_entry) * BPS_PER_UNIT;
    let fee_e = schedule.leg_bps(entry_is_taker).raw();
    let fee_x = schedule.leg_bps(exit_is_taker).raw();
    Bps::new(gross_bps - fee_e - fee_x - funding_cost_bps.raw())
}
