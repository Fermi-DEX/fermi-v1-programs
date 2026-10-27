use crate::error::*;
use crate::state::*;
use anchor_lang::prelude::*;

#[derive(Accounts)]
pub struct PerpUpdateFunding<'info> {
    #[account(
        constraint = group.load()?.is_ix_enabled(IxGate::PerpUpdateFunding) @ FermiError::IxIsDisabled,
    )]
    pub group: AccountLoader<'info, Group>,                                       

    #[account(
        mut,
        has_one = group,
        has_one = bids,
        has_one = asks,
        has_one = oracle,
    )]
    pub perp_market: AccountLoader<'info, PerpMarket>,
    #[account(mut)]
    pub bids: AccountLoader<'info, BookSide>,
    #[account(mut)]
    pub asks: AccountLoader<'info, BookSide>,

                                                                                                       
    pub oracle: UncheckedAccount<'info>,
}
