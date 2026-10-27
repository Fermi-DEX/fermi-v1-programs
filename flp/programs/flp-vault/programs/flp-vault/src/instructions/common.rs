use anchor_lang::prelude::*;
use anchor_spl::associated_token::get_associated_token_address_with_program_id;
use anchor_spl::token::{self, Token};
use anchor_spl::token_2022;

use crate::error::FlpError;
use crate::fermi_cpi::{FermiCore, QuoteBank};
use crate::state::{Vault, AUTHORITY_SEED};

                                                                              
                                                                            
                                                                      
                                                                             
                                                                            
                                                       
pub fn require_recovery_targets(vault: &Vault, account_ai: &AccountInfo,
    targets: Option<&crate::state::RecoveryTargets>) -> Result<()> {
    use mango_v4::state::{FermiAccountFixed, FermiAccountLoader};
    let loader = AccountLoader::<FermiAccountFixed>::try_from(account_ai)?;
    let account = loader.load_full()?;
    let forfeit_at = vault
        .unwind
        .started_at
        .saturating_add(vault.recovery_params.grace_full_s as i64);
    for p in account.active_perp_positions() {
        let target = super::capacity::target_for(vault, targets, p.market_index)?;
        require!(
            p.bids_base_lots == 0
                && p.asks_base_lots == 0
                && p.taker_base_lots == 0
                && p.taker_quote_lots == 0
                && p.base_position_lots
                    .checked_abs()
                    .ok_or(FlpError::MathOverflow)?
                    <= target.checked_abs().ok_or(FlpError::MathOverflow)?,
            FlpError::RecoveryNotComplete
        );
        let quote = p.quote_position_native;
        if vault.unwind.mode == 2 && quote > 0 {
            require!(now_ts()? >= forfeit_at, FlpError::RecoveryNotComplete);
        }
    }
    if vault.unwind.mode == 2 {
        for p in account.active_token_positions() {
            let indexed = p.indexed_position;
            require!(
                if p.token_index == vault.quote_token_index { indexed >= 0 } else { indexed == 0 },
                FlpError::RecoveryNotComplete
            );
        }
    }
    Ok(())
}

                                                                          
                                                                              
                                                                    
pub fn split_event_queues<'a, 'info>(
    fermi_account_ai: &AccountInfo<'info>,
    accounts: &'a [AccountInfo<'info>],
) -> Result<(&'a [AccountInfo<'info>], &'a [AccountInfo<'info>])> {
    use mango_v4::state::{FermiAccountFixed, FermiAccountLoader};
    let loader = AccountLoader::<FermiAccountFixed>::try_from(fermi_account_ai)?;
    let active = loader.load_full()?.active_perp_positions().count();
    require!(accounts.len() >= active, FlpError::WrongEventQueue);
    Ok(accounts.split_at(accounts.len() - active))
}

                                                                             
                                                                
pub fn require_no_pending_vault_events(
    vault: &Vault, account: &AccountInfo, health: &[AccountInfo], queues: &[AccountInfo],
) -> Result<()> {
    require!(pending_vault_events(vault, account, health, queues)? == 0,
        FlpError::UnconsumedVaultFills);
    Ok(())
}

pub fn pending_vault_events(
    vault: &Vault,
    fermi_account_ai: &AccountInfo,
    health_ais: &[AccountInfo],
    queues: &[AccountInfo],
) -> Result<u32> {
    crate::projection::pending(&vault.group, fermi_account_ai, health_ais, queues)
}

                                                                  
                                                              
#[derive(Accounts)]
pub struct FermiVaultAccounts<'info> {
                                                                    
    pub group: UncheckedAccount<'info>,
                                                                            
    #[account(mut)]
    pub fermi_account: UncheckedAccount<'info>,
                                                                         
    #[account(mut)]
    pub quote_bank: UncheckedAccount<'info>,
                                                                                          
    #[account(mut)]
    pub quote_bank_vault: UncheckedAccount<'info>,
                                                                
    pub quote_oracle: UncheckedAccount<'info>,
    pub fermi_program: Program<'info, mango_v4::program::MangoV4>,
    pub token_program: Program<'info, Token>,
}

                                                                                 
                                                                              
                                                                      
pub fn compute_vault_nav<'info>(
    vault: &Vault,
    fermi: &FermiVaultAccounts<'info>,
    authority: &AccountInfo<'info>,
    idle_account: &AccountInfo<'info>,
    health_ais: &[AccountInfo<'info>],
    upnl_haircut_bps: u16,
) -> Result<crate::nav::NavComputation> {
    {
        use mango_v4::state::{FermiAccountFixed, FermiAccountLoader};
        let ai = fermi.fermi_account.to_account_info();
        let loader = AccountLoader::<FermiAccountFixed>::try_from(&ai)?;
        require_keys_eq!(loader.load_full()?.fixed.owner, *authority.key, FlpError::FermiAccountMismatch);
    }
    require_ata(
        idle_account.key,
        authority.key,
        &vault.quote_mint,
        &anchor_spl::token::ID,
    )?;
    let mut nav = crate::nav::compute_nav(
        &vault.group,
        &fermi.fermi_account.to_account_info(),
        health_ais,
        &fermi.quote_bank.to_account_info(),
        &fermi.quote_oracle.to_account_info(),
        upnl_haircut_bps,
    )?;
    nav.include_idle_quote(crate::nav::token_account_amount(idle_account)?)?;
    Ok(nav)
}

pub fn compute_projected_vault_nav<'info>(
    vault: &Vault,
    fermi: &FermiVaultAccounts<'info>,
    authority: &AccountInfo<'info>,
    idle_account: &AccountInfo<'info>,
    health_ais: &[AccountInfo<'info>],
    queues: &[AccountInfo<'info>],
    upnl_haircut_bps: u16,
) -> Result<crate::nav::NavComputation> {
    {
        use mango_v4::state::{FermiAccountFixed, FermiAccountLoader};
        let ai = fermi.fermi_account.to_account_info();
        let loader = AccountLoader::<FermiAccountFixed>::try_from(&ai)?;
        require_keys_eq!(loader.load_full()?.fixed.owner, *authority.key, FlpError::FermiAccountMismatch);
    }
    require_ata(
        idle_account.key,
        authority.key,
        &vault.quote_mint,
        &anchor_spl::token::ID,
    )?;
    let mut nav = crate::nav::compute_projected_nav(
        &fermi.group.to_account_info(),
        &fermi.fermi_account.to_account_info(),
        health_ais,
        queues,
        &fermi.quote_bank.to_account_info(),
        &fermi.quote_oracle.to_account_info(),
        upnl_haircut_bps,
    )?;
    nav.include_idle_quote(crate::nav::token_account_amount(idle_account)?)?;
    Ok(nav)
}

impl<'info> FermiVaultAccounts<'info> {
    pub fn check(&self, vault: &Vault) -> Result<()> {
        require!(vault.version == crate::state::VAULT_VERSION, FlpError::UnsupportedEntrypoint);
        require_keys_eq!(
            self.group.key(),
            vault.group,
            FlpError::FermiAccountMismatch
        );
        require_keys_eq!(
            self.fermi_account.key(),
            vault.fermi_account,
            FlpError::FermiAccountMismatch
        );
        require_keys_eq!(
            self.quote_bank.key(),
            vault.quote_bank,
            FlpError::FermiAccountMismatch
        );
                                                                                     
                                                                             
                                                                        
        let bank_ai = self.quote_bank.to_account_info();
        let bank = AccountLoader::<mango_v4::state::Bank>::try_from(&bank_ai)?;
        let bank = bank.load()?;
        require_keys_eq!(bank.group, vault.group, FlpError::FermiAccountMismatch);
        require_keys_eq!(bank.mint, vault.quote_mint, FlpError::FermiAccountMismatch);
        require!(bank.token_index == vault.quote_token_index, FlpError::FermiAccountMismatch);
        require!(vault.has_expanded_recovery(), FlpError::WrongRecoveryMarket);
        require_keys_eq!(
            self.quote_bank_vault.key(),
            bank.vault,
            FlpError::FermiAccountMismatch
        );
        require_keys_eq!(
            self.quote_oracle.key(),
            bank.oracle,
            FlpError::FermiAccountMismatch
        );
        Ok(())
    }

    pub fn core(&self, authority: &AccountInfo<'info>) -> FermiCore<'info> {
        FermiCore {
            program: self.fermi_program.to_account_info(),
            group: self.group.to_account_info(),
            account: self.fermi_account.to_account_info(),
            authority: authority.clone(),
        }
    }

    pub fn bank(&self) -> QuoteBank<'info> {
        QuoteBank {
            bank: self.quote_bank.to_account_info(),
            vault: self.quote_bank_vault.to_account_info(),
            oracle: self.quote_oracle.to_account_info(),
            token_program: self.token_program.to_account_info(),
        }
    }
}

                                                                          
#[derive(Accounts)]
pub struct FermiEditAccounts<'info> {
                                      
    pub group: UncheckedAccount<'info>,
                                              
    #[account(mut)]
    pub fermi_account: UncheckedAccount<'info>,
    pub fermi_program: Program<'info, mango_v4::program::MangoV4>,
}

impl<'info> FermiEditAccounts<'info> {
    pub fn check(&self, vault: &Vault) -> Result<()> {
        require!(vault.version == crate::state::VAULT_VERSION, FlpError::UnsupportedEntrypoint);
        require_keys_eq!(
            self.group.key(),
            vault.group,
            FlpError::FermiAccountMismatch
        );
        require_keys_eq!(
            self.fermi_account.key(),
            vault.fermi_account,
            FlpError::FermiAccountMismatch
        );
        Ok(())
    }

    pub fn core(&self, authority: &AccountInfo<'info>) -> FermiCore<'info> {
        FermiCore {
            program: self.fermi_program.to_account_info(),
            group: self.group.to_account_info(),
            account: self.fermi_account.to_account_info(),
            authority: authority.clone(),
        }
    }
}

pub fn authority_seeds<'a>(vault_key: &'a Pubkey, bump: &'a [u8; 1]) -> [&'a [u8]; 3] {
    [AUTHORITY_SEED, vault_key.as_ref(), bump]
}

pub fn now_ts() -> Result<i64> {
    Ok(Clock::get()?.unix_timestamp)
}

pub fn require_ata(
    account: &Pubkey,
    wallet: &Pubkey,
    mint: &Pubkey,
    token_program: &Pubkey,
) -> Result<()> {
    let expected = get_associated_token_address_with_program_id(wallet, mint, token_program);
    require_keys_eq!(*account, expected, FlpError::WrongTokenAccount);
    Ok(())
}

pub fn transfer_quote<'info>(
    token_program: &AccountInfo<'info>,
    from: &AccountInfo<'info>,
    to: &AccountInfo<'info>,
    authority: &AccountInfo<'info>,
    signer: &[&[&[u8]]],
    amount: u64,
) -> Result<()> {
    if amount == 0 {
        return Ok(());
    }
    token::transfer(
        CpiContext::new_with_signer(
            token_program.clone(),
            token::Transfer {
                from: from.clone(),
                to: to.clone(),
                authority: authority.clone(),
            },
            signer,
        ),
        amount,
    )
}

pub fn mint_shares<'info>(
    token_2022_program: &AccountInfo<'info>,
    mint: &AccountInfo<'info>,
    to: &AccountInfo<'info>,
    authority: &AccountInfo<'info>,
    signer: &[&[&[u8]]],
    amount: u64,
) -> Result<()> {
    if amount == 0 {
        return Ok(());
    }
    token_2022::mint_to(
        CpiContext::new_with_signer(
            token_2022_program.clone(),
            token_2022::MintTo {
                mint: mint.clone(),
                to: to.clone(),
                authority: authority.clone(),
            },
            signer,
        ),
        amount,
    )
}

                                                                       
                                                                 
pub fn pricing_supply(mint_supply: u64, vault: &Vault) -> Result<u64> {
    mint_supply
        .checked_add(vault.owed_unminted_shares)
        .and_then(|v| v.checked_add(vault.locked_shares))
        .ok_or_else(|| error!(FlpError::MathOverflow))
}

                                                                              
                                                                             
pub fn apply_fee_accrual<'info>(
    vault: &mut Account<'info, Vault>,
    fees: crate::accounting::FeeAccrual,
    now: i64,
    token_program: &AccountInfo<'info>,
    mint: &AccountInfo<'info>,
    recipient: &AccountInfo<'info>,
    authority: &AccountInfo<'info>,
) -> Result<()> {
    let key = vault.key();
    let bump = [vault.authority_bump];
    let seeds = authority_seeds(&key, &bump);
    mint_shares(
        token_program,
        mint,
        recipient,
        authority,
        &[&seeds],
        fees.total()?,
    )?;
    vault.stats.cum_mgmt_fee_shares = vault
        .stats
        .cum_mgmt_fee_shares
        .checked_add(fees.management)
        .ok_or(FlpError::MathOverflow)?;
    vault.stats.cum_perf_fee_shares = vault
        .stats
        .cum_perf_fee_shares
        .checked_add(fees.performance)
        .ok_or(FlpError::MathOverflow)?;
    vault.hwm_price_e12 = fees.hwm;
    vault.last_fee_accrual_ts = now;
    emit_event!(crate::events::FlpFeesAccrued {
        vault: key,
        epoch: vault.epoch,
        mgmt_shares: fees.management,
        perf_shares: fees.performance,
        hwm_price_e12: fees.hwm
    });
    Ok(())
}

pub fn reverse_pending_deposit(vault: &mut Vault, ticket: crate::state::Ticket) -> Result<()> {
    let slot = vault.pending_slot_mut(ticket.epoch)?;
    slot.deposits_gross = slot
        .deposits_gross
        .checked_sub(ticket.amount)
        .ok_or(FlpError::MathOverflow)?;
    slot.deposits_net = slot
        .deposits_net
        .checked_sub(ticket.aux)
        .ok_or(FlpError::MathOverflow)?;
    slot.n_deposits = slot
        .n_deposits
        .checked_sub(1)
        .ok_or(FlpError::MathOverflow)?;
    Ok(())
}

                                                                            
                                                                             
pub fn return_deposit_principal<'info>(
    vault: &Account<'info, Vault>,
    depositor: &mut crate::state::Depositor,
    token_program: &AccountInfo<'info>,
    escrow: &AccountInfo<'info>,
    destination: &AccountInfo<'info>,
    authority: &AccountInfo<'info>,
    now: i64,
) -> Result<()> {
    let ticket = depositor.dep;
    let key = vault.key();
    let bump = [vault.authority_bump];
    let seeds = authority_seeds(&key, &bump);
    transfer_quote(
        token_program,
        escrow,
        destination,
        authority,
        &[&seeds],
        ticket.amount,
    )?;
    depositor.dep = crate::state::Ticket::empty();
    depositor.last_action_ts = now;
    emit_event!(crate::events::FlpDepositCancelled {
        vault: key,
        user: depositor.user,
        request_id: ticket.request_id,
        epoch: ticket.epoch,
        gross: ticket.amount
    });
    Ok(())
}
