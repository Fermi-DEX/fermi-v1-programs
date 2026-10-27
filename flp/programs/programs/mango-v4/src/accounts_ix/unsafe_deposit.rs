use crate::error::*;
use crate::state::*;
use anchor_lang::prelude::*;

#[derive(Accounts)]
pub struct UnsafeDeposit<'info> {
    #[account(
        constraint = group.load()?.is_testing() @ FermiError::SomeError,
        constraint = group.load()?.admin == admin.key() @ FermiError::SomeError,
        constraint = group.load()?.is_ix_enabled(IxGate::TokenDeposit) @ FermiError::IxIsDisabled,
    )]
    pub group: AccountLoader<'info, Group>,

    #[account(
        mut,
        has_one = group,
        constraint = account.load()?.is_operational() @ FermiError::AccountIsFrozen
    )]
    pub account: AccountLoader<'info, FermiAccountFixed>,

    pub admin: Signer<'info>,

    #[account(
        mut,
        has_one = group,
        has_one = oracle,
    )]
    pub bank: AccountLoader<'info, Bank>,

                                                                       
    pub oracle: UncheckedAccount<'info>,
}
