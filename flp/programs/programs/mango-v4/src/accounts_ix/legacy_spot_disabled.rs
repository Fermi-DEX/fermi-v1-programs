use anchor_lang::prelude::*;

#[derive(Clone, Copy, AnchorSerialize, AnchorDeserialize)]
#[repr(u8)]
pub enum Serum3SelfTradeBehavior {
    DecrementTake = 0,
    CancelProvide = 1,
    AbortTransaction = 2,
}

#[derive(Clone, Copy, AnchorSerialize, AnchorDeserialize)]
#[repr(u8)]
pub enum Serum3OrderType {
    Limit = 0,
    ImmediateOrCancel = 1,
    PostOnly = 2,
}

#[derive(Clone, Copy, AnchorSerialize, AnchorDeserialize)]
#[repr(u8)]
pub enum Serum3Side {
    Bid = 0,
    Ask = 1,
}

#[derive(Copy, Clone, AnchorSerialize, AnchorDeserialize)]
#[repr(u8)]
pub enum OpenbookV2PlaceOrderType {
    Limit = 0,
    ImmediateOrCancel = 1,
    PostOnly = 2,
    Market = 3,
    PostOnlySlide = 4,
}

#[derive(Copy, Clone, AnchorSerialize, AnchorDeserialize)]
#[repr(u8)]
pub enum OpenbookV2PostOrderType {
    Limit = 0,
    PostOnly = 2,
    PostOnlySlide = 4,
}

#[derive(Copy, Clone, AnchorSerialize, AnchorDeserialize)]
#[repr(u8)]
pub enum OpenbookV2SelfTradeBehavior {
    DecrementTake = 0,
    CancelProvide = 1,
    AbortTransaction = 2,
}

#[derive(Copy, Clone, AnchorSerialize, AnchorDeserialize)]
#[repr(u8)]
pub enum OpenbookV2Side {
    Bid = 0,
    Ask = 1,
}

macro_rules! disabled_spot_context {
    ($name:ident) => {
        #[derive(Accounts)]
        pub struct $name<'info> {
                                                                           
            pub group: UncheckedAccount<'info>,
        }
    };
}

disabled_spot_context!(Serum3RegisterMarket);
disabled_spot_context!(Serum3EditMarket);
disabled_spot_context!(Serum3DeregisterMarket);
disabled_spot_context!(Serum3CreateOpenOrders);
disabled_spot_context!(Serum3CloseOpenOrders);
disabled_spot_context!(Serum3PlaceOrder);
disabled_spot_context!(Serum3CancelOrder);
disabled_spot_context!(Serum3CancelAllOrders);
disabled_spot_context!(Serum3SettleFunds);
disabled_spot_context!(Serum3SettleFundsV2);
disabled_spot_context!(Serum3LiqForceCancelOrders);

disabled_spot_context!(OpenbookV2RegisterMarket);
disabled_spot_context!(OpenbookV2EditMarket);
disabled_spot_context!(OpenbookV2DeregisterMarket);
disabled_spot_context!(OpenbookV2CreateOpenOrders);
disabled_spot_context!(OpenbookV2CloseOpenOrders);
disabled_spot_context!(OpenbookV2PlaceOrder);
disabled_spot_context!(OpenbookV2CancelOrder);
disabled_spot_context!(OpenbookV2SettleFunds);
disabled_spot_context!(OpenbookV2LiqForceCancelOrders);
