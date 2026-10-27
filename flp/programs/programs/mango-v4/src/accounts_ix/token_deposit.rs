use anchor_lang::prelude::*;
use anchor_spl::token::Token;
use anchor_spl::token::TokenAccount;

use crate::error::*;
use crate::state::*;

                                                      
#[derive(Accounts)]
pub struct TokenDepositIntoExisting<'info> {
    #[account(
        constraint = group.load()?.is_ix_enabled(IxGate::TokenDeposit) @ FermiError::IxIsDisabled,
    )]
    pub group: AccountLoader<'info, Group>,

    #[account(
        mut,
        has_one = group,
        constraint = account.load()?.is_operational() @ FermiError::AccountIsFrozen
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
    pub vault: Account<'info, TokenAccount>,

                                                                       
    pub oracle: UncheckedAccount<'info>,

    #[account(mut)]
    pub token_account: Box<Account<'info, TokenAccount>>,
    pub token_authority: Signer<'info>,

    pub token_program: Program<'info, Token>,
}

#[derive(Accounts)]
pub struct TokenDeposit<'info> {
    #[account(
        constraint = group.load()?.is_ix_enabled(IxGate::TokenDeposit) @ FermiError::IxIsDisabled,
    )]
    pub group: AccountLoader<'info, Group>,

    #[account(
        mut,
        has_one = group,
        constraint = account.load()?.is_operational() @ FermiError::AccountIsFrozen,
        constraint = account.load()?.is_owner_or_delegate(owner.key()),
    )]
    pub account: AccountLoader<'info, FermiAccountFixed>,
    pub owner: Signer<'info>,

    #[account(
        mut,
        has_one = group,
        has_one = vault,
        has_one = oracle,
                                                                                
                                                                        
    )]
    pub bank: AccountLoader<'info, Bank>,

    #[account(mut)]
    pub vault: Account<'info, TokenAccount>,

                                                                       
    pub oracle: UncheckedAccount<'info>,

    #[account(mut)]
    pub token_account: Box<Account<'info, TokenAccount>>,
    pub token_authority: Signer<'info>,

    pub token_program: Program<'info, Token>,
}
