use anchor_lang::prelude::*;

use crate::error::*;
use crate::state::*;

#[derive(Accounts)]
pub struct Serum3CancelAllOrders<'info> {
    #[account(
        constraint = group.load()?.is_ix_enabled(IxGate::Serum3CancelAllOrders) @ FermiError::IxIsDisabled,
    )]
    pub group: AccountLoader<'info, Group>,

    #[account(
        has_one = group,
        constraint = account.load()?.is_operational() @ FermiError::AccountIsFrozen
                                 
    )]
    pub account: AccountLoader<'info, FermiAccountFixed>,
    pub owner: Signer<'info>,

    #[account(mut)]
                                                                                          
    pub open_orders: UncheckedAccount<'info>,

    #[account(
        has_one = group,
        has_one = serum_program,
        has_one = serum_market_external,
    )]
    pub serum_market: AccountLoader<'info, Serum3Market>,
                                                                          
    pub serum_program: UncheckedAccount<'info>,
    #[account(mut)]
                                                                          
    pub serum_market_external: UncheckedAccount<'info>,

                                                                  
                               
    #[account(mut)]
                                              
    pub market_bids: UncheckedAccount<'info>,
    #[account(mut)]
                                              
    pub market_asks: UncheckedAccount<'info>,
    #[account(mut)]
                                              
    pub market_event_queue: UncheckedAccount<'info>,
}
