use anchor_lang::prelude::*;
use anchor_spl::token::Token;

use crate::{error::FermiError, state::*};

#[derive(Accounts)]
pub struct PerpCloseMarket<'info> {
    #[account(
        constraint = group.load()?.is_testing(),
        constraint = group.load()?.is_ix_enabled(IxGate::PerpCloseMarket) @ FermiError::IxIsDisabled,
        has_one = admin,
    )]
    pub group: AccountLoader<'info, Group>,
    pub admin: Signer<'info>,

    #[account(
        mut,
        has_one = group,
        has_one = bids,
        has_one = asks,
        has_one = event_queue,
        close = sol_destination
    )]
    pub perp_market: AccountLoader<'info, PerpMarket>,

    #[account(
        mut,
        close = sol_destination
    )]
    pub bids: AccountLoader<'info, BookSide>,

    #[account(
        mut,
        close = sol_destination
    )]
    pub asks: AccountLoader<'info, BookSide>,

    #[account(
        mut,
        close = sol_destination
    )]
    pub event_queue: AccountLoader<'info, EventQueue>,

    #[account(mut)]
                                                      
    pub sol_destination: UncheckedAccount<'info>,

    pub token_program: Program<'info, Token>,
}
