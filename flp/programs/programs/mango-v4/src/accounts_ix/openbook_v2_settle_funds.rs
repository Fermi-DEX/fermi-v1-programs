use anchor_lang::prelude::*;
use anchor_spl::token::Token;
use openbook_v2::state::OpenOrdersAccount;

use crate::error::*;
use crate::state::*;
use openbook_v2::{program::OpenbookV2, state::Market};

#[derive(Accounts)]
pub struct OpenbookV2SettleFunds<'info> {
                                                        
    pub group: UncheckedAccount<'info>,

    #[account(mut)]
                                                        
    pub account: UncheckedAccount<'info>,

    #[account(mut)]
                                                        
    pub authority: UncheckedAccount<'info>,

    #[account(mut)]
    pub open_orders: UncheckedAccount<'info>,

    pub openbook_v2_market: UncheckedAccount<'info>,

                                                        
    pub openbook_v2_program: UncheckedAccount<'info>,

    #[account(mut)]
    pub openbook_v2_market_external: UncheckedAccount<'info>,

                                                                    
    #[account(mut)]
    pub quote_bank: UncheckedAccount<'info>,

    #[account(mut)]
    pub quote_vault: UncheckedAccount<'info>,

    #[account(mut)]
    pub base_bank: UncheckedAccount<'info>,

    #[account(mut)]
    pub base_vault: UncheckedAccount<'info>,
}
