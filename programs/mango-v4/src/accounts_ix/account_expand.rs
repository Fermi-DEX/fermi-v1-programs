use crate::error::*;
use crate::state::*;
use anchor_lang::prelude::*;

#[derive(Accounts)]
pub struct AccountExpand<'info> {
    #[account(
        constraint = group.load()?.is_ix_enabled(IxGate::AccountExpand) @ FermiError::IxIsDisabled,
    )]
    pub group: AccountLoader<'info, Group>,

    #[account(
        mut,
        has_one = group,
        has_one = owner,
        constraint = account.load()?.is_operational() @ FermiError::AccountIsFrozen
    )]
    pub account: AccountLoader<'info, FermiAccountFixed>,
    pub owner: Signer<'info>,

    #[account(mut)]
    pub payer: Signer<'info>,

    pub system_program: Program<'info, System>,
}
