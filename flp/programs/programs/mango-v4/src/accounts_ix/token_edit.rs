use crate::state::*;
use anchor_lang::prelude::*;

                                 
   
                                                                                 
                      
#[derive(Accounts)]
pub struct TokenEdit<'info> {
    pub group: AccountLoader<'info, Group>,
                                                
    pub admin: Signer<'info>,

    #[account(
        mut,
        has_one = group
    )]
    pub mint_info: AccountLoader<'info, MintInfo>,

                                                                                    
       
                                                                       
    pub oracle: UncheckedAccount<'info>,

                                                                                               
       
                                                                                
    pub fallback_oracle: UncheckedAccount<'info>,
}
