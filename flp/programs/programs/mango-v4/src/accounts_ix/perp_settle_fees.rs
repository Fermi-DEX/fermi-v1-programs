use crate::error::*;
use crate::state::*;
use anchor_lang::prelude::*;

#[derive(Accounts)]
pub struct PerpSettleFees<'info> {
    #[account(
        constraint = group.load()?.is_ix_enabled(IxGate::PerpSettleFees) @ FermiError::IxIsDisabled,
    )]
    pub group: AccountLoader<'info, Group>,

    #[account(mut, has_one = group, has_one = oracle)]
    pub perp_market: AccountLoader<'info, PerpMarket>,

                                    
    #[account(
        mut,
        has_one = group,
        constraint = account.load()?.is_operational() @ FermiError::AccountIsFrozen
    )]
    pub account: AccountLoader<'info, FermiAccountFixed>,

                                                                                             
    pub oracle: UncheckedAccount<'info>,

                                        
    #[account(mut, has_one = group)]
    pub settle_bank: AccountLoader<'info, Bank>,

                                                      
    #[account(address = settle_bank.load()?.oracle)]
    pub settle_oracle: UncheckedAccount<'info>,
}
