use crate::error::*;
use crate::state::*;
use anchor_lang::prelude::*;

#[derive(Accounts)]
pub struct TokenForceCloseBorrowsWithToken<'info> {
    #[account(
        constraint = group.load()?.is_ix_enabled(IxGate::TokenForceCloseBorrowsWithToken) @ FermiError::IxIsDisabled,
    )]
    pub group: AccountLoader<'info, Group>,

    #[account(
        mut,
        has_one = group,
        constraint = liqor.load()?.is_operational() @ FermiError::AccountIsFrozen
        // liqor_owner is checked at #1
    )]
    pub liqor: AccountLoader<'info, FermiAccountFixed>,
    pub liqor_owner: Signer<'info>,

    #[account(
        mut,
        has_one = group,
        constraint = liqee.load()?.is_operational() @ FermiError::AccountIsFrozen
    )]
    pub liqee: AccountLoader<'info, FermiAccountFixed>,
}
