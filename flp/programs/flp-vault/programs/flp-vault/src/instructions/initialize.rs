use anchor_lang::prelude::*;
use anchor_lang::solana_program::program::{invoke, invoke_signed};
use anchor_lang::solana_program::system_instruction;
use anchor_spl::associated_token::AssociatedToken;
use anchor_spl::token::{Mint, Token, TokenAccount};
use anchor_spl::token_2022::spl_token_2022::{
    self, extension::ExtensionType, instruction as t22_ix,
};
use anchor_spl::token_2022::Token2022;
use mango_v4::state::{Bank, Group};

use super::common::*;
use crate::error::FlpError;
use crate::events::*;
use crate::fermi_cpi;
use crate::math::PRICE_SCALE;
use crate::state::*;

#[derive(Accounts)]
#[instruction(vault_id: u16)]
pub struct InitializeVault<'info> {
    #[account(init, payer = admin, space = 8 + FundingWindow::INIT_SPACE,
        seeds = [FUNDING_WINDOW_SEED, vault.key().as_ref()], bump)]
    pub funding_window: Box<Account<'info, FundingWindow>>,
    #[account(
        init,
        payer = admin,
        space = 8 + Vault::INIT_SPACE,
        seeds = [VAULT_SEED, group.key().as_ref(), &vault_id.to_le_bytes()],
        bump,
    )]
    pub vault: Box<Account<'info, Vault>>,

                                                                                 
    #[account(seeds = [AUTHORITY_SEED, vault.key().as_ref()], bump)]
    pub authority: UncheckedAccount<'info>,

    pub group: AccountLoader<'info, Group>,

    #[account(constraint = quote_bank.load()?.group == group.key() @ FlpError::FermiAccountMismatch)]
    pub quote_bank: AccountLoader<'info, Bank>,

    #[account(constraint = quote_mint.key() == quote_bank.load()?.mint @ FlpError::FermiAccountMismatch)]
    pub quote_mint: Box<Account<'info, Mint>>,

                                                                                    
    #[account(mut, seeds = [SHARE_MINT_SEED, vault.key().as_ref()], bump)]
    pub share_mint: UncheckedAccount<'info>,

    #[account(
        init,
        payer = admin,
        seeds = [ESCROW_SEED, vault.key().as_ref(), DEPOSIT_ESCROW_TAG],
        bump,
        token::mint = quote_mint,
        token::authority = authority,
    )]
    pub deposit_escrow: Box<Account<'info, TokenAccount>>,

    #[account(
        init,
        payer = admin,
        seeds = [ESCROW_SEED, vault.key().as_ref(), REDEMPTION_ESCROW_TAG],
        bump,
        token::mint = quote_mint,
        token::authority = authority,
    )]
    pub redemption_escrow: Box<Account<'info, TokenAccount>>,

    #[account(mut)]
    pub admin: Signer<'info>,

    pub token_program: Program<'info, Token>,
    pub token_2022_program: Program<'info, Token2022>,
    pub system_program: Program<'info, System>,
    pub rent: Sysvar<'info, Rent>,
}

pub fn initialize_vault(
    ctx: Context<InitializeVault>,
    vault_id: u16,
    params: VaultParams,
    guardian: Pubkey,
    fee_recipient: Pubkey,
) -> Result<()> {
    params.validate()?;
    let vault_key = ctx.accounts.vault.key();
    let share_mint_bump = ctx.bumps["share_mint"];
    let authority = ctx.accounts.authority.key();

                                                                              
                                                                   
    let space = ExtensionType::get_account_len::<spl_token_2022::state::Mint>(&[
        ExtensionType::NonTransferable,
    ]);
    let lamports = Rent::get()?.minimum_balance(space);
    let share_mint = ctx.accounts.share_mint.to_account_info();
    invoke_signed(
        &system_instruction::create_account(
            ctx.accounts.admin.key,
            share_mint.key,
            lamports,
            space as u64,
            &spl_token_2022::ID,
        ),
        &[
            ctx.accounts.admin.to_account_info(),
            share_mint.clone(),
            ctx.accounts.system_program.to_account_info(),
        ],
        &[&[SHARE_MINT_SEED, vault_key.as_ref(), &[share_mint_bump]]],
    )?;
    invoke(
        &t22_ix::initialize_non_transferable_mint(&spl_token_2022::ID, share_mint.key)?,
        &[share_mint.clone()],
    )?;
    invoke(
        &t22_ix::initialize_mint2(
            &spl_token_2022::ID,
            share_mint.key,
            &authority,
            None,
            ctx.accounts.quote_mint.decimals,
        )?,
        &[share_mint.clone()],
    )?;

    let now = now_ts()?;
    let bank = ctx.accounts.quote_bank.load()?;
    let vault = &mut ctx.accounts.vault;
    vault.version = VAULT_VERSION;
    vault.last_nav.init_ratio_bps = HEALTH_UNAVAILABLE;
    let window = &mut ctx.accounts.funding_window;
    window.vault = vault_key;
    window.version = 1;
    window.closed_epoch = NONE_EPOCH;
    window.eligible = false;
    vault.bump = ctx.bumps["vault"];
    vault.authority_bump = ctx.bumps["authority"];
    vault.share_mint_bump = share_mint_bump;
    vault.status = status::DEPOSITS_PAUSED | status::PRE_DEPOSIT;
    vault.vault_id = vault_id;
    vault.quote_token_index = bank.token_index;
    vault.group = ctx.accounts.group.key();
    vault.fermi_account = Pubkey::default();
    vault.quote_bank = ctx.accounts.quote_bank.key();
    vault.quote_mint = ctx.accounts.quote_mint.key();
    vault.share_mint = share_mint.key();
    vault.deposit_escrow = ctx.accounts.deposit_escrow.key();
    vault.redemption_escrow = ctx.accounts.redemption_escrow.key();
    vault.admin = ctx.accounts.admin.key();
    vault.pending_admin = Pubkey::default();
    vault.guardian = guardian;
    vault.fee_recipient = fee_recipient;
    vault.params = params.into();
    vault.reserved[9] = 1;
    vault.set_equity_window(params.equity_window_s);
                                                                                 
    vault.recovery_params =
        RecoveryParams::initial_for(params.epoch_duration_s, params.equity_window_s);
    vault.epoch = 0;
    vault.epoch_started_at = now;
    vault.pending = [PendingTotals::empty(); PENDING_SLOTS];
    vault.deferred_head = NONE_EPOCH;
    vault.deferred_tail = NONE_EPOCH;
    vault.hwm_price_e12 = PRICE_SCALE;
    vault.breaker_ref_price_e12 = PRICE_SCALE;
    vault.last_fee_accrual_ts = now;
    vault.breaker_ref_ts = now;

    emit_event!(FlpVaultInitialized {
        vault: vault_key,
        group: vault.group,
        vault_id,
        share_mint: vault.share_mint,
    });
    Ok(())
}

#[derive(Accounts)]
pub struct InitFermiAccountV4<'info> {
    #[account(mut, has_one = admin)]
    pub vault: Box<Account<'info, Vault>>,
                                                                    
    #[account(seeds = [AUTHORITY_SEED, vault.key().as_ref()], bump = vault.authority_bump)]
    pub authority: UncheckedAccount<'info>,
    pub fermi: FermiEditAccounts<'info>,
    #[account(init, payer = admin, space = 8 + RecoveryTargets::INIT_SPACE,
        seeds = [RECOVERY_TARGETS_SEED, vault.key().as_ref()], bump)]
    pub recovery_targets: Box<Account<'info, RecoveryTargets>>,
    #[account(mut)]
    pub admin: Signer<'info>,
    pub system_program: Program<'info, System>,
}

                                                                             
                                                                                
                                                                            
                                                                           
                                 
pub fn init_fermi_account_v4(
    ctx: Context<InitFermiAccountV4>,
    perp_count: u8,
    perp_oo_count: u8,
    token_count: u8,
) -> Result<()> {
    require!(
        (1..=MAX_VAULT_PERPS as u8).contains(&perp_count)
            && (1..=64).contains(&perp_oo_count)
            && token_count == 1,
        FlpError::InvalidParams
    );
    require!(
        mango_v4::state::FermiAccount::space(token_count, 0, perp_count, perp_oo_count, 0, 0)
            <= anchor_lang::solana_program::entrypoint::MAX_PERMITTED_DATA_INCREASE,
        FlpError::InvalidParams
    );
    let a = ctx.accounts;
    create_core_account(&mut a.vault, &a.authority, &a.fermi, &a.admin, &a.system_program,
        perp_count, perp_oo_count, token_count)?;
    let key = a.vault.key();
    let targets = &mut a.recovery_targets;
    targets.vault = key;
    targets.mode = 0;
    targets.batch_epoch = NONE_EPOCH;
    targets.started_at = 0;
    targets.market_indexes = [u16::MAX; MAX_VAULT_PERPS];
    targets.target_base_lots = [0; MAX_VAULT_PERPS];
    a.vault.reserved[18] = 1;
    emit_event!(FlpMarketCapacityExpanded {
        vault: key,
        perp_count,
        perp_oo_count
    });
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn create_core_account<'info>(
    vault: &mut Account<'info, Vault>,
    authority: &UncheckedAccount<'info>,
    fermi: &FermiEditAccounts<'info>,
    admin: &Signer<'info>,
    system_program: &Program<'info, System>,
    perp_count: u8,
    perp_oo_count: u8,
    token_count: u8,
) -> Result<()> {
    let vault_key = vault.key();
    require_keys_eq!(
        vault.fermi_account,
        Pubkey::default(),
        FlpError::InvalidParams
    );
    require_keys_eq!(
        fermi.group.key(),
        vault.group,
        FlpError::FermiAccountMismatch
    );

    let account_num = vault.vault_id as u32;
    let (expected, _) = Pubkey::find_program_address(
        &[
            b"FermiAccount",
            vault.group.as_ref(),
            authority.key.as_ref(),
            &account_num.to_le_bytes(),
        ],
        &mango_v4::ID,
    );
    require_keys_eq!(
        fermi.fermi_account.key(),
        expected,
        FlpError::FermiAccountMismatch
    );

    let bump = [vault.authority_bump];
    let seeds = authority_seeds(&vault_key, &bump);
    let signer: &[&[&[u8]]] = &[&seeds];
    let core = fermi.core(&authority.to_account_info());
    fermi_cpi::account_create(
        &core,
        admin.to_account_info(),
        system_program.to_account_info(),
        signer,
        account_num,
        token_count,
        perp_count,
        perp_oo_count,
        format!("flp-vault-{}", vault.vault_id),
    )?;
    let delegate = Pubkey::default();
    vault.fermi_account = expected;
    vault.delegate = delegate;
    emit_event!(FlpFermiAccountCreated {
        vault: vault_key,
        fermi_account: expected,
        delegate,
    });
    Ok(())
}

#[derive(Accounts)]
pub struct SeedVault<'info> {
    #[account(address = vault.quote_mint)]
    pub quote_mint: Box<Account<'info, Mint>>,
                                                                            
    #[account(mut)]
    pub idle_account: UncheckedAccount<'info>,
    #[account(mut, has_one = admin, has_one = share_mint)]
    pub vault: Box<Account<'info, Vault>>,

                                   
    #[account(seeds = [AUTHORITY_SEED, vault.key().as_ref()], bump = vault.authority_bump)]
    pub authority: UncheckedAccount<'info>,

    pub fermi: FermiVaultAccounts<'info>,

    #[account(mut)]
    pub admin: Signer<'info>,
    #[account(mut, token::authority = admin, token::mint = vault.quote_mint)]
    pub admin_quote_account: Box<Account<'info, TokenAccount>>,

                                                                   
    #[account(mut)]
    pub share_mint: UncheckedAccount<'info>,
                                                                           
    #[account(mut)]
    pub dead_share_account: UncheckedAccount<'info>,

    pub token_2022_program: Program<'info, Token2022>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
}

                                                                               
                                                                             
                                                                    
pub fn seed_vault<'info>(
    ctx: Context<'_, '_, '_, 'info, SeedVault<'info>>,
    amount: u64,
) -> Result<()> {
    let vault_key = ctx.accounts.vault.key();
    let vault = &ctx.accounts.vault;
    require!(!vault.is_seeded(), FlpError::AlreadySeeded);
    require!(amount >= 1_000_000, FlpError::BelowMinimum);
    ctx.accounts.fermi.check(vault)?;
    require_ata(
        ctx.accounts.dead_share_account.key,
        ctx.accounts.authority.key,
        &vault.share_mint,
        &anchor_spl::token_2022::ID,
    )?;

    let bump = [vault.authority_bump];
    let seeds = authority_seeds(&vault_key, &bump);
    let signer: &[&[&[u8]]] = &[&seeds];
    let authority = ctx.accounts.authority.to_account_info();

    require_ata(ctx.accounts.idle_account.key, authority.key, &vault.quote_mint, &anchor_spl::token::ID)?;
    anchor_spl::associated_token::create_idempotent(CpiContext::new(
        ctx.accounts.associated_token_program.to_account_info(),
        anchor_spl::associated_token::Create {
            payer: ctx.accounts.admin.to_account_info(),
            associated_token: ctx.accounts.idle_account.to_account_info(),
            authority: authority.clone(), mint: ctx.accounts.quote_mint.to_account_info(),
            system_program: ctx.accounts.system_program.to_account_info(),
            token_program: ctx.accounts.fermi.token_program.to_account_info(),
        },
    ))?;
    transfer_quote(&ctx.accounts.fermi.token_program.to_account_info(),
        &ctx.accounts.admin_quote_account.to_account_info(), &ctx.accounts.idle_account.to_account_info(),
        &ctx.accounts.admin.to_account_info(), &[], amount)?;

    anchor_spl::associated_token::create_idempotent(CpiContext::new(
        ctx.accounts.associated_token_program.to_account_info(),
        anchor_spl::associated_token::Create {
            payer: ctx.accounts.admin.to_account_info(),
            associated_token: ctx.accounts.dead_share_account.to_account_info(),
            authority: authority.clone(),
            mint: ctx.accounts.share_mint.to_account_info(),
            system_program: ctx.accounts.system_program.to_account_info(),
            token_program: ctx.accounts.token_2022_program.to_account_info(),
        },
    ))?;
                                                                                  
    mint_shares(
        &ctx.accounts.token_2022_program.to_account_info(),
        &ctx.accounts.share_mint.to_account_info(),
        &ctx.accounts.dead_share_account.to_account_info(),
        &authority,
        signer,
        amount,
    )?;

    let now = now_ts()?;
    let vault = &mut ctx.accounts.vault;
    vault.dead_shares = amount;
    vault.epoch_started_at = now;
    vault.last_fee_accrual_ts = now;
    vault.breaker_ref_ts = now;
    vault.stats.inception_ts = now;
    vault.stats.total_deposited_gross = amount;
    vault.stats.net_deposits = amount as i64;
    emit_event!(FlpSeeded {
        vault: vault_key,
        amount,
        shares: amount,
    });
    Ok(())
}

