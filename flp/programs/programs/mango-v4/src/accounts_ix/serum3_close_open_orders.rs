use anchor_lang::prelude::*;

use crate::error::FermiError;
use crate::state::*;

#[derive(Accounts)]
pub struct Serum3CloseOpenOrders<'info> {
    #[account(
        constraint = group.load()?.is_ix_enabled(IxGate::Serum3CloseOpenOrders) @ FermiError::IxIsDisabled,
    )]
    pub group: AccountLoader<'info, Group>,

    #[account(
        mut,
        has_one = group,
        constraint = account.load()?.is_operational() @ FermiError::AccountIsFrozen
                                 
    )]
    pub account: AccountLoader<'info, FermiAccountFixed>,
    pub owner: Signer<'info>,

    #[account(
        has_one = group,
        has_one = serum_program,
        has_one = serum_market_external,
    )]
    pub serum_market: AccountLoader<'info, Serum3Market>,
                                                                          
    pub serum_program: UncheckedAccount<'info>,
                                                                          
    pub serum_market_external: UncheckedAccount<'info>,

    #[account(mut)]
                                                                                          
    pub open_orders: UncheckedAccount<'info>,

    #[account(mut)]
                                                      
    pub sol_destination: UncheckedAccount<'info>,
}
