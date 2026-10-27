use anchor_lang::prelude::*;

use super::*;

                         
pub struct Order {
    pub side: Side,

                                  
    pub max_base_lots: i64,

                                                                     
    pub max_quote_lots: i64,

                                           
    pub client_order_id: u64,

                   
    pub reduce_only: bool,

                                                                 
    pub time_in_force: u16,

                                                                      
    pub self_trade_behavior: SelfTradeBehavior,

                                  
    pub params: OrderParams,
}

pub enum OrderParams {
    Market,
    ImmediateOrCancel {
        price_lots: i64,
    },
    Fixed {
        price_lots: i64,
        order_type: PostOrderType,
    },
    OraclePegged {
        price_offset_lots: i64,
        order_type: PostOrderType,
        peg_limit: i64,
        max_oracle_staleness_slots: i32,
    },
}

impl Order {
                                                                  
    pub fn tif_from_expiry(expiry_timestamp: u64) -> Option<u16> {
        let now_ts: u64 = Clock::get().unwrap().unix_timestamp.try_into().unwrap();
        if expiry_timestamp != 0 {
                                                                        
            let tif = expiry_timestamp.saturating_sub(now_ts).min(u16::MAX.into());
            if tif == 0 {
                                                             
                return None;
            }
            Some(tif as u16)
        } else {
                           
            Some(0)
        }
    }

                                                         
       
                                                                                      
                                                               
    pub fn needs_penalty_fee(&self) -> bool {
        matches!(self.params, OrderParams::ImmediateOrCancel { .. })
    }

                                                                                            
    pub fn is_post_only(&self) -> bool {
        let order_type = match self.params {
            OrderParams::Fixed { order_type, .. } => order_type,
            OrderParams::OraclePegged { order_type, .. } => order_type,
            _ => return false,
        };
        order_type == PostOrderType::PostOnly || order_type == PostOrderType::PostOnlySlide
    }

                                                     
    pub fn post_target(&self) -> Option<BookSideOrderTree> {
        match self.params {
            OrderParams::Fixed { .. } => Some(BookSideOrderTree::Fixed),
            OrderParams::OraclePegged { .. } => Some(BookSideOrderTree::OraclePegged),
            _ => None,
        }
    }

                                                                                  
                                                             
    fn price_for_order_type(
        &self,
        now_ts: u64,
        oracle_price_lots: i64,
        price_lots: i64,
        order_type: PostOrderType,
        order_book: &Orderbook,
    ) -> i64 {
        if order_type == PostOrderType::PostOnlySlide {
            if let Some(best_other_price) = order_book
                .bookside(self.side.invert_side())
                .best_price(now_ts, oracle_price_lots)
            {
                post_only_slide_limit(self.side, best_other_price, price_lots)
            } else {
                price_lots
            }
        } else {
            price_lots
        }
    }

                                                                                         
                                                                                      
    pub fn price(
        &self,
        now_ts: u64,
        oracle_price_lots: i64,
        order_book: &Orderbook,
    ) -> Result<(i64, u64)> {
        let price_lots = match self.params {
            OrderParams::Market { .. } => market_order_limit_for_side(self.side),
            OrderParams::ImmediateOrCancel { price_lots, .. } => price_lots,
            OrderParams::Fixed {
                price_lots,
                order_type,
            } => self.price_for_order_type(
                now_ts,
                oracle_price_lots,
                price_lots,
                order_type,
                order_book,
            ),
            OrderParams::OraclePegged {
                price_offset_lots,
                order_type,
                ..
            } => {
                let price_lots = oracle_price_lots + price_offset_lots;
                self.price_for_order_type(
                    now_ts,
                    oracle_price_lots,
                    price_lots,
                    order_type,
                    order_book,
                )
            }
        };
        let price_data = match self.params {
            OrderParams::OraclePegged { .. } => {
                oracle_pegged_price_data(price_lots - oracle_price_lots)
            }
            _ => fixed_price_data(price_lots)?,
        };
        require_gte!(price_lots, 1);
        Ok((price_lots, price_data))
    }

                                                         
    pub fn peg_limit(&self) -> i64 {
        match self.params {
            OrderParams::OraclePegged { peg_limit, .. } => peg_limit,
            _ => -1,
        }
    }
}

                                                     
fn market_order_limit_for_side(side: Side) -> i64 {
    match side {
        Side::Bid => i64::MAX,
        Side::Ask => 1,
    }
}

                                                                          
                           
fn post_only_slide_limit(side: Side, best_other_side: i64, limit: i64) -> i64 {
    match side {
        Side::Bid => limit.min(best_other_side - 1),
        Side::Ask => limit.max(best_other_side + 1),
    }
}
