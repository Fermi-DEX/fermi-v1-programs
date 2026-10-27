use crate::error::*;
use crate::state::*;
use anchor_lang::prelude::*;

#[derive(Accounts)]
pub struct AccountBuybackFeesWithMngo<'info> {
    #[account(
        constraint = group.load()?.is_ix_enabled(IxGate::AccountBuybackFeesWithMngo) @ FermiError::IxIsDisabled,
        constraint = group.load()?.buyback_fees() @ FermiError::SomeError
    )]
    pub group: AccountLoader<'info, Group>,

    #[account(
        mut,
        has_one = group,
        constraint = account.load()?.is_operational() @ FermiError::AccountIsFrozen
                                 
    )]
    pub account: AccountLoader<'info, FermiAccountFixed>,
    pub owner: Signer<'info>,

    #[account(
        mut,
        has_one = group,
        constraint = account.load()?.is_operational() @ FermiError::AccountIsFrozen,
        address = group.load()?.buyback_fees_swap_fermi_account
    )]
    pub dao_account: AccountLoader<'info, FermiAccountFixed>,

    #[account(
        mut,
        has_one = group,
        constraint = mngo_bank.load()?.token_index == group.load()?.mngo_token_index,
        constraint = mngo_bank.load()?.token_index != 0,                       
    )]
    pub mngo_bank: AccountLoader<'info, Bank>,

                                                      
    #[account(address = mngo_bank.load()?.oracle)]
    pub mngo_oracle: UncheckedAccount<'info>,

    #[account(
        mut,
        has_one = group,
        constraint = fees_bank.load()?.token_index == QUOTE_TOKEN_INDEX
    )]
    pub fees_bank: AccountLoader<'info, Bank>,

                                                      
    #[account(address = fees_bank.load()?.oracle)]
    pub fees_oracle: UncheckedAccount<'info>,
}
