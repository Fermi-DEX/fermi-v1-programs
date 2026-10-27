use super::common::*;
use crate::{error::FlpError, events::*, fermi_cpi, math, state::*};
use anchor_lang::prelude::*;
use anchor_spl::token::TokenAccount;
use anchor_spl::token_2022::Token2022;
use anchor_spl::token_interface::{Mint as IMint, TokenAccount as ITokenAccount};

#[derive(Accounts)]
pub struct SetDepositPermission<'info> {
    #[account(has_one = admin)]
    pub vault: Box<Account<'info, Vault>>,
    #[account(mut)]
    pub admin: Signer<'info>,
                                                                            
    pub user: UncheckedAccount<'info>,
    #[account(mut, has_one = vault, has_one = user,
        seeds = [DEPOSITOR_SEED, vault.key().as_ref(), user.key().as_ref()], bump = depositor.bump)]
    pub depositor: Box<Account<'info, Depositor>>,
    pub system_program: Program<'info, System>,
}

pub fn set_deposit_permission(ctx: Context<SetDepositPermission>, approved: bool) -> Result<()> {
    let d = &mut ctx.accounts.depositor;

    d.deposit_approved = approved;
    emit_event!(FlpDepositPermission {
        vault: d.vault,
        user: d.user,
        approved
    });
    Ok(())
}

#[derive(Accounts)]
pub struct PricingAccounts<'info> {
                                                                     
    pub authority: UncheckedAccount<'info>,
    #[account(mut)]
    pub share_mint: Box<InterfaceAccount<'info, IMint>>,
                                                                              
                   
                                                                                     
    #[account(mut)]
    pub fee_share_account: UncheckedAccount<'info>,
                                                                                          
    #[account(mut)]
    pub idle_account: UncheckedAccount<'info>,
    pub fermi: FermiVaultAccounts<'info>,
    pub token_2022_program: Program<'info, Token2022>,
}

                                                                                 
                                                                                      
fn accrue_pricing<'info>(
    v: &mut Account<'info, Vault>,
    p: &PricingAccounts<'info>,
    health: &[AccountInfo<'info>],
    queues: &[AccountInfo<'info>],
) -> Result<(crate::nav::NavComputation, u64)> {
    let key = v.key();
    let bump = [v.authority_bump];
    let seeds = authority_seeds(&key, &bump);
    require_keys_eq!(
        Pubkey::create_program_address(&seeds, &crate::ID)
            .map_err(|_| FlpError::FermiAccountMismatch)?,
        p.authority.key(),
        FlpError::FermiAccountMismatch
    );
    require_keys_eq!(
        v.share_mint,
        p.share_mint.key(),
        FlpError::WrongTokenAccount
    );
    require_ata(
        &p.fee_share_account.key(),
        &v.fee_recipient,
        &v.share_mint,
        &anchor_spl::token_2022::ID,
    )?;
    let nav = compute_projected_vault_nav(
        v,
        &p.fermi,
        &p.authority.to_account_info(),
        &p.idle_account.to_account_info(),
        health, queues,
        v.params.upnl_haircut_bps,
    )?;
    let snap = nav.snapshot;
    let now = now_ts()?;
    let supply = pricing_supply(p.share_mint.supply, v)?;
    let fees = crate::accounting::FeeAccrual::calculate(v, snap.nav_low, supply, now)?;
    apply_fee_accrual(
        v,
        fees,
        now,
        &p.token_2022_program.to_account_info(),
        &p.share_mint.to_account_info(),
        &p.fee_share_account.to_account_info(),
        &p.authority.to_account_info(),
    )?;
    v.last_nav = snap;
    let supply = fees.supply;
    Ok((nav, supply))
}

                                                                              
                                                                                
pub(crate) fn accrue_from_remaining<'info>(
    v: &mut Account<'info, Vault>,
    remaining: &[AccountInfo<'info>],
) -> Result<()> {
    let mut accounts = remaining;
    let mut bumps = std::collections::BTreeMap::new();
    let mut reallocs = std::collections::BTreeSet::new();
    let pricing = PricingAccounts::try_accounts(&crate::ID, &mut accounts, &[], &mut bumps, &mut reallocs)?;
    pricing.fermi.check(v)?;
    let fermi_ai = pricing.fermi.fermi_account.to_account_info();
    let (health, queues) = split_event_queues(&fermi_ai, accounts)?;
    accrue_pricing(v, &pricing, health, queues)?;
    Ok(())
}

#[derive(Accounts)]
pub struct PermissionedDeposit<'info> {
    #[account(mut, has_one = deposit_escrow)]
    pub vault: Box<Account<'info, Vault>>,
    #[account(mut, has_one = vault, has_one = user,
        seeds = [DEPOSITOR_SEED, vault.key().as_ref(), user.key().as_ref()], bump = depositor.bump)]
    pub depositor: Box<Account<'info, Depositor>>,
    pub user: Signer<'info>,
    #[account(mut, token::authority = user, token::mint = vault.quote_mint)]
    pub user_quote_account: Box<Account<'info, TokenAccount>>,
    #[account(mut)]
    pub user_share_account: Box<InterfaceAccount<'info, ITokenAccount>>,
    #[account(mut)]
    pub deposit_escrow: Box<Account<'info, TokenAccount>>,
    pub pricing: PricingAccounts<'info>,
}

pub fn permissioned_deposit<'info>(
    ctx: Context<'_, '_, '_, 'info, PermissionedDeposit<'info>>,
    amount: u64,
    min_shares: u64,
) -> Result<()> {
    let v = &mut ctx.accounts.vault;
    require!(v.is_seeded(), FlpError::WrongVaultMode);
    v.require_deposits_enabled()?;
    require!(
        ctx.accounts.depositor.deposit_approved,
        FlpError::DepositNotApproved
    );
    require!(
        amount >= v.params.min_deposit.max(1),
        FlpError::BelowMinimum
    );
    let p = &ctx.accounts.pricing;
    require_ata(
        &ctx.accounts.user_share_account.key(),
        &ctx.accounts.user.key(),
        &v.share_mint,
        &anchor_spl::token_2022::ID,
    )?;
                                                                                 
    p.fermi.check(v)?;
    let fermi_ai = p.fermi.fermi_account.to_account_info();
    let (health, queues) = split_event_queues(&fermi_ai, ctx.remaining_accounts)?;
    let (mut nav, supply) = accrue_pricing(v, p, health, queues)?;
    let idle_before = crate::nav::token_account_amount(&p.idle_account.to_account_info())?;
    let snap = nav.snapshot;
    let projected = snap.nav_mid.max(0) as u128
        + v.pre_deposit_total as u128
        + v.total_pending_deposits() as u128
        + v.held_deposits as u128
        + amount as u128;
    require!(
        v.params.tvl_cap == 0 || projected <= v.params.tvl_cap as u128,
        FlpError::CapExceeded
    );
    let shares = math::apply_ratio_floor(
        amount - math::fee_ceil(amount, v.params.deposit_fee_bps)?,
        math::deposit_ratio(supply, snap.nav_high)?,
    )?;
    require!(
        shares > 0 && shares >= min_shares,
        FlpError::SlippageExceeded
    );
    let key = v.key();
    let bump = [v.authority_bump];
    let seeds = authority_seeds(&key, &bump);
    transfer_quote(
        &p.fermi.token_program.to_account_info(),
        &ctx.accounts.user_quote_account.to_account_info(),
        &ctx.accounts.deposit_escrow.to_account_info(),
        &ctx.accounts.user.to_account_info(),
        &[],
        amount,
    )?;
    transfer_quote(&p.fermi.token_program.to_account_info(),
        &ctx.accounts.deposit_escrow.to_account_info(), &p.idle_account.to_account_info(),
        &p.authority.to_account_info(), &[&seeds], amount)?;
    mint_shares(
        &p.token_2022_program.to_account_info(),
        &p.share_mint.to_account_info(),
        &ctx.accounts.user_share_account.to_account_info(),
        &p.authority.to_account_info(),
        &[&seeds],
        shares,
    )?;
    let now = now_ts()?;
    let d = &mut ctx.accounts.depositor;
    if d.shares_held == 0 && d.pre_deposit_amount == 0 {
        v.stats.active_depositors += 1;
    }
    if d.first_deposit_ts == 0 {
        d.first_deposit_ts = now;
    }
    d.shares_held = d
        .shares_held
        .checked_add(shares)
        .ok_or(FlpError::MathOverflow)?;
    d.cost_basis = d
        .cost_basis
        .checked_add(amount)
        .ok_or(FlpError::MathOverflow)?;
    d.total_deposited = d
        .total_deposited
        .checked_add(amount)
        .ok_or(FlpError::MathOverflow)?;
    d.last_action_ts = now;
    v.stats.total_deposited_gross = v
        .stats
        .total_deposited_gross
        .checked_add(amount)
        .ok_or(FlpError::MathOverflow)?;
    v.stats.net_deposits = v
        .stats
        .net_deposits
        .checked_add(i64::try_from(amount).map_err(|_| FlpError::MathOverflow)?)
        .ok_or(FlpError::MathOverflow)?;
    nav.replace_idle_quote(idle_before, crate::nav::token_account_amount(&p.idle_account.to_account_info())?)?;
    v.last_nav = nav.snapshot;
    emit_event!(FlpPermissionedDeposit {
        vault: key,
        user: d.user,
        amount,
        shares
    });
    Ok(())
}

#[derive(Accounts)]
pub struct Graduate<'info> {
    #[account(mut, has_one = admin, has_one = deposit_escrow)]
    pub vault: Box<Account<'info, Vault>>,
    pub admin: Signer<'info>,
    #[account(mut)]
    pub deposit_escrow: Box<Account<'info, TokenAccount>>,
    pub pricing: PricingAccounts<'info>,
}

                                                                            
                                                                                
                                                                                 
                                                                                
                                                                                 
                                                            
pub const GRADUATION_MAX_NAV_SPREAD_BPS: u128 = 200;

pub fn graduate<'info>(ctx: Context<'_, '_, '_, 'info, Graduate<'info>>) -> Result<()> {
    let v = &mut ctx.accounts.vault;
    require!(
        v.is_pre_deposit() && v.is_seeded(),
        FlpError::WrongVaultMode
    );
    require!(
        v.status & (status::BREAKER | status::UNWIND | status::CLOSED) == 0,
        FlpError::WrongVaultMode
    );
                                                                    
    require!(
        v.status & status::WITHDRAWALS_PAUSED == 0,
        FlpError::WithdrawalsPaused
    );
    let p = &ctx.accounts.pricing;
                                                                                 
    p.fermi.check(v)?;
    let fermi_ai = p.fermi.fermi_account.to_account_info();
    let (health, queues) = split_event_queues(&fermi_ai, ctx.remaining_accounts)?;
    let (mut nav, supply) = accrue_pricing(v, p, health, queues)?;
    let idle_before = crate::nav::token_account_amount(&p.idle_account.to_account_info())?;
    let snap = nav.snapshot;
    let positive_upnl = nav.positive_upnl()?;
                                                                                    
                                                                                   
                                                                   
    require!(snap.nav_low > 0, FlpError::VaultInsolvent);
    if v.pre_deposit_total > 0 {
                                                                              
                                                                                
        let low = if v.params.upnl_haircut_bps == 0 {
            snap.nav_low
        } else {
            nav.unhaircut_low(v.params.upnl_haircut_bps)?
        };
        let spread = (snap.nav_high as i128 - low as i128).max(0) as u128;
        require!(
            spread * math::BPS <= GRADUATION_MAX_NAV_SPREAD_BPS * snap.nav_mid as u128,
            FlpError::GraduationSpreadTooWide
        );
    }
    let price_low_before = math::price_e12(snap.nav_low, supply)?;
    require!(
        !super::keeper::breaker_conditions(
            &v.params,
            v.breaker_ref_price_e12,
            price_low_before,
            snap.init_ratio_bps
        ),
        FlpError::BreakerTripped
    );
    let ratio = math::deposit_ratio(supply, snap.nav_mid)?;
    let amount = v.pre_deposit_total;
    let shares = math::apply_ratio_floor(amount, ratio)?;
    require!(amount == 0 || shares > 0, FlpError::SlippageExceeded);
    let key = v.key();
    let bump = [v.authority_bump];
    let seeds = authority_seeds(&key, &bump);
                                                                           
                                                                                  
    if v.delegate == Pubkey::default()
        && v.pending_delegate != Pubkey::default()
        && now_ts()? >= v.pending_delegate_eta
    {
        fermi_cpi::set_delegate(
            &p.fermi.core(&p.authority.to_account_info()),
            &[&seeds],
            v.pending_delegate,
        )?;
        v.delegate = v.pending_delegate;
        v.pending_delegate = Pubkey::default();
        v.pending_delegate_eta = 0;
        emit_event!(FlpDelegateChanged {
            vault: key,
            old: Pubkey::default(),
            new: v.delegate,
            action: 1,
            eta: now_ts()?
        });
    }
    transfer_quote(&p.fermi.token_program.to_account_info(),
        &ctx.accounts.deposit_escrow.to_account_info(), &p.idle_account.to_account_info(),
        &p.authority.to_account_info(), &[&seeds], amount)?;
    v.graduation_num = ratio.0;
    v.graduation_den = ratio.1;
    v.graduation_remaining_quote = amount;
    v.graduation_remaining_shares = shares;
    v.owed_unminted_shares = v
        .owed_unminted_shares
        .checked_add(shares)
        .ok_or(FlpError::MathOverflow)?;
    v.pre_deposit_total = 0;
    v.status &= !status::PRE_DEPOSIT;
    v.live_started_at = now_ts()?;
                                                                                   
    nav.replace_idle_quote(idle_before, crate::nav::token_account_amount(&p.idle_account.to_account_info())?)?;
    let after = nav.snapshot;
                                                                                  
                                                                               
    let total = supply.checked_add(shares).ok_or(FlpError::MathOverflow)?;
    let price_low_after = math::price_e12(after.nav_low, total)?;
                                                                                 
                                                                                  
                                                                                      
                                                                                
                                
    v.hwm_price_e12 = math::price_e12(after.nav_mid, total)?;
                                                                                 
                                      
    if price_low_before > 0 && price_low_after > price_low_before {
        v.breaker_ref_price_e12 =
            math::mul_div_floor(v.breaker_ref_price_e12, price_low_after, price_low_before)?;
    }
    v.last_nav = after;
    emit_event!(FlpGraduated {
        vault: key,
        amount,
        shares,
        num: ratio.0,
        den: ratio.1,
        positive_upnl,
        public_share_bps: if total == 0 { 0 } else {
            math::mul_div_floor(shares as u128, math::BPS, total as u128)? as u16
        },
    });
    Ok(())
}

#[derive(Accounts)]
pub struct ClaimPreDeposit<'info> {
    #[account(mut, has_one = share_mint)]
    pub vault: Box<Account<'info, Vault>>,
    #[account(mut, has_one = vault)]
    pub depositor: Box<Account<'info, Depositor>>,
                                   
    #[account(seeds = [AUTHORITY_SEED, vault.key().as_ref()], bump = vault.authority_bump)]
    pub authority: UncheckedAccount<'info>,
    #[account(mut)]
    pub share_mint: Box<InterfaceAccount<'info, IMint>>,
    #[account(mut)]
    pub user_share_account: Box<InterfaceAccount<'info, ITokenAccount>>,
    pub token_2022_program: Program<'info, Token2022>,
}

                                                                               
                                                                                    
pub fn claim_pre_deposit(ctx: Context<ClaimPreDeposit>) -> Result<()> {
    let v = &mut ctx.accounts.vault;
    let d = &mut ctx.accounts.depositor;
    require!(
        !v.is_pre_deposit() && v.graduation_den > 0,
        FlpError::WrongVaultMode
    );
    let amount = d.pre_deposit_amount;
    require!(amount > 0, FlpError::NoTicket);
    require_ata(
        &ctx.accounts.user_share_account.key(),
        &d.user,
        &v.share_mint,
        &anchor_spl::token_2022::ID,
    )?;
    let shares = math::apply_ratio_floor(amount, (v.graduation_num, v.graduation_den))?;
    let key = v.key();
    let bump = [v.authority_bump];
    let seeds = authority_seeds(&key, &bump);
    mint_shares(
        &ctx.accounts.token_2022_program.to_account_info(),
        &ctx.accounts.share_mint.to_account_info(),
        &ctx.accounts.user_share_account.to_account_info(),
        &ctx.accounts.authority.to_account_info(),
        &[&seeds],
        shares,
    )?;
    v.graduation_remaining_quote = v
        .graduation_remaining_quote
        .checked_sub(amount)
        .ok_or(FlpError::MathOverflow)?;
    v.graduation_remaining_shares = v
        .graduation_remaining_shares
        .checked_sub(shares)
        .ok_or(FlpError::MathOverflow)?;
    v.owed_unminted_shares = v
        .owed_unminted_shares
        .checked_sub(shares)
        .ok_or(FlpError::MathOverflow)?;
    if v.graduation_remaining_quote == 0 {
                                                                                   
        v.owed_unminted_shares = v
            .owed_unminted_shares
            .checked_sub(v.graduation_remaining_shares)
            .ok_or(FlpError::MathOverflow)?;
        v.graduation_remaining_shares = 0;
    }
    d.pre_deposit_amount = 0;
    d.shares_held = d
        .shares_held
        .checked_add(shares)
        .ok_or(FlpError::MathOverflow)?;
    d.cost_basis = d
        .cost_basis
        .checked_add(amount)
        .ok_or(FlpError::MathOverflow)?;
    d.last_action_ts = now_ts()?;
    if d.shares_held == 0 {
        v.stats.active_depositors = v.stats.active_depositors.saturating_sub(1);
    }
    emit_event!(FlpPreDepositClaimed {
        vault: key,
        user: d.user,
        amount,
        shares
    });
    Ok(())
}

#[derive(Accounts)]
pub struct PermissionedWithdraw<'info> {
    #[account(mut)]
    pub vault: Box<Account<'info, Vault>>,
    #[account(mut, has_one = vault, has_one = user,
        seeds = [DEPOSITOR_SEED, vault.key().as_ref(), user.key().as_ref()], bump = depositor.bump)]
    pub depositor: Box<Account<'info, Depositor>>,
    pub user: Signer<'info>,
    #[account(mut, token::authority = user, token::mint = vault.quote_mint)]
    pub user_quote_account: Box<Account<'info, TokenAccount>>,
    #[account(mut, token::authority = user, token::mint = vault.share_mint)]
    pub user_share_account: Box<InterfaceAccount<'info, ITokenAccount>>,
    pub pricing: PricingAccounts<'info>,
}

                                                                              
                                                                               
                                                                                 
                                               
                                                                               
                                                                             
                                                  
pub fn permissioned_withdraw<'info>(
    ctx: Context<'_, '_, '_, 'info, PermissionedWithdraw<'info>>,
    shares: u64,
    min_quote: u64,
) -> Result<()> {
    let v = &mut ctx.accounts.vault;
    require!(v.is_pre_deposit() && v.is_seeded(), FlpError::WrongVaultMode);
    require!(
        v.status & (status::BREAKER | status::UNWIND | status::CLOSED) == 0,
        FlpError::WrongVaultMode
    );
    require!(v.status & status::WITHDRAWALS_PAUSED == 0, FlpError::WithdrawalsPaused);
                                                                                
    require!(v.deferred_head == NONE_EPOCH, FlpError::DeferredWithdrawalFirst);
    require!(ctx.accounts.depositor.deposit_approved, FlpError::DepositNotApproved);
    let balance = ctx.accounts.user_share_account.amount;
    require!(
        shares > 0 && shares <= balance && (shares >= v.params.min_withdraw_shares || shares == balance),
        FlpError::BelowMinimum
    );
    require_ata(
        &ctx.accounts.user_quote_account.key(),
        &ctx.accounts.user.key(),
        &v.quote_mint,
        &anchor_spl::token::ID,
    )?;
    let p = &ctx.accounts.pricing;
    p.fermi.check(v)?;
    let fermi_ai = p.fermi.fermi_account.to_account_info();
    let (health, queues) = split_event_queues(&fermi_ai, ctx.remaining_accounts)?;
    let (mut nav, supply) = accrue_pricing(v, p, health, queues)?;
    let ratio = math::withdraw_ratio(supply, nav.snapshot.nav_low)?;
    let payout = crate::accounting::BatchPayout::new(shares, ratio, v.params.withdraw_fee_bps)?;
    require!(payout.net > 0 && payout.net >= min_quote, FlpError::SlippageExceeded);
    let idle_ai = p.idle_account.to_account_info();
    let check = crate::accounting::FundingCheck::calculate(
        &mut nav,
        crate::nav::token_account_amount(&idle_ai)?,
        &payout,
        &p.fermi.quote_bank_vault.to_account_info(),
        false,
    )?;
    require!(check.ok, FlpError::InsufficientLiquidity);
    anchor_spl::token_2022::burn(
        CpiContext::new(
            p.token_2022_program.to_account_info(),
            anchor_spl::token_2022::Burn {
                mint: p.share_mint.to_account_info(),
                from: ctx.accounts.user_share_account.to_account_info(),
                authority: ctx.accounts.user.to_account_info(),
            },
        ),
        shares,
    )?;
    let key = v.key();
    let bump = [v.authority_bump];
    let seeds = authority_seeds(&key, &bump);
    let quote = ctx.accounts.user_quote_account.to_account_info();
    transfer_quote(
        &p.fermi.token_program.to_account_info(),
        &idle_ai,
        &quote,
        &p.authority.to_account_info(),
        &[&seeds],
        check.idle,
    )?;
    fermi_cpi::token_withdraw(
        &p.fermi.core(&p.authority.to_account_info()),
        &p.fermi.bank(),
        quote,
        health,
        &[&seeds],
        check.core,
    )?;
    if check.core > 0 {
        fermi_cpi::assert_payout_health(&p.fermi.core(&p.authority.to_account_info()),
            health, v.params.min_post_withdraw_init_ratio_bps)?;
    }
    let now = now_ts()?;
    let d = &mut ctx.accounts.depositor;
    crate::accounting::record_immediate_redemption(v, d, shares, payout.net, now)?;
    nav.refresh_quote_after_flow(&fermi_ai, &p.fermi.quote_bank.to_account_info(), v.params.upnl_haircut_bps)?;
    nav.include_idle_quote(crate::nav::token_account_amount(&idle_ai)?)?;
    v.last_nav = nav.snapshot;
    emit_event!(FlpPermissionedWithdrawal {
        vault: key,
        user: d.user,
        shares,
        gross: payout.gross,
        fee: payout.fee,
        net: payout.net,
    });
    Ok(())
}

#[derive(Accounts)]
pub struct InitDepositor<'info> {
    pub vault: Box<Account<'info, Vault>>,
                                                                                 
    pub user: UncheckedAccount<'info>,
    #[account(init_if_needed, payer = payer, space = 8 + Depositor::INIT_SPACE,
        seeds = [DEPOSITOR_SEED, vault.key().as_ref(), user.key().as_ref()], bump)]
    pub depositor: Box<Account<'info, Depositor>>,
    #[account(mut)]
    pub payer: Signer<'info>,
    pub system_program: Program<'info, System>,
}

                                                                                 
                                                                               
pub fn init_depositor(ctx: Context<InitDepositor>) -> Result<()> {
    let d = &mut ctx.accounts.depositor;
    let vault = ctx.accounts.vault.key();
    let user = ctx.accounts.user.key();
    let bump = ctx.bumps["depositor"];
    if d.vault == Pubkey::default() {
        d.vault = vault;
        d.user = user;
        d.bump = bump;
        d.deposit_approved = false;
        d.dep = Ticket::empty();
        d.wd = Ticket::empty();
    } else {
        require_keys_eq!(d.vault, vault, FlpError::InvalidParams);
        require_keys_eq!(d.user, user, FlpError::InvalidParams);
        require!(d.bump == bump, FlpError::InvalidParams);
    }
    Ok(())
}
