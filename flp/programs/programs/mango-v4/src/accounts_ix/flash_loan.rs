use crate::error::*;
use crate::state::*;
use anchor_lang::prelude::*;
use anchor_lang::solana_program::sysvar::instructions as tx_instructions;
use anchor_spl::{associated_token::AssociatedToken, token::Token};

pub mod jupiter_mainnet_6 {
    use solana_program::declare_id;
    declare_id!("JUP6LkbZbjS1jKKwapdHNy74zcZ3tLUZoi5QNyVTaV4");
}

pub mod jupiter_mainnet_4 {
    use solana_program::declare_id;
    declare_id!("JUP4Fb2cqiRUcaTHdrPC8h2gNsA2ETXiPDD33WcGuJB");
}
pub mod jupiter_mainnet_3 {
    use solana_program::declare_id;
    declare_id!("JUP3c2Uh3WA4Ng34tw6kPd2G4C5BB21Xo36Je1s32Ph");
}

                                       
   
                                                                    
                         
                                              
                                                                    
                                                 
                      
#[derive(Accounts)]
pub struct FlashLoanBegin<'info> {
    #[account(
        constraint = account.load()?.is_operational() @ FermiError::AccountIsFrozen
    )]
    pub account: AccountLoader<'info, FermiAccountFixed>,
                             
    pub owner: Signer<'info>,

    pub token_program: Program<'info, Token>,

                                                         
                                                
    #[account(address = tx_instructions::ID)]
    pub instructions: UncheckedAccount<'info>,
}

#[derive(Accounts)]
pub struct FlashLoanSwapBegin<'info> {
    #[account(
        constraint = account.load()?.is_operational() @ FermiError::AccountIsFrozen
    )]
    pub account: AccountLoader<'info, FermiAccountFixed>,
                             
    #[account(mut)]
    pub owner: Signer<'info>,

                                                                                
    pub input_mint: UncheckedAccount<'info>,
                                                                                
    pub output_mint: UncheckedAccount<'info>,

    pub system_program: Program<'info, System>,
    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,

                                                         
                                                
    #[account(address = tx_instructions::ID)]
    pub instructions: UncheckedAccount<'info>,
}

                          
   
                                                                    
                                                                                            
                                                               
                                                                        
                                                                     
                      
#[derive(Accounts)]
pub struct FlashLoanEnd<'info> {
    #[account(
        mut,
        constraint = account.load()?.is_operational() @ FermiError::AccountIsFrozen
    )]
    pub account: AccountLoader<'info, FermiAccountFixed>,
                             
    pub owner: Signer<'info>,

    pub token_program: Program<'info, Token>,
}

#[derive(PartialEq, Copy, Clone, Debug, AnchorSerialize, AnchorDeserialize)]
#[repr(u8)]
pub enum FlashLoanType {
                               
    Unknown,
                                                                              
       
                                                                      
    Swap,
                                                           
    SwapWithoutFee,
}
