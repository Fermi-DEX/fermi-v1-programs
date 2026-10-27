                                                                                  
                                                                              
use anchor_lang::prelude::*;
use fixed::types::I80F48;
use mango_v4::{health::{AccountRetriever, Prices}, state::{FermiAccountRef, TokenIndex}};
use crate::error::FlpError;

pub struct PerpValue {
    pub base_lot_size: i64,
    pub base_lots: i64,
    pub quote: I80F48,
    pub base_prices: Prices,
}
pub struct Valuation {
    pub quote_cash: I80F48,
    pub quote_prices: Option<Prices>,
    pub perp_infos: Vec<PerpValue>,
}
impl Valuation {
    pub fn load(account: &FermiAccountRef, retriever: &impl AccountRetriever,
        quote_token_index: TokenIndex) -> Result<Self> {
        let h = &account.header;
        require!(h.token_count() == 1 && h.serum3_count() == 0 && h.openbook_v2_count() == 0
            && h.token_conditional_swap_count() == 0, FlpError::UnsupportedPortfolio);
        let mut quote_cash = I80F48::ZERO;
        let mut quote_prices = None;
        if let Some(position) = account.active_token_positions().next() {
            require!(position.token_index == quote_token_index, FlpError::UnsupportedPortfolio);
            let (bank, oracle) = retriever.bank_and_oracle(&account.fixed.group, 0, quote_token_index)?;
            quote_cash = position.native(bank);
            quote_prices = Some(Prices { oracle, stable: bank.stable_price() });
        }
        let mut perp_infos = Vec::with_capacity(account.active_perp_positions().count());
        for (i, position) in account.active_perp_positions().enumerate() {
            let (market, oracle) = retriever.perp_market_and_oracle_price(
                &account.fixed.group, i, position.market_index)?;
            require!(market.settle_token_index == quote_token_index, FlpError::UnsupportedPortfolio);
                                                                              
            let base_lots = position.base_position_lots().checked_add(position.taker_base_lots)
                .ok_or(FlpError::MathOverflow)?;
            let taker_quote = I80F48::from(position.taker_quote_lots.checked_mul(market.quote_lot_size)
                .ok_or(FlpError::MathOverflow)?);
            let quote = position.quote_position_native() - position.unsettled_funding(market) + taker_quote;
            perp_infos.push(PerpValue {
                base_lot_size: market.base_lot_size,
                base_lots, quote,
                base_prices: Prices { oracle, stable: market.stable_price() },
            });
        }
        Ok(Self { quote_cash, quote_prices, perp_infos })
    }
}
