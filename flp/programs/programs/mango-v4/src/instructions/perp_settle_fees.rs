use anchor_lang::prelude::*;

use fixed::types::I80F48;

use crate::accounts_zerocopy::*;
use crate::error::*;
use crate::health::{compute_health, new_fixed_order_account_retriever, HealthType};
use crate::state::*;

use crate::accounts_ix::*;
use crate::logs::{emit_perp_balances, emit_stack, PerpSettleFeesLog, TokenBalanceLog};
use crate::util::clock_now;

pub fn perp_settle_fees(ctx: Context<PerpSettleFees>, max_settle_amount: u64) -> Result<()> {
                                               
    require!(
        max_settle_amount > 0,
        FermiError::MaxSettleAmountMustBeGreaterThanZero
    );

    let mut account = ctx.accounts.account.load_full_mut()?;
    let mut settle_bank = ctx.accounts.settle_bank.load_mut()?;
    let mut perp_market = ctx.accounts.perp_market.load_mut()?;

                                                           
    require_eq!(
        settle_bank.token_index,
        perp_market.settle_token_index,
        FermiError::InvalidBank
    );

                        
                                                                                  
    let now_slot = Some(Clock::get()?.slot);
    let oracle_ref = &AccountInfoRef::borrow(ctx.accounts.oracle.as_ref())?;
    let oracle_price =
        perp_market.oracle_price(&OracleAccountInfos::from_reader(oracle_ref), now_slot)?;
    let settle_oracle_ref = &AccountInfoRef::borrow(ctx.accounts.settle_oracle.as_ref())?;
    let settle_token_oracle_price = settle_bank.oracle_price(
        &OracleAccountInfos::from_reader(settle_oracle_ref),
        now_slot,
    )?;

                                        
    let perp_position = account.perp_position_mut(perp_market.perp_market_index)?;

                                             
    perp_position.settle_funding(&perp_market);

                    
    let pnl = perp_position.unsettled_pnl(&perp_market, oracle_price)?;

    let settleable_pnl = perp_position.apply_pnl_settle_limit(&perp_market, pnl);

    if !settleable_pnl.is_negative() || !perp_market.fees_accrued.is_positive() {
        msg!(
            "Not settling: pnl {}, perp_market.fees_accrued {}, settleable_pnl {}",
            pnl,
            { perp_market.fees_accrued },
            settleable_pnl
        );
        return Ok(());
    }

                                                                  
    let settlement = settleable_pnl
        .abs()
        .min(perp_market.fees_accrued.abs())
        .min(I80F48::from(max_settle_amount));
    require!(settlement >= 0, FermiError::SettlementAmountMustBePositive);

    perp_position.record_settle(-settlement, &perp_market);                                                     
    perp_market.fees_accrued = { perp_market.fees_accrued } - settlement;

    emit_perp_balances(
        ctx.accounts.group.key(),
        ctx.accounts.account.key(),
        perp_position,
        &perp_market,
    );

                                                                
    let settlement_i64 = settlement.round().to_num::<i64>();

                                                               
    require!(
        settlement_i64 >= 0,
        FermiError::SettlementAmountMustBePositive
    );

    perp_position.perp_spot_transfers -= settlement_i64;
    account.fixed.perp_spot_transfers -= settlement_i64;

                              
    let token_position = account
        .token_position_mut(perp_market.settle_token_index)?
        .0;
    settle_bank.withdraw_without_fee(
        token_position,
        settlement,
        Clock::get()?.unix_timestamp.try_into().unwrap(),
    )?;
                                                      
    perp_market.fees_settled = { perp_market.fees_settled } + settlement;

    emit_stack(TokenBalanceLog {
        fermi_group: ctx.accounts.group.key(),
        fermi_account: ctx.accounts.account.key(),
        token_index: perp_market.settle_token_index,
        indexed_position: token_position.indexed_position.to_bits(),
        deposit_index: settle_bank.deposit_index.to_bits(),
        borrow_index: settle_bank.borrow_index.to_bits(),
    });

    emit_stack(PerpSettleFeesLog {
        fermi_group: ctx.accounts.group.key(),
        fermi_account: ctx.accounts.account.key(),
        perp_market_index: perp_market.perp_market_index,
        settlement: settlement.to_bits(),
    });

                                                                                  
    drop(settle_bank);
    drop(perp_market);

                                                                                                   
    let (now_ts, now_slot) = clock_now();
    let retriever =
        new_fixed_order_account_retriever(ctx.remaining_accounts, &account.borrow(), now_slot)?;
    let health = compute_health(&account.borrow(), HealthType::Init, &retriever, now_ts)?;
    require!(health >= 0, FermiError::HealthMustBePositive);

    msg!("settled fees = {}", settlement);
    Ok(())
}
