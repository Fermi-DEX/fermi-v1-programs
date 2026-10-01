use anchor_lang::prelude::*;

use crate::error::*;
use crate::state::*;

#[derive(Accounts)]
pub struct PerpDeactivatePosition<'info> {
    #[account(
        constraint = group.load()?.is_ix_enabled(IxGate::PerpDeactivatePosition) @ FermiError::IxIsDisabled,
    )]
    pub group: AccountLoader<'info, Group>,

    #[account(
        mut,
        has_one = group,
        constraint = account.load()?.is_operational() @ FermiError::AccountIsFrozen
        // owner is checked at #1
    )]
    pub account: AccountLoader<'info, FermiAccountFixed>,
    pub owner: Signer<'info>,

    #[account(has_one = group)]
    pub perp_market: AccountLoader<'info, PerpMarket>,
}
