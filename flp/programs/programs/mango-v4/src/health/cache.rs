   
                                                                                  
  
                                                                                     
                                                                               
                                                                              
  
                                                                      
  
                                                                             
                                                                          
            
  
                                                                                 
   

use anchor_lang::prelude::*;

use fixed::types::I80F48;
#[cfg(feature = "legacy-spot")]
use openbook_v2::state::OpenOrdersAccount;

use crate::error::*;
use crate::i80f48::LowPrecisionDivision;
#[cfg(feature = "legacy-spot")]
use crate::serum3_cpi::{OpenOrdersAmounts, OpenOrdersSlim};
use crate::state::{
    Bank, FermiAccountRef, OpenbookV2MarketIndex, OpenbookV2Orders, PerpMarket, PerpMarketIndex,
    PerpPosition, Serum3MarketIndex, Serum3Orders, TokenIndex,
};

use super::*;

                                                       
#[derive(Clone, Debug)]
pub struct Prices {
                                
    pub oracle: I80F48,                 

                                                      
    pub stable: I80F48,                 
}

impl Prices {
                         
    pub fn new_single_price(price: I80F48) -> Self {
        Self {
            oracle: price,
            stable: price,
        }
    }

                                                            
    #[inline(always)]
    pub fn liab(&self, health_type: HealthType) -> I80F48 {
        match health_type {
            HealthType::Maint | HealthType::LiquidationEnd => self.oracle,
            HealthType::Init => self.oracle.max(self.stable),
        }
    }

                                                        
    #[inline(always)]
    pub fn asset(&self, health_type: HealthType) -> I80F48 {
        match health_type {
            HealthType::Maint | HealthType::LiquidationEnd => self.oracle,
            HealthType::Init => self.oracle.min(self.stable),
        }
    }
}

                                    
                                                                             
                                                                    
                                                                                 
                                  
   
                   
                                                           
   
                                                                                  
                                                                          
                                                                 
                                                
   
#[derive(PartialEq, Copy, Clone, AnchorSerialize, AnchorDeserialize)]
pub enum HealthType {
    Init,
    Maint,                        
    LiquidationEnd,
}

                                                                    
   
                                                                                        
pub fn compute_health_from_fixed_accounts(
    account: &FermiAccountRef,
    health_type: HealthType,
    ais: &[AccountInfo],
    now_ts: u64,
) -> Result<I80F48> {
    let retriever = new_fixed_order_account_retriever(ais, account, Clock::get()?.slot)?;
    Ok(new_health_cache(account, &retriever, now_ts)?.health(health_type))
}

                                                     
pub fn compute_health(
    account: &FermiAccountRef,
    health_type: HealthType,
    retriever: &impl AccountRetriever,
    now_ts: u64,
) -> Result<I80F48> {
    Ok(new_health_cache(account, retriever, now_ts)?.health(health_type))
}

                                                                          
   
                                     
pub fn spot_amount_taken_for_health_zero(
    mut health: I80F48,
    starting_spot: I80F48,
    asset_weighted_price: I80F48,
    liab_weighted_price: I80F48,
) -> Result<I80F48> {
    if health <= 0 {
        return Ok(I80F48::ZERO);
    }

    let mut taken_spot = I80F48::ZERO;
    if starting_spot > 0 {
        if asset_weighted_price > 0 {
            let asset_max = health / asset_weighted_price;
            if asset_max <= starting_spot {
                return Ok(asset_max);
            }
        }
        taken_spot = starting_spot;
        health -= starting_spot * asset_weighted_price;
    }
    if health > 0 {
        require_gt!(liab_weighted_price, 0);
        taken_spot += health / liab_weighted_price;
    }
    Ok(taken_spot)
}

                                                                      
   
                                    
pub fn spot_amount_given_for_health_zero(
    health: I80F48,
    starting_spot: I80F48,
    asset_weighted_price: I80F48,
    liab_weighted_price: I80F48,
) -> Result<I80F48> {
                                                   
    spot_amount_taken_for_health_zero(
        -health,
        -starting_spot,
        liab_weighted_price,
        asset_weighted_price,
    )
}

#[derive(Clone, Debug)]
pub struct TokenInfo {
    pub token_index: TokenIndex,
    pub maint_asset_weight: I80F48,
    pub init_asset_weight: I80F48,
    pub init_scaled_asset_weight: I80F48,
    pub maint_liab_weight: I80F48,
    pub init_liab_weight: I80F48,
    pub init_scaled_liab_weight: I80F48,
    pub prices: Prices,

                                                    
       
                                                                  
                                                              
    pub balance_spot: I80F48,

    pub allow_asset_liquidation: bool,
}

                                                   
#[derive(Clone, Default)]
pub struct TokenBalance {
                                                                             
    pub spot_and_perp: I80F48,
}

#[derive(Clone, Default)]
pub struct TokenMaxReserved {
                                                                          
    pub max_spot_reserved: I80F48,
}

impl TokenInfo {
    #[inline(always)]
    fn asset_weight(&self, health_type: HealthType) -> I80F48 {
        match health_type {
            HealthType::Init => self.init_scaled_asset_weight,
            HealthType::LiquidationEnd => self.init_asset_weight,
            HealthType::Maint => self.maint_asset_weight,
        }
    }

    #[inline(always)]
    pub fn asset_weighted_price(&self, health_type: HealthType) -> I80F48 {
        self.asset_weight(health_type) * self.prices.asset(health_type)
    }

    #[inline(always)]
    fn liab_weight(&self, health_type: HealthType) -> I80F48 {
        match health_type {
            HealthType::Init => self.init_scaled_liab_weight,
            HealthType::LiquidationEnd => self.init_liab_weight,
            HealthType::Maint => self.maint_liab_weight,
        }
    }

    #[inline(always)]
    pub fn liab_weighted_price(&self, health_type: HealthType) -> I80F48 {
        self.liab_weight(health_type) * self.prices.liab(health_type)
    }

    #[inline(always)]
    pub fn health_contribution(&self, health_type: HealthType, balance: I80F48) -> I80F48 {
        let weighted_price = if balance.is_negative() {
            self.liab_weighted_price(health_type)
        } else {
            self.asset_weighted_price(health_type)
        };
        balance * weighted_price
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum SpotMarketIndex {
    Serum3(Serum3MarketIndex),
    OpenbookV2(OpenbookV2MarketIndex),
}

                                                                                    
   
                                                                         
                                                                         
                                                                       
                               
#[derive(Clone, Debug)]
pub struct SpotInfo {
                                                    
    pub reserved_base: I80F48,
    pub reserved_quote: I80F48,

                                                                                                  
                                                                                                
    pub reserved_base_as_quote_lowest_ask: I80F48,
    pub reserved_quote_as_base_highest_bid: I80F48,

                                               
    pub base_info_index: usize,
    pub quote_info_index: usize,

    pub spot_market_index: SpotMarketIndex,

                                                             
    pub has_zero_funds: bool,
}

impl SpotInfo {
    #[cfg(feature = "legacy-spot")]
    fn new_from_serum(
        serum_account: &Serum3Orders,
        open_orders: &impl OpenOrdersAmounts,
        base_info_index: usize,
        quote_info_index: usize,
    ) -> Self {
                                     
        let reserved_base = I80F48::from(open_orders.native_base_reserved());
        let reserved_quote = I80F48::from(open_orders.native_quote_reserved());

        let reserved_base_as_quote_lowest_ask =
            reserved_base * I80F48::from_num(serum_account.lowest_placed_ask);
        let reserved_quote_as_base_highest_bid =
            reserved_quote * I80F48::from_num(serum_account.highest_placed_bid_inv);

        Self {
            reserved_base,
            reserved_quote,
            reserved_base_as_quote_lowest_ask,
            reserved_quote_as_base_highest_bid,
            base_info_index,
            quote_info_index,
            spot_market_index: SpotMarketIndex::Serum3(serum_account.market_index),
            has_zero_funds: open_orders.native_base_total() == 0
                && open_orders.native_quote_total() == 0
                && open_orders.native_rebates() == 0,
        }
    }

    #[cfg(feature = "legacy-spot")]
    fn new_from_openbook(
        open_orders_account: &OpenOrdersAccount,
        open_orders: &OpenbookV2Orders,
        base_info_index: usize,
        quote_info_index: usize,
    ) -> Self {
                                     
        let reserved_base =
            I80F48::from(open_orders_account.position.asks_base_lots * open_orders.base_lot_size);
        let reserved_quote =
            I80F48::from(open_orders_account.position.bids_quote_lots * open_orders.quote_lot_size);

        let reserved_base_as_quote_lowest_ask =
            reserved_base * I80F48::from_num(open_orders.lowest_placed_ask);
        let reserved_quote_as_base_highest_bid =
            reserved_quote * I80F48::from_num(open_orders.highest_placed_bid_inv);

        Self {
            reserved_base,
            reserved_quote,
            reserved_base_as_quote_lowest_ask,
            reserved_quote_as_base_highest_bid,
            base_info_index,
            quote_info_index,
            spot_market_index: SpotMarketIndex::OpenbookV2(open_orders.market_index),
            has_zero_funds: open_orders_account
                .position
                .is_empty(open_orders_account.version),
        }
    }

    #[inline(always)]
    fn all_reserved_as_base(
        &self,
        health_type: HealthType,
        quote_info: &TokenInfo,
        base_info: &TokenInfo,
    ) -> I80F48 {
        let quote_asset = quote_info.prices.asset(health_type);
        let base_liab = base_info.prices.liab(health_type);
        let reserved_quote_as_base_oracle = (self.reserved_quote * quote_asset)
            .checked_div_f64_precision(base_liab)
            .unwrap();
        if self.reserved_quote_as_base_highest_bid != 0 {
            self.reserved_base
                + reserved_quote_as_base_oracle.min(self.reserved_quote_as_base_highest_bid)
        } else {
            self.reserved_base + reserved_quote_as_base_oracle
        }
    }

    #[inline(always)]
    fn all_reserved_as_quote(
        &self,
        health_type: HealthType,
        quote_info: &TokenInfo,
        base_info: &TokenInfo,
    ) -> I80F48 {
        let base_asset = base_info.prices.asset(health_type);
        let quote_liab = quote_info.prices.liab(health_type);
        let reserved_base_as_quote_oracle = (self.reserved_base * base_asset)
            .checked_div_f64_precision(quote_liab)
            .unwrap();
        if self.reserved_base_as_quote_lowest_ask != 0 {
            self.reserved_quote
                + reserved_base_as_quote_oracle.min(self.reserved_base_as_quote_lowest_ask)
        } else {
            self.reserved_quote + reserved_base_as_quote_oracle
        }
    }

                                                                
       
                                                                                        
                                                          
                                                          
                                                     
       
                                                                      
                                                                                             
                                                                                            
                             
                                                                                           
                                                                                             
                                                                                            
                   
       
                                                                                                  
                                                                                            
                                                                             
       
                                                                                                   
                                                                                                   
                                                                                               
                                                                    
    #[inline(always)]
    fn health_contribution(
        &self,
        health_type: HealthType,
        token_infos: &[TokenInfo],
        token_balances: &[TokenBalance],
        token_max_reserved: &[TokenMaxReserved],
        market_reserved: &SpotReserved,
    ) -> I80F48 {
        if market_reserved.all_reserved_as_base.is_zero()
            || market_reserved.all_reserved_as_quote.is_zero()
        {
            return I80F48::ZERO;
        }

        let base_info = &token_infos[self.base_info_index];
        let quote_info = &token_infos[self.quote_info_index];

                                                                                            
                      
        let compute_health_effect = |token_info: &TokenInfo,
                                     balance: &TokenBalance,
                                     max_reserved: &TokenMaxReserved,
                                     market_reserved: I80F48| {
                                                                                                
                                                                                                                 
            let max_balance = balance.spot_and_perp + max_reserved.max_spot_reserved;

                                                                                               
                                                                                               
                                           
            let (asset_part, liab_part) = if max_balance >= market_reserved {
                (market_reserved, I80F48::ZERO)
            } else if max_balance.is_negative() {
                (I80F48::ZERO, market_reserved)
            } else {
                (max_balance, market_reserved - max_balance)
            };

            let asset_weight = token_info.asset_weight(health_type);
            let liab_weight = token_info.liab_weight(health_type);
            let asset_price = token_info.prices.asset(health_type);
            let liab_price = token_info.prices.liab(health_type);
            asset_part * asset_weight * asset_price + liab_part * liab_weight * liab_price
        };

        let health_base = compute_health_effect(
            base_info,
            &token_balances[self.base_info_index],
            &token_max_reserved[self.base_info_index],
            market_reserved.all_reserved_as_base,
        );
        let health_quote = compute_health_effect(
            quote_info,
            &token_balances[self.quote_info_index],
            &token_max_reserved[self.quote_info_index],
            market_reserved.all_reserved_as_quote,
        );
        health_base.min(health_quote)
    }
}

#[derive(Clone)]
pub(crate) struct SpotReserved {
                                                                                                     
    all_reserved_as_base: I80F48,
                                  
    all_reserved_as_quote: I80F48,
}

                                                                         
   
                                                                                  
                                                                 
#[derive(Clone, Debug)]
pub struct PerpInfo {
    pub perp_market_index: PerpMarketIndex,
    pub settle_token_index: TokenIndex,
    pub maint_base_asset_weight: I80F48,
    pub init_base_asset_weight: I80F48,
    pub maint_base_liab_weight: I80F48,
    pub init_base_liab_weight: I80F48,
    pub maint_overall_asset_weight: I80F48,
    pub init_overall_asset_weight: I80F48,
    pub base_lot_size: i64,
    pub base_lots: i64,
    pub bids_base_lots: i64,
    pub asks_base_lots: i64,
                                                                          
    pub quote: I80F48,
    pub base_prices: Prices,
    pub has_open_orders: bool,
    pub has_open_fills: bool,
}

impl PerpInfo {
    fn new(
        perp_position: &PerpPosition,
        perp_market: &PerpMarket,
        base_prices: Prices,
    ) -> Result<Self> {
        let base_lots = perp_position.base_position_lots() + perp_position.taker_base_lots;

        let unsettled_funding = perp_position.unsettled_funding(perp_market);
        let taker_quote = I80F48::from(perp_position.taker_quote_lots * perp_market.quote_lot_size);
        let quote_current = perp_position.quote_position_native() - unsettled_funding + taker_quote;

        Ok(Self {
            perp_market_index: perp_market.perp_market_index,
            settle_token_index: perp_market.settle_token_index,
            init_base_asset_weight: perp_market.init_base_asset_weight,
            init_base_liab_weight: perp_market.init_base_liab_weight,
            maint_base_asset_weight: perp_market.maint_base_asset_weight,
            maint_base_liab_weight: perp_market.maint_base_liab_weight,
            init_overall_asset_weight: perp_market.init_overall_asset_weight,
            maint_overall_asset_weight: perp_market.maint_overall_asset_weight,
            base_lot_size: perp_market.base_lot_size,
            base_lots,
            bids_base_lots: perp_position.bids_base_lots,
            asks_base_lots: perp_position.asks_base_lots,
            quote: quote_current,
            base_prices,
            has_open_orders: perp_position.has_open_orders(),
            has_open_fills: perp_position.has_open_taker_fills(),
        })
    }

                                                                              
       
                                     
       
                                                                                  
                          
       
                                                                                 
                                                                                      
                                                                                    
                            
       
                                                                                         
                                                                               
       
                                                                                    
                                                                              
                                                                                   
                                                     
       
                                                                                  
                                                                                      
                                              
    #[inline(always)]
    pub fn health_unsettled_pnl(&self, health_type: HealthType) -> I80F48 {
        let contribution = self.unweighted_health_unsettled_pnl(health_type);
        self.weigh_uhupnl_overall(contribution, health_type)
    }

                                                                                             
    #[inline(always)]
    fn weigh_uhupnl_overall(&self, unweighted: I80F48, health_type: HealthType) -> I80F48 {
        if unweighted > 0 {
            let overall_weight = match health_type {
                HealthType::Init | HealthType::LiquidationEnd => self.init_overall_asset_weight,
                HealthType::Maint => self.maint_overall_asset_weight,
            };
            overall_weight * unweighted
        } else {
            unweighted
        }
    }

                                                                                                        
       
                             
       
                                                                                                  
                                                        
       
                                                                                                   
                                                                                   
       
                                          
    #[inline(always)]
    pub fn unweighted_health_unsettled_pnl(&self, health_type: HealthType) -> I80F48 {
        let order_execution_case = |orders_base_lots: i64, order_price: I80F48| {
            let net_base_native =
                I80F48::from((self.base_lots + orders_base_lots) * self.base_lot_size);
            let weight = match (health_type, net_base_native.is_negative()) {
                (HealthType::Init, true) | (HealthType::LiquidationEnd, true) => {
                    self.init_base_liab_weight
                }
                (HealthType::Init, false) | (HealthType::LiquidationEnd, false) => {
                    self.init_base_asset_weight
                }
                (HealthType::Maint, true) => self.maint_base_liab_weight,
                (HealthType::Maint, false) => self.maint_base_asset_weight,
            };
            let base_price = if net_base_native.is_negative() {
                self.base_prices.liab(health_type)
            } else {
                self.base_prices.asset(health_type)
            };

                                                                        
            let base_health = net_base_native * weight * base_price;

            let orders_base_native = I80F48::from(orders_base_lots * self.base_lot_size);
                                                            
            let order_quote = -orders_base_native * order_price;

            base_health + order_quote
        };

                                                                                                               
        let bids_case =
            order_execution_case(self.bids_base_lots, self.base_prices.liab(health_type));
        let asks_case =
            order_execution_case(-self.asks_base_lots, self.base_prices.asset(health_type));
        let worst_case = bids_case.min(asks_case);

        self.quote + worst_case
    }
}

                                                      
   
                                                                                   
                                                                                  
              
   
                                                                                    
                                                                                   
                                                                              
                                                                   
   
                                                                                  
                                                                                
#[allow(unused)]
#[derive(Clone, Debug)]
pub struct HealthCache {
    pub token_infos: Vec<TokenInfo>,
    pub(crate) spot_infos: Vec<SpotInfo>,
    pub(crate) perp_infos: Vec<PerpInfo>,
    #[allow(unused)]
    pub(crate) being_liquidated: bool,
}

impl HealthCache {
    pub fn health(&self, health_type: HealthType) -> I80F48 {
        let token_balances = self.effective_token_balances(health_type);
        let mut health = I80F48::ZERO;
        let sum = |contrib| {
            health += contrib;
        };
        self.health_sum(health_type, sum, &token_balances);
        health
    }

                           
                                                   
                                                   
                                                   
                          
       
                                                                                       
    pub fn health_ratio(&self, health_type: HealthType) -> I80F48 {
        let (assets, liabs) = self.health_assets_and_liabs_stable_liabs(health_type);
        let hundred = I80F48::from(100);
        if liabs > 0 {
                                                          
            (hundred * (assets - liabs)).saturating_div(liabs)
        } else {
            I80F48::MAX
        }
    }

    pub fn health_assets_and_liabs_stable_assets(
        &self,
        health_type: HealthType,
    ) -> (I80F48, I80F48) {
        self.health_assets_and_liabs(health_type, true)
    }
    pub fn health_assets_and_liabs_stable_liabs(
        &self,
        health_type: HealthType,
    ) -> (I80F48, I80F48) {
        self.health_assets_and_liabs(health_type, false)
    }

                                                                                                  
                                                                                         
       
                                                                                                   
                                                                                                 
                                                             
       
                                                                                              
                                           
                                        
                                                              
                                                                                              
       
                                              
                                      
                                       
       
                                                                            
    fn health_assets_and_liabs(
        &self,
        health_type: HealthType,
        stable_assets: bool,
    ) -> (I80F48, I80F48) {
        let mut total_assets = I80F48::ZERO;
        let mut total_liabs = I80F48::ZERO;
        let add = |assets: &mut I80F48, liabs: &mut I80F48, value: I80F48| {
            if value > 0 {
                *assets += value;
            } else {
                *liabs += -value;
            }
        };

        for token_info in self.token_infos.iter() {
                                                                                          
                                                                                             
                                             
                                                                                         
                                                                                    
            let mut asset_balance = I80F48::ZERO;
            let mut liab_balance = I80F48::ZERO;

            add(
                &mut asset_balance,
                &mut liab_balance,
                token_info.balance_spot,
            );

            for perp_info in self.perp_infos.iter() {
                if perp_info.settle_token_index != token_info.token_index {
                    continue;
                }
                let health_unsettled = perp_info.health_unsettled_pnl(health_type);
                add(&mut asset_balance, &mut liab_balance, health_unsettled);
            }

                                                                                 
                                                                                                    
                                             
            if stable_assets {
                let asset_weighted_price = token_info.asset_weighted_price(health_type);
                let assets = asset_balance * asset_weighted_price;
                total_assets += assets;
                if asset_balance >= liab_balance {
                                                 
                    total_liabs += liab_balance * asset_weighted_price;
                } else {
                    let liab_weighted_price = token_info.liab_weighted_price(health_type);
                                                                                         
                    total_liabs += assets + (liab_balance - asset_balance) * liab_weighted_price;
                }
            } else {
                let liab_weighted_price = token_info.liab_weighted_price(health_type);
                let liabs = liab_balance * liab_weighted_price;
                total_liabs += liabs;
                if asset_balance >= liab_balance {
                    let asset_weighted_price = token_info.asset_weighted_price(health_type);
                                                                                         
                    total_assets += liabs + (asset_balance - liab_balance) * asset_weighted_price;
                } else {
                                                  
                    total_assets += asset_balance * liab_weighted_price;
                }
            }
        }

        let token_balances = self.effective_token_balances(health_type);
        let (token_max_reserved, spot_reserved) = self.compute_spot_reservations(health_type);
        for (spot_info, reserved) in self.spot_infos.iter().zip(spot_reserved.iter()) {
            let contrib = spot_info.health_contribution(
                health_type,
                &self.token_infos,
                &token_balances,
                &token_max_reserved,
                reserved,
            );
            add(&mut total_assets, &mut total_liabs, contrib);
        }

        (total_assets, total_liabs)
    }

                                                                     
       
                                                                                     
                                                          
       
                                     
    pub fn assets_and_liabs(&self) -> (I80F48, I80F48) {
        let mut assets = I80F48::ZERO;
        let mut liabs = I80F48::ZERO;

        for token_info in self.token_infos.iter() {
            if token_info.balance_spot.is_negative() {
                liabs -= token_info.balance_spot * token_info.prices.oracle;
            } else {
                assets += token_info.balance_spot * token_info.prices.oracle;
            }
        }

        for spot_info in self.spot_infos.iter() {
            let quote = &self.token_infos[spot_info.quote_info_index];
            let base = &self.token_infos[spot_info.base_info_index];
            assets += spot_info.reserved_base * base.prices.oracle;
            assets += spot_info.reserved_quote * quote.prices.oracle;
        }

        for perp_info in self.perp_infos.iter() {
            let quote_price = self.token_infos[perp_info.settle_token_index as usize]
                .prices
                .oracle;
            let quote_position_value = perp_info.quote * quote_price;
            if perp_info.quote.is_negative() {
                liabs -= quote_position_value;
            } else {
                assets += quote_position_value;
            }

            let base_position_value = I80F48::from(perp_info.base_lots * perp_info.base_lot_size)
                * perp_info.base_prices.oracle
                * quote_price;
            if base_position_value.is_negative() {
                liabs -= base_position_value;
            } else {
                assets += base_position_value;
            }
        }

        return (assets, liabs);
    }

                                                                          
       
                                                                                                 
                                                                                              
                                                                                                  
    pub fn leverage(&self) -> I80F48 {
        let (assets, liabs) = self.assets_and_liabs();
        let equity = assets - liabs;
        liabs / equity.max(I80F48::from_num(0.001))
    }

    pub fn token_info(&self, token_index: TokenIndex) -> Result<&TokenInfo> {
        Ok(&self.token_infos[self.token_info_index(token_index)?])
    }

    pub fn token_info_index(&self, token_index: TokenIndex) -> Result<usize> {
        self.token_infos
            .iter()
            .position(|t| t.token_index == token_index)
            .ok_or_else(|| {
                error_msg_typed!(
                    FermiError::TokenPositionDoesNotExist,
                    "token index {} not found",
                    token_index
                )
            })
    }

    pub fn has_token_info(&self, token_index: TokenIndex) -> bool {
        self.token_infos
            .iter()
            .any(|t| t.token_index == token_index)
    }

    pub fn perp_info(&self, perp_market_index: PerpMarketIndex) -> Result<&PerpInfo> {
        Ok(&self.perp_infos[self.perp_info_index(perp_market_index)?])
    }

    pub(crate) fn perp_info_index(&self, perp_market_index: PerpMarketIndex) -> Result<usize> {
        self.perp_infos
            .iter()
            .position(|t| t.perp_market_index == perp_market_index)
            .ok_or_else(|| {
                error_msg_typed!(
                    FermiError::PerpPositionDoesNotExist,
                    "perp market index {} not found",
                    perp_market_index
                )
            })
    }

                                                      
    pub fn adjust_token_balance(&mut self, bank: &Bank, change: I80F48) -> Result<()> {
        let entry_index = self.token_info_index(bank.token_index)?;
        let mut entry = &mut self.token_infos[entry_index];

                                                                                       
                                     
        entry.init_scaled_asset_weight =
            bank.scaled_init_asset_weight(entry.prices.asset(HealthType::Init));
        entry.init_scaled_liab_weight =
            bank.scaled_init_liab_weight(entry.prices.liab(HealthType::Init));

                                                                                   
                                                                                        
                                     
        let removed_contribution = -change;
        entry.balance_spot -= removed_contribution;
        Ok(())
    }

                                                              
       
                                                                            
                                 
    #[cfg(feature = "legacy-spot")]
    pub fn recompute_serum3_info(
        &mut self,
        serum_account: &Serum3Orders,
        open_orders: &OpenOrdersSlim,
        free_base_change: I80F48,
        free_quote_change: I80F48,
    ) -> Result<()> {
        let spot_info_index = self
            .spot_infos
            .iter_mut()
            .position(|m| {
                m.spot_market_index == SpotMarketIndex::Serum3(serum_account.market_index)
            })
            .ok_or_else(|| error_msg!("serum3 market {} not found", serum_account.market_index))?;

        let spot_info = &self.spot_infos[spot_info_index];
        {
            let base_entry = &mut self.token_infos[spot_info.base_info_index];
            base_entry.balance_spot += free_base_change;
        }
        {
            let quote_entry = &mut self.token_infos[spot_info.quote_info_index];
            quote_entry.balance_spot += free_quote_change;
        }

        let spot_info = &mut self.spot_infos[spot_info_index];
        *spot_info = SpotInfo::new_from_serum(
            serum_account,
            open_orders,
            spot_info.base_info_index,
            spot_info.quote_info_index,
        );
        Ok(())
    }

                                                              
       
                                                                            
                                 
    #[cfg(feature = "legacy-spot")]
    pub fn recompute_openbook_v2_info(
        &mut self,
        open_orders: &OpenbookV2Orders,
        open_orders_account: &OpenOrdersAccount,
        free_base_change: I80F48,
        free_quote_change: I80F48,
    ) -> Result<()> {
        let spot_info_index = self
            .spot_infos
            .iter_mut()
            .position(|m| {
                m.spot_market_index == SpotMarketIndex::OpenbookV2(open_orders.market_index)
            })
            .ok_or_else(|| {
                error_msg!("openbook v2 market {} not found", open_orders.market_index)
            })?;

        let spot_info = &self.spot_infos[spot_info_index];
        {
            let base_entry = &mut self.token_infos[spot_info.base_info_index];
            base_entry.balance_spot += free_base_change;
        }
        {
            let quote_entry = &mut self.token_infos[spot_info.quote_info_index];
            quote_entry.balance_spot += free_quote_change;
        }

        let spot_info = &mut self.spot_infos[spot_info_index];
        *spot_info = SpotInfo::new_from_openbook(
            open_orders_account,
            open_orders,
            spot_info.base_info_index,
            spot_info.quote_info_index,
        );
        Ok(())
    }

    pub fn recompute_perp_info(
        &mut self,
        perp_position: &PerpPosition,
        perp_market: &PerpMarket,
    ) -> Result<()> {
        let perp_entry = self
            .perp_infos
            .iter_mut()
            .find(|m| m.perp_market_index == perp_market.perp_market_index)
            .ok_or_else(|| {
                error_msg!("perp market {} not found", {
                    perp_market.perp_market_index
                })
            })?;
        *perp_entry = PerpInfo::new(perp_position, perp_market, perp_entry.base_prices.clone())?;
        Ok(())
    }

                                                                                                        
                                              
    pub fn has_liq_spot_assets(&self) -> bool {
        let health_token_balances = self.effective_token_balances(HealthType::LiquidationEnd);
        self.token_infos
            .iter()
            .zip(health_token_balances.iter())
            .any(|(ti, b)| {
                                                                  
                ti.balance_spot >= 1 && b.spot_and_perp >= 1 && ti.allow_asset_liquidation
            })
    }

                                                                                                    
    pub fn has_liq_spot_borrows(&self) -> bool {
        let health_token_balances = self.effective_token_balances(HealthType::LiquidationEnd);
        self.token_infos
            .iter()
            .zip(health_token_balances.iter())
            .any(|(ti, b)| ti.balance_spot < 0 && b.spot_and_perp < 0)
    }

                                                                                                                
    pub fn has_possible_spot_liquidations(&self) -> bool {
        let health_token_balances = self.effective_token_balances(HealthType::LiquidationEnd);
        let all_iter = || self.token_infos.iter().zip(health_token_balances.iter());
        all_iter().any(|(ti, b)| ti.balance_spot < 0 && b.spot_and_perp < 0)
            && all_iter().any(|(ti, b)| {
                ti.balance_spot >= 1 && b.spot_and_perp >= 1 && ti.allow_asset_liquidation
            })
    }

    pub fn has_spot_open_orders_funds(&self) -> bool {
        self.spot_infos.iter().any(|si| !si.has_zero_funds)
    }

    pub fn has_perp_open_orders(&self) -> bool {
        self.perp_infos.iter().any(|p| p.has_open_orders)
    }

    pub fn has_perp_base_positions(&self) -> bool {
        self.perp_infos.iter().any(|p| p.base_lots != 0)
    }

    pub fn has_perp_open_fills(&self) -> bool {
        self.perp_infos.iter().any(|p| p.has_open_fills)
    }

    pub fn has_perp_positive_pnl_no_base(&self) -> bool {
        self.perp_infos
            .iter()
            .any(|p| p.base_lots == 0 && p.quote > 0)
    }

    pub fn has_perp_negative_pnl_no_base(&self) -> bool {
        self.perp_infos
            .iter()
            .any(|p| p.base_lots == 0 && p.quote < 0)
    }

                                                                        
                                                    
    pub fn has_phase1_liquidatable(&self) -> bool {
        self.has_spot_open_orders_funds() || self.has_perp_open_orders()
    }

    pub fn require_after_phase1_liquidation(&self) -> Result<()> {
        require!(
            !self.has_spot_open_orders_funds(),
            FermiError::HasOpenOrUnsettledSpotOrders
        );
        require!(!self.has_perp_open_orders(), FermiError::HasOpenPerpOrders);
        Ok(())
    }

    pub fn in_phase1_liquidation(&self) -> bool {
        self.has_phase1_liquidatable()
    }

                      
                                 
                                                                                  
                                                                                  
                                                                
    pub fn has_phase2_liquidatable(&self) -> bool {
        self.has_possible_spot_liquidations()
            || self.has_perp_base_positions()
            || self.has_perp_open_fills()
            || self.has_perp_positive_pnl_no_base()
    }

    pub fn require_after_phase2_liquidation(&self) -> Result<()> {
        self.require_after_phase1_liquidation()?;
        require!(
            !self.has_possible_spot_liquidations(),
            FermiError::HasLiquidatableTokenPosition
        );
        require!(
            !self.has_perp_base_positions(),
            FermiError::HasLiquidatablePerpBasePosition
        );
        require!(
            !self.has_perp_open_fills(),
            FermiError::HasOpenPerpTakerFills
        );
        require!(
            !self.has_perp_positive_pnl_no_base(),
            FermiError::HasLiquidatablePositivePerpPnl
        );
        Ok(())
    }

    pub fn in_phase2_liquidation(&self) -> bool {
        !self.has_phase1_liquidatable() && self.has_phase2_liquidatable()
    }

                             
                          
                         
    pub fn has_phase3_liquidatable(&self) -> bool {
        self.has_liq_spot_borrows() || self.has_perp_negative_pnl_no_base()
    }

    pub fn in_phase3_liquidation(&self) -> bool {
        !self.has_phase1_liquidatable()
            && !self.has_phase2_liquidatable()
            && self.has_phase3_liquidatable()
    }

    pub(crate) fn compute_spot_reservations(
        &self,
        health_type: HealthType,
    ) -> (Vec<TokenMaxReserved>, Vec<SpotReserved>) {
        let mut token_max_reserved = vec![TokenMaxReserved::default(); self.token_infos.len()];

                                                                                              
                                                   
        let mut spot_reserved = Vec::with_capacity(self.spot_infos.len());

        for info in self.spot_infos.iter() {
            let quote_info = &self.token_infos[info.quote_info_index];
            let base_info = &self.token_infos[info.base_info_index];

            let all_reserved_as_base =
                info.all_reserved_as_base(health_type, quote_info, base_info);
            let all_reserved_as_quote =
                info.all_reserved_as_quote(health_type, quote_info, base_info);

            token_max_reserved[info.base_info_index].max_spot_reserved += all_reserved_as_base;
            token_max_reserved[info.quote_info_index].max_spot_reserved += all_reserved_as_quote;

            spot_reserved.push(SpotReserved {
                all_reserved_as_base,
                all_reserved_as_quote,
            });
        }

        (token_max_reserved, spot_reserved)
    }

                                                                           
       
                                                                                         
                                           
       
                                                                                                    
                                                                                                 
                                       
    pub fn effective_token_balances(&self, health_type: HealthType) -> Vec<TokenBalance> {
        self.effective_token_balances_internal(health_type, false)
    }

                                                    
       
                                                                                                    
                                                                                                   
                                                                       
    fn effective_token_balances_internal(
        &self,
        health_type: HealthType,
        ignore_negative_perp: bool,
    ) -> Vec<TokenBalance> {
        let mut token_balances = vec![TokenBalance::default(); self.token_infos.len()];

        for perp_info in self.perp_infos.iter() {
            let settle_token_index = self.token_info_index(perp_info.settle_token_index).unwrap();
            let perp_settle_token = &mut token_balances[settle_token_index];
            let health_unsettled = perp_info.health_unsettled_pnl(health_type);
            if !ignore_negative_perp || health_unsettled > 0 {
                perp_settle_token.spot_and_perp += health_unsettled;
            }
        }

        for (token_info, token_balance) in self.token_infos.iter().zip(token_balances.iter_mut()) {
            token_balance.spot_and_perp += token_info.balance_spot;
        }

        token_balances
    }

    pub(crate) fn health_sum(
        &self,
        health_type: HealthType,
        mut action: impl FnMut(I80F48),
        token_balances: &[TokenBalance],
    ) {
        for (token_info, token_balance) in self.token_infos.iter().zip(token_balances.iter()) {
            let contrib = token_info.health_contribution(health_type, token_balance.spot_and_perp);
            action(contrib);
        }

        let (token_max_reserved, spot_reserved) = self.compute_spot_reservations(health_type);
        for (spot_info, reserved) in self.spot_infos.iter().zip(spot_reserved.iter()) {
            let contrib = spot_info.health_contribution(
                health_type,
                &self.token_infos,
                &token_balances,
                &token_max_reserved,
                reserved,
            );
            action(contrib);
        }
    }

                                                                    
       
                                                                                          
                                                                                     
                                                                                          
                                                                                         
                                                                            
       
                                                                                        
                                                                                           
                            
       
                                                                                       
                                                                                           
                                                                                       
                                        
       
                                                                                             
                                                                          
       
                 
                                                                                             
                                                                                                
                                                         
                                                                                               
                                                                                                     
                                                        
                                                                                              
    pub fn perp_max_settle(&self, settle_token_index: TokenIndex) -> Result<I80F48> {
        let maint_type = HealthType::Maint;

        let token_balances = self.effective_token_balances_internal(maint_type, true);
        let mut perp_settle_health = I80F48::ZERO;
        let sum = |contrib| {
            perp_settle_health += contrib;
        };
        self.health_sum(maint_type, sum, &token_balances);

        let token_info_index = self.token_info_index(settle_token_index)?;
        let token = &self.token_infos[token_info_index];
        spot_amount_taken_for_health_zero(
            perp_settle_health,
            token_balances[token_info_index].spot_and_perp,
            token.asset_weighted_price(maint_type),
            token.liab_weighted_price(maint_type),
        )
    }

    pub fn total_spot_potential(
        &self,
        health_type: HealthType,
        token_index: TokenIndex,
    ) -> Result<I80F48> {
        let target_token_info_index = self.token_info_index(token_index)?;
        let total_reserved = self
            .spot_infos
            .iter()
            .filter_map(|info| {
                if info.quote_info_index == target_token_info_index {
                    Some(info.all_reserved_as_quote(
                        health_type,
                        &self.token_infos[info.quote_info_index],
                        &self.token_infos[info.base_info_index],
                    ))
                } else if info.base_info_index == target_token_info_index {
                    Some(info.all_reserved_as_base(
                        health_type,
                        &self.token_infos[info.quote_info_index],
                        &self.token_infos[info.base_info_index],
                    ))
                } else {
                    None
                }
            })
            .sum();
        Ok(total_reserved)
    }

                                                                                                
                             
    pub fn check_has_all_spot_infos_for_token(
        &self,
        account: &FermiAccountRef,
        token_index: TokenIndex,
    ) -> Result<()> {
        for serum3 in account.active_serum3_orders() {
            if serum3.base_token_index == token_index || serum3.quote_token_index == token_index {
                require_msg!(
                    self.spot_infos.iter().any(|s| s.spot_market_index == SpotMarketIndex::Serum3(serum3.market_index)),
                    "health cache is missing spot info for serum3 market {} involving receiver token {}; passed banks and oracles?",
                    serum3.market_index, token_index
                );
            }
        }
        for oov2 in account.active_openbook_v2_orders() {
            if oov2.base_token_index == token_index || oov2.quote_token_index == token_index {
                require_msg!(
                    self.spot_infos.iter().any(|s| s.spot_market_index == SpotMarketIndex::OpenbookV2(oov2.market_index)),
                    "health cache is missing spot info for oov2 market {} involving receiver token {}; passed banks and oracles?",
                    oov2.market_index, token_index
                );
            }
        }
        Ok(())
    }
}

pub(crate) fn find_token_info_index(infos: &[TokenInfo], token_index: TokenIndex) -> Result<usize> {
    infos
        .iter()
        .position(|ti| ti.token_index == token_index)
        .ok_or_else(|| {
            error_msg_typed!(
                FermiError::TokenPositionDoesNotExist,
                "token index {} not found",
                token_index
            )
        })
}

                                                                  
pub fn new_health_cache(
    account: &FermiAccountRef,
    retriever: &impl AccountRetriever,
    now_ts: u64,
) -> Result<HealthCache> {
    new_health_cache_impl(account, retriever, now_ts, false)
}

                                                                         
                                                                                           
   
                                                                                 
                                                                       
pub fn new_health_cache_skipping_missing_banks_and_bad_oracles(
    account: &FermiAccountRef,
    retriever: &impl AccountRetriever,
    now_ts: u64,
) -> Result<HealthCache> {
    new_health_cache_impl(account, retriever, now_ts, true)
}

                             
                                                                                                        
                                                                                                   
                                            
fn new_health_cache_impl(
    account: &FermiAccountRef,
    retriever: &impl AccountRetriever,
    now_ts: u64,
    allow_skipping_banks: bool,
) -> Result<HealthCache> {
                                             
    let mut token_infos = Vec::with_capacity(account.active_token_positions().count());

                                                                          
    let available_banks_opt = if allow_skipping_banks {
        Some(retriever.available_banks()?)
    } else {
        None
    };

    for (i, position) in account.active_token_positions().enumerate() {
                                                                                        
        if allow_skipping_banks {
            let bank_is_available = available_banks_opt
                .as_ref()
                .unwrap()
                .contains(&{ position.token_index });
            if !bank_is_available {
                require_msg_typed!(
                    { position.indexed_position } >= 0,
                    FermiError::InvalidBank,
                    "the bank for token index {} is a required health account when the account has a negative balance in it",
                    { position.token_index }
                );
                continue;
            }
        }

        let bank_oracle_result =
            retriever.bank_and_oracle(&account.fixed.group, i, position.token_index);

                                                                                      
        if allow_skipping_banks && bank_oracle_result.is_oracle_error() && {
            position.indexed_position
        } >= 0
        {
                                                                                  
            continue;
        }
        let (bank, oracle_price) = bank_oracle_result?;

        let native = position.native(bank);
        let prices = Prices {
            oracle: oracle_price,
            stable: bank.stable_price(),
        };
                                                                                        
                                               
        let liab_price = prices.liab(HealthType::Init);

        let (maint_asset_weight, maint_liab_weight) = bank.maint_weights(now_ts);

        token_infos.push(TokenInfo {
            token_index: bank.token_index,
            maint_asset_weight,
            init_asset_weight: bank.init_asset_weight,
            init_scaled_asset_weight: bank.scaled_init_asset_weight(liab_price),
            maint_liab_weight,
            init_liab_weight: bank.init_liab_weight,
            init_scaled_liab_weight: bank.scaled_init_liab_weight(liab_price),
            prices,
            balance_spot: native,
            allow_asset_liquidation: bank.allows_asset_liquidation(),
        });
    }

                                                                                                             
    #[cfg(feature = "legacy-spot")]
    let spot_infos = {
        let mut spot_infos = Vec::with_capacity(
            account.active_serum3_orders().count() + account.active_openbook_v2_orders().count(),
        );
        for (i, serum_account) in account.active_serum3_orders().enumerate() {
            let oo = retriever.serum_oo(i, &serum_account.open_orders)?;

                                                                         
                                                                                          
            let info_index_results = (
                find_token_info_index(&token_infos, serum_account.base_token_index),
                find_token_info_index(&token_infos, serum_account.quote_token_index),
            );
            let (base_info_index, quote_info_index) = match info_index_results {
                (Ok(base), Ok(quote)) => (base, quote),
                _ => {
                    require_msg_typed!(
                        allow_skipping_banks,
                        FermiError::InvalidBank,
                        "serum market {} misses health accounts for bank {} or {}",
                        serum_account.market_index,
                        serum_account.base_token_index,
                        serum_account.quote_token_index,
                    );
                    continue;
                }
            };

                                                                                       
            let base_free = I80F48::from(oo.native_coin_free);
            let quote_free = I80F48::from(oo.native_pc_free);
            let base_info = &mut token_infos[base_info_index];
            base_info.balance_spot += base_free;
            let quote_info = &mut token_infos[quote_info_index];
            quote_info.balance_spot += quote_free;

            spot_infos.push(SpotInfo::new_from_serum(
                serum_account,
                oo,
                base_info_index,
                quote_info_index,
            ));
        }
        for (i, open_orders_account) in account.active_openbook_v2_orders().enumerate() {
            let oo = retriever.openbook_oo(i, &open_orders_account.open_orders)?;

                                                                         
                                                                                                
            let info_index_results = (
                find_token_info_index(&token_infos, open_orders_account.base_token_index),
                find_token_info_index(&token_infos, open_orders_account.quote_token_index),
            );
            let (base_info_index, quote_info_index) = match info_index_results {
                (Ok(base), Ok(quote)) => (base, quote),
                _ => {
                    require_msg_typed!(
                        allow_skipping_banks,
                        FermiError::InvalidBank,
                        "openbook-v2 market {} misses health accounts for bank {} or {}",
                        open_orders_account.market_index,
                        open_orders_account.base_token_index,
                        open_orders_account.quote_token_index,
                    );
                    continue;
                }
            };

                                                                                       
            let base_free = I80F48::from(oo.position.base_free_native);
            let quote_free = I80F48::from(oo.position.quote_free_native);
            let base_info = &mut token_infos[base_info_index];
            base_info.balance_spot += base_free;
            let quote_info = &mut token_infos[quote_info_index];
            quote_info.balance_spot += quote_free;

            spot_infos.push(SpotInfo::new_from_openbook(
                &oo,
                open_orders_account,
                base_info_index,
                quote_info_index,
            ));
        }
        spot_infos
    };
    #[cfg(not(feature = "legacy-spot"))]
    let spot_infos = {
        require_msg_typed!(
            account.active_serum3_orders().next().is_none(),
            FermiError::SomeError,
            "serum3 spot is disabled in this build"
        );
        require_msg_typed!(
            account.active_openbook_v2_orders().next().is_none(),
            FermiError::SomeError,
            "openbook v2 spot is disabled in this build"
        );
        Vec::new()
    };

                                             
    let mut perp_infos = Vec::with_capacity(account.active_perp_positions().count());
    for (i, perp_position) in account.active_perp_positions().enumerate() {
        let (perp_market, oracle_price) = retriever.perp_market_and_oracle_price(
            &account.fixed.group,
            i,
            perp_position.market_index,
        )?;

                                                                   
        if allow_skipping_banks {
            find_token_info_index(&token_infos, perp_market.settle_token_index)?;
        }

        perp_infos.push(PerpInfo::new(
            perp_position,
            perp_market,
            Prices {
                oracle: oracle_price,
                stable: perp_market.stable_price(),
            },
        )?);
    }

    Ok(HealthCache {
        token_infos,
        spot_infos,
        perp_infos,
        being_liquidated: account.fixed.being_liquidated(),
    })
}

#[cfg(all(test, feature = "legacy-unit-tests"))]
mod tests {
    use super::super::test::*;
    use super::*;
    use crate::state::*;
    use serum_dex::state::OpenOrders;
    use std::str::FromStr;

    #[test]
    fn test_precision() {
                                                 
        assert_ne!(
            I80F48::from_num(1_u128) / I80F48::from_num(2_u128.pow(48)),
            0
        );
        assert_eq!(
            I80F48::from_num(1_u128) / I80F48::from_num(2_u128.pow(49)),
            0
        );

                                                            
        assert_ne!(
            I80F48::from_str(format!("0.{}1", "0".repeat(13)).as_str()).unwrap(),
            0
        );
        assert_eq!(
            I80F48::from_str(format!("0.{}1", "0".repeat(14)).as_str()).unwrap(),
            0
        );
    }

    fn health_eq(a: I80F48, b: f64) -> bool {
        if (a - I80F48::from_num(b)).abs() < 0.001 {
            true
        } else {
            println!("health is {}, but expected {}", a, b);
            false
        }
    }

                                                                                          
    #[test]
    fn test_health0() {
        let buffer = FermiAccount::default_for_tests().try_to_vec().unwrap();
        let mut account = FermiAccountValue::from_bytes(&buffer).unwrap();

        let group = Pubkey::new_unique();

        let (mut bank1, mut oracle1) = mock_bank_and_oracle(group, 0, 1.0, 0.2, 0.1);       
        let (mut bank2, mut oracle2) = mock_bank_and_oracle(group, 4, 5.0, 0.5, 0.3);       
        bank1
            .data()
            .deposit(
                account.ensure_token_position(0).unwrap().0,
                I80F48::from(100),
                DUMMY_NOW_TS,
            )
            .unwrap();
        bank2
            .data()
            .withdraw_without_fee(
                account.ensure_token_position(4).unwrap().0,
                I80F48::from(10),
                DUMMY_NOW_TS,
            )
            .unwrap();
                             
        let mut oo1 = TestAccount::<OpenOrders>::new_zeroed();
        let serum3account = account.create_serum3_orders(2).unwrap();
        serum3account.open_orders = oo1.pubkey;
        serum3account.base_token_index = 4;
        serum3account.quote_token_index = 0;
        oo1.data().native_pc_total = 21;
        oo1.data().native_coin_total = 18;
        oo1.data().native_pc_free = 1;
        oo1.data().native_coin_free = 3;
        oo1.data().referrer_rebates_accrued = 2;

        let mut oo2 = TestAccount::<OpenOrdersAccount>::new_zeroed();
        let openbookv2account = account.create_openbook_v2_orders(2).unwrap();
        openbookv2account.open_orders = oo2.pubkey;
        openbookv2account.base_token_index = 4;
        openbookv2account.quote_token_index = 0;
        openbookv2account.potential_quote_tokens = 20;
        openbookv2account.potential_base_tokens = 15;
        openbookv2account.market_index = 2;
        openbookv2account.base_lot_size = 1;
        openbookv2account.quote_lot_size = 1;
        oo2.data().position.quote_free_native = 1;
        oo2.data().position.base_free_native = 3;
        oo2.data().position.referrer_rebates_available = 2;

        let mut perp1 = mock_perp_market(group, oracle2.pubkey, 5.0, 9, (0.2, 0.1), (0.05, 0.02));
        let perpaccount = account.ensure_perp_position(9, 0).unwrap().0;
        perpaccount.record_trade(perp1.data(), 3, -I80F48::from(310u16));
        perpaccount.bids_base_lots = 7;
        perpaccount.asks_base_lots = 11;
        perpaccount.taker_base_lots = 1;
        perpaccount.taker_quote_lots = 2;

        let oracle2_ai = oracle2.as_account_info();

        let ais = vec![
            bank1.as_account_info(),
            bank2.as_account_info(),
            oracle1.as_account_info(),
            oracle2_ai.clone(),
            perp1.as_account_info(),
            oracle2_ai,
            oo1.as_account_info(),
            oo2.as_account_info(),
        ];

        let retriever = ScanningAccountRetriever::new_with_staleness(&ais, &group, None).unwrap();

                            
                                                         
        let serum1 = 1.0 + (20.0 + 15.0 * 5.0);
        let openbook1 = 1.0 + (20.0 + 15.0 * 5.0);
                                            
        let perp1 =
            (3.0 + 7.0 + 1.0) * 10.0 * 5.0 * 0.8 + (-310.0 + 2.0 * 100.0 - 7.0 * 10.0 * 5.0);
        let health1 = (100.0 + serum1 + openbook1) * 0.8;
                            
        let health2 = (-20.0 + 3.0 + 3.0) * 5.0 * 1.5;
                             
                                                                                           
                                
              
    }

    #[derive(Default)]
    struct BankSettings {
        deposits: u64,
        borrows: u64,
        deposit_weight_scale_start_quote: u64,
        borrow_weight_scale_start_quote: u64,
        potential_serum_tokens: u64,
        potential_openbook_tokens: u64,
    }

    #[derive(Default)]
    struct TestHealth1Case {
        token1: i64,
        token2: i64,
        token3: i64,
        oo_1_2: (u64, u64),
        oo_1_3: (u64, u64),
        oov2_1_2: (u64, u64),
        oov2_1_3: (u64, u64),
        perp1: (i64, i64, i64, i64),
        expected_health: f64,
        bank_settings: [BankSettings; 3],
        extra: Option<fn(&mut FermiAccountValue)>,
    }
    fn test_health1_runner(testcase: &TestHealth1Case) {
        let buffer = FermiAccount::default_for_tests().try_to_vec().unwrap();
        let mut account = FermiAccountValue::from_bytes(&buffer).unwrap();

        let group = Pubkey::new_unique();

        let (mut bank1, mut oracle1) = mock_bank_and_oracle(group, 0, 1.0, 0.2, 0.1);
        let (mut bank2, mut oracle2) = mock_bank_and_oracle(group, 4, 5.0, 0.5, 0.3);
        let (mut bank3, mut oracle3) = mock_bank_and_oracle(group, 5, 10.0, 0.5, 0.3);
        bank1
            .data()
            .change_without_fee(
                account.ensure_token_position(0).unwrap().0,
                I80F48::from(testcase.token1),
                DUMMY_NOW_TS,
            )
            .unwrap();
        bank2
            .data()
            .change_without_fee(
                account.ensure_token_position(4).unwrap().0,
                I80F48::from(testcase.token2),
                DUMMY_NOW_TS,
            )
            .unwrap();
        bank3
            .data()
            .change_without_fee(
                account.ensure_token_position(5).unwrap().0,
                I80F48::from(testcase.token3),
                DUMMY_NOW_TS,
            )
            .unwrap();
        for (settings, bank) in testcase
            .bank_settings
            .iter()
            .zip([&mut bank1, &mut bank2, &mut bank3].iter_mut())
        {
            let bank = bank.data();
            bank.indexed_deposits = I80F48::from(settings.deposits) / bank.deposit_index;
            bank.indexed_borrows = I80F48::from(settings.borrows) / bank.borrow_index;
            bank.potential_serum_tokens = settings.potential_serum_tokens;
            bank.potential_openbook_tokens = settings.potential_openbook_tokens;
            if settings.deposit_weight_scale_start_quote > 0 {
                bank.deposit_weight_scale_start_quote =
                    settings.deposit_weight_scale_start_quote as f64;
            }
            if settings.borrow_weight_scale_start_quote > 0 {
                bank.borrow_weight_scale_start_quote =
                    settings.borrow_weight_scale_start_quote as f64;
            }
        }

        let mut oo1 = TestAccount::<OpenOrders>::new_zeroed();
        let serum3account1 = account.create_serum3_orders(2).unwrap();
        serum3account1.open_orders = oo1.pubkey;
        serum3account1.base_token_index = 4;
        serum3account1.quote_token_index = 0;
        oo1.data().native_pc_total = testcase.oo_1_2.0;
        oo1.data().native_coin_total = testcase.oo_1_2.1;

        let mut oo2 = TestAccount::<OpenOrders>::new_zeroed();
        let serum3account2 = account.create_serum3_orders(3).unwrap();
        serum3account2.open_orders = oo2.pubkey;
        serum3account2.base_token_index = 5;
        serum3account2.quote_token_index = 0;
        oo2.data().native_pc_total = testcase.oo_1_3.0;
        oo2.data().native_coin_total = testcase.oo_1_3.1;

        let mut oov2_1 = TestAccount::<OpenOrdersAccount>::new_zeroed();
        let openbookv2account = account.create_openbook_v2_orders(2).unwrap();
        openbookv2account.open_orders = oov2_1.pubkey;
        openbookv2account.base_token_index = 4;
        openbookv2account.quote_token_index = 0;
        openbookv2account.base_lot_size = 1;
        openbookv2account.quote_lot_size = 1;
        oov2_1.data().position.bids_quote_lots = testcase.oov2_1_2.0 as i64;
        oov2_1.data().position.asks_base_lots = testcase.oov2_1_2.1 as i64;

        let mut oov2_2 = TestAccount::<OpenOrdersAccount>::new_zeroed();
        let openbookv2account2 = account.create_openbook_v2_orders(3).unwrap();
        openbookv2account2.open_orders = oov2_2.pubkey;
        openbookv2account2.base_token_index = 5;
        openbookv2account2.quote_token_index = 0;
        openbookv2account2.base_lot_size = 1;
        openbookv2account2.quote_lot_size = 1;
        oov2_2.data().position.bids_quote_lots = testcase.oov2_1_3.0 as i64;
        oov2_2.data().position.asks_base_lots = testcase.oov2_1_3.1 as i64;

        let mut perp1 = mock_perp_market(group, oracle2.pubkey, 5.0, 9, (0.2, 0.1), (0.05, 0.02));
        let perpaccount = account.ensure_perp_position(9, 0).unwrap().0;
        perpaccount.record_trade(
            perp1.data(),
            testcase.perp1.0,
            I80F48::from(testcase.perp1.1),
        );
        perpaccount.bids_base_lots = testcase.perp1.2;
        perpaccount.asks_base_lots = testcase.perp1.3;

        if let Some(extra_fn) = testcase.extra {
            extra_fn(&mut account);
        }

        let oracle2_ai = oracle2.as_account_info();
        let ais = vec![
            bank1.as_account_info(),
            bank2.as_account_info(),
            bank3.as_account_info(),
            oracle1.as_account_info(),
            oracle2_ai.clone(),
            oracle3.as_account_info(),
            perp1.as_account_info(),
            oracle2_ai,
            oo1.as_account_info(),
            oo2.as_account_info(),
            oov2_1.as_account_info(),
            oov2_2.as_account_info(),
        ];

        let retriever = ScanningAccountRetriever::new_with_staleness(&ais, &group, None).unwrap();

        assert!(health_eq(
            compute_health(&account.borrow(), HealthType::Init, &retriever, 0).unwrap(),
            testcase.expected_health
        ));
    }

                                                
    #[test]
    fn test_health1() {
        let base_price = 5.0;
        let base_lots_to_quote = 10.0 * base_price;
        let testcases = vec![
            TestHealth1Case {     
                token1: 100,
                token2: -10,
                oo_1_2: (20, 15),
                perp1: (3, -131, 7, 11),
                expected_health:
                                 
                    0.8 * (100.0
                                                                     
                    + (20.0 + 15.0 * base_price)
                                                              
                    + (3.0 + 7.0) * base_lots_to_quote * 0.8 + (-131.0 - 7.0 * base_lots_to_quote))
                                 
                    - 10.0 * base_price * 1.5,
                ..Default::default()
            },
            TestHealth1Case {     
                token1: -100,
                token2: 10,
                oo_1_2: (20, 15),
                perp1: (-10, -131, 7, 11),
                expected_health:
                                 
                    1.2 * (-100.0
                                                        
                    + (-10.0 - 11.0) * base_lots_to_quote * 1.2 + (-131.0 + 11.0 * base_lots_to_quote))
                                                                                 
                    + (10.0 * base_price + (20.0 + 15.0 * base_price)) * 0.5,
                ..Default::default()
            },
            TestHealth1Case {
                                                
                perp1: (-1, 100, 0, 0),
                expected_health: 0.8 * 0.95 * (100.0 - 1.2 * 1.0 * base_lots_to_quote),
                ..Default::default()
            },
            TestHealth1Case {
                                                                                      
                perp1: (1, -100, 0, 0),
                expected_health: 1.2 * (-100.0 + 0.8 * 1.0 * base_lots_to_quote),
                ..Default::default()
            },
            TestHealth1Case {
                                 
                perp1: (10, 100, 0, 0),
                expected_health: 0.8 * 0.95 * (100.0 + 0.8 * 10.0 * base_lots_to_quote),
                ..Default::default()
            },
            TestHealth1Case {
                                 
                perp1: (30, -100, 0, 0),
                expected_health: 0.8 * 0.95 * (-100.0 + 0.8 * 30.0 * base_lots_to_quote),
                ..Default::default()
            },
            TestHealth1Case {                        
                token1: -100,
                token2: -10,
                token3: -10,
                oo_1_2: (1, 1),
                oo_1_3: (1, 1),
                expected_health:
                             
                    -100.0 * 1.2 - 10.0 * 5.0 * 1.5 - 10.0 * 10.0 * 1.5
                                         
                    + (1.0 + 5.0) * 1.2
                                         
                    + (1.0 + 10.0) * 1.2,
                ..Default::default()
            },
            TestHealth1Case {                                                     
                token1: -14,
                token2: -10,
                token3: -10,
                oo_1_2: (1, 1),
                oo_1_3: (1, 1),
                expected_health:
                             
                    -14.0 * 1.2 - 10.0 * 5.0 * 1.5 - 10.0 * 10.0 * 1.5
                                         
                    + 3.0 * 1.2 + 3.0 * 0.8
                                         
                    + 8.0 * 1.2 + 3.0 * 0.8,
                ..Default::default()
            },
            TestHealth1Case {                                                
                token1: -100,
                token2: -100,
                token3: -1,
                oo_1_2: (0, 0),
                oo_1_3: (10, 1),
                expected_health:
                             
                    -100.0 * 1.2 - 100.0 * 5.0 * 1.5 - 10.0 * 1.5
                                         
                    + 10.0 * 1.5 + 10.0 * 0.5,
                ..Default::default()
            },
            TestHealth1Case {                                                
                token1: -100,
                token2: -100,
                token3: -1,
                oo_1_2: (100, 0),
                oo_1_3: (10, 1),
                expected_health:
                             
                    -100.0 * 1.2 - 100.0 * 5.0 * 1.5 - 10.0 * 1.5
                                         
                    + 80.0 * 1.2 + 20.0 * 0.8
                                         
                    + 20.0 * 0.8,
                ..Default::default()
            },
            TestHealth1Case {                                 
                token1: 100,
                token2: 100,
                token3: 100,
                bank_settings: [
                    BankSettings {
                        deposits: 100,
                        deposit_weight_scale_start_quote: 1000,
                        ..BankSettings::default()
                    },
                    BankSettings {
                        deposits: 1500,
                        deposit_weight_scale_start_quote: 1000 * 5,
                        ..BankSettings::default()
                    },
                    BankSettings {
                        deposits: 10000,
                        deposit_weight_scale_start_quote: 1000 * 10,
                        ..BankSettings::default()
                    },
                ],
                expected_health:
                             
                    0.8 * 100.0
                             
                    + 0.5 * 100.0 * 5.0 * (5000.0 / (1500.0 * 5.0))
                             
                    + 0.5 * 100.0 * 10.0 * (10000.0 / (10000.0 * 10.0)),
                ..Default::default()
            },
            TestHealth1Case {                             
                token1: -100,
                token2: -100,
                token3: -100,
                bank_settings: [
                    BankSettings {
                        borrows: 100,
                        borrow_weight_scale_start_quote: 1000,
                        ..BankSettings::default()
                    },
                    BankSettings {
                        borrows: 1500,
                        borrow_weight_scale_start_quote: 1000 * 5,
                        ..BankSettings::default()
                    },
                    BankSettings {
                        borrows: 10000,
                        borrow_weight_scale_start_quote: 1000 * 10,
                        ..BankSettings::default()
                    },
                ],
                expected_health:
                             
                    -1.2 * 100.0
                             
                    - 1.5 * 100.0 * 5.0 * (1500.0 * 5.0 / 5000.0)
                             
                    - 1.5 * 100.0 * 10.0 * (10000.0 * 10.0 / 10000.0),
                ..Default::default()
            },
            TestHealth1Case {
                                                                
                token1: -100,
                perp1: (1, 100, 0, 0),
                expected_health: 0.8 * (-100.0 + 0.95 * (100.0 + 0.8 * 1.0 * base_lots_to_quote)),
                ..Default::default()
            },
            TestHealth1Case {
                                                                 
                token1: 100,
                perp1: (-1, -100, 0, 0),
                expected_health: 1.2 * (100.0 - 100.0 - 1.2 * 1.0 * base_lots_to_quote),
                ..Default::default()
            },
            TestHealth1Case {
                                                             
                token1: -100,
                token2: -10,
                token3: 0,
                oo_1_2: (1, 1),
                oo_1_3: (11, 1),
                expected_health:
                             
                    -100.0 * 1.2 - 10.0 * 5.0 * 1.5
                                         
                    + (1.0 + 3.0) * 1.2
                                         
                    + (11.0 / 12.0 + 1.0) * 10.0 * 0.5,
                extra: Some(|account: &mut FermiAccountValue| {
                    let s2 = account.serum3_orders_mut(2).unwrap();
                    s2.lowest_placed_ask = 3.0;
                    let s3 = account.serum3_orders_mut(3).unwrap();
                    s3.highest_placed_bid_inv = 1.0 / 12.0;
                }),
                ..Default::default()
            },
            TestHealth1Case {
                                                                                 
                token1: -100,
                token2: -10,
                token3: 0,
                oo_1_2: (1, 1),
                oo_1_3: (11, 1),
                expected_health:
                             
                    -100.0 * 1.2 - 10.0 * 5.0 * 1.5
                                         
                    + (1.0 + 5.0) * 1.2
                                         
                    + (11.0 / 10.0 + 1.0) * 10.0 * 0.5,
                extra: Some(|account: &mut FermiAccountValue| {
                    let s2 = account.serum3_orders_mut(2).unwrap();
                    s2.lowest_placed_ask = 6.0;
                    let s3 = account.serum3_orders_mut(3).unwrap();
                    s3.highest_placed_bid_inv = 1.0 / 9.0;
                }),
                ..Default::default()
            },
            TestHealth1Case {
                                       
                token1: 100,
                token2: 100,
                token3: 100,
                oo_1_2: (0, 100),
                oo_1_3: (0, 100),
                expected_health:
                             
                    100.0 * 0.8 + 100.0 * 5.0 * 0.5 + 100.0 * 10.0 * 0.5
                                         
                    + 100.0 * 5.0 * 0.5
                                         
                    + 100.0 * 10.0 * 0.5,
                ..Default::default()
            },
            TestHealth1Case {
                                                                               
                token1: 100,
                token2: 100,
                token3: 100,
                oo_1_2: (0, 100),
                oo_1_3: (0, 100),
                bank_settings: [
                    BankSettings {
                        ..BankSettings::default()
                    },
                    BankSettings {
                        deposits: 100,
                        deposit_weight_scale_start_quote: 100 * 5,
                        potential_serum_tokens: 100,
                        ..BankSettings::default()
                    },
                    BankSettings {
                        deposits: 600,
                        deposit_weight_scale_start_quote: 500 * 10,
                        potential_serum_tokens: 100,
                        ..BankSettings::default()
                    },
                ],
                expected_health:
                             
                    100.0 * 0.8 + 100.0 * 5.0 * 0.5 * (100.0 / 200.0) + 100.0 * 10.0 * 0.5 * (500.0 / 700.0)
                                         
                    + 100.0 * 5.0 * 0.5 * (100.0 / 200.0)
                                         
                    + 100.0 * 10.0 * 0.5 * (500.0 / 700.0),
                ..Default::default()
            },
            TestHealth1Case {                        
                token1: 100,
                token2: -10,
                oov2_1_2: (20, 15),
                expected_health:
                                 
                    0.8 * (100.0
                                                                     
                    + (20.0 + 15.0 * base_price))
                                 
                    - 10.0 * base_price * 1.5,
                ..Default::default()
            },
            TestHealth1Case {                        
                token1: -100,
                token2: 10,
                oov2_1_2: (20, 15),
                expected_health:
                                 
                    1.2 * (-100.0)
                                                                                 
                    + (10.0 * base_price + (20.0 + 15.0 * base_price)) * 0.5,
                ..Default::default()
            },
            TestHealth1Case {                                           
                token1: -100,
                token2: -10,
                token3: -10,
                oov2_1_2: (1, 1),
                oov2_1_3: (1, 1),
                expected_health:
                             
                    -100.0 * 1.2 - 10.0 * 5.0 * 1.5 - 10.0 * 10.0 * 1.5
                                         
                    + (1.0 + 5.0) * 1.2
                                         
                    + (1.0 + 10.0) * 1.2,
                ..Default::default()
            },
            TestHealth1Case {                                                                        
                token1: -14,
                token2: -10,
                token3: -10,
                oov2_1_2: (1, 1),
                oov2_1_3: (1, 1),
                expected_health:
                             
                    -14.0 * 1.2 - 10.0 * 5.0 * 1.5 - 10.0 * 10.0 * 1.5
                                         
                    + 3.0 * 1.2 + 3.0 * 0.8
                                         
                    + 8.0 * 1.2 + 3.0 * 0.8,
                ..Default::default()
            },
            TestHealth1Case {                                                                   
                token1: -100,
                token2: -100,
                token3: -1,
                oov2_1_2: (0, 0),
                oov2_1_3: (10, 1),
                expected_health:
                             
                    -100.0 * 1.2 - 100.0 * 5.0 * 1.5 - 10.0 * 1.5
                                         
                    + 10.0 * 1.5 + 10.0 * 0.5,
                ..Default::default()
            },
            TestHealth1Case {                                                                   
                token1: -100,
                token2: -100,
                token3: -1,
                oov2_1_2: (100, 0),
                oov2_1_3: (10, 1),
                expected_health:
                             
                    -100.0 * 1.2 - 100.0 * 5.0 * 1.5 - 10.0 * 1.5
                                         
                    + 80.0 * 1.2 + 20.0 * 0.8
                                         
                    + 20.0 * 0.8,
                ..Default::default()
            },
            TestHealth1Case {
                                                                                
                token1: -100,
                token2: -10,
                token3: 0,
                oov2_1_2: (1, 1),
                oov2_1_3: (11, 1),
                expected_health:
                             
                    -100.0 * 1.2 - 10.0 * 5.0 * 1.5
                                         
                    + (1.0 + 3.0) * 1.2
                                         
                    + (11.0 / 12.0 + 1.0) * 10.0 * 0.5,
                extra: Some(|account: &mut FermiAccountValue| {
                    let s2 = account.openbook_v2_orders_mut(2).unwrap();
                    s2.lowest_placed_ask = 3.0;
                    let s3 = account.openbook_v2_orders_mut(3).unwrap();
                    s3.highest_placed_bid_inv = 1.0 / 12.0;
                }),
                ..Default::default()
            },
            TestHealth1Case {
                                                                                                    
                token1: -100,
                token2: -10,
                token3: 0,
                oov2_1_2: (1, 1),
                oov2_1_3: (11, 1),
                expected_health:
                             
                    -100.0 * 1.2 - 10.0 * 5.0 * 1.5
                                         
                    + (1.0 + 5.0) * 1.2
                                         
                    + (11.0 / 10.0 + 1.0) * 10.0 * 0.5,
                extra: Some(|account: &mut FermiAccountValue| {
                    let s2 = account.openbook_v2_orders_mut(2).unwrap();
                    s2.lowest_placed_ask = 6.0;
                    let s3 = account.openbook_v2_orders_mut(3).unwrap();
                    s3.highest_placed_bid_inv = 1.0 / 9.0;
                }),
                ..Default::default()
            },
            TestHealth1Case {
                                                          
                token1: 100,
                token2: 100,
                token3: 100,
                oov2_1_2: (0, 100),
                oov2_1_3: (0, 100),
                expected_health:
                             
                    100.0 * 0.8 + 100.0 * 5.0 * 0.5 + 100.0 * 10.0 * 0.5
                                         
                    + 100.0 * 5.0 * 0.5
                                         
                    + 100.0 * 10.0 * 0.5,
                ..Default::default()
            },
            TestHealth1Case {
                                                                                                     
                token1: 100,
                token2: 100,
                token3: 100,
                oov2_1_2: (0, 100),
                oov2_1_3: (0, 100),
                bank_settings: [
                    BankSettings {
                        ..BankSettings::default()
                    },
                    BankSettings {
                        deposits: 100,
                        deposit_weight_scale_start_quote: 100 * 5,
                        potential_openbook_tokens: 100,
                        ..BankSettings::default()
                    },
                    BankSettings {
                        deposits: 600,
                        deposit_weight_scale_start_quote: 500 * 10,
                        potential_openbook_tokens: 100,
                        ..BankSettings::default()
                    },
                ],
                expected_health:
                             
                    100.0 * 0.8 + 100.0 * 5.0 * 0.5 * (100.0 / 200.0) + 100.0 * 10.0 * 0.5 * (500.0 / 700.0)
                                         
                    + 100.0 * 5.0 * 0.5 * (100.0 / 200.0)
                                         
                    + 100.0 * 10.0 * 0.5 * (500.0 / 700.0),
                ..Default::default()
            },
        ];

        for (i, testcase) in testcases.iter().enumerate() {
            println!("checking testcase {}", i);
            test_health1_runner(testcase);
        }
    }

    #[test]
    fn test_health_with_skips() {
        let testcase = TestHealth1Case {
                                   
            token1: 100,
            token2: 10,
            token3: -10,
            oo_1_2: (5, 1),
            oo_1_3: (0, 0),
            ..Default::default()
        };

        let buffer = FermiAccount::default_for_tests().try_to_vec().unwrap();
        let mut account = FermiAccountValue::from_bytes(&buffer).unwrap();

        let group = Pubkey::new_unique();
        account.fixed.group = group;

        let (mut bank1, mut oracle1) = mock_bank_and_oracle(group, 0, 1.0, 0.2, 0.1);
        let (mut bank2, mut oracle2) = mock_bank_and_oracle(group, 4, 5.0, 0.5, 0.3);
        let (mut bank3, mut oracle3) = mock_bank_and_oracle(group, 5, 10.0, 0.5, 0.3);
        bank1
            .data()
            .change_without_fee(
                account.ensure_token_position(0).unwrap().0,
                I80F48::from(testcase.token1),
                DUMMY_NOW_TS,
            )
            .unwrap();
        bank2
            .data()
            .change_without_fee(
                account.ensure_token_position(4).unwrap().0,
                I80F48::from(testcase.token2),
                DUMMY_NOW_TS,
            )
            .unwrap();
        bank3
            .data()
            .change_without_fee(
                account.ensure_token_position(5).unwrap().0,
                I80F48::from(testcase.token3),
                DUMMY_NOW_TS,
            )
            .unwrap();

        let mut oo1 = TestAccount::<OpenOrders>::new_zeroed();
        let serum3account1 = account.create_serum3_orders(2).unwrap();
        serum3account1.open_orders = oo1.pubkey;
        serum3account1.base_token_index = 4;
        serum3account1.quote_token_index = 0;
        oo1.data().native_pc_total = testcase.oo_1_2.0;
        oo1.data().native_coin_total = testcase.oo_1_2.1;

        fn compute_health_with_retriever<'a, 'info>(
            ais: &[AccountInfo],
            account: &FermiAccountValue,
            group: Pubkey,
            kind: bool,
        ) -> Result<I80F48> {
            let hc = if kind {
                let retriever =
                    ScanningAccountRetriever::new_with_staleness(&ais, &group, None).unwrap();
                new_health_cache_skipping_missing_banks_and_bad_oracles(
                    &account.borrow(),
                    &retriever,
                    DUMMY_NOW_TS,
                )?
            } else {
                let retriever = new_fixed_order_account_retriever_with_optional_banks(
                    &ais,
                    &account.borrow(),
                    0,
                )
                .unwrap();
                new_health_cache_skipping_missing_banks_and_bad_oracles(
                    &account.borrow(),
                    &retriever,
                    DUMMY_NOW_TS,
                )?
            };
            Ok(hc.health(HealthType::Init))
        }

        for retriever_kind in [false, true] {
                                       
            {
                let ais = vec![
                    bank1.as_account_info(),
                    bank2.as_account_info(),
                    bank3.as_account_info(),
                    oracle1.as_account_info(),
                    oracle2.as_account_info(),
                    oracle3.as_account_info(),
                    oo1.as_account_info(),
                ];

                let health =
                    compute_health_with_retriever(&ais, &account, group, retriever_kind).unwrap();
                assert!(health_eq(
                    health,
                    0.8 * 100.0 + 0.5 * 5.0 * (10.0 + 2.0) - 1.5 * 10.0 * 10.0
                ));
            }

                            
            {
                let ais = vec![
                    bank2.as_account_info(),
                    bank3.as_account_info(),
                    oracle2.as_account_info(),
                    oracle3.as_account_info(),
                    oo1.as_account_info(),
                ];

                let health =
                    compute_health_with_retriever(&ais, &account, group, retriever_kind).unwrap();
                assert!(health_eq(health, 0.5 * 5.0 * 10.0 - 1.5 * 10.0 * 10.0));
            }

                            
            {
                let ais = vec![
                    bank1.as_account_info(),
                    bank3.as_account_info(),
                    oracle1.as_account_info(),
                    oracle3.as_account_info(),
                    oo1.as_account_info(),
                ];

                let health =
                    compute_health_with_retriever(&ais, &account, group, retriever_kind).unwrap();
                assert!(health_eq(health, 0.8 * 100.0 - 1.5 * 10.0 * 10.0));
            }

                                  
            {
                let ais = vec![
                    bank3.as_account_info(),
                    oracle3.as_account_info(),
                    oo1.as_account_info(),
                ];

                let health =
                    compute_health_with_retriever(&ais, &account, group, retriever_kind).unwrap();
                assert!(health_eq(health, -1.5 * 10.0 * 10.0));
            }

                            
            {
                let ais = vec![
                    bank1.as_account_info(),
                    bank2.as_account_info(),
                    oracle1.as_account_info(),
                    oracle2.as_account_info(),
                    oo1.as_account_info(),
                ];

                                                                     
                assert!(
                    compute_health_with_retriever(&ais, &account, group, retriever_kind).is_err()
                );
            }
        }
    }
}
