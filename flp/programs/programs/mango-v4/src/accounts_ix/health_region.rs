use crate::error::*;
use crate::state::*;
use anchor_lang::prelude::*;
use anchor_lang::solana_program::sysvar::instructions as tx_instructions;

                               
   
                                                                          
   
                                                   
#[derive(Accounts)]
pub struct HealthRegionBegin<'info> {
                                                         
                                                
    #[account(address = tx_instructions::ID)]
    pub instructions: UncheckedAccount<'info>,

    #[account(
        constraint = group.load()?.is_ix_enabled(IxGate::HealthRegion) @ FermiError::IxIsDisabled,
    )]
    pub group: AccountLoader<'info, Group>,

    #[account(
        mut,
        has_one = group,
        constraint = account.load()?.is_operational() @ FermiError::AccountIsFrozen
    )]
    pub account: AccountLoader<'info, FermiAccountFixed>,
}

                         
   
                                                   
#[derive(Accounts)]
pub struct HealthRegionEnd<'info> {
    #[account(
        mut,
        constraint = account.load()?.is_operational() @ FermiError::AccountIsFrozen
    )]
    pub account: AccountLoader<'info, FermiAccountFixed>,
}
