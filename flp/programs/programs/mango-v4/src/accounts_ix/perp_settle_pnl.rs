use crate::error::*;
use crate::state::*;
use anchor_lang::prelude::*;

#[derive(Accounts)]
pub struct PerpSettlePnl<'info> {
    #[account(
        constraint = group.load()?.is_ix_enabled(IxGate::PerpSettlePnl) @ FermiError::IxIsDisabled,
    )]
    pub group: AccountLoader<'info, Group>,

    #[account(
        mut,
        has_one = group,
        constraint = settler.load()?.is_operational() @ FermiError::AccountIsFrozen
                                         
    )]
    pub settler: AccountLoader<'info, FermiAccountFixed>,
    pub settler_owner: Signer<'info>,

    #[account(has_one = group, has_one = oracle)]
    pub perp_market: AccountLoader<'info, PerpMarket>,

                                      
    #[account(mut,
        has_one = group,
        constraint = account_a.load()?.is_operational() @ FermiError::AccountIsFrozen
    )]
    pub account_a: AccountLoader<'info, FermiAccountFixed>,
                                    
    #[account(
        mut,
        has_one = group,
        constraint = account_b.load()?.is_operational() @ FermiError::AccountIsFrozen
    )]
    pub account_b: AccountLoader<'info, FermiAccountFixed>,

                                                                                             
    pub oracle: UncheckedAccount<'info>,

                                        
    #[account(mut, has_one = group)]
    pub settle_bank: AccountLoader<'info, Bank>,

                                                      
    #[account(address = settle_bank.load()?.oracle)]
    pub settle_oracle: UncheckedAccount<'info>,
}
