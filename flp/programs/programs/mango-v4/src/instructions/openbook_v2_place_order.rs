use crate::accounts_zerocopy::AccountInfoRef;
use crate::error::*;
use crate::health::*;
use crate::i80f48::ClampToInt;
use crate::instructions::{apply_vault_difference, OODifference};
use crate::logs::{emit_stack, OpenbookV2OpenOrdersBalanceLog};
use crate::serum3_cpi::{OpenOrdersAmounts, OpenOrdersSlim};
use crate::state::*;
use crate::util::clock_now;
use anchor_lang::prelude::*;
use anchor_spl::token::accessor;
use fixed::types::I80F48;
use openbook_v2::cpi::Return;
use openbook_v2::state::Market;
use openbook_v2::state::OpenOrdersAccount;
use openbook_v2::state::{
    Order as OpenbookV2Order, PlaceOrderType as OpenbookV2OrderType, Side as OpenbookV2Side,
    MAX_OPEN_ORDERS,
};

use crate::accounts_ix::*;

pub fn openbook_v2_place_order<'info>(
    ctx: Context<'_, '_, '_, 'info, OpenbookV2PlaceOrder<'info>>,
    order: OpenbookV2Order,
    limit: u8,
) -> Result<()> {
    const EXTRA_CPI_ACCOUNTS: usize = 5;
    require_gte!(order.max_base_lots, 0);
    require_gte!(order.max_quote_lots_including_fees, 0);
    require_gte!(ctx.remaining_accounts.len(), EXTRA_CPI_ACCOUNTS);
    let (health_remaining, cpi_remaining) = ctx
        .remaining_accounts
        .split_at(ctx.remaining_accounts.len() - EXTRA_CPI_ACCOUNTS);
    let bids_ai = cpi_remaining[0].clone();
    let asks_ai = cpi_remaining[1].clone();
    let event_heap_ai = cpi_remaining[2].clone();
    let market_vault_ai = cpi_remaining[3].clone();
    let token_program_ai = cpi_remaining[4].clone();

    let group_loader = AccountLoader::<Group>::try_from(&ctx.accounts.group.to_account_info())?;
    let group = group_loader.load()?;
    require!(
        group.is_ix_enabled(IxGate::OpenbookV2PlaceOrder),
        FermiError::IxIsDisabled
    );
    require_keys_eq!(*token_program_ai.key, anchor_spl::token::ID);
    require!(ctx.accounts.authority.is_signer, FermiError::SomeError);

    let account_loader =
        AccountLoader::<FermiAccountFixed>::try_from(&ctx.accounts.account.to_account_info())?;
    let openbook_market_loader = AccountLoader::<OpenbookV2Market>::try_from(
        &ctx.accounts.openbook_v2_market.to_account_info(),
    )?;
    let openbook_market = openbook_market_loader.load()?;
    let open_orders_loader = AccountLoader::<openbook_v2::state::OpenOrdersAccount>::try_from(
        &ctx.accounts.open_orders.to_account_info(),
    )?;
    let openbook_market_external_loader = AccountLoader::<Market>::try_from(
        &ctx.accounts.openbook_v2_market_external.to_account_info(),
    )?;
    let payer_bank_loader =
        AccountLoader::<Bank>::try_from(&ctx.accounts.payer_bank.to_account_info())?;
    let receiver_bank_loader =
        AccountLoader::<Bank>::try_from(&ctx.accounts.receiver_bank.to_account_info())?;
    require_keys_eq!(openbook_market.group, ctx.accounts.group.key());
    require_keys_eq!(
        openbook_market.openbook_v2_market_external,
        ctx.accounts.openbook_v2_market_external.key()
    );
    require_keys_eq!(
        openbook_market.openbook_v2_program,
        ctx.accounts.openbook_v2_program.key()
    );
    require!(
        !openbook_market.is_reduce_only(),
        FermiError::MarketInReduceOnlyMode
    );

    let receiver_token_index = match order.side {
        OpenbookV2Side::Bid => openbook_market.base_token_index,
        OpenbookV2Side::Ask => openbook_market.quote_token_index,
    };
    let payer_token_index = match order.side {
        OpenbookV2Side::Bid => openbook_market.quote_token_index,
        OpenbookV2Side::Ask => openbook_market.base_token_index,
    };

      
                 
      
    {
        let account = account_loader.load_full()?;
        require_keys_eq!(account.fixed.group, ctx.accounts.group.key());
        require!(account.fixed.is_operational(), FermiError::AccountIsFrozen);
                                
        require!(
            account
                .fixed
                .is_owner_or_delegate(ctx.accounts.authority.key()),
            FermiError::SomeError
        );

                                  
        require!(
            account
                .openbook_v2_orders(openbook_market.market_index)?
                .open_orders
                == ctx.accounts.open_orders.key(),
            FermiError::SomeError
        );
    }
                                 
    let group_key = ctx.accounts.group.key();
    let mut account = account_loader.load_full_mut()?;
    let (now_ts, now_slot) = clock_now();
    let retriever = new_fixed_order_account_retriever_with_optional_banks(
        health_remaining,
        &account.borrow(),
        now_slot,
    )?;

    let (_, _, payer_active_index) = account.ensure_token_position(payer_token_index)?;
    let (_, _, receiver_active_index) = account.ensure_token_position(receiver_token_index)?;

                                                                                           
    let (payer_bank, payer_bank_oracle) =
        retriever.bank_and_oracle(&group_key, payer_active_index, payer_token_index)?;
    let (receiver_bank, receiver_bank_oracle) =
        retriever.bank_and_oracle(&group_key, receiver_active_index, receiver_token_index)?;

    require_keys_eq!(payer_bank_loader.load()?.group, group_key);
    require_keys_eq!(receiver_bank_loader.load()?.group, group_key);
    require_keys_eq!(payer_bank.vault, ctx.accounts.payer_vault.key());

                                     
    require_eq!(payer_bank_loader.load()?.token_index, payer_token_index);
    require_eq!(
        receiver_bank_loader.load()?.token_index,
        receiver_token_index
    );

      
                             
      
    let mut health_cache = new_health_cache_skipping_missing_banks_and_bad_oracles(
        &account.borrow(),
        &retriever,
        now_ts,
    )
    .context("pre init health")?;

                                                                             
    health_cache.token_info_index(payer_token_index)?;
    health_cache.token_info_index(receiver_token_index)?;

    let pre_health_opt = if !account.fixed.is_in_health_region() {
        let pre_init_health = account.check_health_pre(&health_cache)?;
        Some(pre_init_health)
    } else {
        None
    };

    drop(retriever);

                                                       

      
                            
      
    let base_lot_size: u64;
    let quote_lot_size: u64;
    {
        let openbook_market_external = openbook_market_external_loader.load()?;
        require_keys_eq!(openbook_market_external.bids, *bids_ai.key);
        require_keys_eq!(openbook_market_external.asks, *asks_ai.key);
        require_keys_eq!(openbook_market_external.event_heap, *event_heap_ai.key);
        require!(
            openbook_market_external.is_market_vault(*market_vault_ai.key),
            FermiError::SomeError
        );
        base_lot_size = openbook_market_external.base_lot_size.try_into().unwrap();
        quote_lot_size = openbook_market_external.quote_lot_size.try_into().unwrap();
    }

    let before_vault = accessor::amount(&ctx.accounts.payer_vault.to_account_info())?;
    let before_oo_free_slots;
    let before_had_bids;
    let before_had_asks;
    let before_oo = {
        let open_orders = open_orders_loader.load()?;
        before_oo_free_slots = MAX_OPEN_ORDERS - open_orders.all_orders_in_use().count();
        before_had_bids = open_orders.position.bids_base_lots != 0;
        before_had_asks = open_orders.position.asks_base_lots != 0;
        OpenOrdersSlim::from_oo_v2(&open_orders, base_lot_size, quote_lot_size)
    };

                                                                                    
    let max_base_lots: u64 = order.max_base_lots.try_into().unwrap();
    let max_quote_lots: u64 = order.max_quote_lots_including_fees.try_into().unwrap();

    let needed_amount = match order.side {
        OpenbookV2Side::Ask => {
            (max_base_lots * base_lot_size).saturating_sub(before_oo.native_base_free())
        }
        OpenbookV2Side::Bid => {
            (max_quote_lots * quote_lot_size).saturating_sub(before_oo.native_quote_free())
        }
    };
    if before_vault < needed_amount {
        return err!(FermiError::InsufficentBankVaultFunds).with_context(|| {
            format!(
                "bank vault does not have enough tokens, need {} but have {}",
                needed_amount, before_vault
            )
        });
    }

                                                   
    let price_lots;
    {
        let bids = AccountLoader::<openbook_v2::state::BookSide>::try_from(&bids_ai)?;
        let asks = AccountLoader::<openbook_v2::state::BookSide>::try_from(&asks_ai)?;
        let bids = bids.load_mut()?;
        let asks = asks.load_mut()?;
        let order_book = openbook_v2::state::Orderbook { bids, asks };
        price_lots = order.price(now_ts, None, &order_book)?.0;
    }

      
                         
      
    let group = group_loader.load()?;
    let group_seeds = group_seeds!(group);

    cpi_place_order(
        ctx.accounts,
        &[group_seeds],
        &order,
        price_lots,
        limit,
        &bids_ai,
        &asks_ai,
        &event_heap_ai,
        &market_vault_ai,
        &token_program_ai,
    )?;
      
                           
      
    let open_orders = open_orders_loader.load()?;
    let after_oo_free_slots = MAX_OPEN_ORDERS - open_orders.all_orders_in_use().count();
    let after_oo = OpenOrdersSlim::from_oo_v2(&open_orders, base_lot_size, quote_lot_size);
    let oo_difference = OODifference::new(&before_oo, &after_oo);

      
                                                                                          
                                 
      
    let openbook = account.openbook_v2_orders_mut(openbook_market.market_index)?;
    if !before_had_bids {
                                                   
        openbook.highest_placed_bid_inv = 0.0;
        openbook.lowest_placed_bid_inv = 0.0
    }
    if !before_had_asks {
        openbook.lowest_placed_ask = 0.0;
        openbook.highest_placed_ask = 0.0;
    }
                                         
    let limit_price = price_lots as f64 * quote_lot_size as f64 / base_lot_size as f64;

    let new_order_on_book = after_oo_free_slots != before_oo_free_slots;
    if new_order_on_book {
        match order.side {
            OpenbookV2Side::Ask => {
                openbook.lowest_placed_ask = if openbook.lowest_placed_ask == 0.0 {
                    limit_price
                } else {
                    openbook.lowest_placed_ask.min(limit_price)
                };
                openbook.highest_placed_ask = if openbook.highest_placed_ask == 0.0 {
                    limit_price
                } else {
                    openbook.highest_placed_ask.max(limit_price)
                }
            }
            OpenbookV2Side::Bid => {
                                                                         
                let limit_price_inv = 1.0 / limit_price;
                openbook.highest_placed_bid_inv = if openbook.highest_placed_bid_inv == 0.0 {
                    limit_price_inv
                } else {
                                                                
                    openbook.highest_placed_bid_inv.min(limit_price_inv)
                };
                openbook.lowest_placed_bid_inv = if openbook.lowest_placed_bid_inv == 0.0 {
                    limit_price_inv
                } else {
                                                    
                    openbook.lowest_placed_bid_inv.max(limit_price_inv)
                }
            }
        }
    }

    emit_stack(OpenbookV2OpenOrdersBalanceLog {
        fermi_group: ctx.accounts.group.key(),
        fermi_account: ctx.accounts.account.key(),
        market_index: openbook_market.market_index,
        base_token_index: openbook_market.base_token_index,
        quote_token_index: openbook_market.quote_token_index,
        base_total: after_oo.native_base_total(),
        base_free: after_oo.native_base_free(),
        quote_total: after_oo.native_quote_total(),
        quote_free: after_oo.native_quote_free(),
        referrer_rebates_accrued: after_oo.native_rebates(),
    });

    let after_vault = accessor::amount(&ctx.accounts.payer_vault.to_account_info())?;

                                                     
    require_gte!(before_vault, after_vault);

    let before_position_native;
    let vault_difference;
    {
        let mut payer_bank = payer_bank_loader.load_mut()?;
        let mut receiver_bank = receiver_bank_loader.load_mut()?;
        let (base_bank, quote_bank) = match order.side {
            OpenbookV2Side::Bid => (&mut receiver_bank, &mut payer_bank),
            OpenbookV2Side::Ask => (&mut payer_bank, &mut receiver_bank),
        };
        update_bank_potential_tokens(openbook, base_bank, quote_bank, &after_oo);

                                                 
        before_position_native = account
            .token_position_mut(payer_bank.token_index)?
            .0
            .native(&payer_bank);

                                                                       
        vault_difference = {
            apply_vault_difference(
                ctx.accounts.account.key(),
                &mut account.borrow_mut(),
                SpotMarketIndex::OpenbookV2(openbook_market.market_index),
                &mut payer_bank,
                after_vault,
                before_vault,
            )?
        };
    }

                                          
                                                                               
    {
        let receiver_bank = receiver_bank_loader.load()?;
        receiver_bank
            .check_deposit_and_oo_limit()
            .with_context(|| std::format!("on {}", receiver_bank.name()))?;
    }

                                                                                      
    let withdrawn_from_vault = I80F48::from(before_vault - after_vault);
    let payer_bank = payer_bank_loader.load()?;
    if withdrawn_from_vault > before_position_native {
        require_msg_typed!(
            !payer_bank.are_borrows_reduce_only(),
            FermiError::TokenInReduceOnlyMode,
            "the payer tokens cannot be borrowed"
        );
        payer_bank.enforce_max_utilization_on_borrow()?;
        payer_bank.check_net_borrows(payer_bank_oracle)?;

                                           
                                                                                  
                                                                                              
                                                             
          
                                                                                         
                                                                                              
                            
        payer_bank
            .check_deposit_and_oo_limit()
            .with_context(|| std::format!("on {}", payer_bank.name()))?;
    } else {
        payer_bank.enforce_borrows_lte_deposits()?;
    }

                                                                        
                                         
                                          
                                                                                    
                                                                                          
                                                                                 
    let band_threshold = openbook_market.oracle_price_band();
    if new_order_on_book && band_threshold != f32::MAX {
        let (base_oracle, quote_oracle) = match order.side {
            OpenbookV2Side::Bid => (&receiver_bank_oracle, &payer_bank_oracle),
            OpenbookV2Side::Ask => (&payer_bank_oracle, &receiver_bank_oracle),
        };
        let base_oracle_f64 = base_oracle.to_num::<f64>();
        let quote_oracle_f64 = quote_oracle.to_num::<f64>();
                                                                                                 
        let limit_price_in_dollar = limit_price * quote_oracle_f64;
        let band_factor = 1.0 + band_threshold as f64;
        match order.side {
            OpenbookV2Side::Bid => {
                require_msg_typed!(
                    limit_price_in_dollar * band_factor >= base_oracle_f64,
                    FermiError::SpotPriceBandExceeded,
                    "bid price {} must be larger than {} ({}% of oracle)",
                    limit_price,
                    base_oracle_f64 / (quote_oracle_f64 * band_factor),
                    (100.0 / band_factor) as u64,
                );
            }
            OpenbookV2Side::Ask => {
                require_msg_typed!(
                    limit_price_in_dollar <= base_oracle_f64 * band_factor,
                    FermiError::SpotPriceBandExceeded,
                    "ask price {} must be smaller than {} ({}% of oracle)",
                    limit_price,
                    base_oracle_f64 * band_factor / quote_oracle_f64,
                    (100.0 * band_factor) as u64,
                );
            }
        }
    }

                                                         
    let receiver_bank = receiver_bank_loader.load()?;
    let payer_bank = payer_bank_loader.load()?;
                                              
    health_cache.adjust_token_balance(&receiver_bank, I80F48::ZERO)?;
    vault_difference.adjust_health_cache_token_balance(&mut health_cache, &payer_bank)?;
    let openbook_account = account.openbook_v2_orders(openbook_market.market_index)?;
    oo_difference.recompute_health_cache_openbook_v2_state(
        &mut health_cache,
        &openbook_account,
        &open_orders,
    )?;

                                             
      
                                                                                               
                                                                                          
                    
      
                                                                                              
                                                                           
    health_cache.check_has_all_spot_infos_for_token(&account.borrow(), receiver_token_index)?;
    if receiver_bank.are_deposits_reduce_only() {
        let balance = health_cache.token_info(receiver_token_index)?.balance_spot;
        let potential =
            health_cache.total_spot_potential(HealthType::Maint, receiver_token_index)?;
        require_msg_typed!(
            balance + potential < 1,
            FermiError::TokenInReduceOnlyMode,
            "receiver bank does not accept deposits"
        );
    }

      
                   
      
    if let Some(pre_init_health) = pre_health_opt {
        account.check_health_post(&health_cache, pre_init_health)?;
    }

    Ok(())
}

                                                                                
                                                        
pub fn apply_settle_changes(
    group: &Group,
    account_pk: Pubkey,
    account: &mut FermiAccountRefMut,
    base_bank: &mut Bank,
    quote_bank: &mut Bank,
    openbook_market: &OpenbookV2Market,
    before_base_vault: u64,
    before_quote_vault: u64,
    before_oo: &OpenOrdersSlim,
    after_base_vault: u64,
    after_quote_vault: u64,
    after_oo: &OpenOrdersSlim,
    health_cache: Option<&mut HealthCache>,
    fees_to_dao: bool,
    quote_oracle: Option<&AccountInfo>,
    open_orders: &OpenOrdersAccount,
) -> Result<()> {
    let mut received_fees = 0;
    if fees_to_dao {
                                                                               
        received_fees = before_oo
            .native_rebates()
            .saturating_sub(after_oo.native_rebates());
        quote_bank.collected_fees_native =
            { quote_bank.collected_fees_native } + I80F48::from(received_fees);

                                                                           
        if let Some(quote_oracle_ai) = quote_oracle {
            let clock = Clock::get()?;
            let now_ts = clock.unix_timestamp.try_into().unwrap();

            let quote_oracle_ref = &AccountInfoRef::borrow(quote_oracle_ai)?;
            let quote_oracle_price = quote_bank.oracle_price(
                &OracleAccountInfos::from_reader(quote_oracle_ref),
                Some(clock.slot),
            )?;
            let quote_asset_price = quote_oracle_price.min(quote_bank.stable_price());
            account
                .fixed
                .expire_buyback_fees(now_ts, group.buyback_fees_expiry_interval);
            let fees_in_usd = I80F48::from(received_fees) * quote_asset_price;
            account
                .fixed
                .accrue_buyback_fees(fees_in_usd.clamp_to_u64());
        }
    }

                                                                                      
                            
    let after_quote_vault_adjusted = after_quote_vault - received_fees;

                                            
    require_gte!(after_base_vault, before_base_vault);
    require_gte!(after_quote_vault_adjusted, before_quote_vault);

                                                                    
    let base_difference = apply_vault_difference(
        account_pk,
        account,
        SpotMarketIndex::OpenbookV2(openbook_market.market_index),
        base_bank,
        after_base_vault,
        before_base_vault,
    )?;
    let quote_difference = apply_vault_difference(
        account_pk,
        account,
        SpotMarketIndex::OpenbookV2(openbook_market.market_index),
        quote_bank,
        after_quote_vault_adjusted,
        before_quote_vault,
    )?;

                                                                                    
                                               
    {
        let openbook_orders = account.openbook_v2_orders_mut(openbook_market.market_index)?;
        update_bank_potential_tokens(openbook_orders, base_bank, quote_bank, after_oo);
    }

    if let Some(health_cache) = health_cache {
        base_difference.adjust_health_cache_token_balance(health_cache, &base_bank)?;
        quote_difference.adjust_health_cache_token_balance(health_cache, &quote_bank)?;

        let serum_account = account.openbook_v2_orders(openbook_market.market_index)?;
        OODifference::new(&before_oo, &after_oo).recompute_health_cache_openbook_v2_state(
            health_cache,
            serum_account,
            open_orders,
        )?;
    }

    Ok(())
}

fn update_bank_potential_tokens(
    openbook_orders: &mut OpenbookV2Orders,
    base_bank: &mut Bank,
    quote_bank: &mut Bank,
    oo: &OpenOrdersSlim,
) {
    assert_eq!({ openbook_orders.base_token_index }, {
        base_bank.token_index
    });
    assert_eq!({ openbook_orders.quote_token_index }, {
        quote_bank.token_index
    });

                                                                                   
                                                                                    
                                                                     
    let new_base = oo.native_base_total()
        + (oo.native_quote_reserved() as f64 * openbook_orders.lowest_placed_bid_inv) as u64;
    let new_quote = oo.native_quote_total()
        + (oo.native_base_reserved() as f64 * openbook_orders.highest_placed_ask) as u64;

    let old_base = openbook_orders.potential_base_tokens;
    let old_quote = openbook_orders.potential_quote_tokens;

    base_bank.update_potential_openbook_tokens(old_base, new_base);
    quote_bank.update_potential_openbook_tokens(old_quote, new_quote);

    openbook_orders.potential_base_tokens = new_base;
    openbook_orders.potential_quote_tokens = new_quote;
}

fn cpi_place_order<'info>(
    ctx: &OpenbookV2PlaceOrder<'info>,
    seeds: &[&[&[u8]]],
    order: &OpenbookV2Order,
    price_lots: i64,
    limit: u8,
    bids_ai: &AccountInfo<'info>,
    asks_ai: &AccountInfo<'info>,
    event_heap_ai: &AccountInfo<'info>,
    market_vault_ai: &AccountInfo<'info>,
    token_program_ai: &AccountInfo<'info>,
) -> Result<Return<Option<u128>>> {
    let cpi_accounts = openbook_v2::cpi::accounts::PlaceOrder {
        signer: ctx.group.to_account_info(),
        open_orders_account: ctx.open_orders.to_account_info(),
        open_orders_admin: None,
        user_token_account: ctx.payer_vault.to_account_info(),
        market: ctx.openbook_v2_market_external.to_account_info(),
        bids: bids_ai.clone(),
        asks: asks_ai.clone(),
        event_heap: event_heap_ai.clone(),
        market_vault: market_vault_ai.clone(),
        oracle_a: None,                                             
        oracle_b: None,
        token_program: token_program_ai.clone(),
    };

    let cpi_ctx = CpiContext::new_with_signer(
        ctx.openbook_v2_program.to_account_info(),
        cpi_accounts,
        seeds,
    );

    let expiry_timestamp: u64 = if order.time_in_force > 0 {
        Clock::get()
            .unwrap()
            .unix_timestamp
            .saturating_add(order.time_in_force as i64)
            .try_into()
            .unwrap()
    } else {
        0
    };

    let order_type = match order.params {
        openbook_v2::state::OrderParams::Market => OpenbookV2OrderType::Market,
        openbook_v2::state::OrderParams::ImmediateOrCancel { price_lots } => {
            OpenbookV2OrderType::ImmediateOrCancel
        }
        openbook_v2::state::OrderParams::Fixed {
            price_lots,
            order_type,
        } => match order_type {
            openbook_v2::state::PostOrderType::Limit => OpenbookV2OrderType::Limit,
            openbook_v2::state::PostOrderType::PostOnly => OpenbookV2OrderType::PostOnly,
            openbook_v2::state::PostOrderType::PostOnlySlide => OpenbookV2OrderType::PostOnlySlide,
        },
        openbook_v2::state::OrderParams::OraclePegged {
            price_offset_lots,
            order_type,
            peg_limit,
        } => todo!(),
        openbook_v2::state::OrderParams::FillOrKill { price_lots } => {
            OpenbookV2OrderType::ImmediateOrCancel
        }
    };

    let args = openbook_v2::PlaceOrderArgs {
        side: order.side,
        price_lots,
        max_base_lots: order.max_base_lots,
        max_quote_lots_including_fees: order.max_quote_lots_including_fees,
        client_order_id: order.client_order_id,
        order_type,
        expiry_timestamp,
        self_trade_behavior: order.self_trade_behavior,
        limit,
    };

    msg!("args {:?}", args);
    openbook_v2::cpi::place_order(cpi_ctx, args)
}
