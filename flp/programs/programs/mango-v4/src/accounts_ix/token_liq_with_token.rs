use crate::error::*;
use crate::state::*;
use anchor_lang::prelude::*;

#[derive(Accounts)]
pub struct TokenLiqWithToken<'info> {
    #[account(
        constraint = group.load()?.is_ix_enabled(IxGate::TokenLiqWithToken) @ FermiError::IxIsDisabled,
    )]
    pub group: AccountLoader<'info, Group>,

    #[account(
        mut,
        has_one = group,
        constraint = liqor.load()?.is_operational() @ FermiError::AccountIsFrozen
                                       
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
