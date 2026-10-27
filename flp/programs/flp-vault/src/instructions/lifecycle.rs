use anchor_lang::prelude::*;
use anchor_spl::token::{Token, TokenAccount};

use super::common::*;
use crate::{error::FlpError, events::*, state::*};

#[derive(Accounts)]
pub struct PreDepositFunds<'info> {
    #[account(mut, has_one = deposit_escrow)]
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
    #[account(mut)]
    pub deposit_escrow: Box<Account<'info, TokenAccount>>,
    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}

                                                                                                 
pub fn pre_deposit(ctx: Context<PreDepositFunds>, amount: u64) -> Result<()> {
    let now = now_ts()?;
    let key = ctx.accounts.vault.key();
    let vault = &mut ctx.accounts.vault;
    require!(
        vault.is_pre_deposit() && vault.is_seeded(),
        FlpError::WrongVaultMode
    );
    vault.require_public_deposits_enabled()?;
    require!(
        amount >= vault.params.min_deposit.max(1),
        FlpError::BelowMinimum
    );
    let total = vault
        .pre_deposit_total
        .checked_add(amount)
        .ok_or(FlpError::MathOverflow)?;
    require!(
        vault.params.tvl_cap == 0
            || (total as u128
                + vault.last_nav.nav_mid.max(vault.dead_shares as i64).max(0) as u128
                + vault.total_pending_deposits() as u128)
                .checked_add(vault.held_deposits as u128)
                .ok_or(FlpError::MathOverflow)?
                <= vault.params.tvl_cap as u128,
        FlpError::CapExceeded
    );
    let d = &mut ctx.accounts.depositor;

    transfer_quote(
        &ctx.accounts.token_program.to_account_info(),
        &ctx.accounts.user_quote_account.to_account_info(),
        &ctx.accounts.deposit_escrow.to_account_info(),
        &ctx.accounts.user.to_account_info(),
        &[],
        amount,
    )?;
    if d.shares_held == 0 && d.pre_deposit_amount == 0 {
        vault.stats.active_depositors += 1;
    }
    if d.first_deposit_ts == 0 {
        d.first_deposit_ts = now;
    }
    d.pre_deposit_amount = d
        .pre_deposit_amount
        .checked_add(amount)
        .ok_or(FlpError::MathOverflow)?;
    d.total_deposited = d
        .total_deposited
        .checked_add(amount)
        .ok_or(FlpError::MathOverflow)?;
    d.last_action_ts = now;
    vault.pre_deposit_total = total;
    vault.stats.total_deposited_gross = vault
        .stats
        .total_deposited_gross
        .checked_add(amount)
        .ok_or(FlpError::MathOverflow)?;
    vault.stats.net_deposits = vault
        .stats
        .net_deposits
        .checked_add(i64::try_from(amount).map_err(|_| FlpError::MathOverflow)?)
        .ok_or(FlpError::MathOverflow)?;
    emit_event!(FlpPreDepositFlow {
        vault: key,
        user: d.user,
        deposit: true,
        amount,
        escrow_total: total
    });
    Ok(())
}

                                                                               
pub fn pre_withdraw(ctx: Context<PreDepositFunds>, amount: u64) -> Result<()> {
    let now = now_ts()?;
    let key = ctx.accounts.vault.key();
    let vault = &mut ctx.accounts.vault;
    require!(vault.is_pre_deposit(), FlpError::WrongVaultMode);
                                                                              
                                                                   
    let d = &mut ctx.accounts.depositor;
    require!(
        amount > 0 && amount <= d.pre_deposit_amount && amount <= vault.pre_deposit_total,
        FlpError::BelowMinimum
    );
    let bump = [vault.authority_bump];
    let seeds = authority_seeds(&key, &bump);
    transfer_quote(
        &ctx.accounts.token_program.to_account_info(),
        &ctx.accounts.deposit_escrow.to_account_info(),
        &ctx.accounts.user_quote_account.to_account_info(),
        &ctx.accounts.authority.to_account_info(),
        &[&seeds],
        amount,
    )?;
    d.pre_deposit_amount -= amount;
    d.total_withdrawn = d
        .total_withdrawn
        .checked_add(amount)
        .ok_or(FlpError::MathOverflow)?;
    d.last_action_ts = now;
    if d.shares_held == 0 && d.pre_deposit_amount == 0 {
        vault.stats.active_depositors = vault.stats.active_depositors.saturating_sub(1);
    }
    vault.pre_deposit_total -= amount;
    vault.stats.total_withdrawn_net = vault
        .stats
        .total_withdrawn_net
        .checked_add(amount)
        .ok_or(FlpError::MathOverflow)?;
    vault.stats.net_deposits = vault
        .stats
        .net_deposits
        .checked_sub(i64::try_from(amount).map_err(|_| FlpError::MathOverflow)?)
        .ok_or(FlpError::MathOverflow)?;
    emit_event!(FlpPreDepositFlow {
        vault: key,
        user: d.user,
        deposit: false,
        amount,
        escrow_total: vault.pre_deposit_total
    });
    Ok(())
}
