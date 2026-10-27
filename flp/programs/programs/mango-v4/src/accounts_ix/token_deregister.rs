use crate::{error::FermiError, state::*};
use anchor_lang::prelude::*;
use anchor_spl::token::{Token, TokenAccount};

                                                                    
                                                                       
#[derive(Accounts)]
pub struct TokenDeregister<'info> {
    #[account(
        has_one = admin,
        constraint = group.load()?.is_ix_enabled(IxGate::TokenDeregister) @ FermiError::IxIsDisabled,
        constraint = group.load()?.is_testing(),
    )]
    pub group: AccountLoader<'info, Group>,
    pub admin: Signer<'info>,

                              
    #[account(
        mut,
        has_one = group,
        close = sol_destination
    )]
    pub mint_info: AccountLoader<'info, MintInfo>,

    #[account(mut)]
    pub dust_vault: Account<'info, TokenAccount>,

    #[account(mut)]
                                                      
    pub sol_destination: UncheckedAccount<'info>,

    pub token_program: Program<'info, Token>,
}
