#![cfg(feature = "client")]

use anchor_lang::prelude::*;

use fixed::types::I80F48;

use crate::error::*;
use crate::state::Side as PerpOrderSide;
use crate::state::{Bank, FermiAccountValue, PerpMarketIndex};

use super::*;

impl HealthCache {
    pub fn is_liquidatable(&self) -> bool {
        if self.being_liquidated {
            self.health(HealthType::LiquidationEnd).is_negative()
        } else {
            self.health(HealthType::Maint).is_negative()
        }
    }

                                                                                       
       
               
                                                                                  
                                                            
    fn cache_after_swap(
        &self,
        account: &FermiAccountValue,
        source_bank: &Bank,
        source_oracle_price: I80F48,
        target_bank: &Bank,
        amount: I80F48,
        price: I80F48,
    ) -> Result<Self> {
        let now_ts = system_epoch_secs();

        let mut source_position = account.token_position(source_bank.token_index)?.clone();
        let mut target_position = account.token_position(target_bank.token_index)?.clone();

        let target_amount = amount * price;

        let mut source_bank = source_bank.clone();
        source_bank.withdraw_with_fee(&mut source_position, amount, now_ts)?;
        let mut target_bank = target_bank.clone();
        target_bank.deposit(&mut target_position, target_amount, now_ts)?;

        let mut resulting_cache = self.clone();
        resulting_cache.adjust_token_balance(&source_bank, -amount)?;
        resulting_cache.adjust_token_balance(&target_bank, target_amount)?;
        Ok(resulting_cache)
    }

    fn apply_limits_to_swap(
        account: &FermiAccountValue,
        source_bank: &Bank,
        source_oracle_price: I80F48,
        target_bank: &Bank,
        price: I80F48,
        source_unlimited: I80F48,
    ) -> Result<I80F48> {
        let source_pos = account
            .token_position(source_bank.token_index)?
            .native(source_bank);
        let target_pos = account
            .token_position(target_bank.token_index)?
            .native(target_bank);

                                     
        let available_net_borrows = source_bank
            .remaining_net_borrows_quote(source_oracle_price)
            .saturating_div(source_oracle_price);
        let potential_source = source_unlimited
            .min(available_net_borrows.saturating_add(source_pos.max(I80F48::ZERO)));

                                  
        let available_deposits = target_bank.remaining_deposits_until_limit();
        let potential_target_unlimited = potential_source.saturating_mul(price);
        let potential_target = potential_target_unlimited
            .min(available_deposits.saturating_add(-target_pos.min(I80F48::ZERO)));

        let source = potential_source.min(potential_target.saturating_div(price));
        Ok(source)
    }

                                                         
    pub fn max_swap_source_for_health_ratio_ignoring_limits(
        &self,
        account: &FermiAccountValue,
        source_bank: &Bank,
        source_oracle_price: I80F48,
        target_bank: &Bank,
        price: I80F48,
        min_ratio: I80F48,
    ) -> Result<I80F48> {
        self.max_swap_source_for_health_fn(
            account,
            source_bank,
            source_oracle_price,
            target_bank,
            price,
            min_ratio,
            |cache| cache.health_ratio(HealthType::Init),
        )
    }

    pub fn max_swap_source_for_health_ratio_with_limits(
        &self,
        account: &FermiAccountValue,
        source_bank: &Bank,
        source_oracle_price: I80F48,
        target_bank: &Bank,
        price: I80F48,
        min_ratio: I80F48,
    ) -> Result<I80F48> {
        let source_unlimited = self.max_swap_source_for_health_fn(
            account,
            source_bank,
            source_oracle_price,
            target_bank,
            price,
            min_ratio,
            |cache| cache.health_ratio(HealthType::Init),
        )?;

        Self::apply_limits_to_swap(
            account,
            source_bank,
            source_oracle_price,
            target_bank,
            price,
            source_unlimited,
        )
    }

                                                                                    
                                         
       
                                                                                        
                                                                                    
                                                                                     
                                                     
       
                                                                                           
       
                                                                                  
    pub fn max_swap_source_for_health_fn(
        &self,
        account: &FermiAccountValue,
        source_bank: &Bank,
        source_oracle_price: I80F48,
        target_bank: &Bank,
        price: I80F48,
        min_fn_value: I80F48,
        target_fn: fn(&HealthCache) -> I80F48,
    ) -> Result<I80F48> {
                                                                          
                                                                                          
                                                                                    
                                                       
          
                      
                                                                                                      
                                                                                       
                                                                                 

        let health_type = HealthType::Init;

                                                                                     
                                                           
        let source_index = find_token_info_index(&self.token_infos, source_bank.token_index)?;
        let target_index = find_token_info_index(&self.token_infos, target_bank.token_index)?;

        let source = &self.token_infos[source_index];
        let target = &self.token_infos[target_index];

        let (tokens_max_reserved, _) = self.compute_spot_reservations(health_type);
        let source_reserved = tokens_max_reserved[source_index].max_spot_reserved;
        let target_reserved = tokens_max_reserved[target_index].max_spot_reserved;

        let token_balances = self.effective_token_balances(health_type);
        let source_balance = token_balances[source_index].spot_and_perp;
        let target_balance = token_balances[target_index].spot_and_perp;

                                                                                           
                                                                                             
                                                                  
                                                                                     
                                                                                            
                                         
        let final_health_slope = -source.init_scaled_liab_weight * source.prices.liab(health_type)
            + target.init_asset_weight * target.prices.asset(health_type) * price;
        if final_health_slope >= 0 {
                                                                     
            return Ok(I80F48::MAX);
        }

        let cache_after_swap = |amount: I80F48| -> Result<Option<HealthCache>> {
            ignore_net_borrow_limit_errors(self.cache_after_swap(
                account,
                source_bank,
                source_oracle_price,
                target_bank,
                amount,
                price,
            ))
        };
        let fn_value_after_swap = |amount| {
            Ok(cache_after_swap(amount)?
                .as_ref()
                .map(target_fn)
                .unwrap_or(I80F48::MIN))
        };

                                                              
          
                                                                             
                                                            
                                                                                                
          
                                                         
        let (amount_for_max_value, max_value) = {
                                                              
            let rightmost = (source_balance.abs() + source_reserved)
                .max((target_balance.abs() + target_reserved) / price);
            find_maximum(
                I80F48::ZERO,
                rightmost,
                I80F48::from_num(0.1),
                fn_value_after_swap,
            )?
        };
        assert!(amount_for_max_value >= 0);

        if max_value <= min_fn_value {
                                                             
            return Ok(amount_for_max_value);
        }

        let amount = {
                                                                                                           
                                                                        
                                                                                     
                               
                                                                        
                                                                                       
                                                         
                                                                                             
                                                                                                
            let health_at_max_value = cache_after_swap(amount_for_max_value)?
                .map(|c| c.health(health_type))
                .unwrap_or(I80F48::MIN);
            if health_at_max_value == 0 {
                return Ok(amount_for_max_value);
            } else if health_at_max_value < 0 {
                                                                                 
                return Ok(I80F48::ZERO);
            }
            let zero_health_estimate =
                amount_for_max_value - health_at_max_value / final_health_slope;
            let right_bound = scan_right_until_less_than(
                zero_health_estimate,
                min_fn_value,
                fn_value_after_swap,
            )?;
            if right_bound == zero_health_estimate {
                binary_search(
                    amount_for_max_value,
                    max_value,
                    right_bound,
                    min_fn_value,
                    I80F48::from_num(0.1),
                    fn_value_after_swap,
                )?
            } else {
                binary_search(
                    zero_health_estimate,
                    fn_value_after_swap(zero_health_estimate)?,
                    right_bound,
                    min_fn_value,
                    I80F48::from_num(0.1),
                    fn_value_after_swap,
                )?
            }
        };

        assert!(amount >= 0);
        Ok(amount)
    }

                                                                                  
    pub fn max_perp_for_health_ratio(
        &self,
        perp_market_index: PerpMarketIndex,
        price: I80F48,
        side: PerpOrderSide,
        min_ratio: I80F48,
    ) -> Result<i64> {
        let health_type = HealthType::Init;
        let initial_ratio = self.health_ratio(health_type);
        if initial_ratio < 0 {
            return Ok(0);
        }

        let direction = match side {
            PerpOrderSide::Bid => 1,
            PerpOrderSide::Ask => -1,
        };

        let perp_info_index = self.perp_info_index(perp_market_index)?;
        let perp_info = &self.perp_infos[perp_info_index];
        let prices = &perp_info.base_prices;
        let base_lot_size = I80F48::from(perp_info.base_lot_size);

        let settle_info_index = self.token_info_index(perp_info.settle_token_index)?;
        let settle_info = &self.token_infos[settle_info_index];

                                                                                         
                                                                                         
                                                                                               
        let mut final_health_slope = if direction == 1 {
            perp_info.init_base_asset_weight * prices.asset(health_type) - price
        } else {
            -perp_info.init_base_liab_weight * prices.liab(health_type) + price
        };
        if final_health_slope >= 0 {
            return Ok(i64::MAX);
        }
        final_health_slope *= settle_info.liab_weighted_price(health_type);

        let cache_after_trade = |base_lots: i64| -> Result<HealthCache> {
            let mut adjusted_cache = self.clone();
            adjusted_cache.perp_infos[perp_info_index].base_lots += direction * base_lots;
            adjusted_cache.perp_infos[perp_info_index].quote -=
                I80F48::from(direction) * I80F48::from(base_lots) * base_lot_size * price;
            Ok(adjusted_cache)
        };
        let health_ratio_after_trade =
            |base_lots: i64| Ok(cache_after_trade(base_lots)?.health_ratio(health_type));
        let health_ratio_after_trade_trunc =
            |base_lots: I80F48| health_ratio_after_trade(base_lots.round_to_zero().to_num());

        let initial_base_lots = perp_info.base_lots;

                               
                                              
                                                                               
        let has_case2 =
            initial_base_lots > 0 && direction == -1 || initial_base_lots < 0 && direction == 1;

        let (case1_start, case1_start_ratio) = if has_case2 {
            let case1_start = initial_base_lots.abs();
            let case1_start_ratio = health_ratio_after_trade(case1_start)?;
            (case1_start, case1_start_ratio)
        } else {
            (0, initial_ratio)
        };
        let case1_start_i80f48 = I80F48::from(case1_start);

                                                                                 
        let base_lots = if initial_ratio <= min_ratio && case1_start_ratio < min_ratio {
            if case1_start_ratio >= initial_ratio {
                case1_start_i80f48
            } else {
                I80F48::ZERO
            }
        } else if case1_start_ratio >= min_ratio {
                                                               

                                                                                                   
                                                                                    
                                                    
            let mut start_cache = cache_after_trade(case1_start)?;
                                                                                                     
                                                                                                     
                                  
            start_cache.perp_infos[perp_info_index].init_overall_asset_weight = I80F48::ONE;
                                                                                        
                                                                                                
                                                                            
            {
                let settle_info = &mut start_cache.token_infos[settle_info_index];
                settle_info.init_asset_weight = settle_info.init_liab_weight;
                settle_info.init_scaled_asset_weight = settle_info.init_scaled_liab_weight;
            }
            let start_health = start_cache.health(health_type);
            if start_health <= 0 {
                return Ok(0);
            }

                                                                                                        
                                                                                                    
                                                                  
            let zero_health_amount = case1_start_i80f48
                - start_health / (final_health_slope * base_lot_size * I80F48::from_num(0.99))
                + I80F48::ONE;
            let zero_health_ratio = health_ratio_after_trade_trunc(zero_health_amount)?;
            assert!(zero_health_ratio <= 0);

            binary_search(
                case1_start_i80f48,
                case1_start_ratio,
                zero_health_amount,
                min_ratio,
                I80F48::ONE,
                health_ratio_after_trade_trunc,
            )?
        } else {
                                        
            binary_search(
                I80F48::ZERO,
                initial_ratio,
                case1_start_i80f48,
                min_ratio,
                I80F48::ONE,
                health_ratio_after_trade_trunc,
            )?
        };

        Ok(base_lots.round_to_zero().to_num())
    }

    fn max_borrow_for_health_fn(
        &self,
        account: &FermiAccountValue,
        bank: &Bank,
        min_fn_value: I80F48,
        target_fn: fn(&HealthCache) -> I80F48,
    ) -> Result<I80F48> {
                                             
        if target_fn(self) <= min_fn_value {
            return Ok(I80F48::ZERO);
        }

        let health_type = HealthType::Init;

                                                                                     
                                                           
        let token_info_index = find_token_info_index(&self.token_infos, bank.token_index)?;
        let token = &self.token_infos[token_info_index];
        let token_balance =
            self.effective_token_balances(health_type)[token_info_index].spot_and_perp;

        let cache_after_borrow = |amount: I80F48| -> Result<HealthCache> {
            let now_ts = system_epoch_secs();

            let mut position = account.token_position(bank.token_index)?.clone();

            let mut bank = bank.clone();
            bank.withdraw_with_fee(&mut position, amount, now_ts)?;
            bank.check_net_borrows(token.prices.oracle)?;

            let mut resulting_cache = self.clone();
            resulting_cache.adjust_token_balance(&bank, -amount)?;

            Ok(resulting_cache)
        };
        let fn_value_after_borrow = |amount: I80F48| -> Result<I80F48> {
            Ok(ignore_net_borrow_limit_errors(cache_after_borrow(amount))?
                .as_ref()
                .map(target_fn)
                .unwrap_or(I80F48::MIN))
        };

                                                                                    
                                                     
        let limit = token_balance.max(I80F48::ZERO)
            + self.health(health_type).max(I80F48::ZERO) / token.init_scaled_liab_weight;
        if limit <= 0 {
            return Ok(I80F48::ZERO);
        }

        binary_search(
            I80F48::ZERO,
            target_fn(self),
            limit,
            min_fn_value,
            I80F48::ONE,
            fn_value_after_borrow,
        )
    }

    pub fn max_borrow_for_health_ratio(
        &self,
        account: &FermiAccountValue,
        bank: &Bank,
        min_ratio: I80F48,
    ) -> Result<I80F48> {
        self.max_borrow_for_health_fn(account, bank, min_ratio, |cache| {
            cache.health_ratio(HealthType::Init)
        })
    }
}

fn scan_right_until_less_than(
    start: I80F48,
    target: I80F48,
    fun: impl Fn(I80F48) -> Result<I80F48>,
) -> Result<I80F48> {
    let max_iterations = 20;
    let mut current = start;
    for _ in 0..max_iterations {
        let value = fun(current)?;
        if value <= target {
            return Ok(current);
        }
        current = current.max(I80F48::ONE) * I80F48::from(2);
    }
    Err(error_msg!(
        "could not find amount that lead to health ratio <= 0"
    ))
}

fn binary_search(
    mut left: I80F48,
    left_value: I80F48,
    mut right: I80F48,
    target_value: I80F48,
    min_step: I80F48,
    fun: impl Fn(I80F48) -> Result<I80F48>,
) -> Result<I80F48> {
    let max_iterations = 50;
    let target_error = I80F48::from_num(0.1);
    let right_value = fun(right)?;
    require_msg!(
        (left_value <= target_value && right_value >= target_value)
            || (left_value >= target_value && right_value <= target_value),
        "internal error: left {} and right {} don't contain the target value {}",
        left_value,
        right_value,
        target_value
    );
    for _ in 0..max_iterations {
        if (right - left).abs() < min_step {
            return Ok(left);
        }
        let new = I80F48::from_num(0.5) * (left + right);
        let new_value = fun(new)?;
        let error = new_value.saturating_sub(target_value);
        if error > 0 && error < target_error {
            return Ok(new);
        }

        if (new_value > target_value) ^ (right_value > target_value) {
            left = new;
        } else {
            right = new;
        }
    }
    Err(error_msg!("binary search iterations exhausted"))
}

                                                                                                       
                                                                                                  
                                                           
   
                                                                             
fn find_maximum(
    mut left: I80F48,
    mut right: I80F48,
    min_step: I80F48,
    fun: impl Fn(I80F48) -> Result<I80F48>,
) -> Result<(I80F48, I80F48)> {
    assert!(right >= left);
    let half = I80F48::from_num(0.5);
    let mut mid = half * (left + right);
    let mut left_value = fun(left)?;
    let mut right_value = fun(right)?;
    let mut mid_value = fun(mid)?;
    while (right - left) > min_step {
                                                                                       
        if left_value > mid_value {
                                               
            assert!(mid_value >= right_value);
            right = mid;
            right_value = mid_value;
            mid = half * (left + mid);
            mid_value = fun(mid)?
        } else if mid_value <= right_value {
                                                
            assert!(left_value <= mid_value);
            left = mid;
            left_value = mid_value;
            mid = half * (mid + right);
            mid_value = fun(mid)?;
        } else {
                                                                                  
            let leftmid = half * (left + mid);
            let leftmid_value = fun(leftmid)?;
                                                       
            assert!(leftmid_value >= left_value);
            if leftmid_value > mid_value {
                                           
                right = mid;
                right_value = mid_value;
                mid = leftmid;
                mid_value = leftmid_value;
                continue;
            }

            let rightmid = half * (mid + right);
            let rightmid_value = fun(rightmid)?;
                                                         
            assert!(rightmid_value >= right_value);
            if rightmid_value >= mid_value {
                                            
                left = mid;
                left_value = mid_value;
                mid = rightmid;
                mid_value = rightmid_value;
                continue;
            }

                                               
            left = leftmid;
            left_value = leftmid_value;
            right = rightmid;
            right_value = rightmid_value;
        }
    }

    if left_value > mid_value {
        Ok((left, left_value))
    } else if mid_value > right_value {
        Ok((mid, mid_value))
    } else {
        Ok((right, right_value))
    }
}

fn ignore_net_borrow_limit_errors(maybe_cache: Result<HealthCache>) -> Result<Option<HealthCache>> {
                                                                        
                                                                           
    if maybe_cache.is_anchor_error_with_code(FermiError::BankNetBorrowsLimitReached.error_code()) {
        return Ok(None);
    }
    maybe_cache.map(|c| Some(c))
}

fn system_epoch_secs() -> u64 {
    use std::time::SystemTime;
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .expect("system time after epoch start")
        .as_secs()
}

#[cfg(all(test, feature = "legacy-unit-tests"))]
mod tests {
    use super::super::test::*;
    use super::*;
    use crate::state::*;
    use serum_dex::state::OpenOrders;

    fn health_eq(a: I80F48, b: f64) -> bool {
        if (a - I80F48::from_num(b)).abs() < 0.001 {
            true
        } else {
            println!("health is {}, but expected {}", a, b);
            false
        }
    }

    fn leverage_eq(h: &HealthCache, b: f64) -> bool {
        let a = h.leverage();
        if (a - I80F48::from_num(b)).abs() < 0.001 {
            true
        } else {
            println!("leverage is {}, but expected {}", a, b);
            false
        }
    }

    fn default_token_info(x: f64, price: f64) -> TokenInfo {
        TokenInfo {
            token_index: 0,
            maint_asset_weight: I80F48::from_num(1.0 - x),
            init_asset_weight: I80F48::from_num(1.0 - x),
            init_scaled_asset_weight: I80F48::from_num(1.0 - x),
            maint_liab_weight: I80F48::from_num(1.0 + x),
            init_liab_weight: I80F48::from_num(1.0 + x),
            init_scaled_liab_weight: I80F48::from_num(1.0 + x),
            prices: Prices::new_single_price(I80F48::from_num(price)),
            balance_spot: I80F48::ZERO,
            allow_asset_liquidation: true,
        }
    }

    fn default_perp_info(x: f64, price: f64) -> PerpInfo {
        PerpInfo {
            perp_market_index: 0,
            settle_token_index: 0,
            maint_base_asset_weight: I80F48::from_num(1.0 - x),
            init_base_asset_weight: I80F48::from_num(1.0 - x),
            maint_base_liab_weight: I80F48::from_num(1.0 + x),
            init_base_liab_weight: I80F48::from_num(1.0 + x),
            maint_overall_asset_weight: I80F48::from_num(0.6),
            init_overall_asset_weight: I80F48::from_num(0.6),
            base_lot_size: 1,
            base_lots: 0,
            bids_base_lots: 0,
            asks_base_lots: 0,
            quote: I80F48::ZERO,
            base_prices: Prices::new_single_price(I80F48::from_num(price)),
            has_open_orders: false,
            has_open_fills: false,
        }
    }

    #[test]
    fn test_max_swap() {
        let buffer = FermiAccount::default_for_tests().try_to_vec().unwrap();
        let mut account = FermiAccountValue::from_bytes(&buffer).unwrap();
        account.ensure_token_position(0).unwrap();
        account.ensure_token_position(1).unwrap();
        account.ensure_token_position(2).unwrap();

        let group = Pubkey::new_unique();
        let (mut bank0, _) = mock_bank_and_oracle(group, 0, 1.0, 0.1, 0.1);
        let (mut bank1, _) = mock_bank_and_oracle(group, 1, 5.0, 0.2, 0.2);
        let (mut bank2, _) = mock_bank_and_oracle(group, 2, 5.0, 0.3, 0.3);
        let banks = [
            bank0.data().clone(),
            bank1.data().clone(),
            bank2.data().clone(),
        ];

        let health_cache = HealthCache {
            token_infos: vec![
                TokenInfo {
                    token_index: 0,
                    ..default_token_info(0.1, 2.0)
                },
                TokenInfo {
                    token_index: 1,
                    ..default_token_info(0.2, 3.0)
                },
                TokenInfo {
                    token_index: 2,
                    ..default_token_info(0.3, 4.0)
                },
            ],
            spot_infos: vec![],
            perp_infos: vec![],
            being_liquidated: false,
        };

        assert_eq!(health_cache.health(HealthType::Init), I80F48::ZERO);
        assert_eq!(health_cache.health_ratio(HealthType::Init), I80F48::MAX);
        assert_eq!(
            health_cache
                .max_swap_source_for_health_ratio_with_limits(
                    &account,
                    &banks[0],
                    I80F48::from(1),
                    &banks[1],
                    I80F48::from_num(2.0 / 3.0),
                    I80F48::from_num(50.0)
                )
                .unwrap(),
            I80F48::ZERO
        );

        type MaxSwapFn = fn(&HealthCache) -> I80F48;

        let adjust_by_usdc = |c: &mut HealthCache, ti: TokenIndex, usdc: f64| {
            let ti = &mut c.token_infos[ti as usize];
            ti.balance_spot += I80F48::from_num(usdc) / ti.prices.oracle;
        };
        let find_max_swap_actual = |c: &HealthCache,
                                    source: TokenIndex,
                                    target: TokenIndex,
                                    min_value: f64,
                                    price_factor: f64,
                                    banks: [Bank; 3],
                                    max_swap_fn: MaxSwapFn| {
            let source_ti = &c.token_infos[source as usize];
            let source_price = &source_ti.prices;
            let mut source_bank = banks[source as usize].clone();
                                                                                  
                                               
            source_bank.init_asset_weight = source_ti.init_asset_weight;
            source_bank.init_liab_weight = source_ti.init_liab_weight;

            let target_ti = &c.token_infos[target as usize];
            let target_price = &target_ti.prices;
            let mut target_bank = banks[target as usize].clone();
            target_bank.init_asset_weight = target_ti.init_asset_weight;
            target_bank.init_liab_weight = target_ti.init_liab_weight;

            let swap_price =
                I80F48::from_num(price_factor) * source_price.oracle / target_price.oracle;
            let source_unlimited = c
                .max_swap_source_for_health_fn(
                    &account,
                    &source_bank,
                    source_price.oracle,
                    &target_bank,
                    swap_price,
                    I80F48::from_num(min_value),
                    max_swap_fn,
                )
                .unwrap();
            let source_amount = HealthCache::apply_limits_to_swap(
                &account,
                &source_bank,
                source_price.oracle,
                &target_bank,
                swap_price,
                source_unlimited,
            )
            .unwrap();
            if source_amount == I80F48::MAX {
                return (f64::MAX, f64::MAX, f64::MAX, f64::MAX);
            }
            let value_for_amount = |amount| {
                c.cache_after_swap(
                    &account,
                    &source_bank,
                    source_price.oracle,
                    &target_bank,
                    I80F48::from(amount),
                    swap_price,
                )
                .map(|c| max_swap_fn(&c).to_num::<f64>())
                .unwrap_or(f64::MIN)
            };
            (
                source_amount.to_num(),
                value_for_amount(source_amount),
                value_for_amount(source_amount - I80F48::ONE),
                value_for_amount(source_amount + I80F48::ONE),
            )
        };
        let check_max_swap_result = |c: &HealthCache,
                                     source: TokenIndex,
                                     target: TokenIndex,
                                     min_value: f64,
                                     price_factor: f64,
                                     banks: [Bank; 3],
                                     max_swap_fn: MaxSwapFn| {
            let (source_amount, actual_value, minus_value, plus_value) = find_max_swap_actual(
                c,
                source,
                target,
                min_value,
                price_factor,
                banks,
                max_swap_fn,
            );
            println!(
                    "checking {source} to {target} for price_factor: {price_factor}, target {min_value}: actual: {minus_value}/{actual_value}/{plus_value}, amount: {source_amount}",
                );
            if actual_value < min_value {
                                                                     
                assert!(plus_value < actual_value);
            } else {
                assert!(actual_value >= min_value);
                                                                                        
                                            
                assert!(actual_value < min_value + 1.0 || plus_value < min_value);
            }
        };

        let health_fn: Box<MaxSwapFn> = Box::new(|c: &HealthCache| c.health(HealthType::Init));
        let health_ratio_fn: Box<MaxSwapFn> =
            Box::new(|c: &HealthCache| c.health_ratio(HealthType::Init));

        for (test_name, max_swap_fn) in [("health", health_fn), ("health_ratio", health_ratio_fn)] {
            let check = |c: &HealthCache,
                         source: TokenIndex,
                         target: TokenIndex,
                         min_value: f64,
                         price_factor: f64,
                         banks: [Bank; 3]| {
                check_max_swap_result(
                    c,
                    source,
                    target,
                    min_value,
                    price_factor,
                    banks,
                    *max_swap_fn,
                )
            };

            let find_max_swap = |c: &HealthCache,
                                 source: TokenIndex,
                                 target: TokenIndex,
                                 min_value: f64,
                                 price_factor: f64,
                                 banks: [Bank; 3]| {
                find_max_swap_actual(
                    c,
                    source,
                    target,
                    min_value,
                    price_factor,
                    banks,
                    *max_swap_fn,
                )
            };

            {
                println!("test 0 {test_name}");
                let mut health_cache = health_cache.clone();
                adjust_by_usdc(&mut health_cache, 1, 100.0);

                for price_factor in [0.1, 0.9, 1.1] {
                    for target in 1..100 {
                        let target = target as f64;
                        check(&health_cache, 0, 1, target, price_factor, banks);
                        check(&health_cache, 1, 0, target, price_factor, banks);
                        check(&health_cache, 0, 2, target, price_factor, banks);
                    }
                }

                                                                         
                assert!(find_max_swap(&health_cache, 0, 1, 50.0, 1.5, banks).0 > 1e16);
            }

            {
                println!("test 1 {test_name}");
                let mut health_cache = health_cache.clone();
                adjust_by_usdc(&mut health_cache, 0, -20.0);
                adjust_by_usdc(&mut health_cache, 1, 100.0);

                for price_factor in [0.1, 0.9, 1.1] {
                    for target in 1..100 {
                        let target = target as f64;
                        check(&health_cache, 0, 1, target, price_factor, banks);
                        check(&health_cache, 1, 0, target, price_factor, banks);
                        check(&health_cache, 0, 2, target, price_factor, banks);
                        check(&health_cache, 2, 0, target, price_factor, banks);
                    }
                }
            }

            {
                println!("test 2 {test_name}");
                let mut health_cache = health_cache.clone();
                adjust_by_usdc(&mut health_cache, 0, -50.0);
                adjust_by_usdc(&mut health_cache, 1, 100.0);
                                                              
                check(&health_cache, 1, 0, 100.0, 1.0, banks);
            }

            {
                println!("test 3 {test_name}");
                let mut health_cache = health_cache.clone();
                adjust_by_usdc(&mut health_cache, 0, -30.0);
                adjust_by_usdc(&mut health_cache, 1, 100.0);
                adjust_by_usdc(&mut health_cache, 2, -30.0);

                                                                           
                                                                                                     
                let init_ratio = health_cache.health_ratio(HealthType::Init);
                let (amount, actual_ratio, _, _) =
                    find_max_swap(&health_cache, 1, 0, 100.0, 1.0, banks);
                println!(
                    "init {}, after {}, amount {}",
                    init_ratio, actual_ratio, amount
                );
                assert!(actual_ratio / 2.0 > init_ratio);
                assert!((amount as f64 - 100.0 / 3.0).abs() < 1.0);
            }

            {
                println!("test 4 {test_name}");
                let mut health_cache = health_cache.clone();
                adjust_by_usdc(&mut health_cache, 0, 100.0);
                adjust_by_usdc(&mut health_cache, 1, -2.0);
                adjust_by_usdc(&mut health_cache, 2, -65.0);

                let init_ratio = health_cache.health_ratio(HealthType::Init);
                assert!(init_ratio > 3 && init_ratio < 4);

                check(&health_cache, 0, 1, 1.0, 1.0, banks);
                check(&health_cache, 0, 1, 3.0, 1.0, banks);
                check(&health_cache, 0, 1, 4.0, 1.0, banks);
            }

            {
                                               
                println!("test 5 {test_name}");
                let mut health_cache = health_cache.clone();
                adjust_by_usdc(&mut health_cache, 1, 100.0);
                let mut banks = banks.clone();
                banks[0].net_borrow_limit_per_window_quote = 50;

                                                                                
                                                      
                assert!(find_max_swap(&health_cache, 0, 1, 1.0, 1.0, banks).0 < 51.0);
            }

            {
                                           
                println!("test 6 {test_name}");
                let mut health_cache = health_cache.clone();
                health_cache.spot_infos = vec![SpotInfo {
                    base_info_index: 1,
                    quote_info_index: 0,
                    spot_market_index: SpotMarketIndex::Serum3(0),
                    reserved_base: I80F48::from(30 / 3),
                    reserved_quote: I80F48::from(30 / 2),
                    reserved_base_as_quote_lowest_ask: I80F48::ZERO,
                    reserved_quote_as_base_highest_bid: I80F48::ZERO,
                    has_zero_funds: false,
                }];
                adjust_by_usdc(&mut health_cache, 0, -20.0);
                adjust_by_usdc(&mut health_cache, 1, -40.0);
                adjust_by_usdc(&mut health_cache, 2, 120.0);

                for price_factor in [0.9, 1.1] {
                    for target in 1..100 {
                        let target = target as f64;
                        check(&health_cache, 0, 1, target, price_factor, banks);
                        check(&health_cache, 1, 0, target, price_factor, banks);
                        check(&health_cache, 0, 2, target, price_factor, banks);
                        check(&health_cache, 1, 2, target, price_factor, banks);
                        check(&health_cache, 2, 0, target, price_factor, banks);
                        check(&health_cache, 2, 1, target, price_factor, banks);
                    }
                }
            }

            {
                                                                                        
                println!("test 7 {test_name}");
                let mut health_cache = health_cache.clone();
                adjust_by_usdc(&mut health_cache, 0, -20.0);
                adjust_by_usdc(&mut health_cache, 1, 20.0);
                assert!(health_cache.health(HealthType::Init) < 0);

                if test_name == "health" {
                    assert!(find_max_swap(&health_cache, 1, 0, 1.0, 1.0, banks).0 > 0.0);
                }

                for price_factor in [0.9, 1.1] {
                    for target in 1..100 {
                        let target = target as f64;
                        check(&health_cache, 1, 0, target, price_factor, banks);
                    }
                }
            }

            {
                                                                                          
                println!("test 8 {test_name}");
                let mut health_cache = health_cache.clone();
                adjust_by_usdc(&mut health_cache, 0, -20.0);
                adjust_by_usdc(&mut health_cache, 1, 10.0);
                assert!(health_cache.health(HealthType::Init) < 0);

                if test_name == "health" {
                    assert!(find_max_swap(&health_cache, 1, 0, 1.0, 1.0, banks).0 > 0.0);
                }

                for price_factor in [0.9, 1.1] {
                    for target in 1..100 {
                        let target = target as f64;
                        check(&health_cache, 1, 0, target, price_factor, banks);
                    }
                }
            }

            {
                                                                  
                println!("test 9 {test_name}");
                let mut health_cache = health_cache.clone();
                adjust_by_usdc(&mut health_cache, 0, 10.0);
                health_cache.token_infos[1].init_asset_weight = I80F48::from(0);

                assert!(find_max_swap(&health_cache, 0, 1, 1.0, 1.0, banks).0 > 0.0);

                for price_factor in [0.9, 1.1] {
                    for target in 1..100 {
                        let target = target as f64;
                        check(&health_cache, 0, 1, target, price_factor, banks);
                    }
                }
            }

            {
                                                         
                println!("test 10 {test_name}");
                let mut health_cache = health_cache.clone();
                health_cache.perp_infos.push(PerpInfo {
                    perp_market_index: 0,
                    settle_token_index: 1,
                    ..default_perp_info(0.3, 2.0)
                });
                adjust_by_usdc(&mut health_cache, 0, 60.0);

                for perp_quote in [-10, 10] {
                    health_cache.perp_infos[0].quote = I80F48::from_num(perp_quote);
                    for price_factor in [0.9, 1.1] {
                        for target in 1..100 {
                            let target = target as f64;
                            check(&health_cache, 0, 1, target, price_factor, banks);
                            check(&health_cache, 1, 0, target, price_factor, banks);
                        }
                    }
                }
            }

            {
                                                                    
                println!("test 11 {test_name}");
                let mut health_cache = health_cache.clone();
                adjust_by_usdc(&mut health_cache, 0, 10.0);            
                health_cache.token_infos[0].init_asset_weight = I80F48::from(0);
                health_cache.token_infos[1].init_asset_weight = I80F48::from(0);

                let amount = find_max_swap(&health_cache, 0, 1, 1.0, 1.0, banks).0;
                assert_eq!(amount, 5.0);

                for price_factor in [0.9, 1.1] {
                    for target in 1..100 {
                        let target = target as f64;

                                                                       
                        let amount =
                            find_max_swap(&health_cache, 0, 1, target, price_factor, banks).0;
                        assert_eq!(amount, 5.0);
                    }
                }

                adjust_by_usdc(&mut health_cache, 1, 6.0);            

                for price_factor in [0.9, 1.1] {
                    for target in 1..100 {
                        let target = target as f64;

                                                                       
                        let amount =
                            find_max_swap(&health_cache, 0, 1, target, price_factor, banks).0;
                        assert_eq!(amount, 5.0);
                    }
                }
            }
        }
    }

    #[test]
    fn test_max_perp() {
        let base_lot_size = 100;

        let health_cache = HealthCache {
            token_infos: vec![
                TokenInfo {
                    token_index: 0,
                    balance_spot: I80F48::ZERO,
                    ..default_token_info(0.0, 1.0)
                },
                TokenInfo {
                    token_index: 1,
                    balance_spot: I80F48::ZERO,
                    ..default_token_info(0.2, 1.5)
                },
            ],
            spot_infos: vec![],
            perp_infos: vec![PerpInfo {
                perp_market_index: 0,
                settle_token_index: 1,
                base_lot_size,
                ..default_perp_info(0.3, 2.0)
            }],
            being_liquidated: false,
        };

        assert_eq!(health_cache.health(HealthType::Init), I80F48::ZERO);
        assert_eq!(health_cache.health_ratio(HealthType::Init), I80F48::MAX);
        assert_eq!(
            health_cache
                .max_perp_for_health_ratio(
                    0,
                    I80F48::from(2),
                    PerpOrderSide::Bid,
                    I80F48::from_num(50.0)
                )
                .unwrap(),
            I80F48::ZERO
        );

        let adjust_token = |c: &mut HealthCache, info_index: usize, value: f64| {
            let ti = &mut c.token_infos[info_index];
            ti.balance_spot += I80F48::from_num(value);
        };
        let find_max_trade =
            |c: &HealthCache, side: PerpOrderSide, ratio: f64, price_factor: f64| {
                let prices = &c.perp_infos[0].base_prices;
                let trade_price = I80F48::from_num(price_factor) * prices.oracle;
                let base_lots = c
                    .max_perp_for_health_ratio(0, trade_price, side, I80F48::from_num(ratio))
                    .unwrap();
                if base_lots == i64::MAX {
                    return (i64::MAX, f64::MAX, f64::MAX);
                }

                let direction = match side {
                    PerpOrderSide::Bid => 1,
                    PerpOrderSide::Ask => -1,
                };

                                                                             
                let actual_ratio = {
                    let base_lots = direction * base_lots;
                    let base_native = I80F48::from(base_lots * base_lot_size);
                    let mut c = c.clone();
                    c.perp_infos[0].base_lots += base_lots;
                    c.perp_infos[0].quote -= base_native * trade_price;
                    c.health_ratio(HealthType::Init).to_num::<f64>()
                };
                                                                
                let plus_ratio = {
                    let base_lots = direction * (base_lots + 1);
                    let base_native = I80F48::from(base_lots * base_lot_size);
                    let mut c = c.clone();
                    c.perp_infos[0].base_lots += base_lots;
                    c.perp_infos[0].quote -= base_native * trade_price;
                    c.health_ratio(HealthType::Init).to_num::<f64>()
                };
                (base_lots, actual_ratio, plus_ratio)
            };
        let check_max_trade = |c: &HealthCache,
                               side: PerpOrderSide,
                               ratio: f64,
                               price_factor: f64| {
            let (base_lots, actual_ratio, plus_ratio) =
                find_max_trade(c, side, ratio, price_factor);
            println!(
                    "checking for price_factor: {price_factor}, target ratio {ratio}: actual ratio: {actual_ratio}, plus ratio: {plus_ratio}, base_lots: {base_lots}",
                );
            let max_binary_search_error = 0.1;
            assert!(ratio <= actual_ratio);
            assert!(plus_ratio - max_binary_search_error <= ratio);
        };

        {
            let mut health_cache = health_cache.clone();
            adjust_token(&mut health_cache, 0, 3000.0);

            for existing_settle in [-500.0, 0.0, 300.0] {
                let mut c = health_cache.clone();
                adjust_token(&mut c, 1, existing_settle);
                for existing_lots in [-5, 0, 3] {
                    let mut c = c.clone();
                    c.perp_infos[0].base_lots += existing_lots;
                    c.perp_infos[0].quote -= I80F48::from(existing_lots * base_lot_size * 2);

                    for side in [PerpOrderSide::Bid, PerpOrderSide::Ask] {
                        println!(
                            "test 0: lots {existing_lots}, settle {existing_settle}, side {side:?}"
                        );
                        for price_factor in [0.8, 1.0, 1.1] {
                            for ratio in 1..=100 {
                                check_max_trade(&c, side, ratio as f64, price_factor);
                            }
                        }
                    }
                }
            }

                                              
            check_max_trade(&health_cache, PerpOrderSide::Bid, 50.0, 2.0);
            check_max_trade(&health_cache, PerpOrderSide::Ask, 50.0, 0.1);

                                        
            assert_eq!(
                find_max_trade(&health_cache, PerpOrderSide::Bid, 50.0, 0.1).0,
                i64::MAX
            );
            assert_eq!(
                find_max_trade(&health_cache, PerpOrderSide::Ask, 50.0, 1.5).0,
                i64::MAX
            );
        }
    }

    #[test]
    fn test_health_perp_funding() {
        let buffer = FermiAccount::default_for_tests().try_to_vec().unwrap();
        let mut account = FermiAccountValue::from_bytes(&buffer).unwrap();

        let group = Pubkey::new_unique();

        let (mut bank1, mut oracle1) = mock_bank_and_oracle(group, 0, 1.0, 0.2, 0.1);
        bank1
            .data()
            .change_without_fee(
                account.ensure_token_position(0).unwrap().0,
                I80F48::from(100),
                DUMMY_NOW_TS,
            )
            .unwrap();

        let mut perp1 = mock_perp_market(group, oracle1.pubkey, 1.0, 9, (0.2, 0.1), (0.05, 0.02));
        perp1.data().long_funding = I80F48::from_num(10.1);
        let perpaccount = account.ensure_perp_position(9, 0).unwrap().0;
        perpaccount.record_trade(perp1.data(), 10, I80F48::from(-110));
        perpaccount.long_settled_funding = I80F48::from_num(10.0);

        let oracle1_ai = oracle1.as_account_info();
        let ais = vec![
            bank1.as_account_info(),
            oracle1_ai.clone(),
            perp1.as_account_info(),
            oracle1_ai,
        ];

        let retriever = ScanningAccountRetriever::new_with_staleness(&ais, &group, None).unwrap();

        assert!(health_eq(
            compute_health(&account.borrow(), HealthType::Init, &retriever, 0).unwrap(),
                    
            0.8 * (100.0
                        
            + 0.8 * 100.0
                         
            - 110.0
                                                
            - 1.0)
        ));
    }

    #[test]
    fn test_scanning_retreiver_mismatched_oracle_for_perps_throws_error() {
        let group = Pubkey::new_unique();

        let (mut bank1, mut oracle1) = mock_bank_and_oracle(group, 1, 1.0, 0.2, 0.1);
        let (mut bank2, mut oracle2) = mock_bank_and_oracle(group, 4, 5.0, 0.5, 0.3);

        let mut oo1 = TestAccount::<OpenOrders>::new_zeroed();

        let mut perp1 = mock_perp_market(group, oracle1.pubkey, 1.0, 9, (0.2, 0.1), (0.05, 0.02));
        let mut perp2 = mock_perp_market(group, oracle2.pubkey, 5.0, 8, (0.2, 0.1), (0.05, 0.02));

        let oracle1_account_info = oracle1.as_account_info();
        let oracle2_account_info = oracle2.as_account_info();
        let ais = vec![
            bank1.as_account_info(),
            bank2.as_account_info(),
            oracle1_account_info.clone(),
            oracle2_account_info.clone(),
            perp1.as_account_info(),
            perp2.as_account_info(),
            oracle2_account_info,                            
            oracle1_account_info,
            oo1.as_account_info(),
        ];

        let retriever = ScanningAccountRetriever::new_with_staleness(&ais, &group, None).unwrap();
        let result = retriever.perp_market_and_oracle_price(&group, 0, 9);
        assert!(result.is_err());
    }

    #[test]
    fn test_health_stable_price_token() {
        let buffer = FermiAccount::default_for_tests().try_to_vec().unwrap();
        let mut account = FermiAccountValue::from_bytes(&buffer).unwrap();
        let buffer2 = FermiAccount::default_for_tests().try_to_vec().unwrap();
        let mut account2 = FermiAccountValue::from_bytes(&buffer2).unwrap();
        let buffer3 = FermiAccount::default_for_tests().try_to_vec().unwrap();
        let mut account3 = FermiAccountValue::from_bytes(&buffer3).unwrap();

        let group = Pubkey::new_unique();

        let (mut bank1, mut oracle1) = mock_bank_and_oracle(group, 0, 1.0, 0.2, 0.1);
        bank1.data().stable_price_model.stable_price = 0.5;
        bank1
            .data()
            .change_without_fee(
                account.ensure_token_position(0).unwrap().0,
                I80F48::from(100),
                DUMMY_NOW_TS,
            )
            .unwrap();
        bank1
            .data()
            .change_without_fee(
                account2.ensure_token_position(0).unwrap().0,
                I80F48::from(-100),
                DUMMY_NOW_TS,
            )
            .unwrap();

        let mut perp1 = mock_perp_market(group, oracle1.pubkey, 1.0, 9, (0.2, 0.1), (0.05, 0.02));
        perp1.data().stable_price_model.stable_price = 0.5;
        let perpaccount = account3.ensure_perp_position(9, 0).unwrap().0;
        perpaccount.record_trade(perp1.data(), 10, I80F48::from(-100));

        let oracle1_ai = oracle1.as_account_info();
        let ais = vec![
            bank1.as_account_info(),
            oracle1_ai.clone(),
            perp1.as_account_info(),
            oracle1_ai,
        ];

        let retriever = ScanningAccountRetriever::new_with_staleness(&ais, &group, None).unwrap();

        assert!(health_eq(
            compute_health(&account.borrow(), HealthType::Init, &retriever, 0).unwrap(),
            0.8 * 0.5 * 100.0
        ));
        assert!(health_eq(
            compute_health(&account.borrow(), HealthType::Maint, &retriever, 0).unwrap(),
            0.9 * 1.0 * 100.0
        ));
        assert!(health_eq(
            compute_health(&account2.borrow(), HealthType::Init, &retriever, 0).unwrap(),
            -1.2 * 1.0 * 100.0
        ));
        assert!(health_eq(
            compute_health(&account2.borrow(), HealthType::Maint, &retriever, 0).unwrap(),
            -1.1 * 1.0 * 100.0
        ));
        assert!(health_eq(
            compute_health(&account3.borrow(), HealthType::Init, &retriever, 0).unwrap(),
            1.2 * (0.8 * 0.5 * 10.0 * 10.0 - 100.0)
        ));
        assert!(health_eq(
            compute_health(&account3.borrow(), HealthType::Maint, &retriever, 0).unwrap(),
            1.1 * (0.9 * 1.0 * 10.0 * 10.0 - 100.0)
        ));
    }

    #[test]
    fn test_max_borrow() {
        let buffer = FermiAccount::default_for_tests().try_to_vec().unwrap();
        let mut account = FermiAccountValue::from_bytes(&buffer).unwrap();
        account.ensure_token_position(0).unwrap();
        account.ensure_token_position(1).unwrap();

        let group = Pubkey::new_unique();
        let (mut bank0, _) = mock_bank_and_oracle(group, 0, 1.0, 0.0, 0.0);
        let bank0_data = bank0.data();

        let health_cache = HealthCache {
            token_infos: vec![
                TokenInfo {
                    token_index: 0,
                    ..default_token_info(0.0, 1.0)
                },
                TokenInfo {
                    token_index: 1,
                    ..default_token_info(0.2, 2.0)
                },
            ],
            spot_infos: vec![],
            perp_infos: vec![],
            being_liquidated: false,
        };

        assert_eq!(health_cache.health(HealthType::Init), I80F48::ZERO);
        assert_eq!(health_cache.health_ratio(HealthType::Init), I80F48::MAX);
        assert_eq!(
            health_cache
                .max_borrow_for_health_ratio(&account, bank0_data, I80F48::from(50))
                .unwrap(),
            I80F48::ZERO
        );

        let now_ts = system_epoch_secs();

        let cache_after_borrow = |account: &FermiAccountValue,
                                  c: &HealthCache,
                                  bank: &Bank,
                                  amount: I80F48|
         -> Result<HealthCache> {
            let mut position = account.token_position(bank.token_index)?.clone();

            let mut bank = bank.clone();
            bank.withdraw_with_fee(&mut position, amount, now_ts)?;
            bank.check_net_borrows(c.token_info(bank.token_index)?.prices.oracle)?;

            let mut resulting_cache = c.clone();
            resulting_cache.adjust_token_balance(&bank, -amount)?;

            Ok(resulting_cache)
        };

        let find_max_borrow =
            |account: &FermiAccountValue, c: &HealthCache, ratio: f64, bank: &Bank| {
                let max_borrow = c
                    .max_borrow_for_health_ratio(account, bank, I80F48::from_num(ratio))
                    .unwrap();
                                                                             
                let actual_ratio = {
                    let c = cache_after_borrow(account, c, bank, max_borrow).unwrap();
                    c.health_ratio(HealthType::Init).to_num::<f64>()
                };
                                                                 
                let plus_ratio = {
                    let c = cache_after_borrow(account, c, bank, max_borrow + I80F48::ONE).unwrap();
                    c.health_ratio(HealthType::Init).to_num::<f64>()
                };
                (max_borrow, actual_ratio, plus_ratio)
            };
        let check_max_borrow = |account: &FermiAccountValue,
                                c: &HealthCache,
                                ratio: f64,
                                bank: &Bank|
         -> f64 {
            let initial_ratio = c.health_ratio(HealthType::Init).to_num::<f64>();
            let (max_borrow, actual_ratio, plus_ratio) = find_max_borrow(account, c, ratio, bank);
            println!(
                    "checking target ratio {ratio}: initial ratio: {initial_ratio}, actual ratio: {actual_ratio}, plus ratio: {plus_ratio}, borrow: {max_borrow}",
                );
            let max_binary_search_error = 0.1;
            if initial_ratio >= ratio {
                assert!(ratio <= actual_ratio);
                assert!(plus_ratio - max_binary_search_error <= ratio);
            }
            max_borrow.to_num::<f64>()
        };

        {
            let mut health_cache = health_cache.clone();
            health_cache.token_infos[0].balance_spot = I80F48::from_num(100.0);
            assert_eq!(
                check_max_borrow(&account, &health_cache, 50.0, bank0_data),
                100.0
            );
        }
        {
            let mut health_cache = health_cache.clone();
            health_cache.token_infos[1].balance_spot = I80F48::from_num(50.0);                                    
            check_max_borrow(&account, &health_cache, 100.0, bank0_data);
            check_max_borrow(&account, &health_cache, 50.0, bank0_data);
            check_max_borrow(&account, &health_cache, 0.0, bank0_data);
        }
        {
            let mut health_cache = health_cache.clone();
            health_cache.token_infos[0].balance_spot = I80F48::from_num(50.0);
            health_cache.token_infos[1].balance_spot = I80F48::from_num(50.0);
            check_max_borrow(&account, &health_cache, 100.0, bank0_data);
            check_max_borrow(&account, &health_cache, 50.0, bank0_data);
            check_max_borrow(&account, &health_cache, 0.0, bank0_data);
        }
        {
            let mut health_cache = health_cache.clone();
            health_cache.token_infos[0].balance_spot = I80F48::from_num(-50.0);
            health_cache.token_infos[1].balance_spot = I80F48::from_num(50.0);
            check_max_borrow(&account, &health_cache, 100.0, bank0_data);
            check_max_borrow(&account, &health_cache, 50.0, bank0_data);
            check_max_borrow(&account, &health_cache, 0.0, bank0_data);
        }

                                                   
        {
            let mut account = account.clone();
            let mut bank0 = bank0_data.clone();
            let mut health_cache = health_cache.clone();
            let tok0_deposits = I80F48::from_num(500.0);
            health_cache.token_infos[0].balance_spot = tok0_deposits;
            health_cache.token_infos[1].balance_spot = I80F48::from_num(-100.0);                            

                                                                       
            let position = account.token_position_mut(bank0.token_index).unwrap().0;
            bank0.deposit(position, tok0_deposits, now_ts).unwrap();

                                                                                                            
            bank0.deposit_weight_scale_start_quote = 600.0;
            bank0.potential_serum_tokens = 300;
            health_cache.token_infos[0].init_scaled_asset_weight =
                bank0.scaled_init_asset_weight(I80F48::ONE);

            check_max_borrow(&account, &health_cache, 100.0, &bank0);
            check_max_borrow(&account, &health_cache, 50.0, &bank0);

            let max_borrow = check_max_borrow(&account, &health_cache, 0.0, &bank0);
                                                                                  
            assert!((260.0 - max_borrow).abs() < 0.3);

            bank0.deposit_weight_scale_start_quote = 500.0;
            let max_borrow = check_max_borrow(&account, &health_cache, 0.0, &bank0);
                                                             
                                                                               
            assert!((222.6 - max_borrow).abs() < 0.3);
        }
    }

    #[test]
    fn test_assets_and_borrows() {
        let health_cache = HealthCache {
            token_infos: vec![
                TokenInfo {
                    token_index: 0,
                    ..default_token_info(0.0, 1.0)
                },
                TokenInfo {
                    token_index: 1,
                    ..default_token_info(0.2, 2.0)
                },
            ],
            spot_infos: vec![],
            perp_infos: vec![PerpInfo {
                perp_market_index: 0,
                settle_token_index: 0,
                ..default_perp_info(0.3, 2.0)
            }],
            being_liquidated: false,
        };

        {
            let mut hc = health_cache.clone();
            hc.token_infos[1].balance_spot = I80F48::from(10);
            hc.perp_infos[0].quote = I80F48::from(-12);

            let (assets, liabs) = hc.health_assets_and_liabs_stable_assets(HealthType::Init);
            assert!((assets.to_num::<f64>() - 2.0 * 10.0 * 0.8) < 0.01);
            assert!((liabs.to_num::<f64>() - 2.0 * (10.0 * 0.8 + 2.0 * 1.2)) < 0.01);

            let (assets, liabs) = hc.health_assets_and_liabs_stable_liabs(HealthType::Init);
            assert!((liabs.to_num::<f64>() - 2.0 * 12.0 * 1.2) < 0.01);
            assert!((assets.to_num::<f64>() - 2.0 * 10.0 * 1.2) < 0.01);
        }

        {
            let mut hc = health_cache.clone();
            hc.token_infos[1].balance_spot = I80F48::from(12);
            hc.perp_infos[0].quote = I80F48::from(-10);

            let (assets, liabs) = hc.health_assets_and_liabs_stable_assets(HealthType::Init);
            assert!((assets.to_num::<f64>() - 2.0 * 12.0 * 0.8) < 0.01);
            assert!((liabs.to_num::<f64>() - 2.0 * 10.0 * 0.8) < 0.01);

            let (assets, liabs) = hc.health_assets_and_liabs_stable_liabs(HealthType::Init);
            assert!((liabs.to_num::<f64>() - 2.0 * 10.0 * 1.2) < 0.01);
            assert!((assets.to_num::<f64>() - 2.0 * (10.0 * 1.2 + 2.0 * 0.8)) < 0.01);
        }
    }

    #[test]
    fn test_leverage() {
                        
        let health_cache = HealthCache {
            token_infos: vec![
                TokenInfo {
                    token_index: 0,
                    balance_spot: I80F48::ONE,
                    ..default_token_info(0.0, 1.0)
                },
                TokenInfo {
                    token_index: 1,
                    ..default_token_info(0.2, 2.0)
                },
            ],
            spot_infos: vec![],
            perp_infos: vec![],
            being_liquidated: false,
        };
        assert!(leverage_eq(&health_cache, 0.0));

                                                        
        let health_cache = HealthCache {
            token_infos: vec![
                TokenInfo {
                    token_index: 0,
                    balance_spot: I80F48::from_num(-9),
                    ..default_token_info(0.0, 1.0)
                },
                TokenInfo {
                    token_index: 1,
                    balance_spot: I80F48::from_num(5),
                    ..default_token_info(0.2, 2.0)
                },
            ],
            spot_infos: vec![],
            perp_infos: vec![],
            being_liquidated: false,
        };

        assert!(leverage_eq(&health_cache, 9.0));

                                                   
        let health_cache = HealthCache {
            token_infos: vec![
                TokenInfo {
                    token_index: 0,
                    balance_spot: I80F48::ONE,
                    ..default_token_info(0.0, 1.0)
                },
                TokenInfo {
                    token_index: 1,
                    ..default_token_info(0.2, 2.0)
                },
            ],
            spot_infos: vec![],
            perp_infos: vec![PerpInfo {
                perp_market_index: 0,
                base_lot_size: 3,
                base_lots: -3,
                quote: I80F48::from_num(9.9),
                ..default_perp_info(0.1, 1.1)
            }],
            being_liquidated: false,
        };
        assert!(leverage_eq(&health_cache, 9.9));

                                              
        let health_cache = HealthCache {
            token_infos: vec![
                TokenInfo {
                    token_index: 0,
                    balance_spot: I80F48::ONE,
                    ..default_token_info(0.0, 1.0)
                },
                TokenInfo {
                    token_index: 1,
                    balance_spot: I80F48::from_num(-1),
                    ..default_token_info(0.2, 2.0)
                },
            ],
            spot_infos: vec![SpotInfo {
                reserved_base: I80F48::ONE,
                reserved_quote: I80F48::ZERO,
                reserved_base_as_quote_lowest_ask: I80F48::ONE,
                reserved_quote_as_base_highest_bid: I80F48::ZERO,
                base_info_index: 1,
                quote_info_index: 0,
                spot_market_index: SpotMarketIndex::Serum3(0),
                has_zero_funds: true,
            }],
            perp_infos: vec![],
            being_liquidated: false,
        };

        assert!(leverage_eq(&health_cache, 2.0));
    }
}
