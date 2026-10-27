use crate::error::*;
use crate::state::*;
use anchor_lang::prelude::*;

#[derive(Accounts)]
pub struct TokenConditionalSwapCreate<'info> {
                                                        
    pub group: AccountLoader<'info, Group>,

    #[account(
        mut,
        has_one = group,
        constraint = account.load()?.is_operational() @ FermiError::AccountIsFrozen,
        constraint = account.load()?.is_owner_or_delegate(authority.key()),
    )]
    pub account: AccountLoader<'info, FermiAccountFixed>,
    pub authority: Signer<'info>,

    #[account(
        has_one = group,
    )]
    pub buy_bank: AccountLoader<'info, Bank>,
    #[account(
        has_one = group,
    )]
    pub sell_bank: AccountLoader<'info, Bank>,
}
