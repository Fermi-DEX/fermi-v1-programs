use anchor_lang::prelude::*;
use anchor_spl::token::{Mint, TokenAccount};

use crate::state::*;

#[derive(Accounts)]
#[instruction(token_index: TokenIndex)]
pub struct TokenRegisterBootstrap<'info> {
    pub group: AccountLoader<'info, Group>,
    pub admin: Signer<'info>,

    pub mint: Box<Account<'info, Mint>>,
                                                                                                
    #[account(mut)]
    pub bank: UncheckedAccount<'info>,
    pub vault: Box<Account<'info, TokenAccount>>,
                                                                                                
    #[account(mut)]
    pub mint_info: UncheckedAccount<'info>,

                                                                       
    pub oracle: UncheckedAccount<'info>,

                                                                       
    pub fallback_oracle: UncheckedAccount<'info>,
}
