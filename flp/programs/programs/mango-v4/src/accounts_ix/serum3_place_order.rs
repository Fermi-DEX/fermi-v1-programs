use crate::error::*;
use crate::state::*;
use anchor_lang::prelude::*;
use anchor_spl::token::Token;
use num_enum::IntoPrimitive;
use num_enum::TryFromPrimitive;

                                                                                         

#[derive(Clone, Copy, TryFromPrimitive, IntoPrimitive, AnchorSerialize, AnchorDeserialize)]
#[repr(u8)]
pub enum Serum3SelfTradeBehavior {
    DecrementTake = 0,
    CancelProvide = 1,
    AbortTransaction = 2,
}

#[derive(Clone, Copy, TryFromPrimitive, IntoPrimitive, AnchorSerialize, AnchorDeserialize)]
#[repr(u8)]

pub enum Serum3OrderType {
    Limit = 0,
    ImmediateOrCancel = 1,
    PostOnly = 2,
}
#[derive(Clone, Copy, TryFromPrimitive, IntoPrimitive, AnchorSerialize, AnchorDeserialize)]
#[repr(u8)]

pub enum Serum3Side {
    Bid = 0,
    Ask = 1,
}

                                      
#[derive(Accounts)]
pub struct Serum3PlaceOrder<'info> {
                                                        
    pub group: UncheckedAccount<'info>,

    #[account(mut)]
                                                        
    pub account: UncheckedAccount<'info>,
                                                        
    pub owner: UncheckedAccount<'info>,

    #[account(mut)]
                                                                                          
    pub open_orders: UncheckedAccount<'info>,

                                                        
    pub serum_market: UncheckedAccount<'info>,
                                                                          
    pub serum_program: UncheckedAccount<'info>,
    #[account(mut)]
                                                                          
    pub serum_market_external: UncheckedAccount<'info>,

                                                      
                                                                                
    #[account(mut)]
                                                        
    pub payer_bank: UncheckedAccount<'info>,
                                                            
    #[account(mut)]
    pub payer_vault: UncheckedAccount<'info>,
}
