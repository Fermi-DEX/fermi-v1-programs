                                                                            
                                                                            
                        

use anchor_lang::prelude::*;


pub struct FermiCore<'info> {
    pub program: AccountInfo<'info>,
    pub group: AccountInfo<'info>,
    pub account: AccountInfo<'info>,
    pub authority: AccountInfo<'info>,
}

pub struct QuoteBank<'info> {
    pub bank: AccountInfo<'info>,
    pub vault: AccountInfo<'info>,
    pub oracle: AccountInfo<'info>,
    pub token_program: AccountInfo<'info>,
}

pub fn account_create<'info>(
    core: &FermiCore<'info>,
    payer: AccountInfo<'info>,
    system_program: AccountInfo<'info>,
    signer: &[&[&[u8]]],
    account_num: u32,
    token_count: u8,
    perp_count: u8,
    perp_oo_count: u8,
    name: String,
) -> Result<()> {
    invoke(core.program.clone(), &mango_v4::accounts::AccountCreateV2 {
        group: *core.group.key, account: *core.account.key, owner: *core.authority.key,
        payer: *payer.key, system_program: *system_program.key,
    }, &[core.group.clone(), core.account.clone(), core.authority.clone(), payer, system_program],
    &[], signer, &mango_v4::instruction::AccountCreateV2 {
        account_num, token_count, serum3_count: 0, perp_count, perp_oo_count,
        token_conditional_swap_count: 0, name,
    })
}

                                                                              
                                                                            
                                         
pub fn perp_deactivate_position<'info>(
    core: &FermiCore<'info>,
    perp_market: AccountInfo<'info>,
    signer: &[&[&[u8]]],
) -> Result<()> {
    invoke(core.program.clone(), &mango_v4::accounts::PerpDeactivatePosition {
        group: *core.group.key, account: *core.account.key, owner: *core.authority.key,
        perp_market: *perp_market.key,
    }, &[core.group.clone(), core.account.clone(), core.authority.clone(), perp_market],
    &[], signer, &mango_v4::instruction::PerpDeactivatePosition {})
}

pub fn set_delegate<'info>(
    core: &FermiCore<'info>,
    signer: &[&[&[u8]]],
    delegate: Pubkey,
) -> Result<()> {
    invoke(core.program.clone(), &mango_v4::accounts::AccountEdit {
        group: *core.group.key, account: *core.account.key, owner: *core.authority.key,
    }, &[core.group.clone(), core.account.clone(), core.authority.clone()],
    &[], signer, &mango_v4::instruction::AccountEdit {
        name_opt: None, delegate_opt: Some(delegate), temporary_delegate_opt: None,
        temporary_delegate_expiry_opt: None,
    })
}

pub fn token_deposit<'info>(
    core: &FermiCore<'info>,
    bank: &QuoteBank<'info>,
    from: AccountInfo<'info>,
    token_authority: AccountInfo<'info>,
    health_ais: &[AccountInfo<'info>],
    signer: &[&[&[u8]]],
    amount: u64,
) -> Result<()> {
    if amount == 0 {
        return Ok(());
    }
    let fixed = [
        core.group.clone(), core.account.clone(), core.authority.clone(),
        bank.bank.clone(), bank.vault.clone(), bank.oracle.clone(),
        from, token_authority, bank.token_program.clone(),
    ];
    invoke_quote(
        core, &fixed, &TOKEN_DEPOSIT_PRIVILEGES, health_ais, signer,
        &mango_v4::instruction::TokenDeposit { amount, reduce_only: false },
    )
}

pub fn token_withdraw<'info>(
    core: &FermiCore<'info>,
    bank: &QuoteBank<'info>,
    to: AccountInfo<'info>,
    health_ais: &[AccountInfo<'info>],
    signer: &[&[&[u8]]],
    amount: u64,
) -> Result<()> {
    if amount == 0 {
        return Ok(());
    }
    let fixed = [
        core.group.clone(), core.account.clone(), core.authority.clone(),
        bank.bank.clone(), bank.vault.clone(), bank.oracle.clone(),
        to, bank.token_program.clone(),
    ];
    invoke_quote(
        core, &fixed, &TOKEN_WITHDRAW_PRIVILEGES, health_ais, signer,
        &mango_v4::instruction::TokenWithdraw { amount, allow_borrow: false },
    )
}

                                                                             
                                                                                
pub fn assert_payout_health<'info>(core: &FermiCore<'info>, health: &[AccountInfo<'info>], floor_bps: u16) -> Result<()> {
    use mango_v4::accounts_ix::HealthCheckKind;
    let (minimum, kind) = if floor_bps == 0 { (0.0, HealthCheckKind::Init) }
        else { (f64::from(floor_bps) / 100.0, HealthCheckKind::InitRatio) };
    assert_health(core, health, minimum, kind)?;
    emit_event!(crate::events::FlpHealthAssertionV2 {
        account: *core.account.key, metric: if floor_bps == 0 { 2 } else { 3 },
        threshold_bps: floor_bps, passed: true,
    });
    Ok(())
}

pub fn assert_liquidation_end<'info>(core: &FermiCore<'info>, health: &[AccountInfo<'info>]) -> Result<()> {
    assert_health(core, health, 0.0, mango_v4::accounts_ix::HealthCheckKind::LiquidationEnd)
}

fn assert_health<'info>(core: &FermiCore<'info>, health: &[AccountInfo<'info>], minimum: f64,
    kind: mango_v4::accounts_ix::HealthCheckKind) -> Result<()> {
    invoke_quote(core, &[core.group.clone(), core.account.clone()],
        &[(false, false), (true, false)], health, &[],
        &mango_v4::instruction::HealthCheck { min_health_value: minimum, check_kind: kind })
}

                                                                              
                                                                        
                                                                       
const TOKEN_DEPOSIT_PRIVILEGES: [(bool, bool); 9] = [
    (false, false), (true, false), (false, true), (true, false), (true, false),
    (false, false), (true, false), (false, true), (false, false),
];
                                                                               
                                                                      
const TOKEN_WITHDRAW_PRIVILEGES: [(bool, bool); 8] = [
    (false, false), (true, false), (false, true), (true, false), (true, false),
    (false, false), (true, false), (false, false),
];

                                                                             
                                                                           
                                                                       
                                                                            
                                                                             
fn invoke_quote<'info, D: anchor_lang::Discriminator + AnchorSerialize>(
    core: &FermiCore<'info>,
    fixed: &[AccountInfo<'info>],
    privileges: &[(bool, bool)],
    health_ais: &[AccountInfo<'info>],
    signer: &[&[&[u8]]],
    args: &D,
) -> Result<()> {
    debug_assert_eq!(fixed.len(), privileges.len());
    let mut metas = Vec::with_capacity(fixed.len() + health_ais.len());
    metas.extend(fixed.iter().zip(privileges).map(|(ai, (writable, signer))| AccountMeta {
        pubkey: *ai.key, is_signer: *signer, is_writable: *writable,
    }));
    metas.extend(health_ais.iter().map(|ai| AccountMeta {
        pubkey: *ai.key, is_signer: ai.is_signer, is_writable: ai.is_writable,
    }));
    let mut infos = Vec::with_capacity(fixed.len() + health_ais.len() + 1);
    infos.extend_from_slice(fixed);
    infos.extend_from_slice(health_ais);
    infos.push(core.program.clone());
                                                                              
    let mut data = Vec::with_capacity(32);
    data.extend_from_slice(&D::DISCRIMINATOR);
    args.serialize(&mut data)?;
    let ix = anchor_lang::solana_program::instruction::Instruction {
        program_id: *core.program.key, accounts: metas, data,
    };
    invoke_signed_owned(ix, &infos, signer).map_err(Into::into)
}

                                                                                   
                                                                                
pub fn invoke<'info, D: anchor_lang::Discriminator + AnchorSerialize>(
    program: AccountInfo<'info>, accounts: &impl ToAccountMetas,
    fixed: &[AccountInfo<'info>], remaining: &[AccountInfo<'info>],
    signer: &[&[&[u8]]], args: &D,
) -> Result<()> {
    let mut metas = accounts.to_account_metas(None);
    metas.extend(remaining.iter().map(|ai| AccountMeta {
        pubkey: *ai.key, is_signer: ai.is_signer, is_writable: ai.is_writable,
    }));
    let mut infos = Vec::with_capacity(fixed.len() + remaining.len() + 1);
    infos.extend_from_slice(fixed);
    infos.extend_from_slice(remaining);
    infos.push(program.clone());
    let mut data = Vec::with_capacity(128);
    data.extend_from_slice(&D::DISCRIMINATOR);
    args.serialize(&mut data)?;
    invoke_signed_owned(anchor_lang::solana_program::instruction::Instruction {
        program_id: *program.key, accounts: metas, data,
    }, &infos, signer).map_err(Into::into)
}

                                                                      
                                                                               
                                                                               
                                                                     
fn invoke_signed_owned(
    ix: anchor_lang::solana_program::instruction::Instruction,
    infos: &[AccountInfo],
    signer: &[&[&[u8]]],
) -> anchor_lang::solana_program::entrypoint::ProgramResult {
    for meta in ix.accounts.iter() {
        for info in infos.iter() {
            if meta.pubkey == *info.key {
                if meta.is_writable {
                    let _ = info.try_borrow_mut_lamports()?;
                    let _ = info.try_borrow_mut_data()?;
                } else {
                    let _ = info.try_borrow_lamports()?;
                    let _ = info.try_borrow_data()?;
                }
                break;
            }
        }
    }
    #[cfg(target_os = "solana")]
    {
        use anchor_lang::solana_program::stable_layout::stable_instruction::StableInstruction;
        let ix = StableInstruction::from(ix);
        let result = unsafe {
            anchor_lang::solana_program::syscalls::sol_invoke_signed_rust(
                &ix as *const _ as *const u8,
                infos as *const _ as *const u8,
                infos.len() as u64,
                signer as *const _ as *const u8,
                signer.len() as u64,
            )
        };
        match result {
            anchor_lang::solana_program::entrypoint::SUCCESS => Ok(()),
            _ => Err(result.into()),
        }
    }
    #[cfg(not(target_os = "solana"))]
    anchor_lang::solana_program::program::invoke_signed_unchecked(&ix, infos, signer)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quote_cpi_privileges_match_the_core_accounts() {
        let k = |i: u8| Pubkey::new_from_array([i; 32]);
        let deposit = mango_v4::accounts::TokenDeposit {
            group: k(1), account: k(2), owner: k(3), bank: k(4), vault: k(5), oracle: k(6),
            token_account: k(7), token_authority: k(8), token_program: k(9),
        }
        .to_account_metas(None);
        let withdraw = mango_v4::accounts::TokenWithdraw {
            group: k(1), account: k(2), owner: k(3), bank: k(4), vault: k(5), oracle: k(6),
            token_account: k(7), token_program: k(9),
        }
        .to_account_metas(None);
        let flags = |metas: &[AccountMeta]| metas.iter().map(|m| (m.is_writable, m.is_signer)).collect::<Vec<_>>();
        assert_eq!(flags(&deposit), TOKEN_DEPOSIT_PRIVILEGES.to_vec());
        assert_eq!(flags(&withdraw), TOKEN_WITHDRAW_PRIVILEGES.to_vec());
        let data = |d: &dyn Fn(&mut Vec<u8>)| { let mut v = Vec::new(); d(&mut v); v };
        let ix = mango_v4::instruction::TokenDeposit { amount: 7, reduce_only: false };
        let manual = data(&|v: &mut Vec<u8>| {
            v.extend_from_slice(&<mango_v4::instruction::TokenDeposit as anchor_lang::Discriminator>::DISCRIMINATOR);
            ix.serialize(v).unwrap();
        });
        assert_eq!(manual, anchor_lang::InstructionData::data(&ix));
    }
}
