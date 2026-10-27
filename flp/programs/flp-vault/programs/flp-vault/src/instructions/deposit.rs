use anchor_lang::prelude::*;
use anchor_spl::token::{Token, TokenAccount};
use anchor_spl::token_2022::Token2022;
use anchor_spl::token_interface::{Mint as IMint, TokenAccount as ITokenAccount};

use super::common::*;
use crate::error::FlpError;
use crate::events::*;
use crate::math;
use crate::state::*;

#[derive(Accounts)]
pub struct RequestDeposit<'info> {
    #[account(mut, has_one = deposit_escrow)]
    pub vault: Box<Account<'info, Vault>>,

    #[account(mut, has_one = vault, has_one = user,
        seeds = [DEPOSITOR_SEED, vault.key().as_ref(), user.key().as_ref()], bump = depositor.bump)]
    pub depositor: Box<Account<'info, Depositor>>,

    #[account(mut)]
    pub user: Signer<'info>,
    #[account(mut, token::authority = user, token::mint = vault.quote_mint)]
    pub user_quote_account: Box<Account<'info, TokenAccount>>,
    #[account(mut)]
    pub deposit_escrow: Box<Account<'info, TokenAccount>>,

    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}

pub fn request_deposit(ctx: Context<RequestDeposit>, amount: u64) -> Result<()> {
    let now = now_ts()?;
    let vault_key = ctx.accounts.vault.key();
    let vault = &mut ctx.accounts.vault;
    vault.require_trading_open()?;
    require!(!vault.is_pre_deposit(), FlpError::WrongVaultMode);
                                                                            
    require!(vault.status & status::UNWIND == 0, FlpError::WrongVaultMode);
    require!(vault.is_seeded(), FlpError::NotSeeded);
    require!(
        vault.status & status::BREAKER == 0,
        FlpError::BreakerTripped
    );
    require!(
        vault.status & (status::DEPOSITS_PAUSED | status::PUBLIC_DEPOSITS_PAUSED) == 0,
        FlpError::DepositsPaused
    );
    require!(
        amount >= vault.params.min_deposit.max(1),
        FlpError::BelowMinimum
    );
    if vault.params.tvl_cap > 0 {
        let projected = (vault.last_nav.nav_mid.max(0) as u128)
            + vault.total_pending_deposits() as u128
            + vault.held_deposits as u128
            + amount as u128;
        require!(
            projected <= vault.params.tvl_cap as u128,
            FlpError::CapExceeded
        );
    }

    let depositor = &mut ctx.accounts.depositor;


    let target = vault.target_epoch(now);
    let new_ticket = depositor.dep.is_empty();
    require!(
        new_ticket || depositor.dep.epoch == target,
        FlpError::TicketPending
    );

    let fee = math::fee_ceil(amount, vault.params.deposit_fee_bps)?;
    let net = amount - fee;

    transfer_quote(
        &ctx.accounts.token_program.to_account_info(),
        &ctx.accounts.user_quote_account.to_account_info(),
        &ctx.accounts.deposit_escrow.to_account_info(),
        &ctx.accounts.user.to_account_info(),
        &[],
        amount,
    )?;

    let slot = vault.pending_slot_mut(target)?;
    slot.deposits_gross = slot
        .deposits_gross
        .checked_add(amount)
        .ok_or(FlpError::MathOverflow)?;
    slot.deposits_net = slot
        .deposits_net
        .checked_add(net)
        .ok_or(FlpError::MathOverflow)?;
    if new_ticket {
        slot.n_deposits += 1;
        depositor.dep = Ticket {
            request_id: vault.next_request_id,
            epoch: target,
            amount: 0,
            aux: 0,
        };
        vault.next_request_id += 1;
    }
    depositor.dep.amount += amount;
    depositor.dep.aux += net;
    depositor.last_action_ts = now;

    emit_event!(FlpDepositRequested {
        vault: vault_key,
        user: depositor.user,
        request_id: depositor.dep.request_id,
        epoch: target,
        gross: amount,
        net,
    });
    Ok(())
}

#[derive(Accounts)]
pub struct CancelDeposit<'info> {
    #[account(mut, has_one = deposit_escrow)]
    pub vault: Box<Account<'info, Vault>>,
    #[account(mut, has_one = vault, has_one = user)]
    pub depositor: Box<Account<'info, Depositor>>,
                                   
    #[account(seeds = [AUTHORITY_SEED, vault.key().as_ref()], bump = vault.authority_bump)]
    pub authority: UncheckedAccount<'info>,
    pub user: Signer<'info>,
    #[account(mut, token::authority = user, token::mint = vault.quote_mint)]
    pub user_quote_account: Box<Account<'info, TokenAccount>>,
    #[account(mut)]
    pub deposit_escrow: Box<Account<'info, TokenAccount>>,
    pub token_program: Program<'info, Token>,
}

pub fn cancel_deposit(ctx: Context<CancelDeposit>) -> Result<()> {
    let now = now_ts()?;
    let vault = &mut ctx.accounts.vault;
    let depositor = &mut ctx.accounts.depositor;
    let ticket = depositor.dep;
    require!(!ticket.is_empty(), FlpError::NoTicket);
    require!(
        (vault.status & (status::CLOSED | status::UNWIND) != 0 && ticket.epoch >= vault.epoch)
            || vault.can_cancel(ticket.epoch, now),
        FlpError::PastCutoff
    );

    reverse_pending_deposit(vault, ticket)?;
    return_deposit_principal(
        vault,
        depositor,
        &ctx.accounts.token_program.to_account_info(),
        &ctx.accounts.deposit_escrow.to_account_info(),
        &ctx.accounts.user_quote_account.to_account_info(),
        &ctx.accounts.authority.to_account_info(),
        now,
    )?;

    Ok(())
}

#[derive(Accounts)]
pub struct ClaimDeposit<'info> {
    #[account(mut, has_one = share_mint)]
    pub vault: Box<Account<'info, Vault>>,
    #[account(mut, has_one = vault)]
    pub depositor: Box<Account<'info, Depositor>>,
    #[account(
        mut,
        seeds = [EPOCH_SEED, vault.key().as_ref(), &depositor.dep.epoch.to_le_bytes()],
        bump = epoch_record.bump,
    )]
    pub epoch_record: Box<Account<'info, EpochRecord>>,
                                                          
    #[account(seeds = [AUTHORITY_SEED, vault.key().as_ref()], bump = vault.authority_bump)]
    pub authority: UncheckedAccount<'info>,
    #[account(mut)]
    pub share_mint: Box<InterfaceAccount<'info, IMint>>,
                                                                            
    #[account(mut)]
    pub user_share_account: Box<InterfaceAccount<'info, ITokenAccount>>,
    pub token_2022_program: Program<'info, Token2022>,
}

                                                                             
                              
pub fn claim_deposit(ctx: Context<ClaimDeposit>) -> Result<()> {
    let now = now_ts()?;
    let vault_key = ctx.accounts.vault.key();
    let depositor = &mut ctx.accounts.depositor;
    let ticket = depositor.dep;
    require!(!ticket.is_empty(), FlpError::NoTicket);
    let record = &mut ctx.accounts.epoch_record;
    require!(!record.deposits_held(), FlpError::DepositHeldForRefund);
    require!(record.dep_den > 0, FlpError::NotSettled);
    require_ata(
        &ctx.accounts.user_share_account.key(),
        &depositor.user,
        &ctx.accounts.share_mint.key(),
        &anchor_spl::token_2022::ID,
    )?;

    let shares = math::apply_ratio_floor(ticket.aux, (record.dep_num, record.dep_den))?;
    let vault = &mut ctx.accounts.vault;
    let bump = [vault.authority_bump];
    let seeds = authority_seeds(&vault_key, &bump);
    mint_shares(
        &ctx.accounts.token_2022_program.to_account_info(),
        &ctx.accounts.share_mint.to_account_info(),
        &ctx.accounts.user_share_account.to_account_info(),
        &ctx.accounts.authority.to_account_info(),
        &[&seeds],
        shares,
    )?;

    vault.owed_unminted_shares = vault
        .owed_unminted_shares
        .checked_sub(shares)
        .ok_or(FlpError::MathOverflow)?;
    record.dep_claimed_net += ticket.aux;
    record.dep_claimed_shares += shares;
    if ticket.aux > 0 && record.dep_claimed_net == record.dep_net {
                                                                          
                                                                            
                                                                          
        let residue = record.dep_shares.checked_sub(record.dep_claimed_shares)
            .ok_or(FlpError::MathOverflow)?;
        vault.owed_unminted_shares = vault.owed_unminted_shares.checked_sub(residue)
            .ok_or(FlpError::MathOverflow)?;
    }

    if depositor.shares_held == 0 && depositor.pre_deposit_amount == 0 && shares > 0 {
        vault.stats.active_depositors += 1;
    }
    if depositor.first_deposit_ts == 0 {
        depositor.first_deposit_ts = now;
    }
    depositor.shares_held += shares;
    depositor.cost_basis += ticket.amount;
    depositor.total_deposited += ticket.amount;
    depositor.dep = Ticket::empty();
    depositor.last_action_ts = now;

    emit_event!(FlpClaimed {
        vault: vault_key,
        user: depositor.user,
        request_id: ticket.request_id,
        epoch: ticket.epoch,
        kind: 0,
        amount_in: ticket.amount,
        amount_out: shares,
    });
    Ok(())
}

#[derive(Accounts)]
pub struct RefundPendingDeposit<'info> {
    #[account(mut, has_one = deposit_escrow)]
    pub vault: Box<Account<'info, Vault>>,
    #[account(mut, has_one = vault)]
    pub depositor: Box<Account<'info, Depositor>>,
                                   
    #[account(seeds = [AUTHORITY_SEED, vault.key().as_ref()], bump = vault.authority_bump)]
    pub authority: UncheckedAccount<'info>,
    #[account(mut)]
    pub user_quote_account: Box<Account<'info, TokenAccount>>,
    #[account(mut)]
    pub deposit_escrow: Box<Account<'info, TokenAccount>>,
    pub token_program: Program<'info, Token>,
                                                                                
                                             
    #[account(mut)]
    pub epoch_record: Option<Box<Account<'info, EpochRecord>>>,
}

                                                                     
   
                                                                                
                                              
                                                                              
                                                             
   
                                                                                 
pub fn refund_pending_deposit(ctx: Context<RefundPendingDeposit>) -> Result<()> {
    let key = ctx.accounts.vault.key();
    let vault = &mut ctx.accounts.vault;
    let d = &mut ctx.accounts.depositor;
    let ticket = d.dep;
    require!(!ticket.is_empty(), FlpError::NoTicket);
    require_ata(
        &ctx.accounts.user_quote_account.key(),
        &d.user,
        &vault.quote_mint,
        &anchor_spl::token::ID,
    )?;
    if ticket.epoch >= vault.epoch {
        require!(
            vault.status & (status::UNWIND | status::CLOSED) != 0,
            FlpError::WrongVaultMode
        );
        reverse_pending_deposit(vault, ticket)?;
    } else {
        let record = ctx
            .accounts
            .epoch_record
            .as_mut()
            .ok_or(FlpError::WrongEpochRecord)?;
        require!(
            record.vault == key && record.epoch == ticket.epoch,
            FlpError::WrongEpochRecord
        );
        require!(record.deposits_held(), FlpError::PastCutoff);
                                                                          
        record.dep_claimed_net = record
            .dep_claimed_net
            .checked_add(ticket.aux)
            .ok_or(FlpError::MathOverflow)?;
        vault.held_deposits = vault
            .held_deposits
            .checked_sub(ticket.amount)
            .ok_or(FlpError::MathOverflow)?;
    }
    return_deposit_principal(
        vault,
        d,
        &ctx.accounts.token_program.to_account_info(),
        &ctx.accounts.deposit_escrow.to_account_info(),
        &ctx.accounts.user_quote_account.to_account_info(),
        &ctx.accounts.authority.to_account_info(),
        now_ts()?,
    )?;
    Ok(())
}
