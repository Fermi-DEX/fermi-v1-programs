use std::mem::size_of;

use anchor_lang::prelude::*;
use derivative::Derivative;
use fixed::types::I80F48;

use oracle::oracle_log_context;
use static_assertions::const_assert_eq;

use crate::accounts_zerocopy::KeyedAccountReader;
use crate::error::{Contextable, FermiError};
use crate::logs::{emit_stack, PerpUpdateFundingLogV2};
use crate::state::orderbook::Side;
use crate::state::{oracle, TokenIndex};
use crate::util;

use super::{
    orderbook, OracleAccountInfos, OracleConfig, OracleState, Orderbook, StablePriceModel,
    DAY_I80F48,
};

pub type PerpMarketIndex = u16;

#[repr(C, packed)]
#[account(zero_copy)]
#[derive(Derivative)]
#[derivative(Debug)]
pub struct PerpMarket {
                                                  
    pub group: Pubkey,

                                               
       
                                                                  
                                                 
    pub settle_token_index: TokenIndex,

                                                                                   
                                                                                   
    pub perp_market_index: PerpMarketIndex,

                                                                        
    pub blocked1: u8,

                                                           
    pub group_insurance_fund: u8,

                
    pub bump: u8,

                                                   
       
                                                                      
    pub base_decimals: u8,

                                              
    #[derivative(Debug(format_with = "util::format_zero_terminated_utf8_bytes"))]
    pub name: [u8; 16],

                                                
    pub bids: Pubkey,
                                                
    pub asks: Pubkey,
                                         
    pub event_queue: Pubkey,

                              
    pub oracle: Pubkey,
                            
    pub oracle_config: OracleConfig,
                                                                                 
    pub stable_price_model: StablePriceModel,

                                                                     
       
                                                                                
                                                                             
                                                                              
    pub quote_lot_size: i64,

                                                                   
       
                                                                              
                                                                                
                                            
    pub base_lot_size: i64,

                                                                        
                                                                                
    pub maint_base_asset_weight: I80F48,
    pub init_base_asset_weight: I80F48,
    pub maint_base_liab_weight: I80F48,
    pub init_base_liab_weight: I80F48,

                                                                        
       
                                                                                      
                                                         
    pub open_interest: i64,

                                   
    pub seq_num: u64,

                                                               
    pub registration_time: u64,

              
                                                   
    pub min_funding: I80F48,
                                                   
    pub max_funding: I80F48,
                                                                                 
    pub impact_quantity: i64,

                                                                                
                                                            
       
                                                                           
                           
    pub long_funding: I80F48,
                         
    pub short_funding: I80F48,
                                                  
    pub funding_last_updated: u64,

            

                                         
    pub base_liquidation_fee: I80F48,
                                                        
    pub maker_fee: I80F48,
                                                  
    pub taker_fee: I80F48,

                                             
                                                                                                
    pub fees_accrued: I80F48,
                                             
                                                                                
    pub fees_settled: I80F48,

                                                      
    pub fee_penalty: f32,

                          
                                                                                
                                                                                   
    pub settle_fee_flat: f32,
                                                                     
    pub settle_fee_amount_threshold: f32,
                                                                         
                                       
    pub settle_fee_fraction_low_health: f32,

                          
                                                    
                                                     
       
                                                           
                                                                                                       
                                                          
                                                                                                
                                                                         
                                                             
       
                                                              
    pub settle_pnl_limit_factor: f32,

    #[derivative(Debug = "ignore")]
    pub padding3: [u8; 4],

                                                            
    pub settle_pnl_limit_window_size_ts: u64,

                                                                                 
                                                     
    pub reduce_only: u8,
    pub force_close: u8,

    #[derivative(Debug = "ignore")]
    pub padding4: [u8; 6],

                                                        
    pub maint_overall_asset_weight: I80F48,
    pub init_overall_asset_weight: I80F48,

    pub positive_pnl_liquidation_fee: I80F48,

                                                                
                                                                                         
    pub fees_withdrawn: u64,

                                                                                       
    pub platform_liquidation_fee: I80F48,

                                                                             
       
                                                                                         
                                                                                       
    pub accrued_liquidation_fees: I80F48,

                                                                                  
                                                                                 
    pub unsocialized_loss: I80F48,
    #[derivative(Debug = "ignore")]
    pub reserved: [u8; 1832],
}

const_assert_eq!(
    size_of::<PerpMarket>(),
    32 + 2
        + 2
        + 1
        + 1
        + 16
        + 32
        + 32
        + 32
        + 32
        + 96
        + 288
        + 8
        + 8
        + 16 * 4
        + 8
        + 8
        + 1
        + 1
        + 8
        + 16 * 2
        + 8
        + 16 * 2
        + 8
        + 16 * 5
        + 4
        + 4 * 3
        + 8
        + 8
        + 1
        + 7
        + 3 * 16
        + 8
        + 2 * 16
        + 16
        + 1832
);
const_assert_eq!(size_of::<PerpMarket>(), 2808);
const_assert_eq!(size_of::<PerpMarket>() % 8, 0);

impl PerpMarket {
    pub fn name(&self) -> &str {
        std::str::from_utf8(&self.name)
            .unwrap()
            .trim_matches(char::from(0))
    }

    pub fn is_reduce_only(&self) -> bool {
        self.reduce_only == 1
    }

    pub fn is_force_close(&self) -> bool {
        self.force_close == 1
    }

    pub fn elligible_for_group_insurance_fund(&self) -> bool {
        self.group_insurance_fund == 1
    }

    pub fn set_elligible_for_group_insurance_fund(&mut self, v: bool) {
        self.group_insurance_fund = u8::from(v);
    }

    pub fn settle_pnl_limit_factor(&self) -> I80F48 {
        I80F48::from_num(self.settle_pnl_limit_factor)
    }

    pub fn gen_order_id(&mut self, side: Side, price_data: u64) -> u128 {
        self.seq_num += 1;
        orderbook::new_node_key(side, price_data, self.seq_num)
    }

    pub fn oracle_price<T: KeyedAccountReader>(
        &self,
        oracle_acc_infos: &OracleAccountInfos<T>,
        staleness_slot: Option<u64>,
    ) -> Result<I80F48> {
        Ok(self.oracle_state(oracle_acc_infos, staleness_slot)?.price)
    }

    pub fn oracle_state<T: KeyedAccountReader>(
        &self,
        oracle_acc_infos: &OracleAccountInfos<T>,
        staleness_slot: Option<u64>,
    ) -> Result<OracleState> {
        require_keys_eq!(self.oracle, *oracle_acc_infos.oracle.key());
        let state = oracle::oracle_state_unchecked(oracle_acc_infos, self.base_decimals)?;
        let oracle_config = { self.oracle_config };
        state
            .check_confidence_and_maybe_staleness(&oracle_config, staleness_slot)
            .with_context(|| {
                oracle_log_context(self.name(), &state, &oracle_config, staleness_slot)
            })?;
        Ok(state)
    }

    pub fn stable_price(&self) -> I80F48 {
        I80F48::from_num(self.stable_price_model.stable_price)
    }

                                                                                        
    pub fn update_funding_and_stable_price(
        &mut self,
        book: &Orderbook,
        oracle_state: &OracleState,
        now_ts: u64,
    ) -> Result<()> {
        if now_ts <= self.funding_last_updated {
            return Ok(());
        }

        let oracle_price = oracle_state.price;
        let oracle_price_lots = self.native_price_to_lot(oracle_price);

                                                             
        let bid =
            book.bookside(Side::Bid)
                .impact_price(self.impact_quantity, now_ts, oracle_price_lots);
        let ask =
            book.bookside(Side::Ask)
                .impact_price(self.impact_quantity, now_ts, oracle_price_lots);

        let funding_rate = match (bid, ask) {
            (Some(bid), Some(ask)) => {
                                            
                let mid_price = (bid + ask) / 2;
                let book_price = self.lot_to_native_price(mid_price);
                let diff = book_price / oracle_price - I80F48::ONE;
                diff.clamp(self.min_funding, self.max_funding)
            }
            (Some(_bid), None) => self.max_funding,
            (None, Some(_ask)) => self.min_funding,
            (None, None) => I80F48::ZERO,
        };

                                                                                               
                                                                                                 
                                                                                                 
                           
        let max_funding_timestep = 3600;            
        let diff_ts =
            I80F48::from_num((now_ts - self.funding_last_updated as u64).min(max_funding_timestep));

        let time_factor = diff_ts / DAY_I80F48;
        let base_lot_size = I80F48::from_num(self.base_lot_size);

                                                                             
        let funding_delta = oracle_price * base_lot_size * funding_rate * time_factor;

        self.long_funding = { self.long_funding } + funding_delta;
        self.short_funding = { self.short_funding } + funding_delta;
        self.funding_last_updated = now_ts;

        let mut spm = { self.stable_price_model };
        spm.update(now_ts, oracle_price.to_num());
        self.stable_price_model = spm;

        emit_stack(PerpUpdateFundingLogV2 {
            fermi_group: self.group,
            market_index: self.perp_market_index,
            long_funding: self.long_funding.to_bits(),
            short_funding: self.short_funding.to_bits(),
            price: oracle_price.to_bits(),
            oracle_slot: oracle_state.last_update_slot,
            oracle_confidence: oracle_state.deviation.to_bits(),
            oracle_type: oracle_state.oracle_type,
            stable_price: self.stable_price().to_bits(),
            fees_accrued: self.fees_accrued.to_bits(),
            fees_settled: self.fees_settled.to_bits(),
            open_interest: self.open_interest,
            instantaneous_funding_rate: funding_rate.to_bits(),
        });

        Ok(())
    }

                                                                                         
    pub fn lot_to_native_price(&self, price: i64) -> I80F48 {
        I80F48::from_num(price) * I80F48::from_num(self.quote_lot_size)
            / I80F48::from_num(self.base_lot_size)
    }

    pub fn native_price_to_lot(&self, price: I80F48) -> i64 {
        (price * I80F48::from_num(self.base_lot_size) / I80F48::from_num(self.quote_lot_size))
            .to_num()
    }

                                                                                                  
    pub fn inside_price_limit(
        &self,
        side: Side,
        native_price: I80F48,
        oracle_price: I80F48,
    ) -> bool {
        match side {
            Side::Bid => native_price <= (self.maint_base_liab_weight * oracle_price),
            Side::Ask => native_price >= (self.maint_base_asset_weight * oracle_price),
        }
    }

                                                                      
       
                                              
    pub fn socialize_loss(&mut self, loss: I80F48) -> Result<I80F48> {
        require_gte!(0, loss);

                                                         
                                                         
        let socialized_loss = if self.open_interest == 0 {
                                                                                  
                                                                                            
                                    
            self.unsocialized_loss = { self.unsocialized_loss } + loss;                                         
            I80F48::ZERO
        } else {
            loss / I80F48::from(self.open_interest)
        };
        self.long_funding = { self.long_funding } - socialized_loss;
        self.short_funding = { self.short_funding } + socialized_loss;
        Ok(socialized_loss)
    }

                                                                                              
                                                        
    pub fn compute_settle_fee(
        &self,
        settlement: I80F48,
        source_pnl_value: I80F48,
        source_position_value: I80F48,
        source_liq_end_health: I80F48,
        source_maint_health: I80F48,
    ) -> Result<I80F48> {
                                                              
          
                                                                                 
                                                                                          
          
                                                          
        let pnl_at_least_one_percent = I80F48::from(100) * source_pnl_value > source_position_value;
        if !pnl_at_least_one_percent {
            return Ok(I80F48::ZERO);
        }

        assert!(source_maint_health >= source_liq_end_health);

                                                                                           
                                                                                         
                                                                
        let low_health_fee = if source_liq_end_health < 0 {
            let fee_fraction = I80F48::from_num(self.settle_fee_fraction_low_health);
            if source_maint_health < 0 {
                settlement * fee_fraction
            } else {
                settlement
                    * fee_fraction
                    * (-source_liq_end_health / (source_maint_health - source_liq_end_health))
            }
        } else {
            I80F48::ZERO
        };

        let flat_fee = I80F48::from_num(self.settle_fee_flat);

        let mut fee = if settlement >= { self.settle_fee_amount_threshold } {
                                                                 
            flat_fee
        } else {
                                                                               
            low_health_fee.min(flat_fee * I80F48::from(2))
        };

                                                            
        fee = fee.min(settlement);

                                                                   
        require!(fee >= 0, FermiError::SettlementAmountMustBePositive);

        Ok(fee)
    }

                                        
    pub fn default_for_tests() -> PerpMarket {
        PerpMarket {
            group: Pubkey::new_unique(),
            settle_token_index: 0,
            perp_market_index: 0,
            blocked1: 0,
            group_insurance_fund: 0,
            bump: 0,
            base_decimals: 0,
            name: Default::default(),
            bids: Pubkey::new_unique(),
            asks: Pubkey::new_unique(),
            event_queue: Pubkey::new_unique(),
            oracle: Pubkey::new_unique(),
            oracle_config: OracleConfig {
                conf_filter: I80F48::ZERO,
                max_staleness_slots: -1,
                reserved: [0; 72],
            },
            stable_price_model: StablePriceModel::default(),
            quote_lot_size: 1,
            base_lot_size: 1,
            maint_base_asset_weight: I80F48::from(1),
            init_base_asset_weight: I80F48::from(1),
            maint_base_liab_weight: I80F48::from(1),
            init_base_liab_weight: I80F48::from(1),
            open_interest: 0,
            seq_num: 0,
            registration_time: 0,
            min_funding: I80F48::ZERO,
            max_funding: I80F48::ZERO,
            impact_quantity: 0,
            long_funding: I80F48::ZERO,
            short_funding: I80F48::ZERO,
            funding_last_updated: 0,
            base_liquidation_fee: I80F48::ZERO,
            maker_fee: I80F48::ZERO,
            taker_fee: I80F48::ZERO,
            fees_accrued: I80F48::ZERO,
            fees_settled: I80F48::ZERO,
            fee_penalty: 0.0,
            settle_fee_flat: 0.0,
            settle_fee_amount_threshold: 0.0,
            settle_fee_fraction_low_health: 0.0,
            settle_pnl_limit_factor: 0.2,
            padding3: Default::default(),
            settle_pnl_limit_window_size_ts: 24 * 60 * 60,
            reduce_only: 0,
            force_close: 0,
            padding4: Default::default(),
            maint_overall_asset_weight: I80F48::ONE,
            init_overall_asset_weight: I80F48::ONE,
            positive_pnl_liquidation_fee: I80F48::ZERO,
            fees_withdrawn: 0,
            platform_liquidation_fee: I80F48::ZERO,
            accrued_liquidation_fees: I80F48::ZERO,
            unsocialized_loss: I80F48::ZERO,
            reserved: [0; 1832],
        }
    }
}
