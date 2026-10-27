use crate::error::FermiError;
use crate::state::*;
use anchor_lang::prelude::*;

#[derive(Accounts)]
#[instruction(market_index: Serum3MarketIndex)]
pub struct Serum3RegisterMarket<'info> {
    #[account(
        mut,
        constraint = group.load()?.is_ix_enabled(IxGate::Serum3RegisterMarket) @ FermiError::IxIsDisabled,
        constraint = group.load()?.serum3_supported()
    )]
    pub group: AccountLoader<'info, Group>,
                                                        
    pub admin: Signer<'info>,

                                                          
    pub serum_program: UncheckedAccount<'info>,
                                            
    pub serum_market_external: UncheckedAccount<'info>,

                                                                                 
    #[account(
        mut,
        seeds = [b"Serum3Market".as_ref(), group.key().as_ref(), serum_market_external.key().as_ref()],
        bump,
    )]
    pub serum_market: UncheckedAccount<'info>,

                                                                                 
    #[account(
        mut,
        seeds = [b"Serum3Index".as_ref(), group.key().as_ref(), &market_index.to_le_bytes()],
        bump,
    )]
    pub index_reservation: UncheckedAccount<'info>,

    #[account(has_one = group)]
    pub quote_bank: AccountLoader<'info, Bank>,
    #[account(has_one = group)]
    pub base_bank: AccountLoader<'info, Bank>,

    #[account(mut)]
    pub payer: Signer<'info>,

    pub system_program: Program<'info, System>,
}
