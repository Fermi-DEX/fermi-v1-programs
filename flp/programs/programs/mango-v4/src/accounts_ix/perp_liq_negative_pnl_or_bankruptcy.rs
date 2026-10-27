use crate::error::*;
use crate::state::*;
use anchor_lang::prelude::*;
use anchor_spl::token::{Token, TokenAccount};

#[derive(Accounts)]
pub struct PerpLiqNegativePnlOrBankruptcy<'info> {
    pub group: AccountLoader<'info, Group>,

    #[account(mut)]
    pub liqor: AccountLoader<'info, FermiAccountFixed>,
    pub liqor_owner: Signer<'info>,

                                    
    #[account(mut)]
    pub liqee: AccountLoader<'info, FermiAccountFixed>,

    #[account(mut)]
    pub perp_market: AccountLoader<'info, PerpMarket>,

                                                                                             
    pub oracle: UncheckedAccount<'info>,

    #[account(mut)]
                                                        
    pub settle_bank: UncheckedAccount<'info>,

    #[account(mut)]
                                                        
    pub settle_vault: UncheckedAccount<'info>,

                                                      
    pub settle_oracle: UncheckedAccount<'info>,

                                                                  
                                                                 
    #[account(mut)]
                                                        
    pub insurance_vault: UncheckedAccount<'info>,

                                                        
    pub token_program: UncheckedAccount<'info>,
}

#[derive(Accounts)]
pub struct PerpLiqNegativePnlOrBankruptcyV2<'info> {
    pub group: UncheckedAccount<'info>,

    #[account(mut)]
                                                        
    pub liqor: UncheckedAccount<'info>,
    pub liqor_owner: UncheckedAccount<'info>,

                                    
    #[account(mut)]
                                                        
    pub liqee: UncheckedAccount<'info>,

    #[account(mut)]
                                                        
    pub perp_market: UncheckedAccount<'info>,

                                                                                             
    pub oracle: UncheckedAccount<'info>,

    #[account(mut)]
                                                        
    pub settle_bank: UncheckedAccount<'info>,

    #[account(mut)]
                                                        
    pub settle_vault: UncheckedAccount<'info>,

                                                                  
                                                                 
    #[account(mut)]
                                                        
    pub insurance_vault: UncheckedAccount<'info>,

    #[account(mut)]
                                                        
    pub insurance_bank_vault: UncheckedAccount<'info>,
}
