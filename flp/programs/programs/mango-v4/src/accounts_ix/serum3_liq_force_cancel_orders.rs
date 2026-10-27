use anchor_lang::prelude::*;
use anchor_spl::token::Token;

use crate::error::*;
use crate::state::*;

#[derive(Accounts)]
pub struct Serum3LiqForceCancelOrders<'info> {
                                                        
    pub group: UncheckedAccount<'info>,

                                                   
    #[account(mut)]
                                                        
    pub account: UncheckedAccount<'info>,

    #[account(mut)]
                                                                                          
    pub open_orders: UncheckedAccount<'info>,

                                                        
    pub serum_market: UncheckedAccount<'info>,
                                                                          
    pub serum_program: UncheckedAccount<'info>,
    #[account(mut)]
                                                                          
    pub serum_market_external: UncheckedAccount<'info>,

                                                                    
    #[account(mut)]
                                                        
    pub quote_bank: UncheckedAccount<'info>,
    #[account(mut)]
    pub quote_vault: UncheckedAccount<'info>,
    #[account(mut)]
                                                        
    pub base_bank: UncheckedAccount<'info>,
    #[account(mut)]
    pub base_vault: UncheckedAccount<'info>,
}
