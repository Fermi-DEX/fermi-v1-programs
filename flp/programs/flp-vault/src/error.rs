use anchor_lang::prelude::*;

#[error_code]
pub enum FlpError {
    #[msg("math overflow")]
    MathOverflow,
    #[msg("invalid vault parameters")]
    InvalidParams,
    #[msg("signer is not authorized for this action")]
    Unauthorized,
    #[msg("deposits are paused")]
    DepositsPaused,
    #[msg("withdrawals are paused")]
    WithdrawalsPaused,
    #[msg("circuit breaker is tripped")]
    BreakerTripped,
    #[msg("amount is below the minimum")]
    BelowMinimum,
    #[msg("deposit would exceed the vault TVL cap")]
    CapExceeded,
    #[msg("an earlier ticket must be claimed first")]
    TicketPending,
    #[msg("no ticket to act on")]
    NoTicket,
    #[msg("the ticket's epoch is past its request cutoff")]
    PastCutoff,
    #[msg("the ticket's epoch has not been settled yet")]
    NotSettled,
    #[msg("the current epoch has not ended")]
    EpochNotEnded,
    #[msg("pending-totals slot is still in use")]
    PendingSlotBusy,
    #[msg("vault has not been seeded")]
    NotSeeded,
    #[msg("vault is already seeded")]
    AlreadySeeded,
    #[msg("vault NAV is not positive")]
    VaultInsolvent,
    #[msg("vault FermiAccount is being liquidated")]
    BeingLiquidated,
    #[msg("FermiAccount or bank does not match the vault")]
    FermiAccountMismatch,
    #[msg("token account is not the expected canonical account")]
    WrongTokenAccount,
    #[msg("epoch record does not match")]
    WrongEpochRecord,
    #[msg("this batch is not the oldest deferred batch")]
    NotDeferredHead,
    #[msg("the deferred tail record must be supplied")]
    MissingDeferredTail,
    #[msg("withdrawal liquidity is still insufficient")]
    InsufficientLiquidity,
    #[msg("timelock has not elapsed")]
    TimelockActive,
    #[msg("nothing is pending")]
    NothingPending,
    #[msg("breaker conditions are not met")]
    BreakerNotTriggered,
    #[msg("instruction is not allowed in this vault mode")]
    WrongVaultMode,
    #[msg("recovery grace period has not elapsed")]
    RecoveryGraceActive,
    #[msg("supply or trading inventory is not ready for terminal redemption")]
    RecoveryNotComplete,
    #[msg("recovery market accounts or target do not match")]
    WrongRecoveryMarket,
    #[msg("this epoch closed during unwind; its deposits can only be refunded")]
    DepositHeldForRefund,
    #[msg("clear the breaker with reset_breaker, which refreshes its reference price")]
    BreakerResetRequired,
    #[msg("wallet is not approved for direct deposits")]
    DepositNotApproved,
    #[msg("deposit produces fewer shares than the minimum")]
    SlippageExceeded,
    #[msg("NAV_high and NAV_low are too far apart to graduate; wait for stable and oracle prices to converge, or settle positive PnL")]
    GraduationSpreadTooWide,
    #[msg("fee rates change only right after fees accrue; send apply_params after close_epoch in the same transaction")]
    FeeChangeNeedsAccrual,
    #[msg("no whole recovery lot fits current health at the permitted price and fees")]
    RecoveryHealthLimit,
    #[msg("legacy economic mode is unsupported; use graduate and atomic close_epoch_equity")]
    LegacyEconomicModeUnsupported,
    #[msg("initial delegate is unsupported; propose and activate after the immutable timelock")]
    InitialDelegateRequiresTimelock,
    #[msg("recovery grace must cover a full epoch plus its funding window")]
    RecoveryGraceTooShort,
    #[msg("the deferred head has no eligible cash or health funding failure that opens permissionless recovery")]
    RecoveryReasonNotEligible,
    #[msg("vault fills or out events are unconsumed; consume them before confirming settled recovery state")]
    UnconsumedVaultFills,
    #[msg("event queues must follow the health accounts, one per active perp position in position order")]
    WrongEventQueue,
    #[msg("the unwind step did not fill; the attempt reverts so the order penalty is not charged")]
    RecoveryStepNoFill,
    #[msg("during unwind, recovery parameters may only widen the oracle band and step size; grace values are fixed")]
    RecoveryParamsNotWidening,
    #[msg("the current NAV_low price is below the deferred batch's close price; the batch waits for its next close")]
    CureBelowClosePrice,
    #[msg("an older withdrawal batch is deferred; it is funded first (cure_deferred or the next close)")]
    DeferredWithdrawalFirst,
    #[msg("retained event queue or event data is malformed")]
    InvalidPendingEvent,
    #[msg("numerical Init breaker is unsupported in this experimental schema")]
    UnsupportedHealthBreaker,
    #[msg("instruction is retired in this experimental schema")]
    UnsupportedEntrypoint,
    #[msg("funding window is invalid, stale or expired")]
    InvalidFundingWindow,
    #[msg("Unsupported portfolio: one USDC token slot and USDC-settled perps are required")]
    UnsupportedPortfolio,
}
