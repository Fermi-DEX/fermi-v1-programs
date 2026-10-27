use anchor_lang::prelude::*;
use anchor_spl::token;
use anchor_spl::token::Token;
use anchor_spl::token::TokenAccount;
use fixed::types::I80F48;

use crate::accounts_zerocopy::AccountInfoRef;
use crate::error::*;
use crate::health::*;
use crate::state::*;

use crate::accounts_ix::*;
use crate::logs::*;
use crate::util::clock_now;

struct DepositCommon<'a, 'info> {
    pub group: &'a AccountLoader<'info, Group>,
    pub account: &'a AccountLoader<'info, FermiAccountFixed>,
    pub bank: &'a AccountLoader<'info, Bank>,
    pub vault: &'a Account<'info, TokenAccount>,
    pub oracle: &'a UncheckedAccount<'info>,
    pub token_account: &'a Box<Account<'info, TokenAccount>>,
    pub token_authority: &'a Signer<'info>,
    pub token_program: &'a Program<'info, Token>,
}

impl<'a, 'info> DepositCommon<'a, 'info> {
    fn transfer_ctx(&self) -> CpiContext<'_, '_, '_, 'info, token::Transfer<'info>> {
        let program = self.token_program.to_account_info();
        let accounts = token::Transfer {
            from: self.token_account.to_account_info(),
            to: self.vault.to_account_info(),
            authority: self.token_authority.to_account_info(),
        };
        CpiContext::new(program, accounts)
    }

    fn deposit_into_existing(
        &self,
        remaining_accounts: &[AccountInfo],
        amount: u64,
        reduce_only: bool,
        allow_token_account_closure: bool,
    ) -> Result<()> {
        require_msg!(amount > 0, "deposit amount must be positive");

        let mut bank = self.bank.load_mut()?;
        let token_index = bank.token_index;

        let amount_i80f48 = {
                                                              
            let account = self.account.load_full()?;
            let position = account.token_position(token_index)?;

            let amount_i80f48 = if reduce_only || bank.are_deposits_reduce_only() {
                position
                    .native(&bank)
                    .min(I80F48::ZERO)
                    .abs()
                    .ceil()
                    .min(I80F48::from(amount))
            } else {
                I80F48::from(amount)
            };
            if bank.are_deposits_reduce_only() {
                require!(
                    reduce_only || amount_i80f48 == I80F48::from(amount),
                    FermiError::TokenInReduceOnlyMode
                );
            }
            amount_i80f48
        };

                                                          
        let mut account = self.account.load_full_mut()?;

        let (position, raw_token_index) = account.token_position_mut(token_index)?;

        let position_is_active = {
            bank.deposit(
                position,
                amount_i80f48,
                Clock::get()?.unix_timestamp.try_into().unwrap(),
            )?
        };

                                     
        token::transfer(self.transfer_ctx(), amount_i80f48.to_num::<u64>())?;

        let indexed_position = position.indexed_position;

                                                                                     
                                                                                   
        let oracle_ref = &AccountInfoRef::borrow(self.oracle.as_ref())?;
        let unsafe_oracle_state = oracle_state_unchecked(
            &OracleAccountInfos::from_reader(oracle_ref),
            bank.mint_decimals,
        )?;
        let unsafe_oracle_price = unsafe_oracle_state.price;

                                                             
        if indexed_position > 0 {
            bank.check_deposit_and_oo_limit()?;
        }

                                                                                                             
        let amount_usd = (amount_i80f48 * unsafe_oracle_price).to_num::<i64>();
        account.fixed.net_deposits += amount_usd;

        emit_stack(TokenBalanceLog {
            fermi_group: self.group.key(),
            fermi_account: self.account.key(),
            token_index,
            indexed_position: indexed_position.to_bits(),
            deposit_index: bank.deposit_index.to_bits(),
            borrow_index: bank.borrow_index.to_bits(),
        });
        drop(bank);

          
                             
          
        let (now_ts, now_slot) = clock_now();
        let retriever = new_fixed_order_account_retriever_with_optional_banks(
            remaining_accounts,
            &account.borrow(),
            now_slot,
        )?;

                                                                                            
                                                                                       
                                 
        let cache = new_health_cache_skipping_missing_banks_and_bad_oracles(
            &account.borrow(),
            &retriever,
            now_ts,
        )?;

                                                                                                   
                                                                                                           
                                                                                                           
        let was_being_liquidated = account.being_liquidated();
        if !account.fixed.is_in_health_region() && was_being_liquidated {
            let health = cache.health(HealthType::LiquidationEnd);
            msg!("health: {}", health);
                                                                                         

            let recovered = account.fixed.maybe_recover_from_being_liquidated(health);
            require!(recovered, FermiError::DepositsIntoLiquidatingMustRecover);
        }

                                               
        let group = self.group.load()?;
        if group.deposit_limit_quote > 0 {
                                                                                             
                                                             
            require_eq!(
                cache.token_infos.len(),
                account.active_token_positions().count()
            );

            let assets = cache
                .health_assets_and_liabs_stable_assets(HealthType::Init)
                .0
                .round_to_zero()
                .to_num::<u64>();
            require_msg_typed!(
                assets <= group.deposit_limit_quote,
                FermiError::DepositLimit,
                "assets ({}) can't cross deposit limit on the group ({})",
                assets,
                { group.deposit_limit_quote }
            );
        }

          
                                                                                         
                                                                                           
                       
                                                                                   
          
        if allow_token_account_closure && !position_is_active {
            account.deactivate_token_position_and_log(raw_token_index, self.account.key());
        }

        emit_stack(DepositLog {
            fermi_group: self.group.key(),
            fermi_account: self.account.key(),
            signer: self.token_authority.key(),
            token_index,
            quantity: amount_i80f48.to_num::<u64>(),
            price: unsafe_oracle_price.to_bits(),
        });

        Ok(())
    }
}

pub fn token_deposit(ctx: Context<TokenDeposit>, amount: u64, reduce_only: bool) -> Result<()> {
    {
        let token_index = ctx.accounts.bank.load()?.token_index;
        let mut account = ctx.accounts.account.load_full_mut()?;

        let token_position_exists = account
            .all_token_positions()
            .any(|p| p.is_active_for_token(token_index));

                                                                                       
                                                                             
        if !token_position_exists {
            let now_slot = Clock::get()?.slot;
            let bank = ctx.accounts.bank.load()?;

            let oracle_ref = &AccountInfoRef::borrow(ctx.accounts.oracle.as_ref())?;
            let oracle_result =
                bank.oracle_price(&OracleAccountInfos::from_reader(oracle_ref), Some(now_slot));
            if let Err(e) = oracle_result {
                msg!("oracle must be valid when creating a new token position");
                return Err(e);
            }

            account.ensure_token_position(token_index)?;
        }
    }

    DepositCommon {
        group: &ctx.accounts.group,
        account: &ctx.accounts.account,
        bank: &ctx.accounts.bank,
        vault: &ctx.accounts.vault,
        oracle: &ctx.accounts.oracle,
        token_account: &ctx.accounts.token_account,
        token_authority: &ctx.accounts.token_authority,
        token_program: &ctx.accounts.token_program,
    }
    .deposit_into_existing(ctx.remaining_accounts, amount, reduce_only, true)
}

pub fn token_deposit_into_existing(
    ctx: Context<TokenDepositIntoExisting>,
    amount: u64,
    reduce_only: bool,
) -> Result<()> {
    DepositCommon {
        group: &ctx.accounts.group,
        account: &ctx.accounts.account,
        bank: &ctx.accounts.bank,
        vault: &ctx.accounts.vault,
        oracle: &ctx.accounts.oracle,
        token_account: &ctx.accounts.token_account,
        token_authority: &ctx.accounts.token_authority,
        token_program: &ctx.accounts.token_program,
    }
    .deposit_into_existing(ctx.remaining_accounts, amount, reduce_only, false)
}
