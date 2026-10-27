use anchor_lang::prelude::*;

use derivative::Derivative;
use fixed::types::I80F48;
use static_assertions::const_assert_eq;
use std::cmp::Ordering;
use std::mem::size_of;

use crate::i80f48::ClampToInt;
use crate::state::*;

                                                                                      
                                                                         
macro_rules! impl_borsh_for_packed_pod {
    ($t:ty) => {
        impl borsh::ser::BorshSerialize for $t {
            fn serialize<W: std::io::Write>(&self, writer: &mut W) -> std::io::Result<()> {
                let bytes: &[u8] = bytemuck::bytes_of(self);
                writer.write_all(bytes)
            }
        }
        impl borsh::de::BorshDeserialize for $t {
            fn deserialize_reader<R: std::io::Read>(reader: &mut R) -> std::io::Result<Self> {
                let mut buf = [0u8; std::mem::size_of::<$t>()];
                reader.read_exact(&mut buf)?;
                Ok(*bytemuck::from_bytes::<$t>(&buf))
            }
        }
    };
}

pub const FREE_ORDER_SLOT: PerpMarketIndex = PerpMarketIndex::MAX;

#[repr(C, packed)]
#[zero_copy]
#[derive(Derivative, PartialEq)]
#[derivative(Debug)]
pub struct TokenPosition {
                                                                         
                                                                  
                                                           
                                                                           
                                  
                                                                                     
    pub indexed_position: I80F48,

                               
    pub token_index: TokenIndex,

                                                                      
    pub in_use_count: u16,

    #[derivative(Debug = "ignore")]
    pub padding: [u8; 4],

                                                            
                                                                           
    pub previous_index: I80F48,
                     
                                                        
    pub cumulative_deposit_interest: f64,
                     
                                                       
    pub cumulative_borrow_interest: f64,

    #[derivative(Debug = "ignore")]
    pub reserved: [u8; 128],
}

const_assert_eq!(
    size_of::<TokenPosition>(),
    16 + 2 + 2 + 4 + 16 + 8 + 8 + 128
);
const_assert_eq!(size_of::<TokenPosition>(), 184);
const_assert_eq!(size_of::<TokenPosition>() % 8, 0);
impl_borsh_for_packed_pod!(TokenPosition);

impl Default for TokenPosition {
    fn default() -> Self {
        TokenPosition {
            indexed_position: I80F48::ZERO,
            token_index: TokenIndex::MAX,
            in_use_count: 0,
            cumulative_deposit_interest: 0.0,
            cumulative_borrow_interest: 0.0,
            previous_index: I80F48::ZERO,
            padding: Default::default(),
            reserved: [0; 128],
        }
    }
}

impl TokenPosition {
    pub fn is_active(&self) -> bool {
        self.token_index != TokenIndex::MAX
    }

    pub fn is_active_for_token(&self, token_index: TokenIndex) -> bool {
        self.token_index == token_index
    }

    pub fn native(&self, bank: &Bank) -> I80F48 {
        if self.indexed_position.is_positive() {
            self.indexed_position * bank.deposit_index
        } else {
            self.indexed_position * bank.borrow_index
        }
    }

    #[cfg(feature = "client")]
    pub fn ui(&self, bank: &Bank) -> I80F48 {
        if self.indexed_position.is_positive() {
            (self.indexed_position * bank.deposit_index)
                / I80F48::from_num(10u64.pow(bank.mint_decimals as u32))
        } else {
            (self.indexed_position * bank.borrow_index)
                / I80F48::from_num(10u64.pow(bank.mint_decimals as u32))
        }
    }

    pub fn is_in_use(&self) -> bool {
        self.in_use_count > 0
    }

    pub fn increment_in_use(&mut self) {
        self.in_use_count += 1;                     
    }

    pub fn decrement_in_use(&mut self) {
        self.in_use_count = self.in_use_count.saturating_sub(1);
    }
}

#[zero_copy]
#[derive(AnchorSerialize, AnchorDeserialize, Derivative, PartialEq)]
#[derivative(Debug)]
pub struct Serum3Orders {
    pub open_orders: Pubkey,

                                                                                         
                                                                                        
                                                                                    
                                                                           
    pub base_borrows_without_fee: u64,
    pub quote_borrows_without_fee: u64,

    pub market_index: Serum3MarketIndex,

                                                                           
                                                                          
                                             
    pub base_token_index: TokenIndex,
    pub quote_token_index: TokenIndex,

    #[derivative(Debug = "ignore")]
    pub padding: [u8; 2],

                                                                                            
       
                                                                                     
                                                  
       
                                                                                      
                                 
       
                                     
    pub highest_placed_bid_inv: f64,
    pub lowest_placed_ask: f64,

                                                                                               
       
                                                                                               
                                                                             
       
                                                                                   
                                                                             
    pub potential_base_tokens: u64,
    pub potential_quote_tokens: u64,

                                                                             
       
                                     
    pub lowest_placed_bid_inv: f64,
    pub highest_placed_ask: f64,

    #[derivative(Debug = "ignore")]
    pub reserved: [u8; 16],
}
const_assert_eq!(
    size_of::<Serum3Orders>(),
    32 + 8 * 2 + 2 * 3 + 2 + 6 * 8 + 16
);
const_assert_eq!(size_of::<Serum3Orders>(), 120);
const_assert_eq!(size_of::<Serum3Orders>() % 8, 0);

impl Serum3Orders {
    pub fn is_active(&self) -> bool {
        self.market_index != Serum3MarketIndex::MAX
    }

    pub fn is_active_for_market(&self, market_index: Serum3MarketIndex) -> bool {
        self.market_index == market_index
    }
}

impl Default for Serum3Orders {
    fn default() -> Self {
        Self {
            open_orders: Pubkey::default(),
            market_index: Serum3MarketIndex::MAX,
            base_token_index: TokenIndex::MAX,
            quote_token_index: TokenIndex::MAX,
            padding: Default::default(),
            base_borrows_without_fee: 0,
            quote_borrows_without_fee: 0,
            highest_placed_bid_inv: 0.0,
            lowest_placed_ask: 0.0,
            potential_base_tokens: 0,
            potential_quote_tokens: 0,
            lowest_placed_bid_inv: 0.0,
            highest_placed_ask: 0.0,
            reserved: [0; 16],
        }
    }
}

#[zero_copy]
#[derive(AnchorSerialize, AnchorDeserialize, Derivative, PartialEq)]
#[derivative(Debug)]
pub struct OpenbookV2Orders {
    pub open_orders: Pubkey,

                                                                                   
                                                                                        
                                                                                         
                                                                           
    pub base_borrows_without_fee: u64,
    pub quote_borrows_without_fee: u64,

                                                                                            
       
                                                                                     
                                                  
       
                                                                                           
                                 
       
                                     
    pub highest_placed_bid_inv: f64,
    pub lowest_placed_ask: f64,

                                                                                               
       
                                                                                                  
                                                                             
       
                                                                                   
                                                                                  
    pub potential_base_tokens: u64,
    pub potential_quote_tokens: u64,

                                                                             
       
                                     
    pub lowest_placed_bid_inv: f64,
    pub highest_placed_ask: f64,

                                     
       
                                                                                               
                                                                                  
    pub quote_lot_size: i64,
    pub base_lot_size: i64,

    pub market_index: OpenbookV2MarketIndex,

                                                                           
                                                                          
                                             
    pub base_token_index: TokenIndex,
    pub quote_token_index: TokenIndex,

    #[derivative(Debug = "ignore")]
    pub reserved: [u8; 162],
}
const_assert_eq!(size_of::<OpenbookV2Orders>(), 32 + 8 * 10 + 2 * 3 + 162);
const_assert_eq!(size_of::<OpenbookV2Orders>(), 280);
const_assert_eq!(size_of::<OpenbookV2Orders>() % 8, 0);

impl OpenbookV2Orders {
    pub fn is_active(&self) -> bool {
        self.market_index != OpenbookV2MarketIndex::MAX
    }

    pub fn is_active_for_market(&self, market_index: OpenbookV2MarketIndex) -> bool {
        self.market_index == market_index
    }
}

impl Default for OpenbookV2Orders {
    fn default() -> Self {
        Self {
            open_orders: Pubkey::default(),
            market_index: OpenbookV2MarketIndex::MAX,
            base_token_index: TokenIndex::MAX,
            quote_token_index: TokenIndex::MAX,
            base_borrows_without_fee: 0,
            quote_borrows_without_fee: 0,
            highest_placed_bid_inv: 0.0,
            lowest_placed_bid_inv: 0.0,
            highest_placed_ask: 0.0,
            lowest_placed_ask: 0.0,
            potential_base_tokens: 0,
            potential_quote_tokens: 0,
            quote_lot_size: 0,
            base_lot_size: 0,
            reserved: [0; 162],
        }
    }
}

#[repr(C, packed)]
#[zero_copy]
#[derive(Derivative, PartialEq)]
#[derivative(Debug)]
pub struct PerpPosition {
    pub market_index: PerpMarketIndex,

    #[derivative(Debug = "ignore")]
    pub padding: [u8; 2],

                                                    
    pub settle_pnl_limit_window: u32,

                                                                                             
       
                                                       
       
                                                                                              
                                                                                      
                   
    pub settle_pnl_limit_settled_in_current_window_native: i64,

                                                   
    pub base_position_lots: i64,

                                                                                                          
       
                                                                                                       
                                                                                                  
                                                                                                          
                                                                                     
                                                                                                          
                                                                                                    
    pub quote_position_native: I80F48,

                                                                                 
    pub quote_running_native: i64,

                                    
    pub long_settled_funding: I80F48,
                                     
    pub short_settled_funding: I80F48,

                              
    pub bids_base_lots: i64,
                              
    pub asks_base_lots: i64,

                                                                     
    pub taker_base_lots: i64,
                                                                      
    pub taker_quote_lots: i64,

                                                      
                                                                              
                           
       
                      
    pub cumulative_long_funding: f64,
                                                      
                                                                                
       
                      
    pub cumulative_short_funding: f64,

                                                     
       
                      
    pub maker_volume: u64,
                                                     
       
                      
    pub taker_volume: u64,

                                                                                 
                                          
       
                                                                                
                                    
       
                      
    pub perp_spot_transfers: i64,

                                                                                 
                                                               
    pub avg_entry_price_per_base_lot: f64,

                                                                                                    
    pub deprecated_realized_trade_pnl_native: I80F48,

                                               
       
                                                                                   
                                                                        
       
                                                          
    pub oneshot_settle_pnl_allowance: I80F48,

                                                                
       
                                                                                
                                                                                  
                                                            
                                                                                
                                                               
       
                                                                                         
                                                                                        
                                            
       
                                                                  
    pub recurring_settle_pnl_allowance: i64,

                                                                                      
       
                                                             
                                  
       
                                                                                          
                                                                        
    pub realized_pnl_for_position_native: I80F48,

    #[derivative(Debug = "ignore")]
    pub reserved: [u8; 88],
}
const_assert_eq!(
    size_of::<PerpPosition>(),
    2 + 2 + 4 + 8 + 8 + 16 + 8 + 16 * 2 + 8 * 2 + 8 * 2 + 8 * 5 + 8 + 2 * 16 + 8 + 16 + 88
);
const_assert_eq!(size_of::<PerpPosition>(), 304);
impl_borsh_for_packed_pod!(PerpPosition);
const_assert_eq!(size_of::<PerpPosition>() % 8, 0);

impl Default for PerpPosition {
    fn default() -> Self {
        Self {
            market_index: PerpMarketIndex::MAX,
            base_position_lots: 0,
            quote_position_native: I80F48::ZERO,
            quote_running_native: 0,
            bids_base_lots: 0,
            asks_base_lots: 0,
            taker_base_lots: 0,
            taker_quote_lots: 0,
            long_settled_funding: I80F48::ZERO,
            short_settled_funding: I80F48::ZERO,
            padding: Default::default(),
            cumulative_long_funding: 0.0,
            cumulative_short_funding: 0.0,
            maker_volume: 0,
            taker_volume: 0,
            perp_spot_transfers: 0,
            avg_entry_price_per_base_lot: 0.0,
            deprecated_realized_trade_pnl_native: I80F48::ZERO,
            oneshot_settle_pnl_allowance: I80F48::ZERO,
            settle_pnl_limit_window: 0,
            settle_pnl_limit_settled_in_current_window_native: 0,
            recurring_settle_pnl_allowance: 0,
            realized_pnl_for_position_native: I80F48::ZERO,
            reserved: [0; 88],
        }
    }
}

impl PerpPosition {
                                                                                              
    pub fn add_taker_trade(&mut self, side: Side, base_lots: i64, quote_lots: i64) {
        match side {
            Side::Bid => {
                self.taker_base_lots += base_lots;
                self.taker_quote_lots -= quote_lots;
            }
            Side::Ask => {
                self.taker_base_lots -= base_lots;
                self.taker_quote_lots += quote_lots;
            }
        }
    }
                                                                    
    pub fn remove_taker_trade(&mut self, base_change: i64, quote_change: i64) {
        self.taker_base_lots -= base_change;
        self.taker_quote_lots -= quote_change;
    }

    pub fn adjust_maker_lots(&mut self, side: Side, base_lots: i64) {
        match side {
            Side::Bid => {
                self.bids_base_lots += base_lots;
            }
            Side::Ask => {
                self.asks_base_lots += base_lots;
            }
        };
    }

    pub fn is_active(&self) -> bool {
        self.market_index != PerpMarketIndex::MAX
    }

    pub fn is_active_for_market(&self, market_index: PerpMarketIndex) -> bool {
        self.market_index == market_index
    }

                                                             
    pub fn base_position_native(&self, market: &PerpMarket) -> I80F48 {
        I80F48::from(self.base_position_lots * market.base_lot_size)
    }

    pub fn base_position_lots(&self) -> i64 {
        self.base_position_lots
    }

                                                                                                   
    pub fn effective_base_position_lots(&self) -> i64 {
        self.base_position_lots + self.taker_base_lots
    }

    pub fn quote_position_native(&self) -> I80F48 {
        self.quote_position_native
    }

                                                      
    fn change_base_position(&mut self, perp_market: &mut PerpMarket, base_change: i64) {
        let start = self.base_position_lots;
        self.base_position_lots += base_change;
        perp_market.open_interest += self.base_position_lots.abs() - start.abs();
    }

                                                                              
    pub fn unsettled_funding(&self, perp_market: &PerpMarket) -> I80F48 {
        match { self.base_position_lots }.cmp(&0) {
            Ordering::Greater => {
                (perp_market.long_funding - self.long_settled_funding)
                    * I80F48::from_num(self.base_position_lots)
            }
            Ordering::Less => {
                (perp_market.short_funding - self.short_settled_funding)
                    * I80F48::from_num(self.base_position_lots)
            }
            Ordering::Equal => I80F48::ZERO,
        }
    }

                                                                
    pub fn settle_funding(&mut self, perp_market: &PerpMarket) {
        let funding = self.unsettled_funding(perp_market);
        self.quote_position_native = { self.quote_position_native } - funding;
        self.oneshot_settle_pnl_allowance = { self.oneshot_settle_pnl_allowance } - funding;
        self.realized_pnl_for_position_native = { self.realized_pnl_for_position_native } - funding;

        if self.base_position_lots.is_positive() {
            self.cumulative_long_funding += funding.to_num::<f64>();
        } else {
            self.cumulative_short_funding -= funding.to_num::<f64>();
        }

        self.long_settled_funding = perp_market.long_funding;
        self.short_settled_funding = perp_market.short_funding;
    }

                                                                                  
       
                                  
    fn update_trade_stats(
        &mut self,
        base_change: i64,
        quote_change_native: I80F48,
        perp_market: &PerpMarket,
    ) -> I80F48 {
        if base_change == 0 {
            return I80F48::ZERO;
        }

        let old_position = self.base_position_lots;
        let new_position = old_position + base_change;

                                                
                                                         
                                                     
        let reduced_lots;
                             
                                                          
                                                    
        let increased_lots;
                                                                    
        let newly_realized_pnl;

        if new_position == 0 {
            reduced_lots = old_position.abs();
            increased_lots = 0;

            let avg_entry = I80F48::from_num(self.avg_entry_price_per_base_lot);
            newly_realized_pnl = quote_change_native + I80F48::from(base_change) * avg_entry;

                                                                               
            self.avg_entry_price_per_base_lot = 0.0;
            self.quote_running_native = 0;
            self.realized_pnl_for_position_native = I80F48::ZERO;
        } else if old_position.signum() != new_position.signum() {
                                                                                                    
            reduced_lots = old_position.abs();
            increased_lots = new_position.abs();
            let old_position = old_position as f64;
            let new_position = new_position as f64;
            let base_change = base_change as f64;
            let old_avg_entry = self.avg_entry_price_per_base_lot;
            let new_avg_entry = (quote_change_native.to_num::<f64>() / base_change).abs();

                                                                
            newly_realized_pnl = I80F48::from_num(old_position * (new_avg_entry - old_avg_entry));

                                                                         
            self.avg_entry_price_per_base_lot = new_avg_entry;
            self.quote_running_native = (-new_position * new_avg_entry) as i64;

                                                
            self.realized_pnl_for_position_native = I80F48::ZERO;
        } else {
                                                          

            self.quote_running_native += quote_change_native.round_to_zero().to_num::<i64>();

            let is_increasing = old_position.signum() == base_change.signum();
            if is_increasing {
                                                                                    
                reduced_lots = 0;
                increased_lots = base_change.abs();
                newly_realized_pnl = I80F48::ZERO;
                let old_position_abs = old_position.abs() as f64;
                let new_position_abs = new_position.abs() as f64;
                let old_avg_entry = self.avg_entry_price_per_base_lot;
                let new_position_quote_value =
                    old_position_abs * old_avg_entry + quote_change_native.to_num::<f64>().abs();
                self.avg_entry_price_per_base_lot = new_position_quote_value / new_position_abs;
            } else {
                                                                                        
                reduced_lots = base_change.abs();
                increased_lots = 0;
                let avg_entry = I80F48::from_num(self.avg_entry_price_per_base_lot);
                newly_realized_pnl = quote_change_native + I80F48::from(base_change) * avg_entry;
                self.realized_pnl_for_position_native =
                    { self.realized_pnl_for_position_native } + newly_realized_pnl;
            }
        }

        let net_base_increase = increased_lots - reduced_lots;
        self.recurring_settle_pnl_allowance = self.recurring_settle_pnl_allowance.abs();
        self.recurring_settle_pnl_allowance -=
            (I80F48::from(net_base_increase * perp_market.base_lot_size)
                * perp_market.stable_price()
                * I80F48::from_num(perp_market.settle_pnl_limit_factor))
            .clamp_to_i64();
        self.recurring_settle_pnl_allowance = self.recurring_settle_pnl_allowance.max(0);

        newly_realized_pnl
    }

                                                        
    fn apply_recurring_settle_pnl_allowance_constraint(&mut self, perp_market: &PerpMarket) -> i64 {
                                
        self.recurring_settle_pnl_allowance = self.recurring_settle_pnl_allowance.abs();
        self.deprecated_realized_trade_pnl_native = I80F48::ZERO;

        let before = self.recurring_settle_pnl_allowance;

                                                                    
        let upnl = self
            .unsettled_pnl(perp_market, perp_market.stable_price())
            .unwrap();
        let upnl_abs = upnl.abs().ceil().to_num::<i64>();
        self.recurring_settle_pnl_allowance =
            self.recurring_settle_pnl_allowance.min(upnl_abs).max(0);

        self.recurring_settle_pnl_allowance - before
    }

                                                                    
       
                                  
    pub fn record_trade(
        &mut self,
        perp_market: &mut PerpMarket,
        base_change: i64,
        quote_change_native: I80F48,
    ) -> I80F48 {
        assert_eq!({ perp_market.perp_market_index }, { self.market_index });
        let realized_pnl = self.update_trade_stats(base_change, quote_change_native, perp_market);
        self.change_base_position(perp_market, base_change);
        self.change_quote_position(quote_change_native);
        self.apply_recurring_settle_pnl_allowance_constraint(perp_market);
        realized_pnl
    }

    fn change_quote_position(&mut self, quote_change_native: I80F48) {
        self.quote_position_native = { self.quote_position_native } + quote_change_native;
    }

                                                  
       
                                                                            
                                                                           
    pub fn has_open_orders(&self) -> bool {
        self.asks_base_lots != 0 || self.bids_base_lots != 0
    }

                                                           
    pub fn has_open_taker_fills(&self) -> bool {
        self.taker_base_lots != 0 || self.taker_quote_lots != 0
    }

                                                                           
    pub fn has_open_orders_or_fills(&self) -> bool {
        self.has_open_orders() || self.has_open_taker_fills()
    }

                                                                                 
    pub fn avg_entry_price(&self, market: &PerpMarket) -> f64 {
        assert_eq!({ self.market_index }, { market.perp_market_index });
        self.avg_entry_price_per_base_lot / (market.base_lot_size as f64)
    }

                                                                              
    pub fn break_even_price(&self, market: &PerpMarket) -> f64 {
        if self.base_position_lots == 0 {
            return 0.0;
        }
        assert_eq!({ self.market_index }, { market.perp_market_index });
        -(self.quote_running_native as f64)
            / ((self.base_position_lots * market.base_lot_size) as f64)
    }

                                                           
    pub fn unsettled_pnl(&self, perp_market: &PerpMarket, price: I80F48) -> Result<I80F48> {
        require_eq!(self.market_index, perp_market.perp_market_index);
        let base_native = self.base_position_native(perp_market);
        let pnl = self.quote_position_native() + base_native * price;
        Ok(pnl)
    }

                                                                       
                                              
    pub fn update_settle_limit(&mut self, market: &PerpMarket, now_ts: u64) {
        assert_eq!({ self.market_index }, { market.perp_market_index });
        let window_size = market.settle_pnl_limit_window_size_ts;
        let window_start = self.settle_pnl_limit_window as u64 * window_size;
        let window_end = window_start + window_size;
                                                                                     
        let new_window = now_ts >= window_end || now_ts < window_start;
        if new_window {
            self.settle_pnl_limit_window = (now_ts / window_size).try_into().unwrap();
            self.settle_pnl_limit_settled_in_current_window_native = 0;
        }
    }

                                                                                                 
       
                                                                                     
                                     
                                                                                            
                                                                                          
                                                                                                  
    pub fn settle_limit(&self, market: &PerpMarket) -> (i64, i64) {
        assert_eq!({ self.market_index }, { market.perp_market_index });
        if market.settle_pnl_limit_factor < 0.0 {
            return (i64::MIN, i64::MAX);
        }

        let base_native = self.base_position_native(market);
        let position_value = market.stable_price() * base_native;

        let position_value_abs = position_value.abs().to_num::<f64>();
        let unrealized =
            (market.settle_pnl_limit_factor as f64 * position_value_abs).clamp_to_i64();

        let upnl_abs = (self.quote_position_native() + position_value)
            .abs()
            .ceil()
            .to_num::<i64>();

        let mut max_pnl = unrealized
                                                    
                                                                                       
            + self.recurring_settle_pnl_allowance.abs().min(upnl_abs);
        let mut min_pnl = -max_pnl;

        let oneshot = self.oneshot_settle_pnl_allowance;
        if oneshot >= 0 {
            max_pnl = max_pnl.saturating_add(oneshot.ceil().clamp_to_i64());
        } else {
            min_pnl = min_pnl.saturating_add(oneshot.floor().clamp_to_i64());
        };

                                              
        (min_pnl.min(0), max_pnl.max(0))
    }

                                                                                          
                           
       
                                                                                         
                                             
    pub fn available_settle_limit(&self, market: &PerpMarket) -> (i64, i64) {
        assert_eq!({ self.market_index }, { market.perp_market_index });
        if market.settle_pnl_limit_factor < 0.0 {
            return (i64::MIN, i64::MAX);
        }

        let (mut min_pnl, mut max_pnl) = self.settle_limit(market);
        let used = self.settle_pnl_limit_settled_in_current_window_native;

        min_pnl = min_pnl.saturating_sub(used).min(0);
        max_pnl = max_pnl.saturating_sub(used).max(0);

        (min_pnl, max_pnl)
    }

                                                                                 
    pub fn apply_pnl_settle_limit(&self, market: &PerpMarket, pnl: I80F48) -> I80F48 {
        if market.settle_pnl_limit_factor < 0.0 {
            return pnl;
        }

        let (min_pnl, max_pnl) = self.available_settle_limit(market);
        if pnl < 0 {
            pnl.max(I80F48::from(min_pnl))
        } else {
            pnl.min(I80F48::from(max_pnl))
        }
    }

                                                   
       
                                                                                           
    pub fn record_settle(&mut self, settled_pnl: I80F48, perp_market: &PerpMarket) {
        self.change_quote_position(-settled_pnl);

                                                                        
                                                                                                   
        let oneshot_reduction = if settled_pnl > 0 {
            settled_pnl
                .min(self.oneshot_settle_pnl_allowance)
                .max(I80F48::ZERO)
        } else {
            settled_pnl
                .max(self.oneshot_settle_pnl_allowance)
                .min(I80F48::ZERO)
        };
        self.oneshot_settle_pnl_allowance =
            { self.oneshot_settle_pnl_allowance } - oneshot_reduction;

                                       
                                                                                              
                                                                      
        let mut used_settle_limit = (settled_pnl - oneshot_reduction)
            .round_to_zero()
            .clamp_to_i64();

                                                                                                  
                                                                                 
                                                                            
                                                                                             
                                                       
        let recurring_allowance_change =
            self.apply_recurring_settle_pnl_allowance_constraint(perp_market);
        if recurring_allowance_change < 0 {
            if used_settle_limit > 0 {
                used_settle_limit = (used_settle_limit + recurring_allowance_change).max(0);
            } else {
                used_settle_limit = (used_settle_limit - recurring_allowance_change).min(0);
            }
        }

        self.settle_pnl_limit_settled_in_current_window_native += used_settle_limit;
    }

                                                          
    pub fn record_trading_fee(&mut self, fee: I80F48) {
        self.change_quote_position(-fee);
        self.oneshot_settle_pnl_allowance = { self.oneshot_settle_pnl_allowance } - fee;
        self.realized_pnl_for_position_native = { self.realized_pnl_for_position_native } - fee;
    }

                                                                                               
    pub fn record_liquidation_quote_change(&mut self, change: I80F48) {
        self.change_quote_position(change);
        self.oneshot_settle_pnl_allowance = { self.oneshot_settle_pnl_allowance } + change;
    }

                                                                                           
    pub fn record_liquidation_pnl_takeover(
        &mut self,
        change: I80F48,
        recurring_limit: i64,
        oneshot_limit: i64,
    ) {
        self.change_quote_position(change);
        self.recurring_settle_pnl_allowance += recurring_limit;
        self.oneshot_settle_pnl_allowance =
            { self.oneshot_settle_pnl_allowance } + I80F48::from(oneshot_limit);
    }
}

#[repr(C, packed)]
#[zero_copy]
#[derive(Derivative, PartialEq)]
#[derivative(Debug)]
pub struct PerpOpenOrder {
    pub side_and_tree: u8,                                        

    #[derivative(Debug = "ignore")]
    pub padding1: [u8; 1],

    pub market: PerpMarketIndex,

    #[derivative(Debug = "ignore")]
    pub padding2: [u8; 4],

    pub client_id: u64,
    pub id: u128,

    pub quantity: i64,

                                                                              
    #[derivative(Debug = "ignore")]
    pub reserved: [u8; 56],
}
const_assert_eq!(size_of::<PerpOpenOrder>(), 1 + 1 + 2 + 4 + 8 + 16 + 8 + 56);
const_assert_eq!(size_of::<PerpOpenOrder>(), 96);
const_assert_eq!(size_of::<PerpOpenOrder>() % 8, 0);
impl_borsh_for_packed_pod!(PerpOpenOrder);

impl Default for PerpOpenOrder {
    fn default() -> Self {
        Self {
            side_and_tree: SideAndOrderTree::BidFixed.into(),
            padding1: Default::default(),
            market: FREE_ORDER_SLOT,
            padding2: Default::default(),
            client_id: 0,
            id: 0,
            quantity: 0,
            reserved: [0; 56],
        }
    }
}

impl PerpOpenOrder {
    pub fn side_and_tree(&self) -> SideAndOrderTree {
        SideAndOrderTree::try_from(self.side_and_tree).unwrap()
    }

    pub fn is_active_for_market(&self, perp_market_index: PerpMarketIndex) -> bool {
        self.market == perp_market_index
    }

    pub fn is_active(&self) -> bool {
        self.market != FREE_ORDER_SLOT
    }

    pub fn clear(&mut self) {
        self.market = FREE_ORDER_SLOT;
        self.side_and_tree = SideAndOrderTree::BidFixed.into();
        self.id = 0;
        self.client_id = 0;
        self.quantity = 0;
    }
}

#[macro_export]
macro_rules! account_seeds {
    ( $account:expr ) => {
        &[
            b"FermiAccount".as_ref(),
            $account.group.as_ref(),
            $account.owner.as_ref(),
            &$account.account_num.to_le_bytes(),
            &[$account.bump],
        ]
    };
}

pub use account_seeds;

#[cfg(all(test, feature = "legacy-unit-tests"))]
mod tests {
    use crate::state::PerpMarket;
    use fixed::types::I80F48;
    use rand::Rng;

    use super::PerpPosition;

    fn create_perp_position(
        market: &PerpMarket,
        base_pos: i64,
        entry_price_per_lot: i64,
    ) -> PerpPosition {
        let mut pos = PerpPosition::default();
        pos.market_index = market.perp_market_index;
        pos.base_position_lots = base_pos;
        pos.quote_position_native = I80F48::from(-base_pos * entry_price_per_lot);
        pos.quote_running_native = -base_pos * entry_price_per_lot;
        pos.avg_entry_price_per_base_lot = entry_price_per_lot as f64;
        pos
    }

    fn test_perp_market(stable_price: f64) -> PerpMarket {
        let mut m = PerpMarket::default_for_tests();
        m.stable_price_model.stable_price = stable_price;
        m
    }

    #[test]
    fn test_quote_entry_long_increasing_from_zero() {
        let mut market = test_perp_market(10.0);
        let mut pos = create_perp_position(&market, 0, 0);
                          
        let realized = pos.record_trade(&mut market, 10, I80F48::from(-100));
        assert_eq!(pos.quote_running_native, -100);
        assert_eq!(pos.avg_entry_price(&market), 10.0);
        assert_eq!(pos.break_even_price(&market), 10.0);
        assert_eq!(pos.realized_pnl_for_position_native, realized);
        assert_eq!(realized, I80F48::ZERO);
        assert_eq!(pos.oneshot_settle_pnl_allowance, I80F48::ZERO);
        assert_eq!(pos.recurring_settle_pnl_allowance, 0);
        assert_eq!(pos.deprecated_realized_trade_pnl_native, I80F48::from(0));
    }

    #[test]
    fn test_quote_entry_short_increasing_from_zero() {
        let mut market = test_perp_market(10.0);
        let mut pos = create_perp_position(&market, 0, 0);
                           
        let realized = pos.record_trade(&mut market, -10, I80F48::from(100));
        assert_eq!(pos.quote_running_native, 100);
        assert_eq!(pos.avg_entry_price(&market), 10.0);
        assert_eq!(pos.break_even_price(&market), 10.0);
        assert_eq!(pos.realized_pnl_for_position_native, realized);
        assert_eq!(realized, I80F48::ZERO);
        assert_eq!(pos.oneshot_settle_pnl_allowance, I80F48::ZERO);
        assert_eq!(pos.recurring_settle_pnl_allowance, 0);
        assert_eq!(pos.deprecated_realized_trade_pnl_native, I80F48::from(0));
    }

    #[test]
    fn test_quote_entry_long_increasing_from_long() {
        let mut market = test_perp_market(10.0);
        let mut pos = create_perp_position(&market, 10, 10);
                          
        let realized = pos.record_trade(&mut market, 10, I80F48::from(-300));
        assert_eq!(pos.quote_running_native, -400);
        assert_eq!(pos.avg_entry_price(&market), 20.0);
        assert_eq!(pos.break_even_price(&market), 20.0);
        assert_eq!(pos.realized_pnl_for_position_native, realized);
        assert_eq!(realized, I80F48::ZERO);
        assert_eq!(pos.oneshot_settle_pnl_allowance, I80F48::ZERO);
        assert_eq!(pos.recurring_settle_pnl_allowance, 0);
        assert_eq!(pos.deprecated_realized_trade_pnl_native, I80F48::from(0));
    }

    #[test]
    fn test_quote_entry_short_increasing_from_short() {
        let mut market = test_perp_market(10.0);
        let mut pos = create_perp_position(&market, -10, 10);
                           
        let realized = pos.record_trade(&mut market, -10, I80F48::from(300));
        assert_eq!(pos.quote_running_native, 400);
        assert_eq!(pos.avg_entry_price(&market), 20.0);
        assert_eq!(pos.break_even_price(&market), 20.0);
        assert_eq!(pos.realized_pnl_for_position_native, realized);
        assert_eq!(realized, I80F48::ZERO);
        assert_eq!(pos.oneshot_settle_pnl_allowance, I80F48::ZERO);
        assert_eq!(pos.recurring_settle_pnl_allowance, 0);
        assert_eq!(pos.deprecated_realized_trade_pnl_native, I80F48::from(0));
    }

    #[test]
    fn test_quote_entry_long_decreasing_from_short() {
        let mut market = test_perp_market(10.0);
        let mut pos = create_perp_position(&market, -10, 10);
                         
        let realized = pos.record_trade(&mut market, 5, I80F48::from(-250));
        assert_eq!(pos.quote_running_native, -150);
        assert_eq!(pos.avg_entry_price(&market), 10.0);                                                
        assert_eq!(pos.break_even_price(&market), -30.0);                                      
        assert_eq!(pos.realized_pnl_for_position_native, realized);
        assert_eq!(realized, I80F48::from(-200));
        assert_eq!(pos.oneshot_settle_pnl_allowance, I80F48::ZERO);
        assert_eq!(pos.recurring_settle_pnl_allowance, 11);                           
        assert_eq!(pos.deprecated_realized_trade_pnl_native, I80F48::ZERO);
    }

    #[test]
    fn test_quote_entry_short_decreasing_from_long() {
        let mut market = test_perp_market(10.0);
        let mut pos = create_perp_position(&market, 10, 10);
                          
        let realized = pos.record_trade(&mut market, -5, I80F48::from(250));
        assert_eq!(pos.quote_running_native, 150);
        assert_eq!(pos.avg_entry_price(&market), 10.0);                                                
        assert_eq!(pos.break_even_price(&market), -30.0);                      
        assert_eq!(pos.realized_pnl_for_position_native, realized);
        assert_eq!(realized, I80F48::from(200));
        assert_eq!(pos.oneshot_settle_pnl_allowance, I80F48::ZERO);
        assert_eq!(pos.recurring_settle_pnl_allowance, 11);                           
        assert_eq!(pos.deprecated_realized_trade_pnl_native, I80F48::ZERO);
    }

    #[test]
    fn test_quote_entry_long_close_with_short() {
        let mut market = test_perp_market(10.0);
        let mut pos = create_perp_position(&market, 10, 10);
                           
        let realized = pos.record_trade(&mut market, -10, I80F48::from(250));
        assert_eq!(pos.quote_running_native, 0);
        assert_eq!(pos.avg_entry_price(&market), 0.0);                                     
        assert_eq!(pos.break_even_price(&market), 0.0);
        assert_eq!(pos.realized_pnl_for_position_native, I80F48::from(0));
        assert_eq!(realized, I80F48::from(150));
        assert_eq!(pos.oneshot_settle_pnl_allowance, I80F48::ZERO);
        assert_eq!(pos.recurring_settle_pnl_allowance, 21);                            
        assert_eq!(pos.deprecated_realized_trade_pnl_native, I80F48::ZERO);
    }

    #[test]
    fn test_quote_entry_short_close_with_long() {
        let mut market = test_perp_market(10.0);
        let mut pos = create_perp_position(&market, -10, 10);
                          
        let realized = pos.record_trade(&mut market, 10, I80F48::from(-250));
        assert_eq!(pos.quote_running_native, 0);
        assert_eq!(pos.avg_entry_price(&market), 0.0);                                     
        assert_eq!(pos.break_even_price(&market), 0.0);
        assert_eq!(pos.realized_pnl_for_position_native, I80F48::from(0));
        assert_eq!(realized, I80F48::from(-150));
        assert_eq!(pos.oneshot_settle_pnl_allowance, I80F48::ZERO);
        assert_eq!(pos.recurring_settle_pnl_allowance, 21);                            
        assert_eq!(pos.deprecated_realized_trade_pnl_native, I80F48::ZERO);
    }

    #[test]
    fn test_quote_entry_long_close_short_with_overflow() {
        let mut market = test_perp_market(10.0);
        let mut pos = create_perp_position(&market, 10, 10);
                           
        let realized = pos.record_trade(&mut market, -15, I80F48::from(300));
        assert_eq!(pos.quote_running_native, 100);
        assert_eq!(pos.avg_entry_price(&market), 20.0);
        assert_eq!(pos.break_even_price(&market), 20.0);
        assert_eq!(pos.realized_pnl_for_position_native, I80F48::ZERO);                
        assert_eq!(realized, I80F48::from(100));
        assert_eq!(pos.oneshot_settle_pnl_allowance, I80F48::ZERO);
        assert_eq!(pos.recurring_settle_pnl_allowance, 11);                           
        assert_eq!(pos.deprecated_realized_trade_pnl_native, I80F48::ZERO);
    }

    #[test]
    fn test_quote_entry_short_close_long_with_overflow() {
        let mut market = test_perp_market(10.0);
        let mut pos = create_perp_position(&market, -10, 10);
                          
        let realized = pos.record_trade(&mut market, 15, I80F48::from(-300));
        assert_eq!(pos.quote_running_native, -100);
        assert_eq!(pos.avg_entry_price(&market), 20.0);
        assert_eq!(pos.break_even_price(&market), 20.0);
        assert_eq!(pos.realized_pnl_for_position_native, I80F48::ZERO);                
        assert_eq!(realized, I80F48::from(-100));
        assert_eq!(pos.oneshot_settle_pnl_allowance, I80F48::ZERO);
        assert_eq!(pos.recurring_settle_pnl_allowance, 11);                           
        assert_eq!(pos.deprecated_realized_trade_pnl_native, I80F48::ZERO);
    }

    #[test]
    fn test_quote_entry_break_even_price() {
        let mut market = test_perp_market(10.0);
        let mut pos = create_perp_position(&market, 0, 0);
                          
        let realized_buy = pos.record_trade(&mut market, 11, I80F48::from(-11 * 10_000));
                          
        let realized_sell = pos.record_trade(&mut market, -1, I80F48::from(12_000));
        assert_eq!(pos.quote_running_native, -98_000);
        assert_eq!(pos.base_position_lots, 10);
        assert_eq!(pos.break_even_price(&market), 9_800.0);                                                                                 
        assert_eq!(
            pos.realized_pnl_for_position_native,
            realized_buy + realized_sell
        );
        assert_eq!(realized_buy, I80F48::ZERO);
        assert_eq!(realized_sell, I80F48::from(2_000));
        assert_eq!(pos.oneshot_settle_pnl_allowance, I80F48::ZERO);
        assert_eq!(pos.recurring_settle_pnl_allowance, 3);                           
        assert_eq!(pos.deprecated_realized_trade_pnl_native, I80F48::ZERO);
    }

    #[test]
    fn test_entry_and_break_even_prices_with_lots() {
        let mut market = test_perp_market(10.0);
        market.base_lot_size = 10;

        let mut pos = create_perp_position(&market, 0, 0);
                           
        let realized_buy = pos.record_trade(&mut market, 11, I80F48::from(-11 * 10 * 10_000));
                           
        let realized_sell = pos.record_trade(&mut market, -1, I80F48::from(1 * 10 * 12_000));
        assert_eq!(pos.quote_running_native, -980_000);
        assert_eq!(pos.base_position_lots, 10);
        assert_eq!(pos.avg_entry_price_per_base_lot, 100_000.0);
        assert_eq!(pos.avg_entry_price(&market), 10_000.0);
        assert_eq!(pos.break_even_price(&market), 9_800.0);

        assert_eq!(
            pos.realized_pnl_for_position_native,
            realized_buy + realized_sell
        );
        assert_eq!(realized_buy, I80F48::ZERO);
        assert_eq!(realized_sell, I80F48::from(20_000));
        assert_eq!(pos.oneshot_settle_pnl_allowance, I80F48::ZERO);
        assert_eq!(pos.recurring_settle_pnl_allowance, 21);                            
        assert_eq!(pos.deprecated_realized_trade_pnl_native, I80F48::ZERO);
    }

    #[test]
    fn test_perp_realized_settle_limit_no_reduction() {
        let mut market = test_perp_market(10000.0);
        let mut pos = create_perp_position(&market, 0, 0);
                          
        pos.record_trade(&mut market, 11, I80F48::from(-11 * 10_000));

                          
        pos.record_trade(&mut market, -1, I80F48::from(11_000));
        assert_eq!(pos.recurring_settle_pnl_allowance, 1000);                                                

                                                                      
        pos.record_trade(&mut market, -1, I80F48::from(9_500));
        assert_eq!(pos.recurring_settle_pnl_allowance, 500);

                                                        
        pos.record_trade(&mut market, -2, I80F48::from(40_000));
        assert_eq!(pos.recurring_settle_pnl_allowance, 4501);

                                               
        pos.record_trade(&mut market, 1, I80F48::from(-9_000));
        assert_eq!(pos.recurring_settle_pnl_allowance, 2501);

                                            
        market.stable_price_model.stable_price = 8000.0;
        pos.record_trade(&mut market, -1, I80F48::from(8_000));
        assert_eq!(pos.recurring_settle_pnl_allowance, 4102);

        assert_eq!(pos.deprecated_realized_trade_pnl_native, I80F48::ZERO);
    }

    #[test]
    fn test_perp_trade_without_realized_pnl() {
        let mut market = test_perp_market(10_000.0);
        let mut pos = create_perp_position(&market, 0, 0);

                          
        pos.record_trade(&mut market, 11, I80F48::from(-11 * 10_000));

                          
        let realized = pos.record_trade(&mut market, -1, I80F48::from(10_000));
        assert_eq!(realized, I80F48::ZERO);
        assert_eq!(pos.realized_pnl_for_position_native, I80F48::from(0));
        assert_eq!(pos.recurring_settle_pnl_allowance, 0);

                           
        let realized = pos.record_trade(&mut market, -10, I80F48::from(10 * 10_000));
        assert_eq!(realized, I80F48::ZERO);
        assert_eq!(pos.realized_pnl_for_position_native, I80F48::from(0));
        assert_eq!(pos.recurring_settle_pnl_allowance, 0);

        assert_eq!(pos.base_position_lots, 0);
        assert_eq!(pos.quote_position_native, I80F48::ZERO);
    }

    #[test]
    fn test_perp_oneshot_settle_allowance() {
        let mut market = test_perp_market(10_000.0);
        let mut pos = create_perp_position(&market, 0, 0);

        pos.record_trading_fee(I80F48::from(-70));
        assert_eq!(pos.oneshot_settle_pnl_allowance, I80F48::from(70));

        pos.record_liquidation_quote_change(I80F48::from(30));
        assert_eq!(pos.oneshot_settle_pnl_allowance, I80F48::from(100));

                         
        pos.record_trade(&mut market, 1, I80F48::from(-1 * 10_000));

                          
        pos.record_trade(&mut market, -1, I80F48::from(11_000));

        assert_eq!(pos.oneshot_settle_pnl_allowance, I80F48::from(100));
        assert_eq!(pos.recurring_settle_pnl_allowance, 1100);                   

        pos.record_settle(I80F48::from(50), &market);
        assert_eq!(pos.oneshot_settle_pnl_allowance, I80F48::from(50));
        assert_eq!(pos.recurring_settle_pnl_allowance, 1050);

        pos.record_settle(I80F48::from(100), &market);
        assert_eq!(pos.oneshot_settle_pnl_allowance, I80F48::from(0));
        assert_eq!(pos.recurring_settle_pnl_allowance, 950);
    }

    #[test]
    fn test_realized_pnl_fractional() {
        let mut market = test_perp_market(10.0);
        let mut pos = create_perp_position(&market, 0, 0);
        pos.quote_position_native = pos.quote_position_native + I80F48::from_num(0.1);

                    
        pos.record_trade(&mut market, 1, I80F48::from(-1));
                    
        pos.record_trade(&mut market, 2, I80F48::from(-2 * 2));

        assert!((pos.avg_entry_price(&market) - 1.66666).abs() < 0.001);

                     
        let realized1 = pos.record_trade(&mut market, -2, I80F48::from(2 * 4));

        assert!((pos.avg_entry_price(&market) - 1.66666).abs() < 0.001);
        assert!((realized1.to_num::<f64>() - 4.6666).abs() < 0.01);

                     
        let realized2 = pos.record_trade(&mut market, -1, I80F48::from(2));

        assert_eq!(pos.avg_entry_price(&market), 0.0);
        assert!((pos.quote_position_native.to_num::<f64>() - 5.1).abs() < 0.001);
        assert!((realized2.to_num::<f64>() - 0.3333).abs() < 0.01);
    }

    #[test]
    fn test_perp_entry_multiple_random_long() {
        let mut market = test_perp_market(10.0);
        let mut pos = create_perp_position(&market, 0, 0);

                                          
        let mut rng = rand::thread_rng();
        let mut trades: Vec<[i64; 2]> = Vec::with_capacity(500);
        for _ in 0..trades.capacity() {
            let qty: i64 = rng.gen_range(1..=1000);
            let px: f64 = rng.gen_range(0.1..=100.0);
            let quote: i64 = (-qty as f64 * px).round() as i64;
            trades.push([qty, quote]);
        }
                                                
        let mut total_qty = 0;
        let mut total_quote = 0;
        trades.iter().for_each(|[qty, quote]| {
            pos.record_trade(&mut market, *qty, I80F48::from(*quote));
            total_qty += qty.abs();
            total_quote += quote.abs();
            let entry_actual = pos.avg_entry_price(&market);
            let entry_expected = total_quote as f64 / total_qty as f64;
            assert!(((entry_actual - entry_expected) / entry_expected).abs() < 10.0 * f64::EPSILON);
        });
                                                             
        assert_eq!(pos.base_position_lots, total_qty);
                                     
        trades.iter().for_each(|[qty, quote]| {
            pos.record_trade(&mut market, -*qty, I80F48::from(-*quote));
        });
        assert_eq!(pos.base_position_lots, 0);
        assert_eq!(pos.quote_running_native, 0);
        assert_eq!(pos.avg_entry_price_per_base_lot, 0.0);
    }

    #[test]
    fn test_perp_position_pnl_returns_correct_pnl_for_oracle_price() {
        let mut market = test_perp_market(10.0);
        market.base_lot_size = 10;

        let long_pos = create_perp_position(&market, 50, 100);
        let pnl = long_pos.unsettled_pnl(&market, I80F48::from(11)).unwrap();
        assert_eq!(pnl, I80F48::from(50 * 10 * 1), "long profitable");
        let pnl = long_pos.unsettled_pnl(&market, I80F48::from(9)).unwrap();
        assert_eq!(pnl, I80F48::from(50 * 10 * -1), "long unprofitable");

        let short_pos = create_perp_position(&market, -50, 100);
        let pnl = short_pos.unsettled_pnl(&market, I80F48::from(11)).unwrap();
        assert_eq!(pnl, I80F48::from(50 * 10 * -1), "short unprofitable");
        let pnl = short_pos.unsettled_pnl(&market, I80F48::from(9)).unwrap();
        assert_eq!(pnl, I80F48::from(50 * 10 * 1), "short profitable");
    }

    #[test]
    fn test_perp_settle_limit_allowance_consumption() {
        let market = test_perp_market(10.0);

        let mut pos = create_perp_position(&market, 0, 0);

                                                                               
        pos.quote_position_native = I80F48::from(1100);

        pos.recurring_settle_pnl_allowance = 1000;
        pos.record_settle(I80F48::from(10), &market);
        assert_eq!(pos.recurring_settle_pnl_allowance, 1000);
        assert_eq!(pos.settle_pnl_limit_settled_in_current_window_native, 10);

        pos.record_settle(I80F48::from(-2), &market);
        assert_eq!(pos.recurring_settle_pnl_allowance, 1000);
        assert_eq!(pos.settle_pnl_limit_settled_in_current_window_native, 8);

        pos.record_settle(I80F48::from(492), &market);
        assert_eq!(pos.recurring_settle_pnl_allowance, 600);
        assert_eq!(
            pos.settle_pnl_limit_settled_in_current_window_native,
            8 + 492 - 400
        );

        pos.settle_pnl_limit_settled_in_current_window_native = 0;
        pos.recurring_settle_pnl_allowance = 0;
        pos.oneshot_settle_pnl_allowance = I80F48::from(4);
        assert_eq!(pos.available_settle_limit(&market), (0, 4));
        pos.record_settle(I80F48::from(-20), &market);
        assert_eq!(pos.settle_pnl_limit_settled_in_current_window_native, -20);
        assert_eq!(pos.oneshot_settle_pnl_allowance, I80F48::from(4));
        assert_eq!(pos.available_settle_limit(&market), (0, 24));

        pos.record_settle(I80F48::from(2), &market);
        assert_eq!(pos.settle_pnl_limit_settled_in_current_window_native, -20);
        assert_eq!(pos.oneshot_settle_pnl_allowance, I80F48::from(2));
        assert_eq!(pos.available_settle_limit(&market), (0, 22));

        pos.record_settle(I80F48::from(4), &market);
        assert_eq!(pos.settle_pnl_limit_settled_in_current_window_native, -18);
        assert_eq!(pos.oneshot_settle_pnl_allowance, I80F48::from(0));
        assert_eq!(pos.available_settle_limit(&market), (0, 18));

        pos.settle_pnl_limit_settled_in_current_window_native = 0;
        pos.recurring_settle_pnl_allowance = 0;
        pos.oneshot_settle_pnl_allowance = I80F48::from(-4);
        assert_eq!(pos.available_settle_limit(&market), (-4, 0));
        pos.record_settle(I80F48::from(20), &market);
        assert_eq!(pos.settle_pnl_limit_settled_in_current_window_native, 20);
        assert_eq!(pos.oneshot_settle_pnl_allowance, I80F48::from(-4));
        assert_eq!(pos.available_settle_limit(&market), (-24, 0));

        pos.record_settle(I80F48::from(-2), &market);
        assert_eq!(pos.settle_pnl_limit_settled_in_current_window_native, 20);
        assert_eq!(pos.oneshot_settle_pnl_allowance, I80F48::from(-2));
        assert_eq!(pos.available_settle_limit(&market), (-22, 0));

        pos.record_settle(I80F48::from(-4), &market);
        assert_eq!(pos.settle_pnl_limit_settled_in_current_window_native, 18);
        assert_eq!(pos.oneshot_settle_pnl_allowance, I80F48::from(0));
        assert_eq!(pos.available_settle_limit(&market), (-18, 0));
    }

    #[test]
    fn test_perp_settle_limit_window() {
        let mut market = PerpMarket::default_for_tests();
        let mut pos = create_perp_position(&market, 100, -50);

        market.settle_pnl_limit_window_size_ts = 100;
        pos.settle_pnl_limit_settled_in_current_window_native = 10;

        pos.update_settle_limit(&market, 505);
        assert_eq!(pos.settle_pnl_limit_settled_in_current_window_native, 0);
        assert_eq!(pos.settle_pnl_limit_window, 5);

        pos.settle_pnl_limit_settled_in_current_window_native = 10;
        pos.update_settle_limit(&market, 550);
        assert_eq!(pos.settle_pnl_limit_settled_in_current_window_native, 10);
        assert_eq!(pos.settle_pnl_limit_window, 5);

        pos.settle_pnl_limit_settled_in_current_window_native = 10;
        pos.update_settle_limit(&market, 600);
        assert_eq!(pos.settle_pnl_limit_settled_in_current_window_native, 0);
        assert_eq!(pos.settle_pnl_limit_window, 6);

        market.settle_pnl_limit_window_size_ts = 400;
        pos.update_settle_limit(&market, 605);
        assert_eq!(pos.settle_pnl_limit_settled_in_current_window_native, 0);
        assert_eq!(pos.settle_pnl_limit_window, 1);
    }

    #[test]
    fn test_perp_settle_limit() {
        let mut market = test_perp_market(0.5);

        let mut pos = create_perp_position(&market, 100, 1);

        let limited_pnl = |pos: &PerpPosition, market: &PerpMarket, pnl: i64| {
            pos.apply_pnl_settle_limit(market, I80F48::from(pnl))
                .to_num::<f64>()
        };

        assert_eq!(pos.available_settle_limit(&market), (-10, 10));                                            
        assert_eq!(limited_pnl(&pos, &market, 100), 10.0);
        assert_eq!(limited_pnl(&pos, &market, -100), -10.0);

        pos.oneshot_settle_pnl_allowance = I80F48::from_num(-5);
        assert_eq!(pos.available_settle_limit(&market), (-15, 10));
        assert_eq!(limited_pnl(&pos, &market, 100), 10.0);
        assert_eq!(limited_pnl(&pos, &market, -100), -15.0);

        pos.oneshot_settle_pnl_allowance = I80F48::from_num(5);
        assert_eq!(pos.available_settle_limit(&market), (-10, 15));
        assert_eq!(limited_pnl(&pos, &market, 100), 15.0);
        assert_eq!(limited_pnl(&pos, &market, -100), -10.0);

        pos.recurring_settle_pnl_allowance = 11;
        assert_eq!(pos.available_settle_limit(&market), (-21, 26));
        assert_eq!(limited_pnl(&pos, &market, 100), 26.0);
        assert_eq!(limited_pnl(&pos, &market, -100), -21.0);

        pos.settle_pnl_limit_settled_in_current_window_native = 17;
        assert_eq!(pos.available_settle_limit(&market), (-38, 9));

        pos.settle_pnl_limit_settled_in_current_window_native = 27;
        assert_eq!(pos.available_settle_limit(&market), (-48, 0));

        pos.settle_pnl_limit_settled_in_current_window_native = -17;
        assert_eq!(pos.available_settle_limit(&market), (-4, 43));

        pos.settle_pnl_limit_settled_in_current_window_native = -27;
        assert_eq!(pos.available_settle_limit(&market), (0, 53));

        pos.settle_pnl_limit_settled_in_current_window_native = 0;
        market.stable_price_model.stable_price = 1.0;
                                                                      
        assert_eq!(
            pos.unsettled_pnl(&market, I80F48::from_num(1.0)).unwrap(),
            I80F48::ZERO
        );
        assert_eq!(pos.available_settle_limit(&market), (-20, 25));

        pos.quote_position_native = pos.quote_position_native + I80F48::from(7);
        assert_eq!(pos.available_settle_limit(&market), (-27, 32));
    }
}
