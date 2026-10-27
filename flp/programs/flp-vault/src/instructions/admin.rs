use anchor_lang::prelude::*;

use super::common::*;
use crate::error::FlpError;
use crate::events::*;
use crate::fermi_cpi;
use crate::state::*;

#[derive(Accounts)]
pub struct AdminOnly<'info> {
    #[account(mut, has_one = admin)]
    pub vault: Box<Account<'info, Vault>>,
    pub admin: Signer<'info>,
}

pub fn propose_delegate(ctx: Context<AdminOnly>, delegate: Pubkey) -> Result<()> {
    let now = now_ts()?;
    let vault_key = ctx.accounts.vault.key();
    let vault = &mut ctx.accounts.vault;
    require!(delegate != Pubkey::default(), FlpError::InvalidParams);
    vault.pending_delegate = delegate;
    vault.pending_delegate_eta = now.checked_add(vault.params.delegate_delay_s())
        .ok_or(FlpError::MathOverflow)?;
    emit_event!(FlpDelegateChanged {
        vault: vault_key,
        old: vault.delegate,
        new: delegate,
        action: 0,
        eta: vault.pending_delegate_eta,
    });
    Ok(())
}

#[derive(Accounts)]
pub struct EditDelegate<'info> {
    #[account(mut)]
    pub vault: Box<Account<'info, Vault>>,
                                   
    #[account(seeds = [AUTHORITY_SEED, vault.key().as_ref()], bump = vault.authority_bump)]
    pub authority: UncheckedAccount<'info>,
    pub fermi: FermiEditAccounts<'info>,
    pub signer: Signer<'info>,
}

                                                                          
                                  
pub fn activate_delegate(ctx: Context<EditDelegate>) -> Result<()> {
    let now = now_ts()?;
    let vault_key = ctx.accounts.vault.key();
    let vault = &ctx.accounts.vault;
    ctx.accounts.fermi.check(vault)?;
    vault.require_trading_open()?;
    require!(vault.status & status::UNWIND == 0, FlpError::WrongVaultMode);
    require!(vault.pending_delegate != Pubkey::default(), FlpError::NothingPending);
    require!(now >= vault.pending_delegate_eta, FlpError::TimelockActive);
    require!(vault.status & status::BREAKER == 0, FlpError::BreakerTripped);

    let bump = [vault.authority_bump];
    let seeds = authority_seeds(&vault_key, &bump);
    let new = vault.pending_delegate;
    fermi_cpi::set_delegate(
        &ctx.accounts.fermi.core(&ctx.accounts.authority.to_account_info()),
        &[&seeds],
        new,
    )?;
    let vault = &mut ctx.accounts.vault;
    let old = vault.delegate;
    vault.delegate = new;
    vault.pending_delegate = Pubkey::default();
    vault.pending_delegate_eta = 0;
    emit_event!(FlpDelegateChanged {
        vault: vault_key,
        old,
        new,
        action: 1,
        eta: now,
    });
    Ok(())
}

                                                                       
                                                                          
                                     
pub fn remove_delegate(ctx: Context<EditDelegate>) -> Result<()> {
    let vault_key = ctx.accounts.vault.key();
    let vault = &ctx.accounts.vault;
    ctx.accounts.fermi.check(vault)?;
    let who = ctx.accounts.signer.key();
    require!(who == vault.guardian || who == vault.admin, FlpError::Unauthorized);

    let bump = [vault.authority_bump];
    let seeds = authority_seeds(&vault_key, &bump);
    fermi_cpi::set_delegate(
        &ctx.accounts.fermi.core(&ctx.accounts.authority.to_account_info()),
        &[&seeds],
        Pubkey::default(),
    )?;
    let vault = &mut ctx.accounts.vault;
    let old = vault.delegate;
    vault.delegate = Pubkey::default();
    vault.pending_delegate = Pubkey::default();
    vault.pending_delegate_eta = 0;
                                                                                      
    vault.clear_delegate_restore();
    emit_event!(FlpDelegateChanged {
        vault: vault_key,
        old,
        new: Pubkey::default(),
        action: 2,
        eta: 0,
    });
    Ok(())
}

#[derive(Accounts)]
pub struct SetStatus<'info> {
    #[account(mut)]
    pub vault: Box<Account<'info, Vault>>,
    pub signer: Signer<'info>,
}

                                                                               
                                                                              
                                                                             
                                                                          
pub fn set_status(ctx: Context<SetStatus>, set_flags: u8, clear_flags: u8) -> Result<()> {
    let vault_key = ctx.accounts.vault.key();
    let vault = &mut ctx.accounts.vault;
    let who = ctx.accounts.signer.key();
    let is_admin = who == vault.admin;
    require!(is_admin || who == vault.guardian, FlpError::Unauthorized);
    require!(clear_flags == 0 || is_admin, FlpError::Unauthorized);
    require!((set_flags | clear_flags) & !status::SETTABLE == 0, FlpError::WrongVaultMode);
    require!(clear_flags & status::BREAKER == 0, FlpError::BreakerResetRequired);
                                                                           
                                                                             
    if vault.is_unwinding() && set_flags & !clear_flags & status::DEPOSITS_PAUSED != 0 {
        let (delegate, _) = vault.delegate_restore();
        vault.set_delegate_restore(delegate, false);
    }
    let old = vault.status;
    vault.status = (vault.status | set_flags) & !clear_flags;
    emit_event!(FlpStatusChanged {
        vault: vault_key,
        old,
        new: vault.status,
        by: who,
    });
    Ok(())
}

                                                                              
                                                                             
                                                              
fn lp_favourable(vault: &Vault, next: &VaultParams) -> bool {
    let p = &vault.params;
    let timing = vault.scheduled_timing();
    let (epoch, cutoff) = if timing.enabled {
        (timing.epoch_duration_s, timing.request_cutoff_s)
    } else {
        (p.epoch_duration_s, p.request_cutoff_s)
    };
    next.mgmt_fee_bps <= p.mgmt_fee_bps
        && next.perf_fee_bps <= p.perf_fee_bps
        && next.deposit_fee_bps <= p.deposit_fee_bps
        && next.withdraw_fee_bps <= p.withdraw_fee_bps
        && next.min_deposit <= p.min_deposit
        && next.min_withdraw_shares <= p.min_withdraw_shares
        && next.epoch_duration_s == epoch
        && next.request_cutoff_s == cutoff
        && next.timelock_s == p.timelock_s
        && next.upnl_haircut_bps == p.upnl_haircut_bps
        && next.min_post_withdraw_init_ratio_bps == p.min_post_withdraw_init_ratio_bps
        && next.breaker_drawdown_bps == p.breaker_drawdown_bps
        && next.breaker_min_init_ratio_bps == p.breaker_min_init_ratio_bps
        && next.breaker_window_s == p.breaker_window_s
        && next.equity_window_s == vault.equity_window_s()
}

                                                                              
                                                                              
                                     
pub fn stage_params(ctx: Context<AdminOnly>, params: VaultParams) -> Result<()> {
    params.validate()?;
    let now = now_ts()?;
    let vault_key = ctx.accounts.vault.key();
    let vault = &mut ctx.accounts.vault;
                                                                               
                                                                               
                                                       
    let cycle = params.epoch_duration_s as u64 + params.equity_window_s as u64;
    require!(
        cycle <= vault.recovery_params.grace_proportional_s as u64
            && (vault.pending_recovery_eta == 0
                || cycle <= vault.pending_recovery_params.grace_proportional_s as u64),
        FlpError::RecoveryGraceTooShort
    );
    let delay = if lp_favourable(vault, &params) {
        vault.params.delegate_delay_s()
    } else {
        vault.parameter_delay_s()
    };
    vault.pending_params = params.into();
    vault.stage_equity_window(params.equity_window_s);
    vault.pending_params_eta = now + delay;
    emit_event!(FlpParamsChanged {
        vault: vault_key,
        action: 0,
        eta: vault.pending_params_eta,
    });
    Ok(())
}

                                                                                
                                                                         
                                                                        
                                                                             
                                                          
pub fn apply_params<'info>(ctx: Context<'_, '_, '_, 'info, AdminOnly<'info>>) -> Result<()> {
    let now = now_ts()?;
    let vault_key = ctx.accounts.vault.key();
    let vault = &mut ctx.accounts.vault;
    require!(vault.pending_params_eta != 0, FlpError::NothingPending);
    require!(now >= vault.pending_params_eta, FlpError::TimelockActive);
                                                                                
                                                                                  
                                                                              
    let fees_change = vault.pending_params.mgmt_fee_bps != vault.params.mgmt_fee_bps
        || vault.pending_params.perf_fee_bps != vault.params.perf_fee_bps;
    let accruing = !vault.is_closed();
    if fees_change && accruing && vault.last_fee_accrual_ts != now && !ctx.remaining_accounts.is_empty() {
        super::permissioned::accrue_from_remaining(vault, ctx.remaining_accounts)?;
    }
    require!(
        !fees_change || !accruing || vault.last_fee_accrual_ts == now,
        FlpError::FeeChangeNeedsAccrual
    );
                                                                                
                                                                               
    let duration = vault.params.epoch_duration_s;
    let cutoff = vault.params.request_cutoff_s;
    let next = vault.pending_params;
    vault.schedule_epoch_timing(next.epoch_duration_s, next.request_cutoff_s);
    vault.params = vault.pending_params;
    vault.apply_equity_window();
    vault.params.epoch_duration_s = duration;
    vault.params.request_cutoff_s = cutoff;
    vault.pending_params_eta = 0;
    emit_event!(FlpParamsChanged {
        vault: vault_key,
        action: 1,
        eta: now,
    });
    Ok(())
}

pub fn set_roles(ctx: Context<AdminOnly>, guardian: Pubkey, fee_recipient: Pubkey) -> Result<()> {
    let vault = &mut ctx.accounts.vault;
    vault.guardian = guardian;
    vault.fee_recipient = fee_recipient;
    Ok(())
}

pub fn propose_admin(ctx: Context<AdminOnly>, new_admin: Pubkey) -> Result<()> {
    ctx.accounts.vault.pending_admin = new_admin;
    Ok(())
}

#[derive(Accounts)]
pub struct AcceptAdmin<'info> {
    #[account(mut, constraint = vault.pending_admin == new_admin.key() @ FlpError::Unauthorized)]
    pub vault: Box<Account<'info, Vault>>,
    pub new_admin: Signer<'info>,
}

pub fn accept_admin(ctx: Context<AcceptAdmin>) -> Result<()> {
    let vault = &mut ctx.accounts.vault;
    vault.admin = ctx.accounts.new_admin.key();
    vault.pending_admin = Pubkey::default();
    Ok(())
}

