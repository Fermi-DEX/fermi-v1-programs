use anchor_lang::prelude::*;

use crate::error::*;
use crate::state::*;

#[derive(Accounts)]
pub struct Serum3CreateOpenOrders<'info> {
    #[account(
        constraint = group.load()?.is_ix_enabled(IxGate::Serum3CreateOpenOrders) @ FermiError::IxIsDisabled,
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

                                                       
    #[account(
        init,
        seeds = [b"Serum3OO".as_ref(), account.key().as_ref(), serum_market.key().as_ref()],
        bump,
        payer = payer,
        owner = serum_program.key(),
                                                                                        
        space = 12 + std::mem::size_of::<serum_dex::state::OpenOrders>(),
    )]
                                              
    pub open_orders: UncheckedAccount<'info>,

    #[account(mut)]
    pub payer: Signer<'info>,

    pub system_program: Program<'info, System>,
    pub rent: Sysvar<'info, Rent>,
}
