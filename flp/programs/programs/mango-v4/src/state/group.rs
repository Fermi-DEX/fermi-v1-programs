use anchor_lang::prelude::*;
use static_assertions::const_assert_eq;
use std::mem::size_of;

                                                       
pub type TokenIndex = u16;

                                                                       
   
                                                                                
                                                                          
                         
pub const QUOTE_TOKEN_INDEX: TokenIndex = 0;

                                                   
   
                                                                                   
                 
pub const PERP_SETTLE_TOKEN_INDEX: TokenIndex = 0;

                                                                           
pub const FEE_BUYBACK_QUOTE_TOKEN_INDEX: TokenIndex = 0;

#[repr(C, packed)]
#[account(zero_copy)]
#[derive(Debug)]
pub struct Group {
                                                  
    pub creator: Pubkey,

                                                   
    pub group_num: u32,

    pub admin: Pubkey,

                                                                                                  
    pub fast_listing_admin: Pubkey,

                                                                    
    pub mngo_token_index: TokenIndex,
    pub padding: [u8; 2],

    pub insurance_vault: Pubkey,
    pub insurance_mint: Pubkey,

    pub bump: u8,

    pub testing: u8,

    pub version: u8,

                                                                         
    pub buyback_fees: u8,
                                                            
                                                                                                                    
    pub buyback_fees_mngo_bonus_factor: f32,

    pub address_lookup_tables: [Pubkey; 20],

    pub security_admin: Pubkey,

                                                                                                   
                                                                         
    pub deposit_limit_quote: u64,

                                           
                                                                                              
    pub ix_gate: u128,

                              
                                                                               
                                                                        
                                                 
                                                                      
                                                                
                                                            
    pub buyback_fees_swap_fermi_account: Pubkey,

                                                                                                   
       
                                                                                     
                                                                                              
       
                                                         
    pub buyback_fees_expiry_interval: u64,

                                                                                                  
                              
    pub fast_listing_interval_start: u64,

                                                           
    pub fast_listings_in_interval: u16,

                                                             
    pub allowed_fast_listings_per_interval: u16,

    pub padding2: [u8; 4],

                                                    
    pub collateral_fee_interval: u64,

    pub reserved: [u8; 1800],
}
const_assert_eq!(
    size_of::<Group>(),
    32 + 4
        + 32 * 2
        + 4
        + 32 * 2
        + 4
        + 4
        + 20 * 32
        + 32
        + 8
        + 16
        + 32
        + 8
        + 8
        + 2 * 2
        + 4
        + 8
        + 1800
);
const_assert_eq!(size_of::<Group>(), 2736);
const_assert_eq!(size_of::<Group>() % 8, 0);

impl Group {
    pub fn buyback_fees(&self) -> bool {
        self.buyback_fees == 1
    }

    pub fn is_testing(&self) -> bool {
        self.testing == 1
    }

    pub fn multiple_banks_supported(&self) -> bool {
        self.is_testing() || self.version > 1
    }

    pub fn serum3_supported(&self) -> bool {
        self.is_testing() || self.version > 0
    }

    pub fn perps_supported(&self) -> bool {
        self.is_testing() || self.version > 1
    }

    pub fn is_ix_enabled(&self, ix: IxGate) -> bool {
        self.ix_gate & (1 << ix as u128) == 0
    }
}

                                
         
                      
                         
                            
                
                         
                 
               
                
                  
                     
                
#[derive(Copy, Clone, Debug)]
pub enum IxGate {
    AccountClose = 0,
    AccountCreate = 1,
    AccountEdit = 2,
    AccountExpand = 3,
    AccountToggleFreeze = 4,
    AltExtend = 5,
    AltSet = 6,
    FlashLoan = 7,
    GroupClose = 8,
    GroupCreate = 9,
    HealthRegion = 10,
    PerpCancelAllOrders = 11,
    PerpCancelAllOrdersBySide = 12,
    PerpCancelOrder = 13,
    PerpCancelOrderByClientOrderId = 14,
    PerpCloseMarket = 15,
    PerpConsumeEvents = 16,
    PerpCreateMarket = 17,
    PerpDeactivatePosition = 18,
    PerpLiqBaseOrPositivePnl = 19,
    PerpLiqForceCancelOrders = 20,
    PerpLiqNegativePnlOrBankruptcy = 21,
    PerpPlaceOrder = 22,
    PerpSettleFees = 23,
    PerpSettlePnl = 24,
    PerpUpdateFunding = 25,
    Serum3CancelAllOrders = 26,
    Serum3CancelOrder = 27,
    Serum3CloseOpenOrders = 28,
    Serum3CreateOpenOrders = 29,
    Serum3DeregisterMarket = 30,
    Serum3EditMarket = 31,                                                             
    Serum3LiqForceCancelOrders = 32,
    Serum3PlaceOrder = 33,
    Serum3RegisterMarket = 34,
    Serum3SettleFunds = 35,
    StubOracleClose = 36,
    StubOracleCreate = 37,
    StubOracleSet = 38,
    TokenAddBank = 39,
    TokenDeposit = 40,
    TokenDeregister = 41,
    TokenLiqBankruptcy = 42,
    TokenLiqWithToken = 43,
    TokenRegister = 44,
    TokenRegisterTrustless = 45,
    TokenUpdateIndexAndRate = 46,
    TokenWithdraw = 47,
    AccountBuybackFeesWithMngo = 48,
    TokenForceCloseBorrowsWithToken = 49,
    PerpForceClosePosition = 50,
    GroupWithdrawInsuranceFund = 51,
    TokenConditionalSwapCreate = 52,
    TokenConditionalSwapTrigger = 53,
    TokenConditionalSwapCancel = 54,
    OpenbookV2CancelOrder = 55,
    OpenbookV2CloseOpenOrders = 56,
    OpenbookV2CreateOpenOrders = 57,
    OpenbookV2DeregisterMarket = 58,
    OpenbookV2EditMarket = 59,
    OpenbookV2LiqForceCancelOrders = 60,
    OpenbookV2PlaceOrder = 61,
    OpenbookV2PlaceTakeOrder = 62,
    OpenbookV2RegisterMarket = 63,
    OpenbookV2SettleFunds = 64,
    AdminTokenWithdrawFees = 65,
    AdminPerpWithdrawFees = 66,
    AccountSizeMigration = 67,
    TokenConditionalSwapStart = 68,
    TokenConditionalSwapCreatePremiumAuction = 69,
    TokenConditionalSwapCreateLinearAuction = 70,
    Serum3PlaceOrderV2 = 71,
    TokenForceWithdraw = 72,
    SequenceCheck = 73,
    HealthCheck = 74,
    OpenbookV2CancelAllOrders = 75,
    GroupChangeInsuranceFund = 76,
    AdminWithdrawExcessLamports = 77,
    V5DelegateIntents = 78,
                                                                                                 
}

                                                                   
#[macro_export]
macro_rules! group_seeds {
    ( $group:expr ) => {
        &[
            b"Group".as_ref(),
            $group.creator.as_ref(),
            &$group.group_num.to_le_bytes(),
            &[$group.bump],
        ]
    };
}

pub use group_seeds;
