use crate::state::*;
use anchor_lang::prelude::*;

#[derive(Accounts)]
pub struct IxGateSet<'info> {
    #[account(
        mut,
                                                    
    )]
    pub group: AccountLoader<'info, Group>,
    pub admin: Signer<'info>,
}
