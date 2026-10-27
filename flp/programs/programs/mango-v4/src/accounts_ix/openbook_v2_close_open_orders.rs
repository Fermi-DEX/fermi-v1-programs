use anchor_lang::prelude::*;
use anchor_spl::token::Token;

use crate::error::FermiError;
use crate::state::*;
use openbook_v2::{
    program::OpenbookV2,
    state::{Market, OpenOrdersIndexer},
};

#[derive(Accounts)]
pub struct OpenbookV2CloseOpenOrders<'info> {
    #[account(
        constraint = group.load()?.is_ix_enabled(IxGate::OpenbookV2CloseOpenOrders) @ FermiError::IxIsDisabled,
    )]
    pub group: AccountLoader<'info, Group>,

    #[account(
        mut,
        has_one = group,
        constraint = account.load()?.is_operational() @ FermiError::AccountIsFrozen
                                     
    )]
    pub account: AccountLoader<'info, FermiAccountFixed>,

    pub authority: Signer<'info>,

    #[account(
        has_one = group,
        has_one = openbook_v2_program,
        has_one = openbook_v2_market_external,
    )]
    pub openbook_v2_market: AccountLoader<'info, OpenbookV2Market>,

    pub openbook_v2_program: Program<'info, OpenbookV2>,

    pub openbook_v2_market_external: AccountLoader<'info, Market>,

    #[account(mut)]
                                                                                 
                                         
    pub open_orders_indexer: Box<Account<'info, OpenOrdersIndexer>>,

    #[account(mut)]
                                                                                          
    pub open_orders_account: UncheckedAccount<'info>,

    #[account(mut)]
                                                      
    pub sol_destination: UncheckedAccount<'info>,

                                            
    #[account(mut, has_one = group)]
    pub base_bank: AccountLoader<'info, Bank>,

                                            
    #[account(mut, has_one = group)]
    pub quote_bank: AccountLoader<'info, Bank>,

    pub system_program: Program<'info, System>,
    pub token_program: Program<'info, Token>,
}
