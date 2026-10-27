use anchor_lang::prelude::*;
use anchor_spl::token;
use fixed::types::I80F48;

use crate::accounts_zerocopy::*;
use crate::error::*;
use crate::health::*;
use crate::state::*;

use crate::accounts_ix::*;
use crate::logs::{
    emit_stack, LoanOriginationFeeInstruction, TokenBalanceLog, TokenLiqBankruptcyLog,
    WithdrawLoanLog,
};

pub fn token_liq_bankruptcy(
    ctx: Context<TokenLiqBankruptcy>,
    max_liab_transfer: I80F48,
) -> Result<()> {
    let group = ctx.accounts.group.load()?;
    let group_pk = &ctx.accounts.group.key();

                                                     
    let liab_mint_info = ctx.accounts.liab_mint_info.load()?;
    let liab_token_index = liab_mint_info.token_index;
    let (bank_ais, health_ais) = &ctx.remaining_accounts.split_at(liab_mint_info.num_banks());
    liab_mint_info.verify_banks_ais(bank_ais)?;

                                          
    let insurance_mint = ctx.accounts.insurance_vault.mint;
    let insurance_token_index = health_ais
        .iter()
        .find_map(|ai| {
            ai.load::<Bank>()
                .and_then(|b| {
                    if b.mint == insurance_mint {
                        Ok(b.token_index)
                    } else {
                        Err(FermiError::InvalidBank.into())
                    }
                })
                .ok()
        })
        .ok_or_else(|| error_msg!("could not find bank for insurance mint in health accounts"))?;

    require_keys_neq!(ctx.accounts.liqor.key(), ctx.accounts.liqee.key());

    let mut liqor = ctx.accounts.liqor.load_full_mut()?;
                            
    require!(
        liqor
            .fixed
            .is_owner_or_delegate(ctx.accounts.liqor_owner.key()),
        FermiError::SomeError
    );
    require_msg_typed!(
        !liqor.fixed.being_liquidated(),
        FermiError::BeingLiquidated,
        "liqor account"
    );

    let mut account_retriever = ScanningAccountRetriever::new(health_ais, group_pk)?;
    let now_ts: u64 = Clock::get()?.unix_timestamp.try_into().unwrap();

    let mut liqee = ctx.accounts.liqee.load_full_mut()?;
    let mut liqee_health_cache = new_health_cache(&liqee.borrow(), &account_retriever, now_ts)
        .context("create liqee health cache")?;
    liqee_health_cache.require_after_phase2_liquidation()?;
    liqee.fixed.set_being_liquidated(true);

    let liab_is_insurance_token = liab_token_index == insurance_token_index;
    let (liab_bank, liab_oracle_price, opt_insurance_bank_and_price) =
        account_retriever.banks_mut_and_oracles(liab_token_index, insurance_token_index)?;
    assert!(liab_is_insurance_token == opt_insurance_bank_and_price.is_none());

    let mut liab_deposit_index = liab_bank.deposit_index;
    let liab_borrow_index = liab_bank.borrow_index;
    let (liqee_liab, liqee_raw_token_index) = liqee.token_position_mut(liab_token_index)?;
    let initial_liab_native = liqee_liab.native(liab_bank);

    let liqee_health_token_balances =
        liqee_health_cache.effective_token_balances(HealthType::LiquidationEnd);
    let liqee_liab_health_balance = liqee_health_token_balances
        [liqee_health_cache.token_info_index(liab_token_index)?]
    .spot_and_perp;

                                                                                                       
                                                                                           
                                                                                             
                  
    require_gt!(I80F48::ZERO, initial_liab_native);
    require_gt!(I80F48::ZERO, liqee_liab_health_balance);
                          
    let mut remaining_liab_loss = (-initial_liab_native).min(-liqee_liab_health_balance);

                                                    
                                                                
                                                                   
    let liab_to_insurance_with_fee =
        if let Some((_insurance_bank, insurance_price)) = opt_insurance_bank_and_price.as_ref() {
            liab_oracle_price * (I80F48::ONE + liab_bank.liquidation_fee) / insurance_price
        } else {
            I80F48::ONE
        };

    let liab_transfer_unrounded = remaining_liab_loss.min(max_liab_transfer);

    let insurance_vault_amount = if liab_mint_info.elligible_for_group_insurance_fund() {
        ctx.accounts.insurance_vault.amount
    } else {
        0
    };

    let insurance_transfer = (liab_transfer_unrounded * liab_to_insurance_with_fee)
        .ceil()
        .to_num::<u64>()
        .min(insurance_vault_amount);

    let insurance_fund_exhausted = insurance_transfer == insurance_vault_amount;

    let insurance_transfer_i80f48 = I80F48::from(insurance_transfer);

                                                                               
                                                                                       
                                                                              
    let liab_transfer = insurance_transfer_i80f48 / liab_to_insurance_with_fee;

    let mut liqee_liab_active = true;
    if insurance_transfer > 0 {
                                                                                                 
                             
        liqee_liab_active = liab_bank.deposit_with_dusting(liqee_liab, liab_transfer, now_ts)?;
                                                    
        remaining_liab_loss -= liqee_liab.native(liab_bank) - initial_liab_native;

                                                    
        let group_seeds = group_seeds!(group);
        token::transfer(
            ctx.accounts.transfer_ctx().with_signer(&[group_seeds]),
            insurance_transfer,
        )?;

                                                                    
        if let Some((insurance_bank, _)) = opt_insurance_bank_and_price {
                                       
            require_keys_eq!(insurance_bank.vault, ctx.accounts.quote_vault.key());
            require_keys_eq!(insurance_bank.mint, ctx.accounts.insurance_vault.mint);

            let insurance_deposit_index = insurance_bank.deposit_index;
            let insurance_borrow_index = insurance_bank.borrow_index;

                               
            let (liqor_insurance, liqor_insurance_raw_token_index, _) =
                liqor.ensure_token_position(insurance_token_index)?;
            let liqor_insurance_active =
                insurance_bank.deposit(liqor_insurance, insurance_transfer_i80f48, now_ts)?;

                              
            emit_stack(TokenBalanceLog {
                fermi_group: ctx.accounts.group.key(),
                fermi_account: ctx.accounts.liqor.key(),
                token_index: insurance_token_index,
                indexed_position: liqor_insurance.indexed_position.to_bits(),
                deposit_index: insurance_deposit_index.to_bits(),
                borrow_index: insurance_borrow_index.to_bits(),
            });

                                                
            let (liqor_liab, liqor_liab_raw_token_index, _) =
                liqor.ensure_token_position(liab_token_index)?;
            let liqor_liab_withdraw_result =
                liab_bank.withdraw_with_fee(liqor_liab, liab_transfer, now_ts)?;

                         
            emit_stack(TokenBalanceLog {
                fermi_group: ctx.accounts.group.key(),
                fermi_account: ctx.accounts.liqor.key(),
                token_index: liab_token_index,
                indexed_position: liqor_liab.indexed_position.to_bits(),
                deposit_index: liab_deposit_index.to_bits(),
                borrow_index: liab_borrow_index.to_bits(),
            });

                                   
            if !liqor.fixed.is_in_health_region() {
                let liqor_health = compute_health(
                    &liqor.borrow(),
                    HealthType::Init,
                    &account_retriever,
                    now_ts,
                )?;
                require!(liqor_health >= 0, FermiError::HealthMustBePositive);
            }

            if liqor_liab_withdraw_result
                .loan_origination_fee
                .is_positive()
            {
                emit_stack(WithdrawLoanLog {
                    fermi_group: ctx.accounts.group.key(),
                    fermi_account: ctx.accounts.liqor.key(),
                    token_index: liab_token_index,
                    loan_amount: liqor_liab_withdraw_result.loan_amount.to_bits(),
                    loan_origination_fee: liqor_liab_withdraw_result.loan_origination_fee.to_bits(),
                    instruction: LoanOriginationFeeInstruction::LiqTokenBankruptcy,
                    price: Some(liab_oracle_price.to_bits()),
                });
            }

            if !liqor_insurance_active {
                liqor.deactivate_token_position_and_log(
                    liqor_insurance_raw_token_index,
                    ctx.accounts.liqor.key(),
                );
            }
            if !liqor_liab_withdraw_result.position_is_active {
                liqor.deactivate_token_position_and_log(
                    liqor_liab_raw_token_index,
                    ctx.accounts.liqor.key(),
                );
            }
        } else {
                                                                                                              
                                                        
                                       
            require_keys_eq!(liab_bank.vault, ctx.accounts.quote_vault.key());
            require_eq!(liab_token_index, insurance_token_index);
            require_eq!(liab_to_insurance_with_fee, I80F48::ONE);
            require_eq!(insurance_transfer_i80f48, liab_transfer);
        }
    }
    drop(account_retriever);

                                                                       
                                  
    let mut socialized_loss = I80F48::ZERO;
    let starting_deposit_index = liab_deposit_index;
    if insurance_fund_exhausted && remaining_liab_loss.is_positive() {
                                  
        let mut indexed_total_deposits = I80F48::ZERO;
        for bank_ai in bank_ais.iter() {
            let bank = bank_ai.load::<Bank>()?;
            indexed_total_deposits += bank.indexed_deposits;
        }

                                   
                                                                                               
                                                                                               
                                                                                      
        let max_socializable_loss = indexed_total_deposits * liab_deposit_index;
        let capped_remaining_loss = if remaining_liab_loss > max_socializable_loss {
            max_socializable_loss
        } else {
            remaining_liab_loss
        };
        let new_deposit_index = liab_deposit_index - capped_remaining_loss / indexed_total_deposits;
        liab_deposit_index = new_deposit_index;
        socialized_loss = capped_remaining_loss;

        let mut amount_to_credit = remaining_liab_loss;
        for bank_ai in bank_ais.iter() {
            let mut bank = bank_ai.load_mut::<Bank>()?;
            bank.deposit_index = new_deposit_index;

                                                                    
            let amount_for_bank = amount_to_credit.min(bank.native_borrows());
            if amount_for_bank.is_positive() {
                                                                                                       
                                                                               
                liqee_liab_active =
                    bank.deposit_with_dusting(liqee_liab, amount_for_bank, now_ts)?;
                amount_to_credit -= amount_for_bank;
                if amount_to_credit <= 0 {
                    break;
                }
            }
        }

                                                             
        require_eq!({ liqee_liab.indexed_position }, I80F48::ZERO);
    }

                 
    emit_stack(TokenBalanceLog {
        fermi_group: ctx.accounts.group.key(),
        fermi_account: ctx.accounts.liqee.key(),
        token_index: liab_token_index,
        indexed_position: liqee_liab.indexed_position.to_bits(),
        deposit_index: liab_deposit_index.to_bits(),
        borrow_index: liab_borrow_index.to_bits(),
    });

    let liab_bank = bank_ais[0].load::<Bank>()?;
    let end_liab_native = liqee_liab.native(&liab_bank);
    liqee_health_cache.adjust_token_balance(&liab_bank, end_liab_native - initial_liab_native)?;

                               
    let liqee_liq_end_health = liqee_health_cache.health(HealthType::LiquidationEnd);
    liqee
        .fixed
        .maybe_recover_from_being_liquidated(liqee_liq_end_health);

    if !liqee_liab_active {
        liqee.deactivate_token_position_and_log(liqee_raw_token_index, ctx.accounts.liqee.key());
    }

    emit_stack(TokenLiqBankruptcyLog {
        fermi_group: ctx.accounts.group.key(),
        liqee: ctx.accounts.liqee.key(),
        liqor: ctx.accounts.liqor.key(),
        liab_token_index,
        initial_liab_native: initial_liab_native.to_bits(),
        liab_price: liab_oracle_price.to_bits(),
        insurance_token_index,
        insurance_transfer: insurance_transfer_i80f48.to_bits(),
        socialized_loss: socialized_loss.to_bits(),
        starting_liab_deposit_index: starting_deposit_index.to_bits(),
        ending_liab_deposit_index: liab_deposit_index.to_bits(),
    });

    Ok(())
}
