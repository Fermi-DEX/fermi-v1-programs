use crate::accounts_zerocopy::*;
use crate::error::*;
use crate::health::*;
use crate::state::*;
use crate::util::clock_now;
use anchor_lang::prelude::*;
use anchor_spl::associated_token;
use anchor_spl::token;
use fixed::types::I80F48;

use crate::accounts_ix::*;
use crate::logs::{
    emit_stack, LoanOriginationFeeInstruction, TokenBalanceLog, WithdrawLoanLog, WithdrawLog,
};

pub fn token_withdraw(ctx: Context<TokenWithdraw>, amount: u64, allow_borrow: bool) -> Result<()> {
    require_msg!(amount > 0, "withdraw amount must be positive");

    let group = ctx.accounts.group.load()?;
    let token_index = ctx.accounts.bank.load()?.token_index;
    let (now_ts, now_slot) = clock_now();

                                                         
    let mut account = ctx.accounts.account.load_full_mut()?;
    let (_, raw_token_index, _) = account.ensure_token_position(token_index)?;

                                                                     
    let pre_health_opt = if !account.fixed.is_in_health_region() {
        let retriever = new_fixed_order_account_retriever_with_optional_banks(
            ctx.remaining_accounts,
            &account.borrow(),
            now_slot,
        )?;
        let health_cache = new_health_cache_skipping_missing_banks_and_bad_oracles(
            &account.borrow(),
            &retriever,
            now_ts,
        )
        .context("pre-withdraw health cache")?;
        let pre_init_health = account.check_health_pre(&health_cache)?;
        Some((health_cache, pre_init_health))
    } else {
        None
    };

    let mut bank = ctx.accounts.bank.load_mut()?;
    let position = account.token_position_mut_by_raw_index(raw_token_index);
    let native_position = position.native(&bank);

                                                            
    let amount = if amount == u64::MAX && !allow_borrow {
        if !native_position.is_negative() {
                                                                                       
                                                       
            native_position.floor().to_num::<u64>()
        } else {
            return Ok(());
        }
    } else {
        amount
    };

    let is_borrow = amount > native_position;
    require!(allow_borrow || !is_borrow, FermiError::SomeError);
    if bank.are_borrows_reduce_only() {
        require!(!is_borrow, FermiError::TokenInReduceOnlyMode);
    }

    let amount_i80f48 = I80F48::from(amount);

                                                                                 
                                                                                   
    let oracle_ref = &AccountInfoRef::borrow(ctx.accounts.oracle.as_ref())?;
    let unsafe_oracle_state = oracle_state_unchecked(
        &OracleAccountInfos::from_reader(oracle_ref),
        bank.mint_decimals,
    )?;

                                   
    let withdraw_result = bank.withdraw_with_fee(
        position,
        amount_i80f48,
        Clock::get()?.unix_timestamp.try_into().unwrap(),
    )?;
    let native_position_after = position.native(&bank);

                                                                                
    drop(bank);
    let bank = ctx.accounts.bank.load()?;

                                                                                    
    if ctx.accounts.vault.amount < amount {
        return err!(FermiError::InsufficentBankVaultFunds).with_context(|| {
            format!(
                "bank vault does not have enough tokens, need {} but have {}",
                amount, ctx.accounts.vault.amount
            )
        });
    }

                                 
    let group_seeds = group_seeds!(group);
    token::transfer(
        ctx.accounts.transfer_ctx().with_signer(&[group_seeds]),
        amount,
    )?;

    emit_stack(TokenBalanceLog {
        fermi_group: ctx.accounts.group.key(),
        fermi_account: ctx.accounts.account.key(),
        token_index,
        indexed_position: position.indexed_position.to_bits(),
        deposit_index: bank.deposit_index.to_bits(),
        borrow_index: bank.borrow_index.to_bits(),
    });

                                                                                                         
    let amount_usd = (amount_i80f48 * unsafe_oracle_state.price).to_num::<i64>();
    account.fixed.net_deposits -= amount_usd;

                                                         
    if account.fixed.is_delegate(ctx.accounts.owner.key()) {
                                                                     
        let owner_ata = associated_token::get_associated_token_address(
            &account.fixed.owner,
            &ctx.accounts.vault.mint,
        );
        require_keys_eq!(
            ctx.accounts.token_account.key(),
            owner_ata,
            FermiError::DelegateWithdrawOnlyToOwnerAta
        );
        require_keys_eq!(
            ctx.accounts.token_account.owner,
            account.fixed.owner,
            FermiError::DelegateWithdrawOnlyToOwnerAta
        );

                                                  
        require!(
            !withdraw_result.position_is_active,
            FermiError::DelegateWithdrawMustClosePosition
        );
    }

      
                   
      
    if let Some((mut health_cache, pre_init_health_lower_bound)) = pre_health_opt {
        if health_cache.has_token_info(token_index) {
                                                                                      
                                                                            
            health_cache.adjust_token_balance(&bank, native_position_after - native_position)?;
            account.check_health_post(&health_cache, pre_init_health_lower_bound)?;
        } else {
                                                                                            
                                                                                
            require!(!is_borrow, FermiError::BorrowsRequireHealthAccountBank);

                                                                                    
                                      
            let post_init_health_lower_bound = pre_init_health_lower_bound;

                                                                                        
                                                                         
                                                                                           
                                                      
              
                                                                                         
                                                   
            account.check_health_post_checks_strict(post_init_health_lower_bound)?;
        }
    }

      
                                                                                     
                                                                                       
                   
      
    if !withdraw_result.position_is_active {
        account.deactivate_token_position_and_log(raw_token_index, ctx.accounts.account.key());
    }

    emit_stack(WithdrawLog {
        fermi_group: ctx.accounts.group.key(),
        fermi_account: ctx.accounts.account.key(),
        signer: ctx.accounts.owner.key(),
        token_index,
        quantity: amount,
        price: unsafe_oracle_state.price.to_bits(),
    });

    if withdraw_result.loan_origination_fee.is_positive() {
        emit_stack(WithdrawLoanLog {
            fermi_group: ctx.accounts.group.key(),
            fermi_account: ctx.accounts.account.key(),
            token_index,
            loan_amount: withdraw_result.loan_amount.to_bits(),
            loan_origination_fee: withdraw_result.loan_origination_fee.to_bits(),
            instruction: LoanOriginationFeeInstruction::TokenWithdraw,
            price: Some(unsafe_oracle_state.price.to_bits()),
        });
    }

                                                                
    if is_borrow {
        bank.enforce_max_utilization_on_borrow()?;

                                                                                 
                            
        let slot_opt = Some(Clock::get()?.slot);
        let oracle_config = { bank.oracle_config };
        unsafe_oracle_state
            .check_confidence_and_maybe_staleness(&oracle_config, slot_opt)
            .with_context(|| {
                oracle_log_context(bank.name(), &unsafe_oracle_state, &oracle_config, slot_opt)
            })?;
        bank.check_net_borrows(unsafe_oracle_state.price)?;
    } else {
        bank.enforce_borrows_lte_deposits()?;
    }

    Ok(())
}
