use anchor_lang::prelude::*;
use anchor_spl::token::TokenAccount;
use anchor_spl::token_2022::Token2022;
use anchor_spl::token_interface::Mint as IMint;

use super::common::*;
use crate::accounting::{BatchPayout, FeeAccrual, FundingCheck, PricingSnapshot};
use crate::error::FlpError;
use crate::events::*;
use crate::fermi_cpi;
use crate::math;
use crate::nav;
use crate::state::*;

#[derive(Accounts)]
pub struct CloseEpoch<'info> {
    #[account(mut, seeds = [FUNDING_WINDOW_SEED, vault.key().as_ref()], bump,
        has_one = vault, constraint = funding_window.version == 1 @ FlpError::InvalidFundingWindow)]
    pub funding_window: Box<Account<'info, FundingWindow>>,
    #[account(mut, has_one = deposit_escrow, has_one = redemption_escrow, has_one = share_mint)]
    pub vault: Box<Account<'info, Vault>>,

    #[account(
        init,
        payer = payer,
        space = 8 + EpochRecord::INIT_SPACE,
        seeds = [EPOCH_SEED, vault.key().as_ref(), &vault.epoch.to_le_bytes()],
        bump,
    )]
    pub epoch_record: Box<Account<'info, EpochRecord>>,

                                                                             
                                                                    
    #[account(mut)]
    pub deferred_tail_record: Option<UncheckedAccount<'info>>,

                                   
    #[account(seeds = [AUTHORITY_SEED, vault.key().as_ref()], bump = vault.authority_bump)]
    pub authority: UncheckedAccount<'info>,

    #[account(mut)]
    pub payer: Signer<'info>,

    pub fermi: FermiVaultAccounts<'info>,

    #[account(mut)]
    pub deposit_escrow: Box<Account<'info, TokenAccount>>,
    #[account(mut)]
    pub redemption_escrow: Box<Account<'info, TokenAccount>>,
    #[account(mut)]
    pub share_mint: Box<InterfaceAccount<'info, IMint>>,
                                                                             
                                   
                                                                                     
    #[account(mut)]
    pub fee_share_account: UncheckedAccount<'info>,
                                                                                     
    #[account(mut)]
    pub idle_account: UncheckedAccount<'info>,

    pub token_2022_program: Program<'info, Token2022>,
    pub system_program: Program<'info, System>,
}

                                                                                
                                                                              
                                                                                
                                                                               
                                                       
pub const MAX_EQUITY_BATCHES: usize = 8;
                                                                             
                                                                           
pub const DEFAULT_EQUITY_BATCHES: usize = 2;

                                                                            
                                                                              
struct DeferredRecords<'info> {
    records: Vec<(AccountInfo<'info>, EpochRecord)>,
    prefix_len: usize,
    tail_index: Option<usize>,
}
impl<'info> DeferredRecords<'info> {
    fn load(
        vault: &Account<Vault>,
        batches: &[AccountInfo<'info>],
        tail: Option<&UncheckedAccount<'info>>,
        needs_tail: bool,
    ) -> Result<Self> {
        let key = vault.key();
        let mut records = Vec::with_capacity(batches.len() + usize::from(needs_tail));
        let mut next = vault.deferred_head;
        for ai in batches {
            require!(next != NONE_EPOCH, FlpError::WrongEpochRecord);
            let record = read_deferred(ai, &key, next)?;
            next = record.next_deferred;
            records.push((ai.clone(), record));
        }
                                                                                 
                                                                                    
        require!(
            !batches.is_empty() || next == NONE_EPOCH,
            FlpError::MissingDeferredTail
        );
        let mut tail_index = None;
        if needs_tail {
            let ai = tail.ok_or(FlpError::MissingDeferredTail)?.to_account_info();
            let expected = Pubkey::find_program_address(
                &[EPOCH_SEED, key.as_ref(), &vault.deferred_tail.to_le_bytes()],
                &crate::ID,
            )
            .0;
            require_keys_eq!(*ai.key, expected, FlpError::WrongEpochRecord);
            require!(ai.is_writable, FlpError::WrongEpochRecord);
            tail_index = records
                .iter()
                .position(|(account, _)| account.key == ai.key);
            if tail_index.is_none() {
                let record = read_deferred(&ai, &key, vault.deferred_tail)?;
                tail_index = Some(records.len());
                records.push((ai, record));
            }
            require!(
                records[tail_index.unwrap()].1.next_deferred == NONE_EPOCH,
                FlpError::WrongEpochRecord
            );
        }
        Ok(Self {
            records,
            prefix_len: batches.len(),
            tail_index,
        })
    }
    fn write_back(&self) -> Result<()> {
        for (ai, record) in &self.records {
            record.try_serialize(&mut &mut ai.try_borrow_mut_data()?[..])?;
        }
        Ok(())
    }
}
pub(crate) fn read_deferred(ai: &AccountInfo, vault: &Pubkey, epoch: u64) -> Result<EpochRecord> {
    require!(ai.is_writable, FlpError::WrongEpochRecord);
    read_deferred_readonly(ai, vault, epoch)
}

pub(crate) fn read_deferred_readonly(ai: &AccountInfo, vault: &Pubkey, epoch: u64) -> Result<EpochRecord> {
    require_keys_eq!(*ai.owner, crate::ID, FlpError::WrongEpochRecord);
    let expected = Pubkey::find_program_address(
        &[EPOCH_SEED, vault.as_ref(), &epoch.to_le_bytes()],
        &crate::ID,
    )
    .0;
    require_keys_eq!(*ai.key, expected, FlpError::WrongEpochRecord);
    let record = EpochRecord::try_deserialize(&mut &ai.try_borrow_data()?[..])?;
    require!(
        record.vault == *vault
            && record.epoch == epoch
            && record.wd_status == WithdrawStatus::Deferred as u8,
        FlpError::WrongEpochRecord
    );
    Ok(record)
}

                                                                               
                                                                              
                                                                          
pub fn close_epoch<'info>(
    ctx: Context<'_, '_, '_, 'info, CloseEpoch<'info>>,
    deferred_batch_count: u8,
) -> Result<()> {
    let now = now_ts()?;
    let key = ctx.accounts.vault.key();
    let e = ctx.accounts.vault.epoch;
    let params = ctx.accounts.vault.params;
    let pending = ctx.accounts.vault.pending_for(e);
    let count = deferred_batch_count as usize;
    require!(
        count <= MAX_EQUITY_BATCHES && count <= ctx.remaining_accounts.len(),
        FlpError::InvalidParams
    );
                                                                            
    let (remaining, targets_ai) = super::capacity::split_optional_recovery_accounts(
        &ctx.accounts.vault,
        &key,
        ctx.remaining_accounts,
        ctx.accounts.vault.is_unwinding(),
    )?;
    require!(count <= remaining.len(), FlpError::InvalidParams);
                                                                               
    let (batches, priced) = remaining.split_at(count);
    let expanded = if ctx.accounts.vault.is_unwinding() {
        super::capacity::read_targets(&ctx.accounts.vault, targets_ai)?
    } else { None };

                                                                                   
    ctx.accounts.vault.require_trading_open()?;
    require!(ctx.accounts.vault.is_seeded(), FlpError::NotSeeded);
    ctx.accounts.fermi.check(&ctx.accounts.vault)?;
    let fermi_ai = ctx.accounts.fermi.fermi_account.to_account_info();
    let (health, queues) = split_event_queues(&fermi_ai, priced)?;
    require_ata(
        &ctx.accounts.fee_share_account.key(),
        &ctx.accounts.vault.fee_recipient,
        &ctx.accounts.vault.share_mint,
        &anchor_spl::token_2022::ID,
    )?;
    let needs_tail = pending.withdraw_shares > 0 && ctx.accounts.vault.deferred_tail != NONE_EPOCH;
    let mut old = DeferredRecords::load(
        &ctx.accounts.vault,
        batches,
        ctx.accounts.deferred_tail_record.as_ref(),
        needs_tail,
    )?;
    let scheduled_end = ctx.accounts.vault.epoch_end();
    let scheduled = now >= scheduled_end
        && now
            <= scheduled_end
                .checked_add(ctx.accounts.vault.equity_window_s() as i64)
                .ok_or(FlpError::MathOverflow)?;
    let window = &mut ctx.accounts.funding_window;
    window.closed_epoch = e;
    window.scheduled_start = scheduled_end;
    window.deadline = scheduled_end.checked_add(ctx.accounts.vault.equity_window_s() as i64)
        .ok_or(FlpError::MathOverflow)?;
    window.eligible = scheduled;
    let unwinding = ctx.accounts.vault.is_unwinding();
                                                                        
    let targets_met = !unwinding
        || (pending_vault_events(&ctx.accounts.vault, &fermi_ai, health, queues)? == 0
        && require_recovery_targets(
            &ctx.accounts.vault,
            &ctx.accounts.fermi.fermi_account.to_account_info(),
            expanded.as_ref(),
        )
        .is_ok());
    let full_ready = unwinding && ctx.accounts.vault.unwind.mode == 2 && targets_met;
                                                                                  
                                                               
    let haircut = if full_ready { 10_000 } else { params.upnl_haircut_bps };
    require!(now >= scheduled_end || full_ready, FlpError::EpochNotEnded);
    let ineligible_reason: u8 = if !targets_met {
        2
    } else if !(scheduled || full_ready) {
        1
    } else {
        0
    };
    let eligible = ineligible_reason == 0;
    let authority = ctx.accounts.authority.to_account_info();
    let idle = ctx.accounts.idle_account.to_account_info();
    let bump = [ctx.accounts.vault.authority_bump];
    let seeds = authority_seeds(&key, &bump);

                                                                                  
    let mut before = compute_projected_vault_nav(
        &ctx.accounts.vault,
        &ctx.accounts.fermi,
        &authority,
        &idle,
        health, queues,
        haircut,
    )?;
    let supply = pricing_supply(ctx.accounts.share_mint.supply, &ctx.accounts.vault)?;
    let fees = FeeAccrual::calculate(&ctx.accounts.vault, before.snapshot.nav_low, supply, now)?;
    let mut pricing = PricingSnapshot::new(before.snapshot, fees.supply)?;
    if full_ready {
        pricing.withdrawal_ratio = math::recovery_withdraw_ratio(fees.supply, before.snapshot.nav_low)?;
    }
    apply_fee_accrual(
        &mut ctx.accounts.vault,
        fees,
        now,
        &ctx.accounts.token_2022_program.to_account_info(),
        &ctx.accounts.share_mint.to_account_info(),
        &ctx.accounts.fee_share_account.to_account_info(),
        &authority,
    )?;
                                                                                
                                                                               
    let cap_exceeded = params.tvl_cap > 0
        && pricing.nav.nav_mid.max(0) as u128
            + ctx.accounts.vault.pre_deposit_total as u128
            + pending.deposits_gross as u128
            > params.tvl_cap as u128;
    let hold = pending.deposits_gross > 0
        && (unwinding
            || cap_exceeded
            || pricing.nav.nav_high <= 0);
    let dep_ratio = if hold {
        (0, 0)
    } else if pending.deposits_gross == 0 {
        (0, 1)
    } else {
        pricing.deposit_ratio()?
    };
    let dep_shares = if hold {
        0
    } else {
        math::apply_ratio_floor(pending.deposits_net, dep_ratio)?
    };
    let (dep_gross, dep_net) = if hold {
        (0, 0)
    } else {
        (pending.deposits_gross, pending.deposits_net)
    };

                                                                                    
    transfer_quote(&ctx.accounts.fermi.token_program.to_account_info(),
        &ctx.accounts.deposit_escrow.to_account_info(), &idle, &authority, &[&seeds], dep_gross)?;
    before.include_idle_quote(dep_gross)?;
    let mut current = before;
    let record = &mut ctx.accounts.epoch_record;
    record.vault = key;
    record.epoch = e;
    record.bump = ctx.bumps["epoch_record"];
    record.closed_at = now;
    record.closed_slot = pricing.nav.slot;
    record.nav = pricing.nav;
    record.supply = pricing.supply;
    record.mgmt_fee_shares = fees.management;
    record.perf_fee_shares = fees.performance;
    record.dep_gross = pending.deposits_gross;
    record.dep_net = pending.deposits_net;
    record.dep_num = dep_ratio.0;
    record.dep_den = dep_ratio.1;
    record.dep_shares = dep_shares;
    record.wd_shares = pending.withdraw_shares;
    record.wd_status = if record.wd_shares == 0 {
        WithdrawStatus::None
    } else {
        WithdrawStatus::Deferred
    } as u8;
    record.wd_fee_bps = params.withdraw_fee_bps;
                                                                  
    record.wd_close_num = pricing.withdrawal_ratio.0;
    record.wd_close_den = pricing.withdrawal_ratio.1;
    record.next_deferred = NONE_EPOCH;
    if record.wd_shares > 0 {
        if let Some(tail) = old.tail_index {
            old.records[tail].1.next_deferred = e;
        } else {
                                                                             
            ctx.accounts.vault.deferred_head = e;
            ctx.accounts.vault.clear_head_skip();
        }
        ctx.accounts.vault.deferred_tail = e;
    }
    let mut paid = 0u8;
    if eligible {
        for i in 0..old.prefix_len {
            if !pay_batch(
                &mut ctx.accounts.vault,
                &mut old.records[i].1,
                &ctx.accounts.fermi,
                &ctx.accounts.authority,
                &ctx.accounts.redemption_escrow,
                &ctx.accounts.idle_account,
                health,
                &pricing,
                &mut current,
                now,
                e,
                targets_ai,
                haircut,
            )? {
                break;
            }
            paid += 1;
        }
        if (paid as usize) < count.max(DEFAULT_EQUITY_BATCHES)
            && ctx.accounts.vault.deferred_head == e
            && ctx.accounts.epoch_record.wd_status == WithdrawStatus::Deferred as u8
        {
            if pay_batch(
                &mut ctx.accounts.vault,
                &mut ctx.accounts.epoch_record,
                &ctx.accounts.fermi,
                &ctx.accounts.authority,
                &ctx.accounts.redemption_escrow,
                &ctx.accounts.idle_account,
                health,
                &pricing,
                &mut current,
                now,
                e,
                targets_ai,
                haircut,
            )? {
                paid += 1;
            }
        }
    }

                                                                              
    old.write_back()?;
    let vault = &mut ctx.accounts.vault;
    let record = &ctx.accounts.epoch_record;
    if record.wd_status == WithdrawStatus::Deferred as u8 {
        let payout = BatchPayout::new(
            record.wd_shares,
            pricing.withdrawal_ratio,
            record.wd_fee_bps,
        )?;
        emit_event!(FlpWithdrawDeferred {
            vault: key,
            epoch: e,
            shares: record.wd_shares,
            required: payout.net,
            spot_quote: pricing.nav.spot_quote,
            init_ratio_after_bps: current.snapshot.init_ratio_bps,
            reason: 6
        });
    }
    if hold {
        vault.held_deposits = vault
            .held_deposits
            .checked_add(pending.deposits_gross)
            .ok_or(FlpError::MathOverflow)?;
        emit_event!(FlpDepositsHeld {
            vault: key,
            epoch: e,
            gross: pending.deposits_gross,
            count: pending.n_deposits
        });

    }
    vault.owed_unminted_shares = vault
        .owed_unminted_shares
        .checked_add(dep_shares)
        .ok_or(FlpError::MathOverflow)?;
    vault.stats.total_deposited_gross = vault
        .stats
        .total_deposited_gross
        .checked_add(dep_gross)
        .ok_or(FlpError::MathOverflow)?;
    vault.stats.net_deposits = vault
        .stats
        .net_deposits
        .checked_add(i64::try_from(dep_gross).map_err(|_| FlpError::MathOverflow)?)
        .ok_or(FlpError::MathOverflow)?;
    vault.last_nav = current.snapshot;
    let price_low = math::price_e12(pricing.nav.nav_low, pricing.supply)?;
    if now - vault.breaker_ref_ts >= params.breaker_window_s as i64 {
                                                                               
                                                                                    
        if super::keeper::breaker_conditions(
            &params,
            vault.breaker_ref_price_e12,
            price_low,
            pricing.nav.init_ratio_bps,
        ) {
            emit_event!(FlpBreakerConditionAtClose {
                vault: key,
                price_low_e12: price_low,
                ref_price_e12: vault.breaker_ref_price_e12,
                init_ratio_bps: pricing.nav.init_ratio_bps,
            });
        } else {
            vault.breaker_ref_price_e12 = price_low;
            vault.breaker_ref_ts = now;
        }
    }
    vault.clear_pending(e);
    vault.epoch = e.checked_add(1).ok_or(FlpError::MathOverflow)?;
    vault.epoch_started_at =
        if now < scheduled_end || now >= scheduled_end + params.epoch_duration_s as i64 {
            now
        } else {
            scheduled_end
        };
    vault.activate_epoch_timing();
    emit_event!(FlpNav {
        vault: key,
        nav: pricing.nav,
        supply: pricing.supply,
        price_low_e12: price_low
    });
    emit_event!(FlpEpochClosed {
        vault: key,
        epoch: e,
        nav: pricing.nav,
        supply: pricing.supply,
        dep_gross,
        dep_net,
        dep_shares,
        wd_shares: record.wd_shares,
        wd_status: record.wd_status,
        wd_gross: record.wd_gross,
        netted: 0,
        fermi_deposit: dep_gross,
        fermi_withdraw: 0,
    });
    emit_event!(FlpEquityClose {
        vault: key,
        epoch: e,
        scheduled_end,
        eligible,
        ineligible_reason,
        batches_funded: paid,
        remaining_head: vault.deferred_head,
        wd_num: pricing.withdrawal_ratio.0,
        wd_den: pricing.withdrawal_ratio.1
    });
    Ok(())
}

                                                                             
                                                                                
                                                                       
fn pay_batch<'info>(
    vault: &mut Account<'info, Vault>,
    record: &mut EpochRecord,
    fermi: &FermiVaultAccounts<'info>,
    authority: &UncheckedAccount<'info>,
    redemption: &Account<'info, TokenAccount>,
    idle: &UncheckedAccount<'info>,
    health: &[AccountInfo<'info>],
    pricing: &PricingSnapshot,
    current: &mut nav::NavComputation,
    now: i64,
    pricing_epoch: u64,
    targets_ai: Option<&AccountInfo<'info>>,
    haircut: u16,
) -> Result<bool> {
    require!(
        record.epoch == vault.deferred_head
            && record.vault == vault.key()
            && record.wd_status == WithdrawStatus::Deferred as u8,
        FlpError::NotDeferredHead
    );
    let payout = BatchPayout::new(
        record.wd_shares,
        pricing.withdrawal_ratio,
        record.wd_fee_bps,
    )?;
    let mut check = FundingCheck::calculate(
        current,
        nav::token_account_amount(&idle.to_account_info())?,
        &payout,
        &fermi.quote_bank_vault.to_account_info(),
        vault.is_unwinding() && vault.unwind.mode == 2,
    )?;
                                                                               
                                                                             
                                                                                 
                                                                                 
    let full_recovery = vault.is_unwinding() && vault.unwind.mode == 2;
    if pricing.nav.nav_low < 0 || (pricing.nav.nav_low == 0 && !full_recovery) {
        check.ok = false;
        check.reason = FundingStatusV2::NonpositiveNav as u8;
    }
    if check.ok && check.core > 0 {
        check.ok = false;
        check.reason = FundingStatusV2::CoreFundingRequired as u8;
    }
    if !check.ok {
                                                                                 
                                                                      
        vault.set_head_skip(check.reason, now);
        emit_event!(FlpEquityFundingSkipped {
            vault: vault.key(),
            epoch: record.epoch,
            pricing_epoch,
            reason: check.reason,
            required: payout.net,
            idle: check.idle,
            spot_quote: current.snapshot.spot_quote,
            init_ratio_after_bps: HEALTH_UNAVAILABLE,
        });
                                                                             
                                                                       
                                                                              
        if vault.unwind.mode == 1 && vault.unwind.batch_epoch == record.epoch
            && check.reason == 2 {
            let shortfall = cash_shortfall_bps(&check, current.snapshot.spot_quote);
            super::recovery::extend_proportional_targets(vault,
                &fermi.fermi_account.to_account_info(), check.reason, targets_ai, shortfall)?;
        }
        return Ok(false);
    }
    fund_batch(vault, record, fermi, authority, redemption, idle, health,
        pricing.withdrawal_ratio, &payout, &check, current, now, haircut)?;
    Ok(true)
}

                                                                      
fn cash_shortfall_bps(check: &FundingCheck, spot_quote: i64) -> u16 {
    if check.core == 0 { return 0; }
    let missing = check.core.saturating_sub(spot_quote.max(0) as u64);
    ((missing as u128 * 10_000) / check.core as u128) as u16
}

                                                                             
                                                     
#[allow(clippy::too_many_arguments)]
fn fund_batch<'info>(
    vault: &mut Account<'info, Vault>,
    record: &mut EpochRecord,
    fermi: &FermiVaultAccounts<'info>,
    authority: &UncheckedAccount<'info>,
    redemption: &Account<'info, TokenAccount>,
    idle: &UncheckedAccount<'info>,
    health: &[AccountInfo<'info>],
    ratio: (u128, u128),
    payout: &BatchPayout,
    check: &FundingCheck,
    current: &mut nav::NavComputation,
    now: i64,
    haircut: u16,
) -> Result<()> {
    let key = vault.key();
    let bump = [vault.authority_bump];
    let seeds = authority_seeds(&key, &bump);
    transfer_quote(
        &fermi.token_program.to_account_info(),
        &idle.to_account_info(),
        &redemption.to_account_info(),
        &authority.to_account_info(),
        &[&seeds],
        check.idle,
    )?;
    fermi_cpi::token_withdraw(
        &fermi.core(&authority.to_account_info()),
        &fermi.bank(),
        redemption.to_account_info(),
        health,
        &[&seeds],
        check.core,
    )?;
    if check.core > 0 {
        let floor = if vault.is_unwinding() && vault.unwind.mode == 2 { 0 }
            else { vault.params.min_post_withdraw_init_ratio_bps };
        fermi_cpi::assert_payout_health(&fermi.core(&authority.to_account_info()), health, floor)?;
    }
    record.wd_status = WithdrawStatus::Settled as u8;
    record.wd_num = ratio.0;
    record.wd_den = ratio.1;
    record.wd_gross = payout.gross;
    record.wd_fee = payout.fee;
    record.wd_settled_at = now;
    vault.locked_shares = vault
        .locked_shares
        .checked_sub(payout.shares)
        .ok_or(FlpError::MathOverflow)?;
    vault.deferred_head = record.next_deferred;
    vault.clear_head_skip();
    if vault.deferred_head == NONE_EPOCH {
        vault.deferred_tail = NONE_EPOCH;
    }
    vault.stats.total_withdrawn_net = vault
        .stats
        .total_withdrawn_net
        .checked_add(payout.net)
        .ok_or(FlpError::MathOverflow)?;
    vault.stats.net_deposits = vault
        .stats
        .net_deposits
        .checked_sub(i64::try_from(payout.net).map_err(|_| FlpError::MathOverflow)?)
        .ok_or(FlpError::MathOverflow)?;
                                                                                
                                                                    
    current.refresh_quote_after_flow(
        &fermi.fermi_account.to_account_info(),
        &fermi.quote_bank.to_account_info(),
        haircut,
    )?;
    current.include_idle_quote(nav::token_account_amount(&idle.to_account_info())?)?;
    emit_event!(FlpWithdrawSettled {
        vault: key,
        epoch: record.epoch,
        shares: payout.shares,
        gross: payout.gross,
        fee: payout.fee,
        wd_num: record.wd_num,
        wd_den: record.wd_den,
        deferred: true
    });
    Ok(())
}

#[derive(Accounts)]
pub struct CureDeferred<'info> {
    #[account(mut, has_one = redemption_escrow, has_one = share_mint)]
    pub vault: Box<Account<'info, Vault>>,
                                                                            
    #[account(mut)]
    pub head_record: UncheckedAccount<'info>,
                                   
    #[account(seeds = [AUTHORITY_SEED, vault.key().as_ref()], bump = vault.authority_bump)]
    pub authority: UncheckedAccount<'info>,
                                      
    pub signer: Signer<'info>,
    pub fermi: FermiVaultAccounts<'info>,
    #[account(mut)]
    pub redemption_escrow: Box<Account<'info, TokenAccount>>,
    #[account(mut)]
    pub share_mint: Box<InterfaceAccount<'info, IMint>>,
                                                                               
    #[account(mut)]
    pub fee_share_account: UncheckedAccount<'info>,
    pub token_2022_program: Program<'info, Token2022>,
                                                                                     
    #[account(mut)]
    pub idle_account: UncheckedAccount<'info>,
}

                                                                           
                                                                             
                                                              
                                                                                 
                                                                                
                                                                           
                         
pub fn cure_deferred<'info>(ctx: Context<'_, '_, '_, 'info, CureDeferred<'info>>) -> Result<()> {
    let now = now_ts()?;
    let key = ctx.accounts.vault.key();
    let vault = &ctx.accounts.vault;
    let who = ctx.accounts.signer.key();
    require!(
        who == vault.admin || (vault.delegate != Pubkey::default() && who == vault.delegate),
        FlpError::Unauthorized
    );
    require!(vault.status & (status::UNWIND | status::CLOSED) == 0, FlpError::WrongVaultMode);
    ctx.accounts.fermi.check(vault)?;
    let head = vault.deferred_head;
    require!(head != NONE_EPOCH, FlpError::NothingPending);
    let head_ai = ctx.accounts.head_record.to_account_info();
    let mut record = read_deferred(&head_ai, &key, head)?;
    let fermi_ai = ctx.accounts.fermi.fermi_account.to_account_info();
    let (health, queues) = split_event_queues(&fermi_ai, ctx.remaining_accounts)?;
    let authority = ctx.accounts.authority.to_account_info();
    let idle = ctx.accounts.idle_account.to_account_info();
    let haircut = vault.params.upnl_haircut_bps;
    let mut current = compute_projected_vault_nav(vault, &ctx.accounts.fermi, &authority, &idle, health, queues, haircut)?;
    let supply = pricing_supply(ctx.accounts.share_mint.supply, vault)?;
    let fees = FeeAccrual::calculate(vault, current.snapshot.nav_low, supply, now)?;
    let price = math::withdraw_ratio(fees.supply, current.snapshot.nav_low)?;
    let close = (record.wd_close_num, record.wd_close_den);
    require!(close.1 > 0, FlpError::WrongEpochRecord);
    let at_or_above = price
        .0
        .checked_mul(close.1)
        .zip(close.0.checked_mul(price.1))
        .map(|(now_side, close_side)| now_side >= close_side)
        .ok_or(FlpError::MathOverflow)?;
    require!(at_or_above, FlpError::CureBelowClosePrice);
    let payout = BatchPayout::new(record.wd_shares, price, record.wd_fee_bps)?;
    let check = FundingCheck::calculate(
        &mut current,
        nav::token_account_amount(&idle)?,
        &payout,
        &ctx.accounts.fermi.quote_bank_vault.to_account_info(),
        false,
    )?;
    require!(check.ok, FlpError::InsufficientLiquidity);
    require_ata(ctx.accounts.fee_share_account.key, &vault.fee_recipient, &vault.share_mint,
        &anchor_spl::token_2022::ID)?;
    apply_fee_accrual(&mut ctx.accounts.vault, fees, now,
        &ctx.accounts.token_2022_program.to_account_info(), &ctx.accounts.share_mint.to_account_info(),
        &ctx.accounts.fee_share_account.to_account_info(), &authority)?;
    let epoch = record.epoch;
    fund_batch(
        &mut ctx.accounts.vault,
        &mut record,
        &ctx.accounts.fermi,
        &ctx.accounts.authority,
        &ctx.accounts.redemption_escrow,
        &ctx.accounts.idle_account,
        health,
        price,
        &payout,
        &check,
        &mut current,
        now,
        haircut,
    )?;
    record.try_serialize(&mut &mut head_ai.try_borrow_mut_data()?[..])?;
    ctx.accounts.vault.last_nav = current.snapshot;
    emit_event!(FlpDeferredCured {
        vault: key,
        epoch,
        by: who,
        wd_num: price.0,
        wd_den: price.1,
    });
    Ok(())
}


#[derive(Accounts)]
pub struct FundHeadV2<'info> {
    #[account(mut, has_one = redemption_escrow, has_one = share_mint)]
    pub vault: Box<Account<'info, Vault>>,
    #[account(seeds = [FUNDING_WINDOW_SEED, vault.key().as_ref()], bump, has_one = vault)]
    pub funding_window: Box<Account<'info, FundingWindow>>,
                                                                             
    #[account(mut)]
    pub head_record: UncheckedAccount<'info>,
                                         
    #[account(seeds = [AUTHORITY_SEED, vault.key().as_ref()], bump = vault.authority_bump)]
    pub authority: UncheckedAccount<'info>,
    pub fermi: FermiVaultAccounts<'info>,
    #[account(mut)]
    pub redemption_escrow: Box<Account<'info, TokenAccount>>,
    #[account(mut)]
    pub share_mint: Box<InterfaceAccount<'info, IMint>>,
                                                                         
    #[account(mut)]
    pub fee_share_account: UncheckedAccount<'info>,
                                                                   
    #[account(mut)]
    pub idle_account: UncheckedAccount<'info>,
    pub token_2022_program: Program<'info, Token2022>,
}

pub fn fund_head_v2<'info>(ctx: Context<'_, '_, '_, 'info, FundHeadV2<'info>>) -> Result<()> {
    let now = now_ts()?;
    let key = ctx.accounts.vault.key();
    let vault = &ctx.accounts.vault;
    vault.require_trading_open()?;
    ctx.accounts.fermi.check(vault)?;
    ctx.accounts.funding_window.require_current(vault, key)?;
    require!(vault.deferred_head != NONE_EPOCH && vault.deferred_head <= ctx.accounts.funding_window.closed_epoch,
        FlpError::NotDeferredHead);
    let head_ai = ctx.accounts.head_record.to_account_info();
    let mut record = read_deferred(&head_ai, &key, vault.deferred_head)?;
    let (remaining, targets_ai) = super::capacity::split_optional_recovery_accounts(
        vault, &key, ctx.remaining_accounts, vault.is_unwinding())?;
    let fermi_ai = ctx.accounts.fermi.fermi_account.to_account_info();
    let (health, queues) = split_event_queues(&fermi_ai, remaining)?;
    if vault.is_unwinding() {
        require!(pending_vault_events(vault, &fermi_ai, health, queues)? == 0, FlpError::UnconsumedVaultFills);
        let expanded = super::capacity::read_targets(vault, targets_ai)?;
        require_recovery_targets(vault, &fermi_ai, expanded.as_ref())?;
    }
    let full_ready = vault.is_unwinding() && vault.unwind.mode == 2;
    require!(full_ready || ctx.accounts.funding_window.is_open(now), FlpError::InvalidFundingWindow);
    let authority = ctx.accounts.authority.to_account_info();
    let idle = ctx.accounts.idle_account.to_account_info();
    require_ata(ctx.accounts.fee_share_account.key, &vault.fee_recipient, &vault.share_mint,
        &anchor_spl::token_2022::ID)?;
    let haircut = if full_ready { 10_000 } else { vault.params.upnl_haircut_bps };
    let mut current = compute_projected_vault_nav(vault, &ctx.accounts.fermi, &authority, &idle, health, queues, haircut)?;
    let supply = pricing_supply(ctx.accounts.share_mint.supply, vault)?;
    let fees = FeeAccrual::calculate(vault, current.snapshot.nav_low, supply, now)?;
    let price = if full_ready { math::recovery_withdraw_ratio(fees.supply, current.snapshot.nav_low)? }
        else { math::withdraw_ratio(fees.supply, current.snapshot.nav_low)? };
    let payout = BatchPayout::new(record.wd_shares, price, record.wd_fee_bps)?;
    let check = FundingCheck::calculate(&mut current, nav::token_account_amount(&idle)?, &payout, &ctx.accounts.fermi.quote_bank_vault.to_account_info(), full_ready)?;
    require!(check.ok, FlpError::InsufficientLiquidity);
    apply_fee_accrual(&mut ctx.accounts.vault, fees, now, &ctx.accounts.token_2022_program.to_account_info(),
        &ctx.accounts.share_mint.to_account_info(), &ctx.accounts.fee_share_account.to_account_info(), &authority)?;
    fund_batch(&mut ctx.accounts.vault, &mut record, &ctx.accounts.fermi, &ctx.accounts.authority,
        &ctx.accounts.redemption_escrow, &ctx.accounts.idle_account, health, price, &payout, &check,
        &mut current, now, haircut)?;
    record.try_serialize(&mut &mut head_ai.try_borrow_mut_data()?[..])?;
    ctx.accounts.vault.last_nav = current.snapshot;
    Ok(())
}
