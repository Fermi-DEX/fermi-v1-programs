                                                                              
                                                                                 
use super::common::*;
use crate::{error::FlpError, events::*, fermi_cpi, math, nav, state::*};
use anchor_lang::prelude::*;
use anchor_spl::token::{Token, TokenAccount};
use anchor_spl::token_2022::{self, Token2022};
use anchor_spl::token_interface::{Mint as IMint, TokenAccount as ITokenAccount};
use fixed::types::I80F48;
use mango_v4::accounts_zerocopy::AccountInfoRef;
use mango_v4::state::{
    BookSide, EventQueue, FermiAccountFixed, FermiAccountLoader, OracleAccountInfos, PerpMarket,
    PlaceOrderType, SelfTradeBehavior, Side,
};

                                                                              
                                                                              
                                                                          
                                                                              
                                                       
pub fn stage_recovery_params(ctx: Context<super::AdminOnly>, params: RecoveryParams) -> Result<()> {
    params.validate_staged()?;
    let now = now_ts()?;
    let vault = &mut ctx.accounts.vault;
    require!(!vault.is_closed(), FlpError::WrongVaultMode);
    let delay = if vault.is_unwinding() {
        require!(
            params.only_widens(&vault.recovery_params),
            FlpError::RecoveryParamsNotWidening
        );
        vault.params.delegate_delay_s()
    } else {
        require!(
            params.grace_proportional_s as u64 >= vault.longest_close_cycle_s(),
            FlpError::RecoveryGraceTooShort
        );
        if !vault.is_seeded() {
            vault.recovery_params = params;
            vault.pending_recovery_eta = 0;
            emit_event!(FlpRecoveryParamsChanged {
                vault: vault.key(),
                action: 1,
                eta: now,
                params
            });
            return Ok(());
        }
        vault.parameter_delay_s()
    };
    vault.pending_recovery_params = params;
    vault.pending_recovery_eta = now.checked_add(delay).ok_or(FlpError::MathOverflow)?;
    emit_event!(FlpRecoveryParamsChanged {
        vault: vault.key(),
        action: 0,
        eta: vault.pending_recovery_eta,
        params
    });
    Ok(())
}
pub fn apply_recovery_params(ctx: Context<super::AdminOnly>) -> Result<()> {
    let vault = &mut ctx.accounts.vault;
    require!(!vault.is_closed(), FlpError::WrongVaultMode);
    require!(vault.pending_recovery_eta > 0, FlpError::NothingPending);
    require!(
        !vault.is_unwinding()
            || vault.pending_recovery_params.only_widens(&vault.recovery_params),
        FlpError::RecoveryParamsNotWidening
    );
    require!(
        now_ts()? >= vault.pending_recovery_eta,
        FlpError::TimelockActive
    );
    vault.recovery_params = vault.pending_recovery_params;
    vault.pending_recovery_eta = 0;
    emit_event!(FlpRecoveryParamsChanged {
        vault: vault.key(),
        action: 1,
        eta: now_ts()?,
        params: vault.recovery_params
    });
    Ok(())
}

#[derive(Accounts)]
pub struct TriggerUnwind<'info> {
    #[account(mut, has_one = share_mint)]
    pub vault: Box<Account<'info, Vault>>,
                                   
    #[account(seeds = [AUTHORITY_SEED, vault.key().as_ref()], bump = vault.authority_bump)]
    pub authority: UncheckedAccount<'info>,
    pub fermi: FermiEditAccounts<'info>,
    pub share_mint: Box<InterfaceAccount<'info, IMint>>,
    pub deferred_record: Option<Box<Account<'info, EpochRecord>>>,
    pub signer: Signer<'info>,
}

                                                                                 
                                                                                  
                                                                           
   
                                                                              
                                                                           
                                                
                                                                          
                                                                              
                                                                                
                                                                            
                          
pub fn trigger_unwind<'info>(
    ctx: Context<'_, '_, '_, 'info, TriggerUnwind<'info>>,
    full: bool,
) -> Result<()> {
    let now = now_ts()?;
    let key = ctx.accounts.vault.key();
    let vault = &ctx.accounts.vault;
    vault.require_trading_open()?;
    ctx.accounts.fermi.check(vault)?;
    vault.recovery_params.validate()?;
    let resize = !full && vault.unwind.mode == 1 && vault.deferred_head != vault.unwind.batch_epoch;
    require!(
        vault.unwind.mode == 0 || (full && vault.unwind.mode == 1) || resize,
        FlpError::WrongVaultMode
    );
    let privileged =
        ctx.accounts.signer.key() == vault.admin || ctx.accounts.signer.key() == vault.guardian;
    let mut fraction = (1u64, 1u64);
    let mut batch = NONE_EPOCH;
    if !(full && privileged) {
        let record = ctx
            .accounts
            .deferred_record
            .as_ref()
            .ok_or(FlpError::NotDeferredHead)?;
        require_keys_eq!(record.vault, key, FlpError::WrongEpochRecord);
        require!(
            record.epoch == vault.deferred_head
                && record.wd_status == WithdrawStatus::Deferred as u8,
            FlpError::NotDeferredHead
        );
        if privileged {
                                                                                  
            let grace = vault.recovery_params.grace_proportional_s;
            require!(
                now >= record
                    .closed_at
                    .checked_add(grace as i64)
                    .ok_or(FlpError::MathOverflow)?,
                FlpError::RecoveryGraceActive
            );
        } else {
            require!(!full || vault.unwind.mode == 1, FlpError::RecoveryReasonNotEligible);
            let at = vault
                .permissionless_trigger_at(full)
                .ok_or(FlpError::RecoveryReasonNotEligible)?;
            require!(now >= at, FlpError::RecoveryGraceActive);
        }
                                                                                  
                                                                              
                                                                               
                                                                             
        let supply = pricing_supply(ctx.accounts.share_mint.supply, vault)?;
        require!(supply > 0, FlpError::MathOverflow);
        fraction = if full {
            (1, 1)
        } else {
            (record.wd_shares.min(supply), supply)
        };
        batch = record.epoch;
    }
    begin_unwind(ctx, full, fraction, batch, now)
}

                                                                           
                                                                             
                                                                           
                                                       
pub fn trigger_partial_unwind<'info>(
    ctx: Context<'_, '_, '_, 'info, TriggerUnwind<'info>>,
    fraction_bps: u16,
) -> Result<()> {
    let now = now_ts()?;
    let vault = &ctx.accounts.vault;
    vault.require_trading_open()?;
    ctx.accounts.fermi.check(vault)?;
    vault.recovery_params.validate()?;
    let who = ctx.accounts.signer.key();
    require!(who == vault.admin || who == vault.guardian, FlpError::Unauthorized);
    require!(vault.unwind.mode == 0, FlpError::WrongVaultMode);
    require!((1..=10_000).contains(&fraction_bps), FlpError::InvalidParams);
    begin_unwind(ctx, false, (fraction_bps as u64, 10_000), NONE_EPOCH, now)
}

                                                                           
                                                                            
fn begin_unwind<'info>(
    ctx: Context<'_, '_, '_, 'info, TriggerUnwind<'info>>,
    full: bool,
    fraction: (u64, u64),
    batch: u64,
    now: i64,
) -> Result<()> {
    let key = ctx.accounts.vault.key();
    let vault = &ctx.accounts.vault;
    let (remaining, targets_ai) = super::capacity::split_recovery_accounts(vault, &key, ctx.remaining_accounts)?;
    let ai = ctx.accounts.fermi.fermi_account.to_account_info();
    let loader = AccountLoader::<FermiAccountFixed>::try_from(&ai)?;
    let positions: Vec<(u16, i64)> = {
        let account = loader.load_full()?;
        require_keys_eq!(
            account.fixed.owner,
            ctx.accounts.authority.key(),
            FlpError::FermiAccountMismatch
        );
        account
            .active_perp_positions()
            .map(|p| {
                p.base_position_lots
                    .checked_add(p.taker_base_lots)
                    .map(|lots| (p.market_index, lots))
                    .ok_or(FlpError::MathOverflow)
            })
            .collect::<std::result::Result<Vec<_>, _>>()?
    };
    let width = if !full && !positions.is_empty() && remaining.len() == positions.len() * 4 {
        4
    } else {
        3
    };
    require!(
        positions.len() <= MAX_VAULT_PERPS
            && remaining.len() == positions.len() * width,
        FlpError::WrongRecoveryMarket
    );
    let bump = [vault.authority_bump];
    let seeds = authority_seeds(&key, &bump);
    let signer: &[&[&[u8]]] = &[&seeds];
    let authority = ctx.accounts.authority.to_account_info();
    let core = ctx.accounts.fermi.core(&authority);
    fermi_cpi::set_delegate(&core, signer, Pubkey::default())?;
    let state = UnwindState {
        mode: if full { 2 } else { 1 },
        batch_epoch: batch,
        started_at: now,
        market_indexes: [u16::MAX; 4],
        target_base_lots: [0; 4],
    };
    let mut expanded = RecoveryTargets {
        vault: key, mode: state.mode, batch_epoch: batch, started_at: now,
        market_indexes: [u16::MAX; MAX_VAULT_PERPS],
        target_base_lots: [0; MAX_VAULT_PERPS],
    };
    for (slot, ((index, lots), accounts)) in positions
        .iter()
        .zip(remaining.chunks_exact(width))
        .enumerate()
    {
        let market = AccountLoader::<PerpMarket>::try_from(&accounts[0])?;
        require!(market.load()?.perp_market_index == *index, FlpError::WrongRecoveryMarket);
        cancel(&core, &accounts[0], &accounts[1], &accounts[2], signer)?;
        expanded.market_indexes[slot] = *index;
        let magnitude = lots.checked_abs().ok_or(FlpError::MathOverflow)? as u64;
        let reduce = math::to_u64(math::mul_div_ceil(
            magnitude as u128,
            fraction.0 as u128,
            fraction.1 as u128,
        )?)?;
        let target = i64::try_from(magnitude - reduce).map_err(|_| FlpError::MathOverflow)?;
        expanded.target_base_lots[slot] = if *lots < 0 { -target } else { target };

    }
    if !full {
                                                                            
                                                                             
                                                                             
                                                           
        let account = loader.load_full()?;
        for p in account.active_perp_positions() {
            let (bids, asks) = (p.bids_base_lots, p.asks_base_lots);
            require!(bids == 0 && asks == 0, FlpError::UnconsumedVaultFills);
        }
    }
    super::capacity::write_targets(targets_ai.ok_or(FlpError::WrongRecoveryMarket)?, &expanded)?;
    let vault = &mut ctx.accounts.vault;
    if full {
        vault.clear_delegate_restore();
    } else if vault.unwind.mode == 0 {
                                                                          
                                                                    
        let deposits_open = vault.status & status::DEPOSITS_PAUSED == 0;
        let delegate = vault.delegate;
        vault.set_delegate_restore(delegate, deposits_open);
    }
    vault.status |= status::UNWIND | status::DEPOSITS_PAUSED;
    vault.delegate = Pubkey::default();
    vault.pending_delegate = Pubkey::default();
    vault.pending_delegate_eta = 0;
    vault.unwind = state;
    emit_event!(FlpUnwindStarted {
        vault: key,
        full,
        batch_epoch: batch,
        started_at: now
    });
    Ok(())
}

                                                                             
                                                     
                                                                           
                                                                                
                                                                          
                                                                           
                                                                                
pub(super) fn extend_proportional_targets(
    vault: &mut Account<Vault>, account_ai: &AccountInfo, reason: u8,
    targets_ai: Option<&AccountInfo>, shortfall_bps: u16,
) -> Result<()> {
    require!(reason == 2, FlpError::WrongVaultMode);
    if extend_targets(vault, account_ai, targets_ai, shortfall_bps.clamp(5_000, 10_000))? {
        emit_event!(FlpUnwindTargetsExtended {
            vault: vault.key(), batch_epoch: vault.unwind.batch_epoch, reason,
            target_base_lots: vault.unwind.target_base_lots,
        });
    }
    Ok(())
}

fn extend_targets(vault: &mut Account<Vault>, account_ai: &AccountInfo,
    targets_ai: Option<&AccountInfo>, reduction_bps: u16) -> Result<bool> {
    require!(vault.unwind.mode == 1 && vault.is_unwinding(), FlpError::WrongVaultMode);
    require!((5_000..=10_000).contains(&reduction_bps), FlpError::InvalidParams);
    let keep_bps = 10_000 - i64::from(reduction_bps);
    let mut expanded = super::capacity::read_targets(vault, targets_ai)?;
    require_recovery_targets(vault, account_ai, expanded.as_ref())?;
    let loader = AccountLoader::<FermiAccountFixed>::try_from(account_ai)?;
    let account = loader.load_full()?;
    let mut changed = false;
    for p in account.active_perp_positions() {
        let current = p.base_position_lots;
                                                                           
        let target = current
            .checked_mul(keep_bps)
            .ok_or(FlpError::MathOverflow)?
            / 10_000;
        changed |= target != super::capacity::target_for(vault, expanded.as_ref(), p.market_index)?;
        if let Some(t) = &mut expanded {
            let slot = t.market_indexes.iter().position(|i| *i == p.market_index)
                .ok_or(FlpError::WrongRecoveryMarket)?;
            t.target_base_lots[slot] = target;
        }

    }
    if let (Some(ai), Some(t)) = (targets_ai, expanded.as_ref()) { super::capacity::write_targets(ai, t)?; }
    Ok(changed)
}

#[derive(Accounts)]
pub struct ExtendPartialUnwindV2<'info> {
    #[account(mut)]
    pub vault: Box<Account<'info, Vault>>,
    pub signer: Signer<'info>,
    pub fermi: FermiEditAccounts<'info>,
                                                                  
    #[account(mut)]
    pub head_record: UncheckedAccount<'info>,
}
pub fn extend_partial_unwind_v2<'info>(ctx: Context<'_, '_, '_, 'info, ExtendPartialUnwindV2<'info>>,
    reduction_bps: u16) -> Result<()> {
    let vault = &ctx.accounts.vault;
    require!(ctx.accounts.signer.key() == vault.admin || ctx.accounts.signer.key() == vault.guardian,
        FlpError::Unauthorized);
    require!(vault.deferred_head != NONE_EPOCH && vault.unwind.batch_epoch == vault.deferred_head,
        FlpError::NotDeferredHead);
    ctx.accounts.fermi.check(vault)?;
    super::close_epoch::read_deferred(&ctx.accounts.head_record.to_account_info(), &vault.key(), vault.deferred_head)?;
    let (remaining, targets) = super::capacity::split_recovery_accounts(vault, &vault.key(), ctx.remaining_accounts)?;
    let account = ctx.accounts.fermi.fermi_account.to_account_info();
    let (health, queues) = split_event_queues(&account, remaining)?;
    require!(pending_vault_events(vault, &account, health, queues)? == 0, FlpError::UnconsumedVaultFills);
    require!(extend_targets(&mut ctx.accounts.vault, &account, targets, reduction_bps)?, FlpError::NothingPending);
    emit_event!(FlpOperatorUnwindTargetsExtendedV2 {
        vault: ctx.accounts.vault.key(), batch_epoch: ctx.accounts.vault.deferred_head,
        by: ctx.accounts.signer.key(), reduction_bps,
    });
    Ok(())
}

fn cancel<'info>(
    core: &fermi_cpi::FermiCore<'info>,
    market: &AccountInfo<'info>,
    bids: &AccountInfo<'info>,
    asks: &AccountInfo<'info>,
    signer: &[&[&[u8]]],
) -> Result<()> {
    fermi_cpi::invoke(core.program.clone(), &mango_v4::accounts::PerpCancelAllOrders {
        group: *core.group.key, account: *core.account.key, owner: *core.authority.key,
        perp_market: *market.key, bids: *bids.key, asks: *asks.key,
    }, &[core.group.clone(), core.account.clone(), core.authority.clone(), market.clone(), bids.clone(), asks.clone()],
    &[], signer, &mango_v4::instruction::PerpCancelAllOrders { limit: u8::MAX })
}

#[derive(Accounts)]
pub struct CancelOrders<'info> {
    pub vault: Box<Account<'info, Vault>>,
                                   
    #[account(seeds = [AUTHORITY_SEED, vault.key().as_ref()], bump = vault.authority_bump)]
    pub authority: UncheckedAccount<'info>,
    pub fermi: FermiEditAccounts<'info>,
    pub signer: Signer<'info>,
}

                                                                                
                                                                                
                                                                              
                                                                           
                                                                             
                                                                              
                                           
pub fn cancel_orders<'info>(ctx: Context<'_, '_, '_, 'info, CancelOrders<'info>>) -> Result<()> {
    let key = ctx.accounts.vault.key();
    let vault = &ctx.accounts.vault;
    ctx.accounts.fermi.check(vault)?;
    let who = ctx.accounts.signer.key();
    require!(
        who == vault.admin || who == vault.guardian || vault.delegate == Pubkey::default(),
        FlpError::Unauthorized
    );
    let triples = ctx.remaining_accounts;
    let markets = MAX_VAULT_PERPS;
    require!(
        !triples.is_empty() && triples.len() % 3 == 0 && triples.len() <= 3 * markets,
        FlpError::WrongRecoveryMarket
    );
    let bump = [vault.authority_bump];
    let seeds = authority_seeds(&key, &bump);
    let signer: &[&[&[u8]]] = &[&seeds];
    let core = ctx.accounts.fermi.core(&ctx.accounts.authority.to_account_info());
    let mut market_indexes = [u16::MAX; MAX_VAULT_PERPS];
    for (slot, accounts) in triples.chunks_exact(3).enumerate() {
        market_indexes[slot] = AccountLoader::<PerpMarket>::try_from(&accounts[0])?
            .load()?
            .perp_market_index;
        cancel(&core, &accounts[0], &accounts[1], &accounts[2], signer)?;
    }
    if triples.len() <= 12 {
        let mut legacy = [u16::MAX; 4];
        legacy.copy_from_slice(&market_indexes[..4]);
        emit_event!(FlpOrdersCancelled {
            vault: key,
            by: who,
            market_indexes: legacy
        });
    } else {
        emit_event!(FlpOrdersCancelledExpanded {
            vault: key,
            by: who,
            market_indexes
        });
    }
    Ok(())
}

#[derive(Accounts)]
pub struct UnwindStep<'info> {
    pub vault: Box<Account<'info, Vault>>,
                                   
    #[account(seeds = [AUTHORITY_SEED, vault.key().as_ref()], bump = vault.authority_bump)]
    pub authority: UncheckedAccount<'info>,
    pub fermi: FermiEditAccounts<'info>,
    #[account(mut, constraint = perp_market.load()?.group == vault.group @ FlpError::WrongRecoveryMarket,
        has_one = bids, has_one = asks, has_one = event_queue, has_one = oracle)]
    pub perp_market: AccountLoader<'info, PerpMarket>,
    #[account(mut)]
    pub bids: AccountLoader<'info, BookSide>,
    #[account(mut)]
    pub asks: AccountLoader<'info, BookSide>,
    #[account(mut)]
    pub event_queue: AccountLoader<'info, EventQueue>,
                                                                                    
    pub oracle: UncheckedAccount<'info>,
}

                                                                               
                                                                                    
pub fn unwind_step<'info>(ctx: Context<'_, '_, '_, 'info, UnwindStep<'info>>, max_base_lots: i64) -> Result<()> {
    let vault = &ctx.accounts.vault;
    let (health, targets_ai) = super::capacity::split_recovery_accounts(vault, &vault.key(), ctx.remaining_accounts)?;
    let expanded = super::capacity::read_targets(vault, targets_ai)?;
    require!(
        vault.status & status::UNWIND != 0 && vault.unwind.mode > 0,
        FlpError::WrongVaultMode
    );
    ctx.accounts.fermi.check(vault)?;
                                                                              
                                                                              
    fermi_cpi::invoke(ctx.accounts.fermi.fermi_program.to_account_info(),
        &mango_v4::accounts::PerpUpdateFunding {
            group: ctx.accounts.fermi.group.key(), perp_market: ctx.accounts.perp_market.key(),
            bids: ctx.accounts.bids.key(), asks: ctx.accounts.asks.key(), oracle: ctx.accounts.oracle.key(),
        }, &[ctx.accounts.fermi.group.to_account_info(), ctx.accounts.perp_market.to_account_info(),
            ctx.accounts.bids.to_account_info(), ctx.accounts.asks.to_account_info(), ctx.accounts.oracle.to_account_info()],
        &[], &[], &mango_v4::instruction::PerpUpdateFunding {})?;
    let m = ctx.accounts.perp_market.load()?;
    let index = m.perp_market_index;
    let target = super::capacity::target_for(vault, expanded.as_ref(), index)?;
    let ai = ctx.accounts.fermi.fermi_account.to_account_info();
    let loader = AccountLoader::<FermiAccountFixed>::try_from(&ai)?;
    let position = {
        let account = loader.load_full()?;
        let p = account.perp_position(index)?;
        require!(
            p.bids_base_lots == 0
                && p.asks_base_lots == 0
                && p.taker_base_lots == 0
                && p.taker_quote_lots == 0,
            FlpError::RecoveryNotComplete
        );
        *p
    };
    let current = position.base_position_lots;
    let magnitude = current.checked_abs().ok_or(FlpError::MathOverflow)?;
    require!(
        current.signum() == target.signum() || target == 0 || current == 0,
        FlpError::WrongRecoveryMarket
    );
    let needed = magnitude.saturating_sub(target.checked_abs().ok_or(FlpError::MathOverflow)?);
    require!(needed > 0, FlpError::NothingPending);
    let side = if current > 0 { Side::Ask } else { Side::Bid };
    let clock = Clock::get()?;
    let oracle_ref = AccountInfoRef::borrow(ctx.accounts.oracle.as_ref())?;
    let price = m.oracle_price(
        &OracleAccountInfos::from_reader(&oracle_ref),
        Some(clock.slot),
    )?;
    require!(
        price > 0 && m.base_lot_size > 0 && m.quote_lot_size > 0,
        FlpError::InvalidParams
    );
    let policy = vault.recovery_params;
    let one_lot_quote = price * I80F48::from_num(m.base_lot_size);
    let cap = (I80F48::from_num(policy.max_step_quote) / one_lot_quote)
        .checked_to_num::<i64>()
        .ok_or(FlpError::MathOverflow)?;
    let policy_lots = needed.min(cap);
    require!(policy_lots > 0, FlpError::BelowMinimum);
    let band = I80F48::from_num(policy.oracle_band_bps) / I80F48::from_num(10_000);
    let native_limit = price
        * if side == Side::Ask {
            I80F48::ONE - band
        } else {
            I80F48::ONE + band
        };
    let limit_fixed =
        native_limit * I80F48::from_num(m.base_lot_size) / I80F48::from_num(m.quote_lot_size);
    let price_lots = (if side == Side::Ask {
        limit_fixed.ceil()
    } else {
        limit_fixed.floor()
    })
    .checked_to_num::<i64>()
    .ok_or(FlpError::MathOverflow)?;
    let max_quote_lots = i64::try_from(policy.max_step_quote / m.quote_lot_size as u64)
        .map_err(|_| FlpError::MathOverflow)?;
    require!(price_lots > 0 && max_quote_lots > 0, FlpError::BelowMinimum);
    let policy_lots = policy_lots.min(max_quote_lots / price_lots);
    let flagged = {
        let account = loader.load_full()?;
        require!(!account.fixed.is_in_health_region(), FlpError::WrongVaultMode);
        account.fixed.being_liquidated()
    };
    require!(max_base_lots > 0, FlpError::InvalidParams);
    let lots = max_base_lots.min(policy_lots);
    require!(lots > 0, FlpError::BelowMinimum);
    drop(oracle_ref);
    drop(m);
    let key = vault.key();
    let bump = [vault.authority_bump];
    let seeds = authority_seeds(&key, &bump);
    if flagged {
        fermi_cpi::assert_liquidation_end(&ctx.accounts.fermi.core(&ctx.accounts.authority.to_account_info()), health)?;
    }
    fermi_cpi::invoke(ctx.accounts.fermi.fermi_program.to_account_info(),
        &mango_v4::accounts::PerpPlaceOrder {
            group: ctx.accounts.fermi.group.key(), account: *ai.key, owner: ctx.accounts.authority.key(),
            perp_market: ctx.accounts.perp_market.key(), bids: ctx.accounts.bids.key(),
            asks: ctx.accounts.asks.key(), event_queue: ctx.accounts.event_queue.key(), oracle: ctx.accounts.oracle.key(),
        }, &[ctx.accounts.fermi.group.to_account_info(), ai, ctx.accounts.authority.to_account_info(),
            ctx.accounts.perp_market.to_account_info(), ctx.accounts.bids.to_account_info(),
            ctx.accounts.asks.to_account_info(), ctx.accounts.event_queue.to_account_info(), ctx.accounts.oracle.to_account_info()],
        health, &[&seeds], &mango_v4::instruction::PerpPlaceOrderV2 {
            side, price_lots, max_base_lots: lots, max_quote_lots, client_order_id: 0,
            order_type: PlaceOrderType::ImmediateOrCancel, self_trade_behavior: SelfTradeBehavior::AbortTransaction,
            reduce_only: true, expiry_timestamp: 0, limit: 20,
        })?;
                                                                               
                                                                              
    let taker_lots = { loader.load_full()?.perp_position(index)?.taker_base_lots };
    let filled = taker_lots != 0;
    require!(filled, FlpError::RecoveryStepNoFill);
    emit_event!(FlpUnwindStep {
        vault: key,
        market_index: index,
        base_lots_before: current,
        target_base_lots: target,
        max_base_lots: lots,
        price_lots
    });
    Ok(())
}

#[derive(Accounts)]
pub struct FinishUnwind<'info> {
    #[account(mut, has_one = share_mint, has_one = redemption_escrow)]
    pub vault: Box<Account<'info, Vault>>,
                                   
    #[account(seeds = [AUTHORITY_SEED, vault.key().as_ref()], bump = vault.authority_bump)]
    pub authority: UncheckedAccount<'info>,
    pub fermi: FermiVaultAccounts<'info>,
    pub share_mint: Box<InterfaceAccount<'info, IMint>>,
    #[account(mut)]
    pub redemption_escrow: Box<Account<'info, TokenAccount>>,
                                                                                
                                                            
    #[account(mut)]
    pub idle_account: UncheckedAccount<'info>,
}

                                                                             
                                                                                 
pub fn finish_unwind<'info>(ctx: Context<'_, '_, '_, 'info, FinishUnwind<'info>>) -> Result<()> {
    let key = ctx.accounts.vault.key();
    let vault = &ctx.accounts.vault;
    let (health, targets_ai) = super::capacity::split_recovery_accounts(vault, &key, ctx.remaining_accounts)?;
    let expanded = super::capacity::read_targets(vault, targets_ai)?;
    require!(vault.status & status::UNWIND != 0, FlpError::WrongVaultMode);
    ctx.accounts.fermi.check(vault)?;
    let ai = ctx.accounts.fermi.fermi_account.to_account_info();
    require_recovery_targets(vault, &ai, expanded.as_ref())?;
    require!(
        vault.deferred_head == NONE_EPOCH && vault.locked_shares == 0,
        FlpError::RecoveryNotComplete
    );
    let full = vault.unwind.mode == 2;
    if full {
                                                                             
                                                                          
                                                                                  
        let (reserve, forfeited) = {
            let loader = AccountLoader::<FermiAccountFixed>::try_from(&ai)?;
            let account = loader.load_full()?;
            let (mut reserve, mut forfeited) = (0u64, 0u64);
            for p in account.active_perp_positions() {
                let quote = p.quote_position_native;
                let (sum, part) = if quote < 0 {
                    (&mut reserve, (-quote).ceil())
                } else {
                    (&mut forfeited, quote.floor())
                };
                *sum = sum
                    .checked_add(part.checked_to_num::<u64>().ok_or(FlpError::MathOverflow)?)
                    .ok_or(FlpError::MathOverflow)?;
            }
            (reserve, forfeited)
        };
                                                                            
                                                                               
        let mut nav = compute_vault_nav(
            vault,
            &ctx.accounts.fermi,
            &ctx.accounts.authority.to_account_info(),
            &ctx.accounts.idle_account.to_account_info(),
            health,
            0,
        )?;
        require!(nav.snapshot.spot_quote >= 0, FlpError::RecoveryNotComplete);
        let amount = (nav.snapshot.spot_quote as u64)
            .checked_sub(reserve)
            .ok_or(FlpError::RecoveryNotComplete)?;
        let idle = nav::token_account_amount(&ctx.accounts.idle_account.to_account_info())?;
        let terminal_assets = amount.checked_add(idle).ok_or(FlpError::MathOverflow)?;
        let bump = [vault.authority_bump];
        let seeds = authority_seeds(&key, &bump);
        transfer_quote(
            &ctx.accounts.fermi.token_program.to_account_info(),
            &ctx.accounts.idle_account.to_account_info(),
            &ctx.accounts.redemption_escrow.to_account_info(),
            &ctx.accounts.authority.to_account_info(),
            &[&seeds],
            idle,
        )?;
        fermi_cpi::token_withdraw(
            &ctx.accounts
                .fermi
                .core(&ctx.accounts.authority.to_account_info()),
            &ctx.accounts.fermi.bank(),
            ctx.accounts.redemption_escrow.to_account_info(),
            health,
            &[&seeds],
            amount,
        )?;
        if amount > 0 {
            fermi_cpi::assert_payout_health(&ctx.accounts.fermi.core(&ctx.accounts.authority.to_account_info()), health, 0)?;
        }
        let supply = pricing_supply(ctx.accounts.share_mint.supply, vault)?;
        let vault = &mut ctx.accounts.vault;
        vault.terminal_assets = terminal_assets;
        vault.terminal_supply = supply;
        vault.last_nav = nav.snapshot;
        vault.status |= status::CLOSED | status::DEPOSITS_PAUSED;
        emit_event!(FlpTerminalResidual {
            vault: key,
            reserved_quote: reserve,
            forfeited_quote: forfeited,
        });
    }
    let (restore, deposits_open) = ctx.accounts.vault.delegate_restore();
    let breaker = ctx.accounts.vault.status & status::BREAKER != 0;
    if !full && !breaker && restore != Pubkey::default() {
                                                                             
                                                         
        let bump = [ctx.accounts.vault.authority_bump];
        let seeds = authority_seeds(&key, &bump);
        fermi_cpi::set_delegate(
            &ctx.accounts.fermi.core(&ctx.accounts.authority.to_account_info()),
            &[&seeds],
            restore,
        )?;
        let vault = &mut ctx.accounts.vault;
        vault.delegate = restore;
        if deposits_open {
            vault.status &= !status::DEPOSITS_PAUSED;
        }
        emit_event!(FlpDelegateChanged {
            vault: key,
            old: Pubkey::default(),
            new: restore,
            action: 1,
            eta: now_ts()?,
        });
    }
    let vault = &mut ctx.accounts.vault;
    vault.clear_delegate_restore();
    vault.status &= !status::UNWIND;
    vault.unwind.mode = 0;
                                                                             
                                                                               
                                                
    emit_event!(FlpUnwindFinished {
        vault: key,
        closed: full,
        terminal_assets: vault.terminal_assets,
        terminal_supply: vault.terminal_supply
    });
    Ok(())
}

                                                                                
                                                                                  
                                                                         
                                                                               
                                                                       
                                                                            
                                                                       
pub fn abort_unwind<'info>(ctx: Context<'_, '_, '_, 'info, super::AdminOnly<'info>>) -> Result<()> {
    let key = ctx.accounts.vault.key();
    let vault = &ctx.accounts.vault;
    require!(vault.is_unwinding() && !vault.is_closed(), FlpError::WrongVaultMode);
    let (rest, targets_ai) =
        super::capacity::split_recovery_accounts(vault, &key, ctx.remaining_accounts)?;
    require!(rest.is_empty(), FlpError::WrongRecoveryMarket);
    let (mode, batch_epoch) = (vault.unwind.mode, vault.unwind.batch_epoch);
    if let Some(ai) = targets_ai {
        super::capacity::write_targets(ai, &RecoveryTargets {
            vault: key,
            mode: 0,
            batch_epoch: NONE_EPOCH,
            started_at: 0,
            market_indexes: [u16::MAX; MAX_VAULT_PERPS],
            target_base_lots: [0; MAX_VAULT_PERPS],
        })?;
    }
    let vault = &mut ctx.accounts.vault;
    vault.status &= !status::UNWIND;
    vault.status |= status::DEPOSITS_PAUSED;
    vault.unwind = UnwindState::default();
    vault.delegate = Pubkey::default();
    vault.clear_delegate_restore();
    emit_event!(FlpUnwindAborted {
        vault: key,
        mode,
        batch_epoch
    });
    Ok(())
}

#[derive(Accounts)]
pub struct TerminalRedeem<'info> {
    #[account(mut, has_one = share_mint, has_one = redemption_escrow)]
    pub vault: Box<Account<'info, Vault>>,
    #[account(mut, has_one = vault, has_one = user,
        seeds = [DEPOSITOR_SEED, vault.key().as_ref(), user.key().as_ref()], bump = depositor.bump)]
    pub depositor: Box<Account<'info, Depositor>>,
                                   
    #[account(seeds = [AUTHORITY_SEED, vault.key().as_ref()], bump = vault.authority_bump)]
    pub authority: UncheckedAccount<'info>,
    #[account(mut)]
    pub user: Signer<'info>,
    #[account(mut, token::authority = user, token::mint = vault.quote_mint)]
    pub user_quote_account: Box<Account<'info, TokenAccount>>,
    #[account(mut, token::authority = user, token::mint = share_mint)]
    pub user_share_account: Box<InterfaceAccount<'info, ITokenAccount>>,
    #[account(mut)]
    pub share_mint: Box<InterfaceAccount<'info, IMint>>,
    #[account(mut)]
    pub redemption_escrow: Box<Account<'info, TokenAccount>>,
    pub token_program: Program<'info, Token>,
    pub token_2022_program: Program<'info, Token2022>,
    pub system_program: Program<'info, System>,
}

                                                                           
                                                                               
pub fn terminal_redeem(ctx: Context<TerminalRedeem>, shares: u64) -> Result<()> {
    let key = ctx.accounts.vault.key();
    let vault = &mut ctx.accounts.vault;
    require!(vault.status & status::CLOSED != 0, FlpError::WrongVaultMode);
                                                                            
                                                                
    require!(
        shares > 0 && vault.terminal_supply > 0,
        FlpError::BelowMinimum
    );
    let payout = math::to_u64(math::mul_div_floor(
        shares as u128,
        vault.terminal_assets as u128,
        vault.terminal_supply as u128,
    )?)?;
    token_2022::burn(
        CpiContext::new(
            ctx.accounts.token_2022_program.to_account_info(),
            token_2022::Burn {
                mint: ctx.accounts.share_mint.to_account_info(),
                from: ctx.accounts.user_share_account.to_account_info(),
                authority: ctx.accounts.user.to_account_info(),
            },
        ),
        shares,
    )?;
    let bump = [vault.authority_bump];
    let seeds = authority_seeds(&key, &bump);
    transfer_quote(
        &ctx.accounts.token_program.to_account_info(),
        &ctx.accounts.redemption_escrow.to_account_info(),
        &ctx.accounts.user_quote_account.to_account_info(),
        &ctx.accounts.authority.to_account_info(),
        &[&seeds],
        payout,
    )?;
    let d = &mut ctx.accounts.depositor;

    crate::accounting::record_immediate_redemption(vault, d, shares, payout, now_ts()?)?;
    emit_event!(FlpTerminalRedeemed {
        vault: key,
        user: d.user,
        shares,
        payout
    });
    Ok(())
}

