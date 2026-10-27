use anchor_lang::prelude::*;
use anchor_spl::associated_token::get_associated_token_address;
use anchor_spl::token::Token;
use anchor_spl::token::TokenAccount;

use crate::error::*;
use crate::state::*;

#[derive(Accounts)]
pub struct TokenForceWithdraw<'info> {
    #[account(
        constraint = group.load()?.is_ix_enabled(IxGate::TokenForceWithdraw) @ FermiError::IxIsDisabled,
    )]
    pub group: AccountLoader<'info, Group>,

    #[account(
        mut,
        has_one = group,
        constraint = account.load()?.is_operational() @ FermiError::AccountIsFrozen,
    )]
    pub account: AccountLoader<'info, FermiAccountFixed>,

    #[account(
        mut,
        has_one = group,
        has_one = vault,
        has_one = oracle,
                                                                                 
                                                                        
    )]
    pub bank: AccountLoader<'info, Bank>,

    #[account(mut)]
    pub vault: Box<Account<'info, TokenAccount>>,

                                                                       
    pub oracle: UncheckedAccount<'info>,

    #[account(
        mut,
        address = get_associated_token_address(&account.load()?.owner, &vault.mint),
                                                                                     
    )]
    pub owner_ata_token_account: Box<Account<'info, TokenAccount>>,

                                                                                           
    #[account(
        mut,
        constraint = alternate_owner_token_account.owner == account.load()?.owner,
    )]
    pub alternate_owner_token_account: Box<Account<'info, TokenAccount>>,

    pub token_program: Program<'info, Token>,
}
