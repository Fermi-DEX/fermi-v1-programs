use anchor_lang::prelude::*;

use crate::error::*;
use crate::state::*;
use openbook_v2::{
    program::OpenbookV2,
    state::{Market, OpenOrdersAccount},
};

#[derive(Accounts)]
pub struct OpenbookV2CancelOrder<'info> {
                                                                                                             
    pub group: AccountLoader<'info, Group>,

    #[account(
        mut,
        has_one = group,
        constraint = account.load()?.is_operational() @ FermiError::AccountIsFrozen
                                     
    )]
    pub account: AccountLoader<'info, FermiAccountFixed>,

    pub authority: Signer<'info>,

    #[account(mut)]
                                                                                          
    pub open_orders: AccountLoader<'info, OpenOrdersAccount>,

    #[account(
        has_one = group,
        has_one = openbook_v2_program,
        has_one = openbook_v2_market_external,
    )]
    pub openbook_v2_market: AccountLoader<'info, OpenbookV2Market>,

    pub openbook_v2_program: Program<'info, OpenbookV2>,

    #[account(
        has_one = bids,
        has_one = asks,
    )]
    pub openbook_v2_market_external: AccountLoader<'info, Market>,

    #[account(mut)]
                                                  
    pub bids: UncheckedAccount<'info>,

    #[account(mut)]
                                                  
    pub asks: UncheckedAccount<'info>,
}
