use anchor_lang::prelude::*;

use crate::error::*;
use crate::state::*;

/// Withdraws lamports above the live rent-exempt minimum from one
/// protocol-owned account of `group`.
///
/// For a `BookSide` or `EventQueue` target, the owning `PerpMarket` must be
/// passed as the single remaining account.
#[derive(Accounts)]
pub struct AdminWithdrawExcessLamports<'info> {
    #[account(
        constraint = group.load()?.is_ix_enabled(IxGate::AdminWithdrawExcessLamports) @ FermiError::IxIsDisabled,
        has_one = admin,
    )]
    pub group: AccountLoader<'info, Group>,

    pub admin: Signer<'info>,

    #[account(
        mut,
        owner = crate::ID,
    )]
    /// CHECK: type and group membership are validated in the handler
    pub target: UncheckedAccount<'info>,

    #[account(
        mut,
        constraint = destination.key() != target.key() @ FermiError::SomeError,
    )]
    /// CHECK: any writable account chosen by the admin
    pub destination: UncheckedAccount<'info>,
}
