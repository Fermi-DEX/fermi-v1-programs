                                                                                        
use super::common::*;
use crate::{error::FlpError, events::*, fermi_cpi, state::*};
use anchor_lang::prelude::*;
use anchor_spl::token::{Mint, Token, TokenAccount};
use mango_v4::state::{Bank, FermiAccountFixed, FermiAccountLoader, FREE_ORDER_SLOT};

#[derive(Accounts)]
pub struct RescueForeignToken<'info> {
    #[account(has_one = admin)]
    pub vault: Box<Account<'info, Vault>>,
                                         
    #[account(seeds = [AUTHORITY_SEED, vault.key().as_ref()], bump = vault.authority_bump)]
    pub authority: UncheckedAccount<'info>,
    pub admin: Signer<'info>,
    pub mint: Box<Account<'info, Mint>>,
    #[account(mut, token::authority = authority, token::mint = mint)]
    pub source: Box<Account<'info, TokenAccount>>,
    #[account(mut, token::authority = admin, token::mint = mint)]
    pub destination: Box<Account<'info, TokenAccount>>,
    pub token_program: Program<'info, Token>,
}

pub fn rescue_foreign_token(ctx: Context<RescueForeignToken>, amount: u64) -> Result<()> {
    let v = &ctx.accounts.vault;
    require!(v.version == VAULT_VERSION, FlpError::UnsupportedEntrypoint);
    require!(ctx.remaining_accounts.is_empty(), FlpError::InvalidParams);
    let mint = ctx.accounts.mint.key();
    require!(mint != v.quote_mint && mint != v.share_mint, FlpError::WrongTokenAccount);
    let source = ctx.accounts.source.key();
    let destination = ctx.accounts.destination.key();
    require!(source != destination && source != v.deposit_escrow && source != v.redemption_escrow,
        FlpError::WrongTokenAccount);
                                                                                           
    require_ata(&source, &ctx.accounts.authority.key(), &mint, &anchor_spl::token::ID)?;
    require_ata(&destination, &ctx.accounts.admin.key(), &mint, &anchor_spl::token::ID)?;
    require!(amount > 0 && amount <= ctx.accounts.source.amount, FlpError::BelowMinimum);
    let key = v.key();
    let bump = [v.authority_bump];
    let seeds = authority_seeds(&key, &bump);
    transfer_quote(&ctx.accounts.token_program.to_account_info(), &ctx.accounts.source.to_account_info(),
        &ctx.accounts.destination.to_account_info(), &ctx.accounts.authority.to_account_info(), &[&seeds], amount)?;
    emit_event!(FlpForeignTokenRescued { vault: key, admin: ctx.accounts.admin.key(), mint, amount, source, destination });
    Ok(())
}

#[derive(Accounts)]
pub struct ClearForeignCoreToken<'info> {
    #[account(has_one = admin)]
    pub vault: Box<Account<'info, Vault>>,
                                                                    
    #[account(seeds = [AUTHORITY_SEED, vault.key().as_ref()], bump = vault.authority_bump)]
    pub authority: UncheckedAccount<'info>,
    pub admin: Signer<'info>,
    pub fermi: FermiEditAccounts<'info>,
    #[account(mut)]
    pub bank: AccountLoader<'info, Bank>,
    #[account(mut, token::mint = mint, token::authority = fermi.group)]
    pub bank_vault: Box<Account<'info, TokenAccount>>,
                                                                                        
    pub oracle: UncheckedAccount<'info>,
    pub mint: Box<Account<'info, Mint>>,
    #[account(mut, token::mint = mint, token::authority = authority)]
    pub destination: Box<Account<'info, TokenAccount>>,
    pub token_program: Program<'info, Token>,
}

pub fn clear_foreign_core_token<'info>(ctx: Context<'_, '_, '_, 'info, ClearForeignCoreToken<'info>>) -> Result<()> {
    let v = &ctx.accounts.vault;
    ctx.accounts.fermi.check(v)?;
    require!(v.has_expanded_recovery(), FlpError::WrongRecoveryMarket);
    let mint = ctx.accounts.mint.key();
    require!(mint != v.quote_mint && mint != v.share_mint, FlpError::WrongTokenAccount);
    require_ata(&ctx.accounts.destination.key(), &ctx.accounts.authority.key(), &mint, &anchor_spl::token::ID)?;
    let token_index;
    {
        let bank = ctx.accounts.bank.load()?;
        require_keys_eq!(bank.group, v.group, FlpError::FermiAccountMismatch);
        require_keys_eq!(bank.vault, ctx.accounts.bank_vault.key(), FlpError::WrongTokenAccount);
        require_keys_eq!(bank.oracle, ctx.accounts.oracle.key(), FlpError::WrongTokenAccount);
        require_keys_eq!(bank.mint, mint, FlpError::WrongTokenAccount);
        token_index = bank.token_index;
        require!(token_index != v.quote_token_index, FlpError::UnsupportedPortfolio);
        let ai = ctx.accounts.fermi.fermi_account.to_account_info();
        let loader = AccountLoader::<FermiAccountFixed>::try_from(&ai)?;
        let account = loader.load_full()?;
        require_keys_eq!(account.fixed.owner, ctx.accounts.authority.key(), FlpError::FermiAccountMismatch);
        require_keys_eq!(account.fixed.group, v.group, FlpError::FermiAccountMismatch);
        let h = &account.header;
        require!(h.token_count() == 1 && h.serum3_count() == 0 && h.openbook_v2_count() == 0
            && h.token_conditional_swap_count() == 0 && account.active_perp_positions().next().is_none()
            && account.all_perp_orders().all(|o| o.market == FREE_ORDER_SLOT)
            && !account.fixed.is_in_health_region() && !account.fixed.being_liquidated(), FlpError::UnsupportedPortfolio);
        let position = account.token_position(token_index)?;
        require!(position.native(&bank) >= 0, FlpError::UnsupportedPortfolio);
    }
    let key = v.key();
    let bump = [v.authority_bump];
    let seeds = authority_seeds(&key, &bump);
    let destination = ctx.accounts.destination.to_account_info();
    let before = ctx.accounts.destination.amount;
    let bank = fermi_cpi::QuoteBank { bank: ctx.accounts.bank.to_account_info(),
        vault: ctx.accounts.bank_vault.to_account_info(), oracle: ctx.accounts.oracle.to_account_info(),
        token_program: ctx.accounts.token_program.to_account_info() };
    fermi_cpi::token_withdraw(&ctx.accounts.fermi.core(&ctx.accounts.authority.to_account_info()),
        &bank, destination.clone(), ctx.remaining_accounts, &[&seeds], u64::MAX)?;
    {
        let ai = ctx.accounts.fermi.fermi_account.to_account_info();
        let loader = AccountLoader::<FermiAccountFixed>::try_from(&ai)?;
        require!(loader.load_full()?.active_token_positions().next().is_none(), FlpError::UnsupportedPortfolio);
    }
    let amount = crate::nav::token_account_amount(&destination)?.checked_sub(before).ok_or(FlpError::MathOverflow)?;
    emit_event!(FlpForeignCoreTokenCleared { vault: key, admin: ctx.accounts.admin.key(), mint,
        token_index, amount, destination: *destination.key });
    Ok(())
}
