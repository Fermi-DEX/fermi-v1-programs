use crate::error::FermiError;
use crate::state::*;
use anchor_lang::prelude::*;
use openbook_v2::{program::OpenbookV2, state::Market};

#[derive(Accounts)]
#[instruction(market_index: OpenbookV2MarketIndex)]
pub struct OpenbookV2RegisterMarket<'info> {
    #[account(
        mut,
        constraint = group.load()?.is_ix_enabled(IxGate::OpenbookV2RegisterMarket) @ FermiError::IxIsDisabled,
    )]
    pub group: AccountLoader<'info, Group>,
                                                        
    pub admin: Signer<'info>,

    pub openbook_v2_program: Program<'info, OpenbookV2>,

    #[account(
        constraint = openbook_v2_market_external.load()?.base_mint == base_bank.load()?.mint,
        constraint = openbook_v2_market_external.load()?.quote_mint == quote_bank.load()?.mint,
        constraint = openbook_v2_market_external.load()?.close_market_admin.is_none(),
        constraint = openbook_v2_market_external.load()?.open_orders_admin.is_none(),
        constraint = openbook_v2_market_external.load()?.consume_events_admin.is_none(),
    )]
    pub openbook_v2_market_external: AccountLoader<'info, Market>,

                                                                                 
    #[account(
        mut,
        seeds = [b"OpenbookV2Market".as_ref(), group.key().as_ref(), openbook_v2_market_external.key().as_ref()],
        bump,
    )]
    pub openbook_v2_market: UncheckedAccount<'info>,

                                                                                 
    #[account(
        mut,
        seeds = [b"OpenbookV2Index".as_ref(), group.key().as_ref(), &market_index.to_le_bytes()],
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
