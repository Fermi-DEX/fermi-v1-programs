use crate::error::FermiError;
use crate::state::{BookSide, FermiAccountFixed, Group, IxGate, PerpMarket};
use anchor_lang::prelude::*;

#[derive(Accounts)]
pub struct PerpCancelAllOrdersBySide<'info> {
    #[account(
        constraint = group.load()?.is_ix_enabled(IxGate::PerpCancelAllOrdersBySide) @ FermiError::IxIsDisabled,
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
        mut,
        has_one = group,
        has_one = bids,
        has_one = asks,
    )]
    pub perp_market: AccountLoader<'info, PerpMarket>,
    #[account(mut)]
    pub bids: AccountLoader<'info, BookSide>,
    #[account(mut)]
    pub asks: AccountLoader<'info, BookSide>,
}
