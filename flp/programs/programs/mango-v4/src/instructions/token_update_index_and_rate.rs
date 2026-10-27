use anchor_lang::prelude::*;

use crate::accounts_ix::*;
use crate::error::FermiError;
use crate::logs::{emit_stack, UpdateIndexLog, UpdateRateLogV2};
use crate::state::{OracleAccountInfos, HOUR};
use crate::{
    accounts_zerocopy::{AccountInfoRef, LoadMutZeroCopyRef, LoadZeroCopyRef},
    state::Bank,
};
use anchor_lang::solana_program::sysvar::instructions as tx_instructions;
use anchor_lang::Discriminator;

use fixed::types::I80F48;

pub mod compute_budget {
    use solana_program::declare_id;
    declare_id!("ComputeBudget111111111111111111111111111111");
}

pub fn token_update_index_and_rate(
    ctx: Context<TokenUpdateIndexAndRate>,
    early_exit_on_invalid_oracle: bool,
) -> Result<()> {
    {
        let ixs = ctx.accounts.instructions.as_ref();

        let mut index = 0;
        loop {
            let ix = match tx_instructions::load_instruction_at_checked(index, ixs) {
                Ok(ix) => ix,
                Err(ProgramError::InvalidArgument) => break,
                Err(e) => return Err(e.into()),
            };

                                                                                
                                                                                                      
                                                                                        
                                                                                
                                                                                                          
            require!(
                (ix.program_id == crate::id()
                    && ix.data[0..8]
                        == crate::instruction::TokenUpdateIndexAndRate::discriminator())
                    || (ix.program_id == compute_budget::id()),
                FermiError::SomeError
            );

            index += 1;
        }
    }

    let mint_info = ctx.accounts.mint_info.load()?;

    ctx.accounts
        .mint_info
        .load()?
        .verify_banks_ais(ctx.remaining_accounts)?;

    let clock = Clock::get()?;
    let now_ts: u64 = clock.unix_timestamp.try_into().unwrap();

                            
    let mut indexed_total_deposits = I80F48::ZERO;
    let mut indexed_total_borrows = I80F48::ZERO;
    for ai in ctx.remaining_accounts.iter() {
        let bank = ai.load::<Bank>()?;
        indexed_total_deposits += bank.indexed_deposits;
        indexed_total_borrows += bank.indexed_borrows;
    }

                                                                        
                                        
    {
        let mut some_bank = ctx.remaining_accounts[0].load_mut::<Bank>()?;

        let oracle_ref = &AccountInfoRef::borrow(ctx.accounts.oracle.as_ref())?;
        let price = some_bank.oracle_price(
            &OracleAccountInfos::from_reader(oracle_ref),
            Some(clock.slot),
        );

                                          
                                                             
        let price = match price {
            Ok(p) => p,
            Err(e) => {
                return if early_exit_on_invalid_oracle {
                    msg!("Invalid oracle state: {}", e);
                    Ok(())
                } else {
                    Err(e)
                }
            }
        };

                                                                                                
                                                                                               
                                                                                             
        let max_interest_timestep = 3600;        
        let diff_ts =
            I80F48::from_num((now_ts - some_bank.index_last_updated).min(max_interest_timestep));

        let (deposit_index, borrow_index, borrow_fees, borrow_rate, deposit_rate) =
            some_bank.compute_index(indexed_total_deposits, indexed_total_borrows, diff_ts)?;

        some_bank.collected_fees_native = { some_bank.collected_fees_native } + borrow_fees;

        let new_avg_utilization = some_bank.compute_new_avg_utilization(
            indexed_total_deposits,
            indexed_total_borrows,
            now_ts,
        );

        let mut spm = { some_bank.stable_price_model };
        spm.update(now_ts as u64, price.to_num());
        some_bank.stable_price_model = spm;
        let stable_price_model = { some_bank.stable_price_model };

                                                                                  
                                               
        let maint_shift_done = some_bank.maint_weight_shift_duration_inv.is_positive()
            && now_ts >= some_bank.maint_weight_shift_end;

        emit_stack(UpdateIndexLog {
            fermi_group: mint_info.group.key(),
            token_index: mint_info.token_index,
            deposit_index: deposit_index.to_bits(),
            borrow_index: borrow_index.to_bits(),
            avg_utilization: new_avg_utilization.to_bits(),
            price: price.to_bits(),
            stable_price: some_bank.stable_price().to_bits(),
            collected_fees: some_bank.collected_fees_native.to_bits(),
            loan_fee_rate: some_bank.loan_fee_rate.to_bits(),
            total_deposits: (deposit_index * indexed_total_deposits).to_bits(),
            total_borrows: (borrow_index * indexed_total_borrows).to_bits(),
            borrow_rate: borrow_rate.to_bits(),
            deposit_rate: deposit_rate.to_bits(),
        });

        drop(some_bank);

        msg!("indexed_total_deposits {}", indexed_total_deposits);
        msg!("indexed_total_borrows {}", indexed_total_borrows);
        msg!("diff_ts {}", diff_ts);
        msg!("deposit_index {}", deposit_index);
        msg!("borrow_index {}", borrow_index);
        msg!("avg_utilization {}", new_avg_utilization);

        for ai in ctx.remaining_accounts.iter() {
            let mut bank = ai.load_mut::<Bank>()?;

            bank.index_last_updated = now_ts;

            bank.deposit_index = deposit_index;
            bank.borrow_index = borrow_index;

            bank.avg_utilization = new_avg_utilization;

            bank.stable_price_model = stable_price_model;

            if maint_shift_done {
                bank.maint_asset_weight = bank.maint_weight_shift_asset_target;
                bank.maint_liab_weight = bank.maint_weight_shift_liab_target;
                bank.maint_weight_shift_duration_inv = I80F48::ZERO;
                bank.maint_weight_shift_asset_target = I80F48::ZERO;
                bank.maint_weight_shift_liab_target = I80F48::ZERO;
                bank.maint_weight_shift_start = 0;
                bank.maint_weight_shift_end = 0;
            }
        }
    }

                                                                   
    {
        let mut some_bank = ctx.remaining_accounts[0].load_mut::<Bank>()?;

        let diff_ts = I80F48::from_num(now_ts - some_bank.bank_rate_last_updated);

                           
        if diff_ts > HOUR {
                                                             
            if some_bank.interest_curve_scaling == 0.0 {
                let old_max_rate = 0.5;
                some_bank.interest_curve_scaling =
                    some_bank.max_rate.to_num::<f64>() / old_max_rate;
                some_bank.interest_target_utilization = some_bank.util0.to_num();

                let descale_factor = I80F48::from_num(1.0 / some_bank.interest_curve_scaling);
                some_bank.rate0 = { some_bank.rate0 } * descale_factor;
                some_bank.rate1 = { some_bank.rate1 } * descale_factor;
                some_bank.max_rate = { some_bank.max_rate } * descale_factor;
            }

            some_bank.update_interest_rate_scaling();

            let rate0 = some_bank.rate0;
            let rate1 = some_bank.rate1;
            let max_rate = some_bank.max_rate;
            let scaling = some_bank.interest_curve_scaling;
            let target_util = some_bank.interest_target_utilization;

            emit_stack(UpdateRateLogV2 {
                fermi_group: mint_info.group.key(),
                token_index: mint_info.token_index,
                rate0: rate0.to_bits(),
                util0: some_bank.util0.to_bits(),
                rate1: rate1.to_bits(),
                util1: some_bank.util1.to_bits(),
                max_rate: max_rate.to_bits(),
                curve_scaling: some_bank.interest_curve_scaling,
                target_utilization: some_bank.interest_target_utilization,
            });

            drop(some_bank);

                                                    
            for ai in ctx.remaining_accounts.iter() {
                let mut bank = ai.load_mut::<Bank>()?;

                bank.bank_rate_last_updated = now_ts;
                bank.interest_curve_scaling = scaling;
                bank.interest_target_utilization = target_util;
                bank.rate0 = rate0;
                bank.rate1 = rate1;
                bank.max_rate = max_rate;
            }
        }
    }

    Ok(())
}
