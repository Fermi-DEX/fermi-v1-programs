use anchor_lang::prelude::*;

use crate::error::*;
use crate::state::*;

                                                                  
                                      
   
                                                                            
                                           
#[derive(Accounts)]
pub struct AdminWithdrawExcessLamports<'info> {
    #[account(
        constraint = group.load()?.is_ix_enabled(IxGate::AdminWithdrawExcessLamports) @ FermiError::IxIsDisabled,
        has_one = admin,
    )]
    pub group: AccountLoader<'info, Group>,

    pub admin: Signer<'info>,

    #[account(
        mut,
        owner = crate::ID,
    )]
                                                                     
    pub target: UncheckedAccount<'info>,

    #[account(
        mut,
        constraint = destination.key() != target.key() @ FermiError::SomeError,
    )]
                                                       
    pub destination: UncheckedAccount<'info>,
}
