use anchor_lang::prelude::*;

                                                                                 
                                                       

#[derive(AnchorSerialize, AnchorDeserialize, Default)]
pub struct TokenIndex {
    val: u16,
}

#[derive(AnchorSerialize, AnchorDeserialize, Default)]
pub struct Serum3MarketIndex {
    val: u16,
}

#[derive(AnchorSerialize, AnchorDeserialize, Default)]
pub struct PerpMarketIndex {
    val: u16,
}

#[derive(AnchorSerialize, AnchorDeserialize, Default)]
pub struct I80F48 {
    val: i128,
}
