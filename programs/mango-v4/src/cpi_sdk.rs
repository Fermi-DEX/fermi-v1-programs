//! Helpers for programs that submit v5 direct-market intents to Fermi by CPI.
//!
//! A CPI caller still needs a top-level ed25519 verification instruction before
//! its own instruction. Fermi reads the transaction instructions sysvar during
//! the CPI and verifies that the user signed the direct intent hash.

use anchor_lang::prelude::*;
use anchor_lang::solana_program::sysvar::instructions as tx_instructions;

use crate::instructions::DirectEnqueueArgsV5;

pub const QUEUE_AUTHORITY_SEED: &[u8] = b"queue-authority";
pub const EXECUTION_QUEUE_V5_SEED: &[u8] = b"execution-queue-v5";
pub const EXECUTION_QUEUE_V5_DIRECT_SEED: &[u8] = b"execution-queue-v5-direct";
pub const V5_DIRECT_INTENT_DOMAIN: &[u8] = b"mango-v5-direct-intent-v1";

pub fn queue_authority_state_pda(program_id: &Pubkey, group: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(&[QUEUE_AUTHORITY_SEED, group.as_ref()], program_id)
}

pub fn execution_queue_v5_pda(
    program_id: &Pubkey,
    group: &Pubkey,
    market_index: u16,
) -> (Pubkey, u8) {
    Pubkey::find_program_address(
        &[
            EXECUTION_QUEUE_V5_SEED,
            group.as_ref(),
            &market_index.to_le_bytes(),
        ],
        program_id,
    )
}

pub fn execution_queue_v5_direct_pool_pda(
    program_id: &Pubkey,
    group: &Pubkey,
    market_index: u16,
) -> (Pubkey, u8) {
    Pubkey::find_program_address(
        &[
            EXECUTION_QUEUE_V5_DIRECT_SEED,
            group.as_ref(),
            &market_index.to_le_bytes(),
        ],
        program_id,
    )
}

pub fn instructions_sysvar() -> Pubkey {
    tx_instructions::ID
}

pub struct EnqueueDirectMarketCpi<'a, 'info> {
    pub fermi_program: AccountInfo<'info>,
    pub group: AccountInfo<'info>,
    pub authority_state: AccountInfo<'info>,
    pub queue: AccountInfo<'info>,
    pub direct_pool: AccountInfo<'info>,
    pub instructions: AccountInfo<'info>,
    pub dispatch_accounts: &'a [AccountInfo<'info>],
}

/// Invoke `execution_queue_v5_enqueue_direct_market`.
///
/// `dispatch_accounts` must be the canonical Fermi dispatch account list that
/// the user signed over. For perp taker place-order payloads the fixed prefix is
/// group, fermi account, user owner/delegate, perp market, bids, asks, event
/// queue, oracle, followed by health accounts.
pub fn enqueue_direct_market<'a, 'info>(
    accounts: EnqueueDirectMarketCpi<'a, 'info>,
    market_index: u16,
    args: DirectEnqueueArgsV5,
) -> Result<()> {
    let cpi_accounts = crate::cpi::accounts::ExecutionQueueV5EnqueueDirectMarket {
        group: accounts.group,
        authority_state: accounts.authority_state,
        queue: accounts.queue,
        direct_pool: accounts.direct_pool,
        instructions: accounts.instructions,
    };
    let cpi_ctx = CpiContext::new(accounts.fermi_program, cpi_accounts)
        .with_remaining_accounts(accounts.dispatch_accounts.to_vec());
    crate::cpi::execution_queue_v5_enqueue_direct_market(cpi_ctx, market_index, args)
}

pub struct ExecuteMarketCpi<'a, 'info> {
    pub fermi_program: AccountInfo<'info>,
    pub group: AccountInfo<'info>,
    pub authority_state: AccountInfo<'info>,
    pub queue: AccountInfo<'info>,
    pub direct_pool: AccountInfo<'info>,
    pub instructions: AccountInfo<'info>,
    pub remaining_accounts: &'a [AccountInfo<'info>],
}

/// Invoke `execution_queue_v5_execute_market` with no CTM reveals.
///
/// This is useful for permissionless direct-pool promotion/execution from an
/// integrator program after the direct speedbump has elapsed.
pub fn execute_direct_market_head<'a, 'info>(
    accounts: ExecuteMarketCpi<'a, 'info>,
    market_index: u16,
) -> Result<()> {
    let cpi_accounts = crate::cpi::accounts::ExecutionQueueV5ExecuteMarket {
        group: accounts.group,
        authority_state: accounts.authority_state,
        queue: accounts.queue,
        direct_pool: accounts.direct_pool,
        instructions: accounts.instructions,
    };
    let cpi_ctx = CpiContext::new(accounts.fermi_program, cpi_accounts)
        .with_remaining_accounts(accounts.remaining_accounts.to_vec());
    crate::cpi::execution_queue_v5_execute_market(cpi_ctx, market_index, Vec::new())
}
