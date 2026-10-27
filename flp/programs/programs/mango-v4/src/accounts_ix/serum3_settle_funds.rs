use anchor_lang::prelude::*;
use anchor_spl::token::{Token, TokenAccount};

use crate::error::*;
use crate::state::*;

#[derive(Accounts)]
pub struct Serum3SettleFunds<'info> {
    #[account(
        constraint = group.load()?.is_ix_enabled(IxGate::Serum3SettleFunds) @ FermiError::IxIsDisabled,
    )]
    pub group: AccountLoader<'info, Group>,

    #[account(
        mut,
        has_one = group,
        constraint = account.load()?.is_operational() @ FermiError::AccountIsFrozen
                                 
    )]
    pub account: AccountLoader<'info, FermiAccountFixed>,
    pub owner: Signer<'info>,

    #[account(mut)]
                                                                                          
    pub open_orders: UncheckedAccount<'info>,

    #[account(
        has_one = group,
        has_one = serum_program,
        has_one = serum_market_external,
    )]
    pub serum_market: AccountLoader<'info, Serum3Market>,
                                                                          
    pub serum_program: UncheckedAccount<'info>,
    #[account(mut)]
                                                                          
    pub serum_market_external: UncheckedAccount<'info>,

                                                                  
                               
    #[account(mut)]
                                              
    pub market_base_vault: UncheckedAccount<'info>,
    #[account(mut)]
                                              
    pub market_quote_vault: UncheckedAccount<'info>,
                                                  
                                              
    pub market_vault_signer: UncheckedAccount<'info>,

                                                                    
    #[account(mut, has_one = group)]
    pub quote_bank: AccountLoader<'info, Bank>,
    #[account(mut)]
    pub quote_vault: Box<Account<'info, TokenAccount>>,
    #[account(mut, has_one = group)]
    pub base_bank: AccountLoader<'info, Bank>,
    #[account(mut)]
    pub base_vault: Box<Account<'info, TokenAccount>>,

    pub token_program: Program<'info, Token>,
}

#[derive(Accounts)]
pub struct Serum3SettleFundsV2Extra<'info> {
                                                                                                               
    pub quote_oracle: UncheckedAccount<'info>,
                                                                                                               
    pub base_oracle: UncheckedAccount<'info>,
}

#[derive(Accounts)]
pub struct Serum3SettleFundsV2<'info> {
    pub v1: Serum3SettleFunds<'info>,
    #[account(
        constraint = v2.quote_oracle.key() == v1.quote_bank.load()?.oracle,
        constraint = v2.base_oracle.key() == v1.base_bank.load()?.oracle,
    )]
    pub v2: Serum3SettleFundsV2Extra<'info>,
}
