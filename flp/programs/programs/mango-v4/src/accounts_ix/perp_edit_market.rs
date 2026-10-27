use crate::state::*;
use anchor_lang::prelude::*;

#[derive(Accounts)]
pub struct PerpEditMarket<'info> {
    pub group: AccountLoader<'info, Group>,
                                                
    pub admin: Signer<'info>,

    #[account(
        mut,
        has_one = group
    )]
    pub perp_market: AccountLoader<'info, PerpMarket>,

                                                                                    
       
                                                                       
    pub oracle: UncheckedAccount<'info>,
}
