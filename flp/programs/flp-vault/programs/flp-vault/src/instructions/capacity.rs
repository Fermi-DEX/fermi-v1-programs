use super::common::*;
use crate::{error::FlpError, state::*};
use anchor_lang::prelude::*;
use mango_v4::state::{FermiAccountFixed, FermiAccountLoader};

#[derive(Accounts)]
pub struct ExpandMarketCapacity<'info> {
    #[account(mut, has_one = admin)]
    pub vault: Box<Account<'info, Vault>>,
                                                                     
    #[account(seeds = [AUTHORITY_SEED, vault.key().as_ref()], bump = vault.authority_bump)]
    pub authority: UncheckedAccount<'info>,
    pub fermi: FermiEditAccounts<'info>,
    #[account(mut, has_one = vault,
        seeds = [RECOVERY_TARGETS_SEED, vault.key().as_ref()], bump)]
    pub recovery_targets: Box<Account<'info, RecoveryTargets>>,
    #[account(mut)]
    pub admin: Signer<'info>,
    pub system_program: Program<'info, System>,
}

                                                                             
                                                                              
                                                                              
                     
pub fn expand_market_capacity(
    ctx: Context<ExpandMarketCapacity>,
    perp_count: u8,
    perp_oo_count: u8,
) -> Result<()> {
    let v = &ctx.accounts.vault;
    ctx.accounts.fermi.check(v)?;
    require!(
        v.version == VAULT_VERSION && v.has_expanded_recovery(),
        FlpError::InvalidParams
    );
    require!(!v.is_unwinding() && !v.is_closed(), FlpError::WrongVaultMode);
    require!(
        (1..=MAX_VAULT_PERPS as u8).contains(&perp_count) && (1..=64).contains(&perp_oo_count),
        FlpError::InvalidParams
    );
    let ai = ctx.accounts.fermi.fermi_account.to_account_info();
    let loader = AccountLoader::<FermiAccountFixed>::try_from(&ai)?;
    {
        let account = loader.load_full()?;
        require_keys_eq!(
            account.fixed.owner,
            ctx.accounts.authority.key(),
            FlpError::FermiAccountMismatch
        );
        let h = &account.header;
        require!(
            h.token_count() == 1
                && h.serum3_count() == 0
                && h.openbook_v2_count() == 0
                && h.token_conditional_swap_count() == 0
                && usize::from(perp_count) >= h.perp_count()
                && usize::from(perp_oo_count) >= h.perp_oo_count(),
            FlpError::InvalidParams
        );
    }
    let key = v.key();
    let bump = [v.authority_bump];
    let seeds = authority_seeds(&key, &bump);
    crate::fermi_cpi::invoke(ctx.accounts.fermi.fermi_program.to_account_info(),
        &mango_v4::accounts::AccountExpand {
            group: ctx.accounts.fermi.group.key(), account: *ai.key,
            owner: ctx.accounts.authority.key(), payer: ctx.accounts.admin.key(),
            system_program: ctx.accounts.system_program.key(),
        }, &[ctx.accounts.fermi.group.to_account_info(), ai, ctx.accounts.authority.to_account_info(),
            ctx.accounts.admin.to_account_info(), ctx.accounts.system_program.to_account_info()],
        &[], &[&seeds], &mango_v4::instruction::AccountExpand {
            token_count: 1, serum3_count: 0, perp_count, perp_oo_count,
        })?;
    emit_event!(crate::events::FlpMarketCapacityExpanded {
        vault: key,
        perp_count,
        perp_oo_count
    });
    Ok(())
}

                                                                           
pub fn split_recovery_accounts<'a, 'info>(
    vault: &Vault,
    key: &Pubkey,
    accounts: &'a [AccountInfo<'info>],
) -> Result<(&'a [AccountInfo<'info>], Option<&'a AccountInfo<'info>>)> {
    split_optional_recovery_accounts(vault, key, accounts, true)
}

                                                                             
                                                                           
pub fn split_optional_recovery_accounts<'a, 'info>(
    vault: &Vault,
    key: &Pubkey,
    accounts: &'a [AccountInfo<'info>],
    required: bool,
) -> Result<(&'a [AccountInfo<'info>], Option<&'a AccountInfo<'info>>)> {
    require!(vault.has_expanded_recovery(), FlpError::WrongRecoveryMarket);
    let expected =
        Pubkey::find_program_address(&[RECOVERY_TARGETS_SEED, key.as_ref()], &crate::ID).0;
    let last = match accounts.split_last() {
        Some((last, rest)) if *last.key == expected => (last, rest),
        _ if required => return err!(FlpError::WrongRecoveryMarket),
        _ => return Ok((accounts, None)),
    };
    require_keys_eq!(*last.0.owner, crate::ID, FlpError::WrongRecoveryMarket);
    let targets = RecoveryTargets::try_deserialize(&mut &last.0.try_borrow_data()?[..])?;
    require_keys_eq!(targets.vault, *key, FlpError::WrongRecoveryMarket);
    if vault.is_unwinding() { targets.check_generation(vault)?; }
    Ok((last.1, Some(last.0)))
}

pub fn read_targets(vault: &Vault, ai: Option<&AccountInfo>) -> Result<Option<RecoveryTargets>> {
    require!(vault.has_expanded_recovery(), FlpError::WrongRecoveryMarket);
    let ai = ai.ok_or(FlpError::WrongRecoveryMarket)?;
    let targets = RecoveryTargets::try_deserialize(&mut &ai.try_borrow_data()?[..])?;
    targets.check_generation(vault)?;
    Ok(Some(targets))
}

pub fn write_targets(ai: &AccountInfo, targets: &RecoveryTargets) -> Result<()> {
    require!(ai.is_writable, FlpError::WrongRecoveryMarket);
    targets.try_serialize(&mut &mut ai.try_borrow_mut_data()?[..])?;
    emit_event!(crate::events::FlpExpandedRecoveryTargets {
        vault: targets.vault,
        mode: targets.mode,
        batch_epoch: targets.batch_epoch,
        started_at: targets.started_at,
        market_indexes: targets.market_indexes,
        target_base_lots: targets.target_base_lots,
    });
    Ok(())
}

pub fn target_for(vault: &Vault, targets: Option<&RecoveryTargets>, market: u16) -> Result<i64> {
    require!(vault.has_expanded_recovery(), FlpError::WrongRecoveryMarket);
    let t = targets.ok_or(FlpError::WrongRecoveryMarket)?;
    t.check_generation(vault)?;
    let (indexes, lots) = (&t.market_indexes, &t.target_base_lots);
    let slot = indexes
        .iter()
        .position(|i| *i == market)
        .ok_or(FlpError::WrongRecoveryMarket)?;
    Ok(lots[slot])
}
