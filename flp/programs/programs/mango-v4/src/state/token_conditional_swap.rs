use anchor_lang::prelude::*;

use derivative::Derivative;
use fixed::types::I80F48;
use num_enum::{IntoPrimitive, TryFromPrimitive};
use static_assertions::const_assert_eq;
use std::mem::size_of;

use crate::error::FermiError;
use crate::i80f48::ClampToInt;
use crate::state::*;

                                                              
pub const TCS_START_INCENTIVE: u64 = 1_000;                                      

#[derive(
    Clone, Copy, PartialEq, Eq, IntoPrimitive, TryFromPrimitive, AnchorDeserialize, AnchorSerialize,
)]
#[repr(u8)]
pub enum TokenConditionalSwapDisplayPriceStyle {
    SellTokenPerBuyToken,
    BuyTokenPerSellToken,
}

#[derive(
    Clone, Copy, PartialEq, Eq, IntoPrimitive, TryFromPrimitive, AnchorDeserialize, AnchorSerialize,
)]
#[repr(u8)]
pub enum TokenConditionalSwapIntention {
    Unknown,
                                                     
    StopLoss,
                                                      
    TakeProfit,
}

#[derive(
    Clone, Copy, PartialEq, Eq, IntoPrimitive, TryFromPrimitive, AnchorDeserialize, AnchorSerialize,
)]
#[repr(u8)]
pub enum TokenConditionalSwapType {
    FixedPremium,
    PremiumAuction,
    LinearAuction,
}

#[zero_copy]
#[derive(AnchorDeserialize, AnchorSerialize, Derivative, PartialEq)]
#[derivative(Debug)]
pub struct TokenConditionalSwap {
    pub id: u64,

                                                      
    pub max_buy: u64,
    pub max_sell: u64,

                                                       
    pub bought: u64,
    pub sold: u64,

                                                           
    pub expiry_timestamp: u64,

                                    
                                                                                     
                                                                               
                                                                                       
       
                                                                                      
                                                                
       
                                            
       
                                                                
       
                                                                                  
                                                                                     
       
                                                                                       
                                                                                        
                                                                                         
                                                                           
       
                                                                                
                                                                                 
                                                            
    pub price_lower_limit: f64,

                                                                              
    pub price_upper_limit: f64,

                                                                      
    pub price_premium_rate: f64,

                                                                    
    pub taker_fee_rate: f32,

                                                                 
    pub maker_fee_rate: f32,

                                      
    pub buy_token_index: TokenIndex,
    pub sell_token_index: TokenIndex,

                                                                           
    pub is_configured: u8,

                                                                                           
    pub allow_creating_deposits: u8,
                                                                                      
    pub allow_creating_borrows: u8,

                                                                                
                                                                                       
                                        
       
                                                                 
    pub display_price_style: u8,

                                                                        
       
                                                         
    pub intention: u8,

                                                    
    pub tcs_type: u8,

    pub padding: [u8; 6],

                                                    
       
                                                                  
                                                                                                  
                                                                                                    
    pub start_timestamp: u64,

                                         
       
                             
                                                                                              
                                                                                         
    pub duration_seconds: u64,

    #[derivative(Debug = "ignore")]
    pub reserved: [u8; 88],
}

const_assert_eq!(
    size_of::<TokenConditionalSwap>(),
    8 * 6 + 8 * 3 + 2 * 4 + 2 * 2 + 1 * 6 + 6 + 2 * 8 + 88
);
const_assert_eq!(size_of::<TokenConditionalSwap>(), 200);
const_assert_eq!(size_of::<TokenConditionalSwap>() % 8, 0);

impl Default for TokenConditionalSwap {
    fn default() -> Self {
        Self {
            id: 0,
            max_buy: 0,
            max_sell: 0,
            bought: 0,
            sold: 0,
            expiry_timestamp: u64::MAX,
            price_lower_limit: 0.0,
            price_upper_limit: 0.0,
            price_premium_rate: 0.0,
            taker_fee_rate: 0.0,
            maker_fee_rate: 0.0,
            buy_token_index: TokenIndex::MAX,
            sell_token_index: TokenIndex::MAX,
            is_configured: 0,
            allow_creating_borrows: 0,
            allow_creating_deposits: 0,
            display_price_style: TokenConditionalSwapDisplayPriceStyle::SellTokenPerBuyToken.into(),
            intention: TokenConditionalSwapIntention::Unknown.into(),
            tcs_type: TokenConditionalSwapType::FixedPremium.into(),
            padding: Default::default(),
            start_timestamp: 0,
            duration_seconds: 0,
            reserved: [0; 88],
        }
    }
}

impl TokenConditionalSwap {
                                   
       
                                                                          
                                                
    pub fn is_configured(&self) -> bool {
        self.is_configured == 1
    }

    pub fn set_is_configured(&mut self, is_configured: bool) {
        self.is_configured = u8::from(is_configured);
    }

    pub fn tcs_type(&self) -> TokenConditionalSwapType {
        self.tcs_type.try_into().unwrap()
    }

    pub fn is_expired(&self, now_ts: u64) -> bool {
        now_ts >= self.expiry_timestamp
    }

    pub fn passed_start(&self, now_ts: u64) -> bool {
        self.start_timestamp > 0 && now_ts >= self.start_timestamp
    }

                                                                          
    pub fn is_startable_type(&self) -> bool {
        self.tcs_type() == TokenConditionalSwapType::PremiumAuction
    }

    pub fn allow_creating_deposits(&self) -> bool {
        self.allow_creating_deposits == 1
    }

    pub fn allow_creating_borrows(&self) -> bool {
        self.allow_creating_borrows == 1
    }

    pub fn remaining_buy(&self) -> u64 {
        self.max_buy - self.bought
    }

    pub fn remaining_sell(&self) -> u64 {
        self.max_sell - self.sold
    }

    fn start_timestamp_or_now(&self, now_ts: u64) -> u64 {
        if self.start_timestamp > 0 {
            self.start_timestamp
        } else {
            now_ts
        }
    }

                                           
       
                                                                        
    pub fn premium_price(&self, base_price: f64, now_ts: u64) -> f64 {
        match self.tcs_type() {
            TokenConditionalSwapType::FixedPremium => base_price * (1.0 + self.price_premium_rate),
            TokenConditionalSwapType::PremiumAuction => {
                                                     
                let start = self.start_timestamp_or_now(now_ts);
                assert!(start <= now_ts);

                let duration = self.duration_seconds as f64;
                let current = (now_ts - start) as f64;
                let progress = (current / duration).min(1.0);
                base_price * (1.0 + progress * self.price_premium_rate)
            }
            TokenConditionalSwapType::LinearAuction => {
                                      
                assert!(self.passed_start(now_ts));

                let duration = self.duration_seconds;
                let current = now_ts - self.start_timestamp;
                if current < duration {
                    let progress = (current as f64) / (duration as f64);
                    self.price_lower_limit
                        + progress * (self.price_upper_limit - self.price_lower_limit)
                } else {
                                                                         
                    self.price_upper_limit
                }
            }
        }
    }

                                                
    pub fn maker_price(&self, premium_price: f64) -> f64 {
        premium_price * (1.0 + self.maker_fee_rate as f64)
    }

                                                
    pub fn taker_price(&self, premium_price: f64) -> f64 {
        premium_price * (1.0 - self.taker_fee_rate as f64)
    }

    pub fn maker_fee(&self, base_sell_amount: I80F48) -> u64 {
        (base_sell_amount * I80F48::from_num(self.maker_fee_rate))
            .floor()
            .to_num()
    }

    pub fn taker_fee(&self, base_sell_amount: I80F48) -> u64 {
        (base_sell_amount * I80F48::from_num(self.taker_fee_rate))
            .floor()
            .to_num()
    }

    fn price_in_range(&self, price: f64) -> bool {
        price >= self.price_lower_limit && price <= self.price_upper_limit
    }

                                                              
    pub fn check_startable(&self, price: f64, now_ts: u64) -> Result<()> {
        require!(
            !self.is_expired(now_ts),
            FermiError::TokenConditionalSwapExpired
        );
        require!(
            self.start_timestamp == 0,
            FermiError::TokenConditionalSwapAlreadyStarted
        );
        require!(
            self.is_startable_type(),
            FermiError::TokenConditionalSwapTypeNotStartable
        );
        require!(
            self.price_in_range(price),
            FermiError::TokenConditionalSwapPriceNotInRange
        );
        Ok(())
    }

    pub fn is_startable(&self, price: f64, now_ts: u64) -> bool {
        self.check_startable(price, now_ts).is_ok()
    }

    pub fn check_triggerable(&self, price: f64, now_ts: u64) -> Result<()> {
        require!(
            !self.is_expired(now_ts),
            FermiError::TokenConditionalSwapExpired
        );
        match self.tcs_type() {
            TokenConditionalSwapType::FixedPremium => {
                require!(
                    self.price_in_range(price),
                    FermiError::TokenConditionalSwapPriceNotInRange
                );
            }
            TokenConditionalSwapType::PremiumAuction | TokenConditionalSwapType::LinearAuction => {
                                                                              
                require!(
                    self.passed_start(now_ts),
                    FermiError::TokenConditionalSwapNotStarted
                );
            }
        }
        Ok(())
    }

    pub fn is_triggerable(&self, price: f64, now_ts: u64) -> bool {
        self.check_triggerable(price, now_ts).is_ok()
    }

                                                                           
                                                   
       
                                                                      
    pub fn max_buy_for_position(&self, buy_position: I80F48, buy_bank: &Bank) -> u64 {
        self.remaining_buy().min(
            if self.allow_creating_deposits() && !buy_bank.are_deposits_reduce_only() {
                u64::MAX
            } else {
                                                                                
                (-buy_position).ceil().clamp_to_u64()
            },
        )
    }

                                                                             
                                                    
       
                                                                      
    pub fn max_sell_for_position(&self, sell_position: I80F48, sell_bank: &Bank) -> u64 {
        self.remaining_sell().min(
            if self.allow_creating_borrows() && !sell_bank.are_borrows_reduce_only() {
                u64::MAX
            } else {
                                                 
                sell_position.floor().clamp_to_u64()
            },
        )
    }
}
