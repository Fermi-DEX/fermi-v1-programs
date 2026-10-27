use anchor_lang::prelude::*;
use anchor_spl::token_interface::Mint as IMint;
use mango_v4::state::PerpMarket;

use super::common::*;
use crate::error::FlpError;
use crate::events::*;
use crate::math;
use crate::fermi_cpi;
use crate::state::*;

#[derive(Accounts)]
pub struct DeactivatePerpPosition<'info> {
    pub vault: Box<Account<'info, Vault>>,
                                                                    
    #[account(seeds = [AUTHORITY_SEED, vault.key().as_ref()], bump = vault.authority_bump)]
    pub authority: UncheckedAccount<'info>,
    pub fermi: FermiEditAccounts<'info>,
    #[account(constraint = perp_market.load()?.group == vault.group @ FlpError::WrongRecoveryMarket)]
    pub perp_market: AccountLoader<'info, PerpMarket>,
}

                                                                               
                                                                             
                                                                            
                                                                             
                                                                                
                                                       
pub fn deactivate_perp_position<'info>(
    ctx: Context<'_, '_, '_, 'info, DeactivatePerpPosition<'info>>,
) -> Result<()> {
    let vault_key = ctx.accounts.vault.key();
    ctx.accounts.fermi.check(&ctx.accounts.vault)?;
    let market_index = ctx.accounts.perp_market.load()?.perp_market_index;
    let bump = [ctx.accounts.vault.authority_bump];
    let seeds = authority_seeds(&vault_key, &bump);
    fermi_cpi::perp_deactivate_position(
        &ctx.accounts.fermi.core(&ctx.accounts.authority.to_account_info()),
        ctx.accounts.perp_market.to_account_info(),
        &[&seeds],
    )?;
    emit_event!(FlpPerpPositionDeactivated {
        vault: vault_key,
        market_index,
    });
    Ok(())
}

#[derive(Accounts)]
pub struct SweepIdle<'info> {
                                                                
    pub head_record: Option<UncheckedAccount<'info>>,
    pub share_mint: Box<InterfaceAccount<'info, IMint>>,
    #[account(has_one = share_mint)]
    pub vault: Box<Account<'info, Vault>>,
                                   
    #[account(seeds = [AUTHORITY_SEED, vault.key().as_ref()], bump = vault.authority_bump)]
    pub authority: UncheckedAccount<'info>,
    pub fermi: FermiVaultAccounts<'info>,
                                                                            
                                                                                
                                                                       
                                                                  
    #[account(mut)]
    pub idle_account: UncheckedAccount<'info>,
}

                                                                                                                               
                                      
pub fn sweep_idle<'info>(ctx: Context<'_, '_, '_, 'info, SweepIdle<'info>>, max_amount: u64) -> Result<()> {
    let vault = &ctx.accounts.vault;
    let key = vault.key();
    require!(!vault.is_closed() && !vault.is_unwinding(), FlpError::WrongVaultMode);
    ctx.accounts.fermi.check(vault)?;
    require_keys_eq!(ctx.accounts.share_mint.key(), vault.share_mint, FlpError::WrongTokenAccount);
    let fermi_ai = ctx.accounts.fermi.fermi_account.to_account_info();
    let (health, queues) = split_event_queues(&fermi_ai, ctx.remaining_accounts)?;
    let authority = ctx.accounts.authority.to_account_info();
    let idle = ctx.accounts.idle_account.to_account_info();
    let nav = compute_projected_vault_nav(vault, &ctx.accounts.fermi, &authority, &idle, health, queues,
        vault.params.upnl_haircut_bps)?;
    let reserve = if vault.deferred_head == NONE_EPOCH {
        require!(ctx.accounts.head_record.is_none(), FlpError::WrongEpochRecord);
        0
    } else {
        let ai = ctx.accounts.head_record.as_ref().ok_or(FlpError::NotDeferredHead)?.to_account_info();
        let head = super::close_epoch::read_deferred_readonly(&ai, &key, vault.deferred_head)?;
        let supply = pricing_supply(ctx.accounts.share_mint.supply, vault)?;
        let fees = crate::accounting::FeeAccrual::calculate(vault, nav.snapshot.nav_low, supply, now_ts()?)?;
        let ratio = math::withdraw_ratio(fees.supply, nav.snapshot.nav_low)?;
        crate::accounting::BatchPayout::new(head.wd_shares, ratio, head.wd_fee_bps)?.net
    };
    let amount = max_amount.min(crate::nav::token_account_amount(&idle)?.saturating_sub(reserve));
    require!(amount > 0, FlpError::NothingPending);
    let bump = [vault.authority_bump];
    let seeds = authority_seeds(&key, &bump);
    fermi_cpi::token_deposit(&ctx.accounts.fermi.core(&authority), &ctx.accounts.fermi.bank(),
        idle, authority.clone(), health, &[&seeds], amount)?;
    emit_event!(FlpIdleSwept { vault: key, amount });
    Ok(())
}

                                                                         
                                                                              
pub fn breaker_conditions(
    params: &LegacyVaultParams,
    reference_e12: u128,
    price_low_e12: u128,
    init_ratio_bps: i64,
) -> bool {
    let drawdown_hit = params.breaker_drawdown_bps > 0
        && reference_e12 > 0
        && price_low_e12 * math::BPS
            < reference_e12 * (math::BPS - params.breaker_drawdown_bps as u128);
    let _ = init_ratio_bps;
    drawdown_hit
}

#[derive(Accounts)]
pub struct CheckBreaker<'info> {
    #[account(mut, has_one = share_mint)]
    pub vault: Box<Account<'info, Vault>>,
                                   
    #[account(seeds = [AUTHORITY_SEED, vault.key().as_ref()], bump = vault.authority_bump)]
    pub authority: UncheckedAccount<'info>,
    pub fermi: FermiVaultAccounts<'info>,
    pub share_mint: Box<InterfaceAccount<'info, IMint>>,
                                                                                               
    #[account(mut)]
    pub idle_account: UncheckedAccount<'info>,
}

                                                                              
                                                                            
                                                                             
                                                                                
                                 
                                                                      
                                                                            
                                                                             
                                                                                
pub fn check_breaker<'info>(ctx: Context<'_, '_, '_, 'info, CheckBreaker<'info>>) -> Result<()> {
    ctx.accounts.vault.require_trading_open()?;
    let vault_key = ctx.accounts.vault.key();
    ctx.accounts.fermi.check(&ctx.accounts.vault)?;
    let params = ctx.accounts.vault.params;
                                                                                 
    let fermi_ai = ctx.accounts.fermi.fermi_account.to_account_info();
    let (health, queues) = split_event_queues(&fermi_ai, ctx.remaining_accounts)?;
    let pending_events = pending_vault_events(&ctx.accounts.vault, &fermi_ai, health, queues)?;
    let being_liquidated = {
        use mango_v4::state::{FermiAccountFixed, FermiAccountLoader};
        let loader = AccountLoader::<FermiAccountFixed>::try_from(&fermi_ai)?;
        let account = loader.load_full()?;
        account.fixed.being_liquidated()
    };
                                                                                
                                                                   
    let snapshot = if being_liquidated { None } else {
        Some(compute_projected_vault_nav(
            &ctx.accounts.vault, &ctx.accounts.fermi,
            &ctx.accounts.authority.to_account_info(),
            &ctx.accounts.idle_account.to_account_info(), health, queues, params.upnl_haircut_bps,
        )?.snapshot)
    };
    let supply = pricing_supply(ctx.accounts.share_mint.supply, &ctx.accounts.vault)?;
    let price_low = match snapshot {
        Some(nav) => math::price_e12(nav.nav_low, supply)?,
        None => 0,
    };
    let reference = ctx.accounts.vault.breaker_ref_price_e12;
    let ratio = snapshot.map(|nav| nav.init_ratio_bps).unwrap_or(i64::MIN);
    require!(being_liquidated || breaker_conditions(&params, reference, price_low, ratio),
        FlpError::BreakerNotTriggered);

    let bump = [ctx.accounts.vault.authority_bump];
    let seeds = authority_seeds(&vault_key, &bump);
    let authority = ctx.accounts.authority.to_account_info();
    let old_delegate = ctx.accounts.vault.delegate;
    if old_delegate != Pubkey::default() {
        fermi_cpi::set_delegate(&ctx.accounts.fermi.core(&authority), &[&seeds], Pubkey::default())?;
    }

    let vault = &mut ctx.accounts.vault;
    let old_status = vault.status;
    vault.status |= status::BREAKER | status::DEPOSITS_PAUSED;
    vault.delegate = Pubkey::default();
                                                                            
                                                                             
    if let Some(nav) = snapshot { vault.last_nav = nav; }
    emit_event!(FlpBreakerObservation { vault: vault_key, pending_events, being_liquidated });
    emit_event!(FlpBreakerTripped {
        vault: vault_key,
        price_low_e12: price_low,
        ref_price_e12: reference,
        init_ratio_bps: ratio,
    });
    emit_event!(FlpDelegateChanged {
        vault: vault_key,
        old: old_delegate,
        new: Pubkey::default(),
        action: 2,
        eta: 0,
    });
    emit_event!(FlpStatusChanged {
        vault: vault_key,
        old: old_status,
        new: vault.status,
        by: Pubkey::default(),
    });
    Ok(())
}

#[derive(Accounts)]
pub struct ResetBreaker<'info> {
    #[account(mut, has_one = admin, has_one = share_mint)]
    pub vault: Box<Account<'info, Vault>>,
    pub admin: Signer<'info>,
                                   
    #[account(seeds = [AUTHORITY_SEED, vault.key().as_ref()], bump = vault.authority_bump)]
    pub authority: UncheckedAccount<'info>,
    pub fermi: FermiVaultAccounts<'info>,
    pub share_mint: Box<InterfaceAccount<'info, IMint>>,
                                                                                               
    #[account(mut)]
    pub idle_account: UncheckedAccount<'info>,
}

                                                                          
                                                                                
                                                                                
                                                                           
                                   
pub fn reset_breaker<'info>(ctx: Context<'_, '_, '_, 'info, ResetBreaker<'info>>) -> Result<()> {
    let now = now_ts()?;
    let vault_key = ctx.accounts.vault.key();
    {
        let vault = &ctx.accounts.vault;
        require!(vault.status & status::CLOSED == 0, FlpError::WrongVaultMode);
        require!(vault.status & status::BREAKER != 0, FlpError::NothingPending);
        ctx.accounts.fermi.check(vault)?;
    }
                                                                                 
    let fermi_ai = ctx.accounts.fermi.fermi_account.to_account_info();
    let (health, queues) = split_event_queues(&fermi_ai, ctx.remaining_accounts)?;
    let (price_low, snapshot) = {
        let nav = compute_projected_vault_nav(
            &ctx.accounts.vault,
            &ctx.accounts.fermi,
            &ctx.accounts.authority.to_account_info(),
            &ctx.accounts.idle_account.to_account_info(),
            health, queues,
            ctx.accounts.vault.params.upnl_haircut_bps,
        )?;
        let supply = pricing_supply(ctx.accounts.share_mint.supply, &ctx.accounts.vault)?;
        (math::price_e12(nav.snapshot.nav_low, supply)?, nav.snapshot)
    };

    let vault = &mut ctx.accounts.vault;
    let old_status = vault.status;
    vault.status &= !status::BREAKER;
    vault.breaker_ref_price_e12 = price_low;
    vault.breaker_ref_ts = now;
    vault.last_nav = snapshot;
    emit_event!(FlpBreakerReset {
        vault: vault_key,
        price_low_e12: price_low,
        init_ratio_bps: snapshot.init_ratio_bps,
        by: ctx.accounts.admin.key(),
    });
    emit_event!(FlpStatusChanged {
        vault: vault_key,
        old: old_status,
        new: vault.status,
        by: ctx.accounts.admin.key(),
    });
    Ok(())
}

