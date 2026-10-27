use crate::error::*;
use crate::state::*;
use anchor_lang::prelude::*;
use anchor_spl::token::TokenAccount;

#[derive(Accounts)]
pub struct PerpLiqBaseOrPositivePnl<'info> {
    #[account(
        constraint = group.load()?.is_ix_enabled(IxGate::PerpLiqBaseOrPositivePnl) @ FermiError::IxIsDisabled
    )]
    pub group: AccountLoader<'info, Group>,

    #[account(mut, has_one = group, has_one = oracle)]
    pub perp_market: AccountLoader<'info, PerpMarket>,

                                                                                             
    pub oracle: UncheckedAccount<'info>,

    #[account(
        mut,
        has_one = group,
        constraint = liqor.load()?.is_operational() @ FermiError::AccountIsFrozen
                                       
    )]
    pub liqor: AccountLoader<'info, FermiAccountFixed>,
    pub liqor_owner: Signer<'info>,

    #[account(
        mut,
        has_one = group,
        constraint = liqee.load()?.is_operational() @ FermiError::AccountIsFrozen
    )]
    pub liqee: AccountLoader<'info, FermiAccountFixed>,

    #[account(
        mut,
        has_one = group,
        constraint = settle_bank.load()?.token_index == perp_market.load()?.settle_token_index @ FermiError::InvalidBank
    )]
    pub settle_bank: AccountLoader<'info, Bank>,

    #[account(
        mut,
        address = settle_bank.load()?.vault
    )]
    pub settle_vault: Account<'info, TokenAccount>,

                                                      
    #[account(address = settle_bank.load()?.oracle)]
    pub settle_oracle: UncheckedAccount<'info>,
}
