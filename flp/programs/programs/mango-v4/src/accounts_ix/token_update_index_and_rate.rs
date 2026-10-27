use anchor_lang::prelude::*;
use anchor_lang::solana_program::sysvar::instructions as tx_instructions;

use crate::error::FermiError;
use crate::state::*;

pub mod compute_budget {
    use solana_program::declare_id;
    declare_id!("ComputeBudget111111111111111111111111111111");
}

                                              
   
                                                                                 
                      
   
                                                                                   
                                  
#[derive(Accounts)]
pub struct TokenUpdateIndexAndRate<'info> {
    #[account(
        constraint = group.load()?.is_ix_enabled(IxGate::TokenUpdateIndexAndRate) @ FermiError::IxIsDisabled,
    )]
    pub group: AccountLoader<'info, Group>,                                       

    #[account(
        has_one = oracle,
        has_one = group,
    )]
    pub mint_info: AccountLoader<'info, MintInfo>,

                                                          
    pub oracle: UncheckedAccount<'info>,

                                                
    #[account(address = tx_instructions::ID)]
    pub instructions: UncheckedAccount<'info>,
}
