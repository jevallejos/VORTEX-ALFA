use crate::domain::error::DomainError;

/// Exact fixed-point 1e8 integer equity ledger in `i128` guaranteeing zero drift.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct EquityLedgerE8 {
    initial_equity_e8: i128,
    current_equity_e8: i128,
    cumulative_gross_pnl_e8: i128,
    cumulative_fees_e8: i128,
    cumulative_funding_e8: i128,
    trade_count: u64,
}

impl EquityLedgerE8 {
    pub fn new(initial_equity_e8: i128) -> Result<Self, DomainError> {
        if initial_equity_e8 < 0 {
            return Err(DomainError::InvalidNotional(initial_equity_e8));
        }
        Ok(Self {
            initial_equity_e8,
            current_equity_e8: initial_equity_e8,
            cumulative_gross_pnl_e8: 0,
            cumulative_fees_e8: 0,
            cumulative_funding_e8: 0,
            trade_count: 0,
        })
    }

    /// Records a closed trade with checked `i128` arithmetic.
    /// `net = gross_pnl_e8 - entry_fee_e8 - exit_fee_e8 - funding_cost_e8`.
    pub fn record_trade(
        &mut self,
        gross_pnl_e8: i128,
        entry_fee_e8: i128,
        exit_fee_e8: i128,
        funding_cost_e8: i128,
    ) -> Result<i128, DomainError> {
        if entry_fee_e8 < 0 || exit_fee_e8 < 0 {
            return Err(DomainError::InvalidFeeBps);
        }
        let total_fee = entry_fee_e8
            .checked_add(exit_fee_e8)
            .ok_or(DomainError::ArithmeticOverflow("ledger_total_fee"))?;
        let net_e8 = gross_pnl_e8
            .checked_sub(total_fee)
            .and_then(|v| v.checked_sub(funding_cost_e8))
            .ok_or(DomainError::ArithmeticOverflow("ledger_net_pnl"))?;
        self.current_equity_e8 = self
            .current_equity_e8
            .checked_add(net_e8)
            .ok_or(DomainError::ArithmeticOverflow("ledger_current_equity"))?;
        self.cumulative_gross_pnl_e8 = self
            .cumulative_gross_pnl_e8
            .checked_add(gross_pnl_e8)
            .ok_or(DomainError::ArithmeticOverflow("ledger_cum_gross"))?;
        self.cumulative_fees_e8 = self
            .cumulative_fees_e8
            .checked_add(total_fee)
            .ok_or(DomainError::ArithmeticOverflow("ledger_cum_fees"))?;
        self.cumulative_funding_e8 = self
            .cumulative_funding_e8
            .checked_add(funding_cost_e8)
            .ok_or(DomainError::ArithmeticOverflow("ledger_cum_funding"))?;
        self.trade_count = self
            .trade_count
            .checked_add(1)
            .ok_or(DomainError::ArithmeticOverflow("ledger_trade_count"))?;
        Ok(net_e8)
    }

    /// Accounting invariant: `current - (initial + cum_gross - cum_fees - cum_funding)`.
    pub fn drift_e8(&self) -> Result<i128, DomainError> {
        let expected = self
            .initial_equity_e8
            .checked_add(self.cumulative_gross_pnl_e8)
            .and_then(|v| v.checked_sub(self.cumulative_fees_e8))
            .and_then(|v| v.checked_sub(self.cumulative_funding_e8))
            .ok_or(DomainError::ArithmeticOverflow("ledger_drift_expected"))?;
        self.current_equity_e8
            .checked_sub(expected)
            .ok_or(DomainError::ArithmeticOverflow("ledger_drift_sub"))
    }

    #[must_use]
    pub const fn initial_equity_e8(&self) -> i128 {
        self.initial_equity_e8
    }

    #[must_use]
    pub const fn current_equity_e8(&self) -> i128 {
        self.current_equity_e8
    }

    #[must_use]
    pub const fn cumulative_gross_pnl_e8(&self) -> i128 {
        self.cumulative_gross_pnl_e8
    }

    #[must_use]
    pub const fn cumulative_fees_e8(&self) -> i128 {
        self.cumulative_fees_e8
    }

    #[must_use]
    pub const fn cumulative_funding_e8(&self) -> i128 {
        self.cumulative_funding_e8
    }

    #[must_use]
    pub const fn trade_count(&self) -> u64 {
        self.trade_count
    }
}
