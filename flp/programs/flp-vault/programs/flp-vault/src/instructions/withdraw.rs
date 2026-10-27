use anchor_lang::prelude::*;
use anchor_spl::token::{Token, TokenAccount};
use anchor_spl::token_2022::{self, Token2022};
use anchor_spl::token_interface::{Mint as IMint, TokenAccount as ITokenAccount};

use super::common::*;
use crate::error::FlpError;
use crate::events::*;
use crate::math;
use crate::state::*;

#[derive(Accounts)]
pub struct RequestWithdraw<'info> {
    #[account(mut, has_one = share_mint)]
    pub vault: Box<Account<'info, Vault>>,
    #[account(mut, has_one = vault, has_one = user)]
    pub depositor: Box<Account<'info, Depositor>>,
    pub user: Signer<'info>,
    #[account(mut)]
    pub share_mint: Box<InterfaceAccount<'info, IMint>>,
    #[account(mut, token::authority = user, token::mint = share_mint)]
    pub user_share_account: Box<InterfaceAccount<'info, ITokenAccount>>,
    pub token_2022_program: Program<'info, Token2022>,
}

                                                                           
                                                                              
                                 
pub fn request_withdraw(ctx: Context<RequestWithdraw>, shares: u64) -> Result<()> {
    let now = now_ts()?;
    let vault_key = ctx.accounts.vault.key();
    let vault = &mut ctx.accounts.vault;
    vault.require_trading_open()?;
    require!(vault.status & status::UNWIND == 0, FlpError::WrongVaultMode);
    require!(vault.status & status::WITHDRAWALS_PAUSED == 0, FlpError::WithdrawalsPaused);
    require!(shares > 0, FlpError::BelowMinimum);
    let balance = ctx.accounts.user_share_account.amount;
    require!(
        shares >= vault.params.min_withdraw_shares || shares == balance,
        FlpError::BelowMinimum
    );

    let depositor = &mut ctx.accounts.depositor;
    let target = vault.target_epoch(now);
    let new_ticket = depositor.wd.is_empty();
    require!(new_ticket || depositor.wd.epoch == target, FlpError::TicketPending);

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

                                                        
    let held = depositor.shares_held;
    let released_shares = shares.min(held);
    let basis_released = if held > 0 {
        math::to_u64(math::mul_div_floor(
            depositor.cost_basis as u128,
            released_shares as u128,
            held as u128,
        )?)?
    } else {
        0
    };
    depositor.cost_basis -= basis_released;
    depositor.shares_held = held - released_shares;
    if held > 0 && depositor.shares_held == 0 && depositor.pre_deposit_amount == 0 {
        vault.stats.active_depositors = vault.stats.active_depositors.saturating_sub(1);
    }

    vault.locked_shares = vault.locked_shares.checked_add(shares).ok_or(FlpError::MathOverflow)?;
    let slot = vault.pending_slot_mut(target)?;
    slot.withdraw_shares += shares;
    if new_ticket {
        slot.n_withdraws += 1;
        depositor.wd = Ticket {
            request_id: vault.next_request_id,
            epoch: target,
            amount: 0,
            aux: 0,
        };
        vault.next_request_id += 1;
    }
    depositor.wd.amount += shares;
    depositor.wd.aux += basis_released;
    depositor.last_action_ts = now;

    emit_event!(FlpWithdrawRequested {
        vault: vault_key,
        user: depositor.user,
        request_id: depositor.wd.request_id,
        epoch: target,
        shares,
        basis_released,
    });
    Ok(())
}

#[derive(Accounts)]
pub struct CancelWithdraw<'info> {
    #[account(mut, has_one = share_mint)]
    pub vault: Box<Account<'info, Vault>>,
    #[account(mut, has_one = vault, has_one = user)]
    pub depositor: Box<Account<'info, Depositor>>,
                                                          
    #[account(seeds = [AUTHORITY_SEED, vault.key().as_ref()], bump = vault.authority_bump)]
    pub authority: UncheckedAccount<'info>,
    pub user: Signer<'info>,
    #[account(mut)]
    pub share_mint: Box<InterfaceAccount<'info, IMint>>,
    #[account(mut, token::authority = user, token::mint = share_mint)]
    pub user_share_account: Box<InterfaceAccount<'info, ITokenAccount>>,
    pub token_2022_program: Program<'info, Token2022>,
}

pub fn cancel_withdraw(ctx: Context<CancelWithdraw>) -> Result<()> {
    let now = now_ts()?;
    let vault_key = ctx.accounts.vault.key();
    let vault = &mut ctx.accounts.vault;
    let depositor = &mut ctx.accounts.depositor;
    let ticket = depositor.wd;
    require!(!ticket.is_empty(), FlpError::NoTicket);
    require!(vault.can_cancel(ticket.epoch, now), FlpError::PastCutoff);

    let bump = [vault.authority_bump];
    let seeds = authority_seeds(&vault_key, &bump);
    mint_shares(
        &ctx.accounts.token_2022_program.to_account_info(),
        &ctx.accounts.share_mint.to_account_info(),
        &ctx.accounts.user_share_account.to_account_info(),
        &ctx.accounts.authority.to_account_info(),
        &[&seeds],
        ticket.amount,
    )?;

    vault.locked_shares -= ticket.amount;
    let slot = vault.pending_slot_mut(ticket.epoch)?;
    slot.withdraw_shares -= ticket.amount;
    slot.n_withdraws -= 1;
    if depositor.shares_held == 0 && depositor.pre_deposit_amount == 0 {
        vault.stats.active_depositors += 1;
    }
    depositor.shares_held += ticket.amount;
    depositor.cost_basis += ticket.aux;
    depositor.wd = Ticket::empty();
    depositor.last_action_ts = now;

    emit_event!(FlpWithdrawCancelled {
        vault: vault_key,
        user: depositor.user,
        request_id: ticket.request_id,
        epoch: ticket.epoch,
        shares: ticket.amount,
    });
    Ok(())
}

#[derive(Accounts)]
pub struct ClaimWithdrawal<'info> {
    #[account(has_one = redemption_escrow)]
    pub vault: Box<Account<'info, Vault>>,
    #[account(mut, has_one = vault)]
    pub depositor: Box<Account<'info, Depositor>>,
    #[account(
        mut,
        seeds = [EPOCH_SEED, vault.key().as_ref(), &depositor.wd.epoch.to_le_bytes()],
        bump = epoch_record.bump,
    )]
    pub epoch_record: Box<Account<'info, EpochRecord>>,
                                                      
    #[account(seeds = [AUTHORITY_SEED, vault.key().as_ref()], bump = vault.authority_bump)]
    pub authority: UncheckedAccount<'info>,
    #[account(mut)]
    pub redemption_escrow: Box<Account<'info, TokenAccount>>,
                                        
    #[account(mut)]
    pub user_quote_account: Box<Account<'info, TokenAccount>>,
    pub token_program: Program<'info, Token>,
}

                                                                           
              
pub fn claim_withdrawal(ctx: Context<ClaimWithdrawal>) -> Result<()> {
    let now = now_ts()?;
    let vault_key = ctx.accounts.vault.key();
    let vault = &ctx.accounts.vault;
    let depositor = &mut ctx.accounts.depositor;
    let ticket = depositor.wd;
    require!(!ticket.is_empty(), FlpError::NoTicket);
    let record = &mut ctx.accounts.epoch_record;
    require!(
        record.wd_status == WithdrawStatus::Settled as u8,
        FlpError::NotSettled
    );
    require_ata(
        &ctx.accounts.user_quote_account.key(),
        &depositor.user,
        &vault.quote_mint,
        &anchor_spl::token::ID,
    )?;

    let gross = math::apply_ratio_floor(ticket.amount, (record.wd_num, record.wd_den))?;
    let fee = math::fee_ceil(gross, record.wd_fee_bps)?;
    let payout = gross - fee;

    let bump = [vault.authority_bump];
    let seeds = authority_seeds(&vault_key, &bump);
    transfer_quote(
        &ctx.accounts.token_program.to_account_info(),
        &ctx.accounts.redemption_escrow.to_account_info(),
        &ctx.accounts.user_quote_account.to_account_info(),
        &ctx.accounts.authority.to_account_info(),
        &[&seeds],
        payout,
    )?;

    record.wd_claimed_shares += ticket.amount;
    record.wd_claimed_usdc += payout;
    depositor.realized_pnl += payout as i64 - ticket.aux as i64;
    depositor.total_withdrawn += payout;
    depositor.wd = Ticket::empty();
    depositor.last_action_ts = now;

    emit_event!(FlpClaimed {
        vault: vault_key,
        user: depositor.user,
        request_id: ticket.request_id,
        epoch: ticket.epoch,
        kind: 1,
        amount_in: ticket.amount,
        amount_out: payout,
    });
    Ok(())
}
