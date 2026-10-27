                                                                       
                                                                 
                                                                   
                                                  

use crate::accounts_ix::*;
use crate::error::*;
use crate::state::*;
use anchor_lang::prelude::*;
use anchor_lang::solana_program::entrypoint::MAX_PERMITTED_DATA_INCREASE;
use anchor_lang::solana_program::hash::hashv;
use anchor_lang::solana_program::instruction::AccountMeta;
use anchor_lang::solana_program::program::invoke;
use anchor_lang::solana_program::system_instruction;
use anchor_lang::solana_program::sysvar::instructions as tx_instructions;

use super::execution_queue::{
    account_metas_from_infos, authorize_v5_intent_signer, canonical_user_intent_message_v3,
    decode_queue_payload, dispatch_queue_payload, hash_accounts, hash_accounts_for_v5_ctm_reveal,
    prevalidate_terminal_ctm_payload, queue_health_region_begin, queue_health_region_end,
    queue_health_region_spec, queue_item_kind_for_payload_variant, require_dispatch_market_index,
    terminal_ctm_failure_msg, validate_queue_payload_dispatch_accounts,
    variant_uses_user_signature, verify_ed25519_preinstruction, verify_user_ed25519_preinstruction,
    DecodedQueuePayload, QueueFailureCode, QueueItemEnqueued, QueueItemProcessed,
    TerminalCtmFailureReason, UserIntentTargetKind, EXECUTION_QUEUE_GAP_SKIP_LIMIT_PER_EXECUTE,
};

const COMMIT_ITEM_V5_FLAG_DIRECT_PROMOTED: u8 = 1;
const EXECUTION_QUEUE_V5_MAX_DIRECT_PROMOTIONS_PER_EXECUTE: usize = 8;

                                                                              
                                     
                                                                              

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug)]
pub struct ExecutionQueueV5ConfigureMarketParams {
    pub market_index: u16,
    pub shard_id: u8,
    pub soft_limit: u16,
    pub gap_wait_slots: u16,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug)]
pub struct ExecutionQueueV5SubQueueConfigParams {
    pub market_index: u16,
    pub soft_limit: u16,
    pub gap_wait_slots: u16,
    pub pause_ingress: bool,
    pub pause_execute: bool,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug)]
pub struct ExecutionQueueV5GlobalPauseParams {
    pub pause_ingress: bool,
    pub pause_execute: bool,
}

                                                                     
                                                                            
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug)]
pub struct CommitEntryV5 {
    pub commit_hash: [u8; 32],
    pub min_execute_slot: u64,
    pub expires_at_slot: u64,
}

                                                               
                                                                  
                                                                      
                                                                   
                             
   
                                                                      
                                                                     
                                                                 
                                                                     
                         
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug)]
pub struct RevealArgsV5 {
    pub payload: Vec<u8>,
    pub kind: u8,
    pub dispatch_accounts_count: u8,
    pub client_order_id: u64,
    pub fermi_account: Pubkey,
    pub user_owner: Pubkey,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug)]
pub struct DirectEnqueueArgsV5 {
    pub payload: Vec<u8>,
    pub kind: u8,
    pub expires_at_slot: u64,
    pub nonce: u64,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExecutionQueueV5ReclaimReason {
    LostRevealMaterial,
    RevealEnvelopeMismatch,
    RetryExhausted,
    Expired,
    NoLaneMatch,
    WatchdogStaleHead,
    AdminForce,
}

impl ExecutionQueueV5ReclaimReason {
    fn failure_code(self) -> QueueFailureCode {
        match self {
            Self::LostRevealMaterial => QueueFailureCode::LostRevealMaterial,
            Self::RevealEnvelopeMismatch => QueueFailureCode::RevealEnvelopeMismatch,
            Self::RetryExhausted => QueueFailureCode::RetriesExhausted,
            Self::Expired => QueueFailureCode::Expired,
            Self::NoLaneMatch => QueueFailureCode::NoLaneMatch,
            Self::WatchdogStaleHead => QueueFailureCode::WatchdogStaleHead,
            Self::AdminForce => QueueFailureCode::AdminForce,
        }
    }
}

#[event]
pub struct QueueItemDirectAcceptedV5 {
    pub group: Pubkey,
    pub market_index: u16,
    pub item_id: u64,
    pub kind: u8,
    pub eligible_at_slot: u64,
    pub expires_at_slot: u64,
    pub user_owner: Pubkey,
    pub fermi_account: Pubkey,
}

#[event]
pub struct QueueItemDirectPromotedV5 {
    pub group: Pubkey,
    pub market_index: u16,
    pub item_id: u64,
    pub sequence: u64,
    pub kind: u8,
}

fn canonical_commit_batch_message_v5(
    group: Pubkey,
    market_index: u16,
    shard_id: u8,
    first_sequence: u64,
    entries: &[CommitEntryV5],
) -> [u8; 32] {
    let mut buf = Vec::with_capacity(entries.len() * 48 + 64);
    buf.extend_from_slice(b"mango-v4-commit-batch-v1");
    buf.extend_from_slice(group.as_ref());
    buf.extend_from_slice(&market_index.to_le_bytes());
    buf.push(shard_id);
    buf.extend_from_slice(&first_sequence.to_le_bytes());
    buf.extend_from_slice(&(entries.len() as u16).to_le_bytes());
    for e in entries {
        buf.extend_from_slice(&e.commit_hash);
        buf.extend_from_slice(&e.min_execute_slot.to_le_bytes());
        buf.extend_from_slice(&e.expires_at_slot.to_le_bytes());
    }
    hashv(&[&buf]).to_bytes()
}

fn canonical_direct_intent_message_v1(
    group: Pubkey,
    fermi_account: Pubkey,
    user_owner: Pubkey,
    kind: u8,
    market_index: u16,
    payload_hash: &[u8; 32],
    accounts_hash: &[u8; 32],
    expires_at_slot: u64,
    nonce: u64,
) -> [u8; 32] {
    hashv(&[
        b"mango-v5-direct-intent-v1",
        group.as_ref(),
        fermi_account.as_ref(),
        user_owner.as_ref(),
        &[kind],
        &[UserIntentTargetKind::PerpMarket as u8],
        &market_index.to_le_bytes(),
        payload_hash,
        accounts_hash,
        &expires_at_slot.to_le_bytes(),
        &nonce.to_le_bytes(),
    ])
    .to_bytes()
}

fn canonical_v5_direct_dispatch_account_metas(
    group: Pubkey,
    queue: Pubkey,
    dispatch_accounts: &[AccountMeta],
    user_owner: Pubkey,
) -> Vec<AccountMeta> {
    let mut metas = dispatch_accounts.to_vec();
    for meta in &mut metas {
        if meta.pubkey == group || meta.pubkey == queue {
            meta.is_signer = false;
            meta.is_writable = true;
        } else if meta.pubkey == tx_instructions::ID {
            meta.is_signer = false;
            meta.is_writable = false;
        }
    }
    if let Some(owner_meta) = metas.iter_mut().find(|meta| meta.pubkey == user_owner) {
        owner_meta.is_signer = false;
        owner_meta.is_writable = false;
    }
    metas
}

                                                                          
   
                                                                              
                                                                            
                                           
   
                   
                           
                                                                           
                                                                             
                              
                                                           
                              
                              
                                                                   
                                                                   
   
                                                                             
                                                                          
                                                                             
                                                                           
                                                                       
                                                                        
                                     
   
                                                                      
                                                                              
                                                                           
                                                                        
                                                                           
                                                                      
fn validate_v5_direct_canonical_fixed_prefix(
    group_key: Pubkey,
    fermi_account: Pubkey,
    user_owner: Pubkey,
    payload: &DecodedQueuePayload,
    dispatch_accounts: &[AccountInfo],
) -> Result<()> {
    require!(
        dispatch_accounts.len() >= 6,
        FermiError::ExecutionQueueDispatchAccountLayoutInvalid
    );
    require_keys_eq!(
        *dispatch_accounts[0].key,
        group_key,
        FermiError::ExecutionQueueDispatchAccountLayoutInvalid
    );
    require_keys_eq!(
        *dispatch_accounts[1].key,
        fermi_account,
        FermiError::ExecutionQueueDispatchAccountLayoutInvalid
    );
    require_keys_eq!(
        *dispatch_accounts[2].key,
        user_owner,
        FermiError::ExecutionQueueDispatchAccountLayoutInvalid
    );

    let perp_market_loader = AccountLoader::<PerpMarket>::try_from(&dispatch_accounts[3])
        .map_err(|_| error!(FermiError::ExecutionQueueDispatchAccountLayoutInvalid))?;
    let perp_market = perp_market_loader
        .load()
        .map_err(|_| error!(FermiError::ExecutionQueueDispatchAccountLayoutInvalid))?;
    require!(
        perp_market.group == group_key,
        FermiError::ExecutionQueueDispatchAccountLayoutInvalid
    );
    require_keys_eq!(
        *dispatch_accounts[4].key,
        perp_market.bids,
        FermiError::ExecutionQueueDispatchAccountLayoutInvalid
    );
    require_keys_eq!(
        *dispatch_accounts[5].key,
        perp_market.asks,
        FermiError::ExecutionQueueDispatchAccountLayoutInvalid
    );

    if queue_health_region_spec(payload).is_some() {
        require!(
            dispatch_accounts.len() >= 8,
            FermiError::ExecutionQueueDispatchAccountLayoutInvalid
        );
        require_keys_eq!(
            *dispatch_accounts[6].key,
            perp_market.event_queue,
            FermiError::ExecutionQueueDispatchAccountLayoutInvalid
        );
        require_keys_eq!(
            *dispatch_accounts[7].key,
            perp_market.oracle,
            FermiError::ExecutionQueueDispatchAccountLayoutInvalid
        );
    }

    Ok(())
}

                                                                            
                       
   
                                                                     
                                                                         
                                                                              
                                                                           
                                                      
                                                                            
                                                                             
                                                                             
                                                                             
                                                             
   
                                                                
                                                                             
                                                    
pub(crate) fn validate_direct_envelope_timing(
    now_slot: u64,
    speedbump_slots: u16,
    expires_at_slot: u64,
) -> Result<u64> {
    let eligible_at_slot = now_slot.saturating_add(speedbump_slots as u64);
    let max_delay_slot = now_slot.saturating_add(EXECUTION_QUEUE_V5_MAX_DELAY_SLOTS);
    require!(
        eligible_at_slot <= max_delay_slot,
        FermiError::ExecutionQueueEnvelopeDelayExceeded
    );
    require!(
        expires_at_slot != 0,
        FermiError::ExecutionQueueEnvelopeExpiryRequired
    );
    require!(
        expires_at_slot >= now_slot,
        FermiError::ExecutionQueueEnvelopeExpired
    );
    let max_ttl_slot = now_slot.saturating_add(EXECUTION_QUEUE_V5_MAX_TTL_SLOTS);
    require!(
        expires_at_slot <= max_ttl_slot,
        FermiError::ExecutionQueueEnvelopeTtlExceeded
    );
    require!(
        eligible_at_slot <= expires_at_slot,
        FermiError::ExecutionQueueEnvelopeTimingInvalid
    );
    Ok(eligible_at_slot)
}

fn commit_item_direct_item_id(item: &CommitItemV5) -> Option<u64> {
    if item.flags & COMMIT_ITEM_V5_FLAG_DIRECT_PROMOTED == 0 {
        return None;
    }
    Some(u64::from_le_bytes(
        item.reserved[..8].try_into().unwrap_or([0; 8]),
    ))
}

fn set_commit_item_direct_item_id(item: &mut CommitItemV5, direct_item_id: u64) {
    item.flags |= COMMIT_ITEM_V5_FLAG_DIRECT_PROMOTED;
    item.reserved[..8].copy_from_slice(&direct_item_id.to_le_bytes());
}

                                                                              
                                                            
                                                                              

pub fn execution_queue_v5_create(
    _ctx: Context<ExecutionQueueV5Create>,
    _market_index: u16,
) -> Result<()> {
                                                                     
                                                                       
                                                                         
                                                                       
                                                                          
                                                                       
                                                                       
             
    Ok(())
}

pub fn execution_queue_v5_resize(
    ctx: Context<ExecutionQueueV5Resize>,
    _market_index: u16,
) -> Result<()> {
    let queue_ai = ctx.accounts.queue.to_account_info();
    let current_len = queue_ai.data_len();
    let target_len = EXECUTION_QUEUE_V5_ACCOUNT_SPACE;
    require!(current_len > 0, FermiError::SomeError);
    if current_len >= target_len {
        return Ok(());
    }
    let next_len = (current_len + MAX_PERMITTED_DATA_INCREASE).min(target_len);
    let rent = Rent::get()?;
    let needed_lamports = rent
        .minimum_balance(next_len)
        .saturating_sub(queue_ai.lamports());
    if needed_lamports > 0 {
        invoke(
            &system_instruction::transfer(&ctx.accounts.payer.key(), queue_ai.key, needed_lamports),
            &[
                ctx.accounts.payer.to_account_info(),
                queue_ai.clone(),
                ctx.accounts.system_program.to_account_info(),
            ],
        )?;
    }
    queue_ai.realloc(next_len, true)?;
    Ok(())
}

pub fn execution_queue_v5_init(
    ctx: Context<ExecutionQueueV5Init>,
    _market_index: u16,
) -> Result<()> {
    require!(
        ctx.accounts.queue.to_account_info().data_len() == EXECUTION_QUEUE_V5_ACCOUNT_SPACE,
        FermiError::ExecutionQueueV5LayoutNotReady
    );
    let bump = *ctx
        .bumps
        .get("queue")
        .ok_or_else(|| error!(FermiError::SomeError))?;
    let mut queue = ctx.accounts.queue.load_init()?;
    queue.init(
        ctx.accounts.group.key(),
        ctx.accounts.authority_state.key(),
        bump,
    );
    Ok(())
}

pub fn execution_queue_v5_direct_create(
    _ctx: Context<ExecutionQueueV5DirectCreate>,
    _market_index: u16,
) -> Result<()> {
    Ok(())
}

pub fn execution_queue_v5_direct_resize(
    ctx: Context<ExecutionQueueV5DirectResize>,
    _market_index: u16,
) -> Result<()> {
    let direct_pool_ai = ctx.accounts.direct_pool.to_account_info();
    let current_len = direct_pool_ai.data_len();
    let target_len = EXECUTION_QUEUE_V5_DIRECT_ACCOUNT_SPACE;
    require!(current_len > 0, FermiError::SomeError);
    if current_len >= target_len {
        return Ok(());
    }
    let next_len = (current_len + MAX_PERMITTED_DATA_INCREASE).min(target_len);
    let rent = Rent::get()?;
    let needed_lamports = rent
        .minimum_balance(next_len)
        .saturating_sub(direct_pool_ai.lamports());
    if needed_lamports > 0 {
        invoke(
            &system_instruction::transfer(
                &ctx.accounts.payer.key(),
                direct_pool_ai.key,
                needed_lamports,
            ),
            &[
                ctx.accounts.payer.to_account_info(),
                direct_pool_ai.clone(),
                ctx.accounts.system_program.to_account_info(),
            ],
        )?;
    }
    direct_pool_ai.realloc(next_len, true)?;
    Ok(())
}

pub fn execution_queue_v5_direct_init(
    ctx: Context<ExecutionQueueV5DirectInit>,
    market_index: u16,
) -> Result<()> {
    require!(
        ctx.accounts.direct_pool.to_account_info().data_len()
            == EXECUTION_QUEUE_V5_DIRECT_ACCOUNT_SPACE,
        FermiError::ExecutionQueueV5DirectLayoutNotReady
    );
    let bump = *ctx
        .bumps
        .get("direct_pool")
        .ok_or_else(|| error!(FermiError::SomeError))?;
    let mut direct_pool = ctx.accounts.direct_pool.load_init()?;
    direct_pool.init(
        ctx.accounts.group.key(),
        ctx.accounts.authority_state.key(),
        ctx.accounts.queue.key(),
        market_index,
        bump,
    );
    Ok(())
}

pub fn execution_queue_v5_configure_market(
    ctx: Context<ExecutionQueueV5Admin>,
    params: ExecutionQueueV5ConfigureMarketParams,
) -> Result<()> {
    require!(
        params.shard_id == 0,
        FermiError::ExecutionQueueUnsupportedShardId
    );
    let mut queue = ctx.accounts.queue.load_mut()?;
    require!(
        queue.header.layout_version == EXECUTION_QUEUE_V5_LAYOUT_VERSION,
        FermiError::ExecutionQueueSubQueueLayoutVersionMismatch
    );
    let soft_limit = if params.soft_limit == 0 {
        EXECUTION_QUEUE_V5_DEFAULT_SOFT_LIMIT
    } else {
        require!(
            (params.soft_limit as usize) <= EXECUTION_QUEUE_V5_PER_MARKET_CAPACITY,
            FermiError::ExecutionQueueInvalidSoftLimit
        );
        params.soft_limit
    };
    let gap_wait = if params.gap_wait_slots == 0 {
        EXECUTION_QUEUE_V5_DEFAULT_GAP_WAIT_SLOTS
    } else {
        params.gap_wait_slots
    };
    queue.allocate_sub_queue(params.market_index, params.shard_id, soft_limit, gap_wait)?;
    Ok(())
}

pub fn execution_queue_v5_configure_sub_queue(
    ctx: Context<ExecutionQueueV5Admin>,
    params: ExecutionQueueV5SubQueueConfigParams,
) -> Result<()> {
    let mut queue = ctx.accounts.queue.load_mut()?;
    let idx = queue
        .find_sub_queue(params.market_index)
        .ok_or_else(|| error!(FermiError::ExecutionQueueV5SubQueueNotConfigured))?;
    let sqh = &mut queue.sub_queue_headers[idx];
    if params.soft_limit != 0 {
        require!(
            (params.soft_limit as usize) <= EXECUTION_QUEUE_V5_PER_MARKET_CAPACITY,
            FermiError::ExecutionQueueInvalidSoftLimit
        );
        sqh.soft_limit = params.soft_limit;
    }
    if params.gap_wait_slots != 0 {
        sqh.gap_wait_slots = params.gap_wait_slots;
    }
    sqh.paused_ingress = u8::from(params.pause_ingress);
    sqh.paused_execute = u8::from(params.pause_execute);
    Ok(())
}

pub fn execution_queue_v5_set_global_pause(
    ctx: Context<ExecutionQueueV5Admin>,
    params: ExecutionQueueV5GlobalPauseParams,
) -> Result<()> {
    let mut queue = ctx.accounts.queue.load_mut()?;
    queue.header.paused_ingress = u8::from(params.pause_ingress);
    queue.header.paused_execute = u8::from(params.pause_execute);
    Ok(())
}

                                                                        
   
                                                                      
                                                   
                                                                            
                                                                            
                                  
pub fn execution_queue_v5_set_max_retries(
    ctx: Context<ExecutionQueueV5Admin>,
    max_retries: u8,
) -> Result<()> {
    require!(
        max_retries <= EXECUTION_QUEUE_V5_MAX_RETRIES_CAP,
        FermiError::ExecutionQueueV5InvalidMaxRetries
    );
    let mut queue = ctx.accounts.queue.load_mut()?;
    let previous = queue.header.max_retries;
    queue.header.max_retries = max_retries;
    let effective = queue.header.max_retries_effective();
    msg!(
        "execution_queue_v5_set_max_retries: previous={} stored={} effective={}",
        previous,
        max_retries,
        effective
    );
    Ok(())
}

pub fn execution_queue_v5_configure_direct_pool(
    ctx: Context<ExecutionQueueV5DirectAdmin>,
    params: ExecutionQueueV5DirectPoolConfigParams,
) -> Result<()> {
    let mut direct_pool = ctx.accounts.direct_pool.load_mut()?;
    require!(
        direct_pool.header.layout_version == EXECUTION_QUEUE_V5_DIRECT_LAYOUT_VERSION,
        FermiError::ExecutionQueueV5DirectLayoutNotReady
    );
    let speedbump_slots = if params.speedbump_slots == 0 {
        EXECUTION_QUEUE_V5_DIRECT_DEFAULT_SPEEDBUMP_SLOTS
    } else {
        params.speedbump_slots
    };
    direct_pool.header.paused_ingress = u8::from(params.pause_ingress);
    direct_pool.header.speedbump_slots = speedbump_slots;
    Ok(())
}

pub fn execution_queue_v5_close(_ctx: Context<ExecutionQueueV5Close>) -> Result<()> {
                                                                                  
                                                          
    Ok(())
}

pub fn execution_queue_v5_direct_close(_ctx: Context<ExecutionQueueV5DirectClose>) -> Result<()> {
    Ok(())
}

                                                                              
                                                                            
                                                                              

pub fn execution_queue_v5_drop_head_market(
    ctx: Context<ExecutionQueueV5Admin>,
    market_index: u16,
    count: u16,
) -> Result<()> {
    let mut queue = ctx.accounts.queue.load_mut()?;
    require!(
        queue.header.paused_execute != 0,
        FermiError::ExecutionQueueAdminActionRequiresPause
    );
    let idx = queue
        .find_sub_queue(market_index)
        .ok_or_else(|| error!(FermiError::ExecutionQueueV5SubQueueNotConfigured))?;
    let mut dropped = 0u16;
    for _ in 0..count {
        if queue.sub_queue_headers[idx].live_count == 0 {
            break;
        }
        queue.clear_head(idx)?;
        dropped = dropped.saturating_add(1);
    }
    msg!(
        "drop_head_market_v5: market={} dropped={}",
        market_index,
        dropped
    );
    Ok(())
}

                                                                              
                                                         
                                                                              

pub fn execution_queue_v5_reclaim_seq_market(
    ctx: Context<ExecutionQueueV5Admin>,
    market_index: u16,
    sequence: u64,
    reason: ExecutionQueueV5ReclaimReason,
) -> Result<()> {
    let mut queue = ctx.accounts.queue.load_mut()?;
    require!(
        queue.header.paused_execute != 0,
        FermiError::ExecutionQueueAdminActionRequiresPause
    );
    let idx = queue
        .find_sub_queue(market_index)
        .ok_or_else(|| error!(FermiError::ExecutionQueueV5SubQueueNotConfigured))?;
    let head = queue.sub_queue_headers[idx].next_sequence_to_execute;
    require!(
        head == sequence,
        FermiError::ExecutionQueueSequenceNotPending
    );
    queue.clear_head(idx)?;
    let group = queue.header.group;
    emit!(QueueItemProcessed {
        group,
        market_index,
        sequence,
        kind: QueueItemKind::CtmWrapped as u8,
        status: CommitStatusV5::Failed as u8,
        failure_code: reason.failure_code() as u8,
    });
    msg!(
        "reclaim_seq_market_v5: market={} sequence={} reason={:?}",
        market_index,
        sequence,
        reason
    );
    Ok(())
}

                                                                              
                                                           
                                                                              

pub fn execution_queue_v5_commit_market(
    ctx: Context<ExecutionQueueV5CommitMarket>,
    market_index: u16,
    first_sequence: u64,
    entries: Vec<CommitEntryV5>,
) -> Result<()> {
    require!(
        !entries.is_empty() && entries.len() <= EXECUTION_QUEUE_V5_MAX_COMMIT_BATCH,
        FermiError::ExecutionQueueV5InvalidCommitBatch
    );

    let clock = Clock::get()?;
    ctx.accounts
        .authority_state
        .maybe_activate_pending_ctm(clock.slot);

    let mut queue = ctx.accounts.queue.load_mut()?;
    require!(
        queue.header.paused_ingress == 0,
        FermiError::ExecutionQueueIngressPaused
    );
    let idx = queue
        .find_sub_queue(market_index)
        .ok_or_else(|| error!(FermiError::ExecutionQueueV5SubQueueNotConfigured))?;
    let shard_id = queue.sub_queue_headers[idx].shard_id;
    require!(shard_id == 0, FermiError::ExecutionQueueUnsupportedShardId);
    require!(
        queue.sub_queue_headers[idx].paused_ingress == 0,
        FermiError::ExecutionQueueIngressPaused
    );

                                                              
    let batch_msg = canonical_commit_batch_message_v5(
        ctx.accounts.group.key(),
        market_index,
        shard_id,
        first_sequence,
        &entries,
    );
    verify_ed25519_preinstruction(
        ctx.accounts.instructions.as_ref(),
        ctx.accounts.authority_state.ctm_signer,
        batch_msg.as_ref(),
    )?;

                                                                            
                                                                           
                                                                   
                                                                             
                                                                   
    let max_ttl_slot = clock.slot.saturating_add(EXECUTION_QUEUE_V5_MAX_TTL_SLOTS);
    let max_delay_slot = clock
        .slot
        .saturating_add(EXECUTION_QUEUE_V5_MAX_DELAY_SLOTS);
                                                                        
                                                                       
                                                                     
                        
    let log_per_entry = entries.len() == 1;
    if !log_per_entry {
        msg!(
            "v5 commit batch first_seq={} count={} market={} ingress={}",
            first_sequence,
            entries.len(),
            market_index,
            clock.slot,
        );
    }
    for (i, entry) in entries.iter().enumerate() {
        let seq = first_sequence + i as u64;
        queue.sub_queue_headers[idx].validate_commit_sequence(seq)?;
        require!(
            entry.min_execute_slot <= max_delay_slot,
            FermiError::ExecutionQueueEnvelopeDelayExceeded
        );
        if entry.expires_at_slot != 0 {
            require!(
                entry.expires_at_slot >= clock.slot,
                FermiError::ExecutionQueueEnvelopeExpired
            );
            require!(
                entry.expires_at_slot <= max_ttl_slot,
                FermiError::ExecutionQueueEnvelopeTtlExceeded
            );
            require!(
                entry.min_execute_slot <= entry.expires_at_slot,
                FermiError::ExecutionQueueEnvelopeTimingInvalid
            );
        }
        let mut item = CommitItemV5::default();
        item.sequence = seq;
        item.commit_hash = entry.commit_hash;
        item.min_execute_slot = entry.min_execute_slot;
        item.expires_at_slot = entry.expires_at_slot;
        item.ingress_slot = clock.slot;
        item.status = CommitStatusV5::Committed as u8;
        queue.write_commit(idx, item)?;

                                                                           
                                                                         
                                                                          
        if log_per_entry {
            let hash_prefix = u64::from_le_bytes([
                entry.commit_hash[0],
                entry.commit_hash[1],
                entry.commit_hash[2],
                entry.commit_hash[3],
                entry.commit_hash[4],
                entry.commit_hash[5],
                entry.commit_hash[6],
                entry.commit_hash[7],
            ]);
            msg!(
                "v5 commit seq={} market={} hash_prefix={:#x} min_exec={} exp={} ingress={}",
                seq,
                market_index,
                hash_prefix,
                entry.min_execute_slot,
                entry.expires_at_slot,
                clock.slot,
            );
        }

        emit!(QueueItemEnqueued {
            group: ctx.accounts.group.key(),
            market_index,
            sequence: seq,
            kind: QueueItemKind::CtmWrapped as u8,
            min_execute_slot: entry.min_execute_slot,
        });
    }
    Ok(())
}

pub fn execution_queue_v5_enqueue_direct_market(
    ctx: Context<ExecutionQueueV5EnqueueDirectMarket>,
    market_index: u16,
    args: DirectEnqueueArgsV5,
) -> Result<()> {
    require!(
        !ctx.remaining_accounts.is_empty(),
        FermiError::ExecutionQueueDispatchAccountLayoutInvalid
    );
    require!(
        args.payload.len() <= EXECUTION_QUEUE_PAYLOAD_MAX,
        FermiError::ExecutionQueuePayloadTooLarge
    );
    require!(
        ctx.remaining_accounts.len() <= u16::MAX as usize,
        FermiError::ExecutionQueueDispatchAccountLayoutInvalid
    );

    let group_key = ctx.accounts.group.key();
    let delegate_intents_enabled = ctx
        .accounts
        .group
        .load()?
        .is_ix_enabled(IxGate::V5DelegateIntents);
    let clock = Clock::get()?;
    let now_ts: u64 = clock.unix_timestamp.try_into().unwrap_or(0);
    let dispatch_accounts = ctx.remaining_accounts;

    {
        let queue = ctx.accounts.queue.load()?;
        require!(
            queue.header.paused_ingress == 0,
            FermiError::ExecutionQueueIngressPaused
        );
        let idx = queue
            .find_sub_queue(market_index)
            .ok_or_else(|| error!(FermiError::ExecutionQueueV5SubQueueNotConfigured))?;
        require!(
            queue.sub_queue_headers[idx].paused_ingress == 0,
            FermiError::ExecutionQueueIngressPaused
        );
    }
    let speedbump_slots = {
        let direct_pool = ctx.accounts.direct_pool.load()?;
        require!(
            direct_pool.header.layout_version == EXECUTION_QUEUE_V5_DIRECT_LAYOUT_VERSION,
            FermiError::ExecutionQueueV5DirectLayoutNotReady
        );
        require!(
            direct_pool.header.paused_ingress == 0,
            FermiError::ExecutionQueueIngressPaused
        );
        require!(
            direct_pool.header.market_index == market_index,
            FermiError::ExecutionQueueV5DirectItemMismatch
        );
        direct_pool.header.speedbump_slots
    };

    let decoded_payload = match decode_queue_payload(&args.payload) {
        Ok(p) if p.flags == 0 => p,
        Ok(_) => return err!(FermiError::ExecutionQueuePayloadVariantInvalid),
        Err(err) => return Err(err),
    };
    require!(
        queue_item_kind_for_payload_variant(decoded_payload.variant) == args.kind,
        FermiError::ExecutionQueuePayloadKindMismatch
    );
    require!(
        variant_uses_user_signature(decoded_payload.variant),
        FermiError::ExecutionQueueInvalidItemKind
    );
    require_dispatch_market_index(dispatch_accounts, market_index)?;
    validate_queue_payload_dispatch_accounts(group_key, &decoded_payload, dispatch_accounts)?;

                                                                        
                                                                         
    require!(
        dispatch_accounts.len() >= 3,
        FermiError::ExecutionQueueDispatchAccountLayoutInvalid
    );
    let user_owner = *dispatch_accounts[2].key;
    let (fermi_account, signer_authorized) = authorize_v5_intent_signer(
        group_key,
        dispatch_accounts,
        user_owner,
        now_ts,
        delegate_intents_enabled,
    )?;
    require!(
        signer_authorized,
        FermiError::ExecutionQueueIntentSignerNotAuthorized
    );
                                                                         
                                                                               
                                                                          
                                                                              
                                                                       
                
    validate_v5_direct_canonical_fixed_prefix(
        group_key,
        fermi_account,
        user_owner,
        &decoded_payload,
        dispatch_accounts,
    )?;

    if let Some(reason) = prevalidate_terminal_ctm_payload(&decoded_payload, now_ts) {
        msg!(
            "{} market={} reason={:?}",
            terminal_ctm_failure_msg(reason),
            market_index,
            reason
        );
        return err!(FermiError::ExecutionQueueEnvelopeExpired);
    }

    let payload_hash = hashv(&[&args.payload]).to_bytes();
    let accounts_hash = hash_accounts(&canonical_v5_direct_dispatch_account_metas(
        group_key,
        ctx.accounts.queue.key(),
        &account_metas_from_infos(dispatch_accounts),
        user_owner,
    ));
    let direct_intent_hash = canonical_direct_intent_message_v1(
        group_key,
        fermi_account,
        user_owner,
        args.kind,
        market_index,
        &payload_hash,
        &accounts_hash,
        args.expires_at_slot,
        args.nonce,
    );
    verify_user_ed25519_preinstruction(
        ctx.accounts.instructions.as_ref(),
        user_owner,
        &[direct_intent_hash],
    )?;
    ctx.accounts
        .queue
        .load_mut()?
        .record_consumed_intent_hash(direct_intent_hash, clock.slot)?;

                                                                     
                                                                             
    let eligible_at_slot =
        validate_direct_envelope_timing(clock.slot, speedbump_slots, args.expires_at_slot)?;

    let mut direct_item = DirectItemV5::default();
    direct_item.eligible_at_slot = eligible_at_slot;
    direct_item.expires_at_slot = args.expires_at_slot;
    direct_item.ingress_slot = clock.slot;
    direct_item.direct_intent_hash = direct_intent_hash;
    direct_item.payload_hash = payload_hash;
    direct_item.accounts_hash = accounts_hash;
    direct_item.user_owner = user_owner;
    direct_item.fermi_account = fermi_account;
    direct_item.payload_len = args.payload.len() as u16;
    direct_item.dispatch_accounts_count = ctx.remaining_accounts.len() as u16;
    direct_item.kind = args.kind;
    direct_item.payload[..args.payload.len()].copy_from_slice(&args.payload);

    let item_id = ctx
        .accounts
        .direct_pool
        .load_mut()?
        .write_staged(direct_item)?;

    emit!(QueueItemDirectAcceptedV5 {
        group: group_key,
        market_index,
        item_id,
        kind: args.kind,
        eligible_at_slot,
        expires_at_slot: args.expires_at_slot,
        user_owner,
        fermi_account,
    });
    Ok(())
}

fn promote_eligible_direct_items_v5(
    queue: &mut ExecutionQueueV5,
    direct_pool: &mut ExecutionQueueV5DirectPool,
    group: Pubkey,
    market_index: u16,
    sub_queue_idx: usize,
    current_slot: u64,
    max_promotions: usize,
) -> Result<usize> {
    let mut promoted = 0usize;
    while promoted < max_promotions {
        let Some(staged_item) = direct_pool.staged_head() else {
            break;
        };
        let item_id = staged_item.item_id;
        let eligible_at_slot = staged_item.eligible_at_slot;
        let expires_at_slot = staged_item.expires_at_slot;
        let ingress_slot = staged_item.ingress_slot;
        let kind = staged_item.kind;
        let direct_intent_hash = staged_item.direct_intent_hash;

        if eligible_at_slot > current_slot {
            break;
        }
        if expires_at_slot != 0 && current_slot > expires_at_slot {
            msg!(
                "v5 direct item expired before promotion: market={} item_id={} now_slot={} expires_at_slot={}",
                market_index,
                item_id,
                current_slot,
                expires_at_slot,
            );
            direct_pool.mark_terminal(item_id, DirectItemStatusV5::Expired)?;
            continue;
        }

        let sequence = queue.sub_queue_headers[sub_queue_idx].next_enqueue_sequence();
        if queue.sub_queue_headers[sub_queue_idx]
            .validate_commit_sequence(sequence)
            .is_err()
        {
            break;
        }

        let mut item = CommitItemV5::default();
        item.sequence = sequence;
        item.commit_hash = direct_intent_hash;
        item.min_execute_slot = eligible_at_slot;
        item.expires_at_slot = expires_at_slot;
        item.ingress_slot = ingress_slot;
        item.status = CommitStatusV5::Committed as u8;
        set_commit_item_direct_item_id(&mut item, item_id);
        queue.write_commit(sub_queue_idx, item)?;
        direct_pool.mark_promoted(item_id, sequence)?;

        emit!(QueueItemDirectPromotedV5 {
            group,
            market_index,
            item_id,
            sequence,
            kind,
        });
        emit!(QueueItemEnqueued {
            group,
            market_index,
            sequence,
            kind,
            min_execute_slot: eligible_at_slot,
        });
        promoted = promoted.saturating_add(1);
    }
    Ok(promoted)
}

                                                                             
                                                                              
                                                                        
                                                                           
                                                                          
fn resolve_head_v5(
    queue_loader: &AccountLoader<ExecutionQueueV5>,
    sub_queue_idx: usize,
    replay_hash: Option<[u8; 32]>,
    slot: u64,
) -> Result<()> {
    let mut queue = queue_loader.load_mut()?;
    queue.clear_head(sub_queue_idx)?;
    if let Some(hash) = replay_hash {
        queue.push_consumed_intent_hash(hash, slot);
    }
    Ok(())
}

fn finalize_direct_head_v5(
    direct_pool_loader: &AccountLoader<ExecutionQueueV5DirectPool>,
    head_item: &CommitItemV5,
    terminal_status: DirectItemStatusV5,
) -> Result<()> {
    if let Some(item_id) = commit_item_direct_item_id(head_item) {
        direct_pool_loader
            .load_mut()?
            .mark_terminal(item_id, terminal_status)?;
    }
    Ok(())
}

                                                                          
                                            
fn classify_predispatch_validation_failure(err: &anchor_lang::error::Error) -> QueueFailureCode {
    use anchor_lang::error::Error::*;
    let code = match err {
        AnchorError(ae) => ae.error_code_number,
        _ => return QueueFailureCode::DispatchAccountsInvalid,
    };
    match code.checked_sub(6000) {
        Some(c) if c == FermiError::ExecutionQueuePerpHealthAccountsInvalid as u32 => {
            QueueFailureCode::HealthAccountsInvalid
        }
        Some(c) if c == FermiError::ExecutionQueueInvalidUserAccount as u32 => {
            QueueFailureCode::UserAccountInvalid
        }
        _ => QueueFailureCode::DispatchAccountsInvalid,
    }
}

                                                                         
                                                                           
                                                                           
                                                                              
                                                                               
                                                                          
                                                                        
                                                                              
#[allow(clippy::too_many_arguments)]
fn fail_head_v5(
    queue: &AccountLoader<ExecutionQueueV5>,
    direct_pool: Option<&AccountLoader<ExecutionQueueV5DirectPool>>,
    sub_queue_idx: usize,
    head_item: &CommitItemV5,
    group: Pubkey,
    market_index: u16,
    kind: u8,
    failure_code: QueueFailureCode,
) -> Result<()> {
    queue.load_mut()?.clear_head(sub_queue_idx)?;
    if let Some(direct_pool) = direct_pool {
        finalize_direct_head_v5(direct_pool, head_item, DirectItemStatusV5::Failed)?;
    }
    emit!(QueueItemProcessed {
        group,
        market_index,
        sequence: head_item.sequence,
        kind,
        status: CommitStatusV5::Failed as u8,
        failure_code: failure_code as u8,
    });
    Ok(())
}

                                                                              
                                                                              
                                                                              

                                                                              
                                                                             
                                                                         
                             
fn skip_sub_queue_head_gaps_v5(
    queue: &mut ExecutionQueueV5,
    idx: usize,
    group: Pubkey,
    market_index: u16,
) {
    let mut skipped = 0u16;
    loop {
        let sqh = &queue.sub_queue_headers[idx];
        if skipped >= EXECUTION_QUEUE_GAP_SKIP_LIMIT_PER_EXECUTE {
            break;
        }
                                                                       
                                                               
                                                                     
                                                                       
                                                                     
                                                                    
                                                                 
                                                                     
                                                                       
                                                                 
                                                                     
                        
        if sqh.max_seen_sequence < sqh.next_sequence_to_execute {
            break;
        }
        let next_seq = sqh.next_sequence_to_execute;
        let live_count = sqh.live_count;
                                                                    
                                                                   
                   
                                                                 
                                                                    
                                                                    
                                                                  
                                                                    
                                                                  
                                                                     
                                                                  
                                                             
                                                                    
                                                                
                                                                 
                                                                  
                                                          
        let item_present = queue.get_item(idx, next_seq).is_some();
        if item_present && live_count > 0 {
            break;
        }
        emit!(QueueItemProcessed {
            group,
            market_index,
            sequence: next_seq,
            kind: QueueItemKind::CtmWrapped as u8,
            status: CommitStatusV5::Failed as u8,
            failure_code: QueueFailureCode::GapSkipped as u8,
        });
                                                                     
                                                                    
                                                                
        let ring_offset = SubQueueHeaderV5::ring_offset_for_sequence(next_seq);
        let phys = ExecutionQueueV5::physical_index(idx, ring_offset);
        queue.items[phys] = CommitItemV5::default();
        queue.sub_queue_headers[idx].next_sequence_to_execute = next_seq.saturating_add(1);
        queue.sub_queue_headers[idx].gap_observed_slot = 0;
        skipped = skipped.saturating_add(1);
    }
}

pub fn execution_queue_v5_reveal_execute_market(
    ctx: Context<ExecutionQueueV5RevealExecuteMarket>,
    market_index: u16,
    reveals: Vec<RevealArgsV5>,
) -> Result<()> {
    require!(
        !reveals.is_empty() && reveals.len() <= EXECUTION_QUEUE_V5_MAX_REVEAL_BATCH,
        FermiError::ExecutionQueueV5InvalidRevealBatch
    );

    let group_key = ctx.accounts.group.key();
    let delegate_intents_enabled = ctx
        .accounts
        .group
        .load()?
        .is_ix_enabled(IxGate::V5DelegateIntents);
    let clock = Clock::get()?;
    let now_ts: u64 = clock.unix_timestamp.try_into().unwrap_or(0);

                                                                                    
    let sub_queue_idx = {
        let queue = ctx.accounts.queue.load()?;
        require!(
            queue.header.paused_execute == 0,
            FermiError::ExecutionQueueExecutePaused
        );
        let idx = queue
            .find_sub_queue(market_index)
            .ok_or_else(|| error!(FermiError::ExecutionQueueV5SubQueueNotConfigured))?;
        require!(
            queue.sub_queue_headers[idx].paused_execute == 0,
            FermiError::ExecutionQueueExecutePaused
        );
        idx
    };

    let mut cursor: usize = 0;
    let total_accounts = ctx.remaining_accounts.len();

                                                                           
                                                                          
                                                                       
                                                                        
                                                                 
                                                                         
                                                                     
                                                                    
                                                                      
                                                                     
                                                                  
    {
        let mut queue = ctx.accounts.queue.load_mut()?;
        let needs_sweep = {
            let sqh = &queue.sub_queue_headers[sub_queue_idx];
            sqh.live_count == 0 && sqh.max_seen_sequence >= sqh.next_sequence_to_execute
        };
        if needs_sweep {
            skip_sub_queue_head_gaps_v5(&mut queue, sub_queue_idx, group_key, market_index);
        }
    }

    for (reveal_index, reveal) in reveals.iter().enumerate() {
                                             
        let (next_sequence, gap_wait_slots, head_item) = {
            let queue = ctx.accounts.queue.load()?;
            let sqh = &queue.sub_queue_headers[sub_queue_idx];
            if sqh.live_count == 0 {
                break;
            }
            let next_sequence = sqh.next_sequence_to_execute;
            let gap_wait_slots = sqh.gap_wait_slots as u64;
            let head_item = queue.get_item(sub_queue_idx, next_sequence).cloned();
            (next_sequence, gap_wait_slots, head_item)
        };

                                                                              
                                                               
        let Some(head_item) = head_item else {
            let should_break = {
                let mut queue = ctx.accounts.queue.load_mut()?;
                let sqh = &mut queue.sub_queue_headers[sub_queue_idx];
                if sqh.max_seen_sequence < next_sequence {
                    true
                } else {
                    if sqh.gap_observed_slot == 0 {
                        sqh.gap_observed_slot = clock.slot;
                    }
                    if clock.slot >= sqh.gap_observed_slot.saturating_add(gap_wait_slots) {
                        skip_sub_queue_head_gaps_v5(
                            &mut queue,
                            sub_queue_idx,
                            group_key,
                            market_index,
                        );
                        false
                    } else {
                        true
                    }
                }
            };
            if should_break {
                break;
            }
                                                                    
                                                                     
            cursor = cursor.saturating_add(reveal.dispatch_accounts_count as usize);
            continue;
        };

                                                                                 
                                                               
        if clock.slot < head_item.min_execute_slot {
            break;
        }
        if head_item.expires_at_slot != 0 && clock.slot > head_item.expires_at_slot {
            msg!(
                "v5 reveal seq={} market_index={} expired: clock.slot={} expires_at_slot={}",
                head_item.sequence,
                market_index,
                clock.slot,
                head_item.expires_at_slot
            );
            ctx.accounts.queue.load_mut()?.clear_head(sub_queue_idx)?;
            emit!(QueueItemProcessed {
                group: group_key,
                market_index,
                sequence: head_item.sequence,
                kind: QueueItemKind::CtmWrapped as u8,
                status: CommitStatusV5::Failed as u8,
                failure_code: QueueFailureCode::Expired as u8,
            });
                                                                  
            cursor = cursor.saturating_add(reveal.dispatch_accounts_count as usize);
            continue;
        }

                                                 
        let count = reveal.dispatch_accounts_count as usize;
        require!(
            count > 0 && cursor + count <= total_accounts,
            FermiError::ExecutionQueueDispatchAccountLayoutInvalid
        );
        let dispatch_accounts = &ctx.remaining_accounts[cursor..cursor + count];
        cursor += count;
        let dispatch_account_metas = account_metas_from_infos(dispatch_accounts);
        let accounts_hash = hash_accounts_for_v5_ctm_reveal(
            group_key,
            ctx.accounts.queue.key(),
            &dispatch_account_metas,
        );

                                                                           
                                                                        
                                                                     
                                                                         
                                                                    
          
                                                                            
                                                                           
                                                                            
        let payload_hash = hashv(&[&reveal.payload]).to_bytes();
        let expected_commit_hash = canonical_user_intent_message_v3(
            group_key,
            reveal.fermi_account,
            reveal.user_owner,
            reveal.kind,
            UserIntentTargetKind::PerpMarket,
            market_index,
            &payload_hash,
            &accounts_hash,
            head_item.min_execute_slot,
            head_item.expires_at_slot,
            reveal.client_order_id,
        );
        if expected_commit_hash != head_item.commit_hash {
                                                                           
                                                                          
                                                      
            let expected_prefix = u64::from_le_bytes([
                expected_commit_hash[0],
                expected_commit_hash[1],
                expected_commit_hash[2],
                expected_commit_hash[3],
                expected_commit_hash[4],
                expected_commit_hash[5],
                expected_commit_hash[6],
                expected_commit_hash[7],
            ]);
            let stored_prefix = u64::from_le_bytes([
                head_item.commit_hash[0],
                head_item.commit_hash[1],
                head_item.commit_hash[2],
                head_item.commit_hash[3],
                head_item.commit_hash[4],
                head_item.commit_hash[5],
                head_item.commit_hash[6],
                head_item.commit_hash[7],
            ]);
            let payload_prefix = u64::from_le_bytes([
                payload_hash[0],
                payload_hash[1],
                payload_hash[2],
                payload_hash[3],
                payload_hash[4],
                payload_hash[5],
                payload_hash[6],
                payload_hash[7],
            ]);
            msg!(
                "v5 reveal commit_hash mismatch: seq={} market={} kind={} \
                 expected_prefix={:#x} stored_prefix={:#x} \
                 payload_prefix={:#x} payload_len={} \
                 client_order_id={} fermi_account={} user_owner={}",
                head_item.sequence,
                market_index,
                reveal.kind,
                expected_prefix,
                stored_prefix,
                payload_prefix,
                reveal.payload.len(),
                reveal.client_order_id,
                reveal.fermi_account,
                reveal.user_owner,
            );
                                                                            
                                                                          
                                                                              
                                                                   
            if reveal_index == 0 {
                return err!(FermiError::ExecutionQueueV5CommitRevealMismatch);
            }
            msg!(
                "v5 reveal seq={} market_index={} stopping batch at reveal {} on commit_hash mismatch",
                head_item.sequence,
                market_index,
                reveal_index
            );
            break;
        }

                                                 
        let decoded_payload = match decode_queue_payload(&reveal.payload) {
            Ok(p) if p.flags == 0 => p,
            Ok(p) => {
                msg!(
                    "v5 reveal seq={} market_index={} rejected: non-zero payload flags={}",
                    head_item.sequence,
                    market_index,
                    p.flags
                );
                ctx.accounts.queue.load_mut()?.clear_head(sub_queue_idx)?;
                emit!(QueueItemProcessed {
                    group: group_key,
                    market_index,
                    sequence: head_item.sequence,
                    kind: reveal.kind,
                    status: CommitStatusV5::Failed as u8,
                    failure_code: QueueFailureCode::PayloadVariantInvalid as u8,
                });
                continue;
            }
            Err(err) => {
                msg!(
                    "v5 reveal seq={} market_index={} payload decode failed: {:?}",
                    head_item.sequence,
                    market_index,
                    err
                );
                ctx.accounts.queue.load_mut()?.clear_head(sub_queue_idx)?;
                emit!(QueueItemProcessed {
                    group: group_key,
                    market_index,
                    sequence: head_item.sequence,
                    kind: reveal.kind,
                    status: CommitStatusV5::Failed as u8,
                    failure_code: QueueFailureCode::DecodeFailed as u8,
                });
                continue;
            }
        };
        let expected_kind = queue_item_kind_for_payload_variant(decoded_payload.variant);
        if expected_kind != reveal.kind {
                                                                            
                                                                           
            msg!(
                "v5 reveal seq={} market_index={} kind mismatch: reveal={} decoded={}; clearing head",
                head_item.sequence,
                market_index,
                reveal.kind,
                expected_kind
            );
            fail_head_v5(
                &ctx.accounts.queue,
                None,
                sub_queue_idx,
                &head_item,
                group_key,
                market_index,
                reveal.kind,
                QueueFailureCode::PayloadKindMismatch,
            )?;
            continue;
        }

        if let Some(reason) = prevalidate_terminal_ctm_payload(&decoded_payload, now_ts) {
            ctx.accounts.queue.load_mut()?.clear_head(sub_queue_idx)?;
            emit!(QueueItemProcessed {
                group: group_key,
                market_index,
                sequence: head_item.sequence,
                kind: reveal.kind,
                status: CommitStatusV5::Failed as u8,
                failure_code: QueueFailureCode::Expired as u8,
            });
            msg!(
                "{} seq={} reason={:?}",
                terminal_ctm_failure_msg(reason),
                head_item.sequence,
                reason
            );
            continue;
        }

                                                                               
                                                                           
                                                                            
                                                                            
                                                                           
                                                                             
                             
        if let Err(err) =
            require_dispatch_market_index(dispatch_accounts, market_index).and_then(|_| {
                validate_queue_payload_dispatch_accounts(
                    group_key,
                    &decoded_payload,
                    dispatch_accounts,
                )
            })
        {
            let failure_code = classify_predispatch_validation_failure(&err);
            msg!(
                "v5 reveal seq={} market_index={} dispatch accounts invalid ({:?}); clearing head: {:?}",
                head_item.sequence,
                market_index,
                failure_code,
                err
            );
            fail_head_v5(
                &ctx.accounts.queue,
                None,
                sub_queue_idx,
                &head_item,
                group_key,
                market_index,
                reveal.kind,
                failure_code,
            )?;
            continue;
        }

                                                                    
                                                                            
                                                                        
                                                       
                                                                      
                                                                        
                                                                         
                                                                          
                   
        msg!(
            "v5 reveal dispatch variant={:?} seq={} market_index={}",
            decoded_payload.variant,
            head_item.sequence,
            market_index
        );

                                                                        
                                                                         
                                                                          
                                                                        
          
                                                                            
                                                                      
                                                                        
                                                                    
                                                  
                                                                             
                                                               
                                                                      
        let mut pending_replay_hash: Option<[u8; 32]> = None;
        if variant_uses_user_signature(decoded_payload.variant) {
            let intent_signer = reveal.user_owner;
            let (fermi_account_key, signer_authorized) = match authorize_v5_intent_signer(
                group_key,
                dispatch_accounts,
                intent_signer,
                now_ts,
                delegate_intents_enabled,
            ) {
                Ok(authorized) => authorized,
                Err(err) => {
                                                                              
                                                                              
                                                                       
                    let failure_code = classify_predispatch_validation_failure(&err);
                    msg!(
                        "v5 reveal seq={} market_index={} user account invalid ({:?}); clearing head: {:?}",
                        head_item.sequence,
                        market_index,
                        failure_code,
                        err
                    );
                    fail_head_v5(
                        &ctx.accounts.queue,
                        None,
                        sub_queue_idx,
                        &head_item,
                        group_key,
                        market_index,
                        reveal.kind,
                        failure_code,
                    )?;
                    continue;
                }
            };
            if fermi_account_key != reveal.fermi_account {
                                                                             
                                                                
                msg!(
                    "v5 reveal seq={} market_index={} fermi_account {} disagrees with signed dispatch account {}; clearing head",
                    head_item.sequence,
                    market_index,
                    reveal.fermi_account,
                    fermi_account_key
                );
                fail_head_v5(
                    &ctx.accounts.queue,
                    None,
                    sub_queue_idx,
                    &head_item,
                    group_key,
                    market_index,
                    reveal.kind,
                    QueueFailureCode::UserAccountInvalid,
                )?;
                continue;
            }
            verify_user_ed25519_preinstruction(
                ctx.accounts.instructions.as_ref(),
                intent_signer,
                &[expected_commit_hash],
            )?;
            if !signer_authorized {
                msg!(
                    "v5 reveal seq={} market_index={} signer {} not authorized for {}",
                    head_item.sequence,
                    market_index,
                    intent_signer,
                    fermi_account_key
                );
                ctx.accounts.queue.load_mut()?.clear_head(sub_queue_idx)?;
                emit!(QueueItemProcessed {
                    group: group_key,
                    market_index,
                    sequence: head_item.sequence,
                    kind: reveal.kind,
                    status: CommitStatusV5::Failed as u8,
                    failure_code: QueueFailureCode::IntentSignerNotAuthorized as u8,
                });
                continue;
            }
            let replay_intent_hash = expected_commit_hash;
            let mut queue = ctx.accounts.queue.load_mut()?;
            if queue.has_recent_consumed_intent_hash(&replay_intent_hash, clock.slot) {
                msg!(
                    "v5 reveal seq={} market_index={} duplicate exact user intent client_order_id={}",
                    head_item.sequence,
                    market_index,
                    reveal.client_order_id
                );
                queue.clear_head(sub_queue_idx)?;
                drop(queue);
                emit!(QueueItemProcessed {
                    group: group_key,
                    market_index,
                    sequence: head_item.sequence,
                    kind: reveal.kind,
                    status: CommitStatusV5::Failed as u8,
                    failure_code: QueueFailureCode::IntentReplay as u8,
                });
                continue;
            }
            pending_replay_hash = Some(replay_intent_hash);
        }

                                         
        let item_health_region = queue_health_region_spec(&decoded_payload);
        if let Some(spec) = item_health_region {
            if let Err(err) = queue_health_region_begin(dispatch_accounts, spec) {
                let (terminal, mut failure_code) = classify_dispatch_failure(&err);
                                                                     
                                                                      
                                                        
                if failure_code == QueueFailureCode::Other {
                    failure_code = QueueFailureCode::HealthRegionBeginFailed;
                }
                msg!(
                    "v5 reveal seq={} market_index={} health_region_begin failed (terminal={}): {:?}",
                    head_item.sequence,
                    market_index,
                    terminal,
                    err
                );
                let (retries, max_retries) = {
                    let mut queue = ctx.accounts.queue.load_mut()?;
                    let max = queue.header.max_retries_effective();
                    let r = queue.increment_retry(sub_queue_idx, head_item.sequence, clock.slot)?;
                    (r, max)
                };
                if terminal || retries >= max_retries {
                    if !terminal {
                        failure_code = QueueFailureCode::RetriesExhausted;
                    }
                    msg!(
                        "v5 reveal seq={} market_index={} terminalizing health-region failure (retries={}/{}, terminal={}, code={:?}) — clearing head",
                        head_item.sequence,
                        market_index,
                        retries,
                        max_retries,
                        terminal,
                        failure_code,
                    );
                    resolve_head_v5(
                        &ctx.accounts.queue,
                        sub_queue_idx,
                        pending_replay_hash,
                        clock.slot,
                    )?;
                    emit!(QueueItemProcessed {
                        group: group_key,
                        market_index,
                        sequence: head_item.sequence,
                        kind: reveal.kind,
                        status: CommitStatusV5::Failed as u8,
                        failure_code: failure_code as u8,
                    });
                    continue;
                }
                break;
            }
        }

                                                                          
                                                            
        let queue_key = ctx.accounts.queue.key();
        let queue_bump = { ctx.accounts.queue.load()?.header.bump };
        let dispatch_result = dispatch_queue_payload(
            &decoded_payload,
            dispatch_accounts,
            &[],
            group_key,
            queue_key,
            queue_bump,
            market_index,
            head_item.sequence,
        );
        let dispatch_result = if let Some(spec) = item_health_region {
            match dispatch_result {
                Ok(()) => match queue_health_region_end(dispatch_accounts, spec) {
                    Ok(()) => Ok(()),
                    Err(err) => {
                        msg!(
                            "v5 reveal seq={} market_index={} health_region_end failed; returning error so dispatch state rolls back: {:?}",
                            head_item.sequence,
                            market_index,
                            err
                        );
                        return Err(err);
                    }
                },
                Err(err) => {
                    msg!(
                        "v5 reveal seq={} market_index={} health-region dispatch failed; returning error so partial dispatch state rolls back: {:?}",
                        head_item.sequence,
                        market_index,
                        err
                    );
                    return Err(err);
                }
            }
        } else {
            dispatch_result
        };

        match dispatch_result {
            Ok(_) => {
                resolve_head_v5(
                    &ctx.accounts.queue,
                    sub_queue_idx,
                    pending_replay_hash,
                    clock.slot,
                )?;
                emit!(QueueItemProcessed {
                    group: group_key,
                    market_index,
                    sequence: head_item.sequence,
                    kind: reveal.kind,
                    status: CommitStatusV5::Revealed as u8,
                    failure_code: QueueFailureCode::None as u8,
                });
            }
            Err(err) => {
                                                                             
                                                                             
                                                                           
                                                                      
                                                                         
                  
                                                                                
                                                                        
                                                                
                                                                           
                                                                          
                                                                          
                                                  
                let (terminal, mut failure_code) = classify_dispatch_failure(&err);
                msg!(
                    "v5 reveal seq={} market_index={} dispatch failed (terminal={}): {:?}",
                    head_item.sequence,
                    market_index,
                    terminal,
                    err
                );
                let (retries, max_retries) = {
                    let mut queue = ctx.accounts.queue.load_mut()?;
                    let max = queue.header.max_retries_effective();
                    let r = queue.increment_retry(sub_queue_idx, head_item.sequence, clock.slot)?;
                    (r, max)
                };
                if terminal || retries >= max_retries {
                    if !terminal {
                                                                        
                                                                        
                                                                        
                                                  
                        failure_code = QueueFailureCode::RetriesExhausted;
                    }
                    msg!(
                        "v5 reveal seq={} market={} terminalizing (retries={}/{}, terminal={}, code={:?}) — clearing head",
                        head_item.sequence,
                        market_index,
                        retries,
                        max_retries,
                        terminal,
                        failure_code,
                    );
                    resolve_head_v5(
                        &ctx.accounts.queue,
                        sub_queue_idx,
                        pending_replay_hash,
                        clock.slot,
                    )?;
                    emit!(QueueItemProcessed {
                        group: group_key,
                        market_index,
                        sequence: head_item.sequence,
                        kind: reveal.kind,
                        status: CommitStatusV5::Failed as u8,
                        failure_code: failure_code as u8,
                    });
                    continue;
                }
                break;
            }
        }
    }
    Ok(())
}

pub fn execution_queue_v5_execute_market(
    ctx: Context<ExecutionQueueV5ExecuteMarket>,
    market_index: u16,
    reveals: Vec<RevealArgsV5>,
) -> Result<()> {
    require!(
        reveals.len() <= EXECUTION_QUEUE_V5_MAX_REVEAL_BATCH,
        FermiError::ExecutionQueueV5InvalidRevealBatch
    );

    let group_key = ctx.accounts.group.key();
    let delegate_intents_enabled = ctx
        .accounts
        .group
        .load()?
        .is_ix_enabled(IxGate::V5DelegateIntents);
    let clock = Clock::get()?;
    let now_ts: u64 = clock.unix_timestamp.try_into().unwrap_or(0);

    let sub_queue_idx = {
        let queue = ctx.accounts.queue.load()?;
        require!(
            queue.header.paused_execute == 0,
            FermiError::ExecutionQueueExecutePaused
        );
        let idx = queue
            .find_sub_queue(market_index)
            .ok_or_else(|| error!(FermiError::ExecutionQueueV5SubQueueNotConfigured))?;
        require!(
            queue.sub_queue_headers[idx].paused_execute == 0,
            FermiError::ExecutionQueueExecutePaused
        );
        let direct_pool = ctx.accounts.direct_pool.load()?;
        require!(
            direct_pool.header.layout_version == EXECUTION_QUEUE_V5_DIRECT_LAYOUT_VERSION,
            FermiError::ExecutionQueueV5DirectLayoutNotReady
        );
        require!(
            direct_pool.header.market_index == market_index,
            FermiError::ExecutionQueueV5DirectItemMismatch
        );
        idx
    };

    {
        let mut queue = ctx.accounts.queue.load_mut()?;
        let needs_sweep = {
            let sqh = &queue.sub_queue_headers[sub_queue_idx];
            sqh.live_count == 0 && sqh.max_seen_sequence >= sqh.next_sequence_to_execute
        };
        if needs_sweep {
            skip_sub_queue_head_gaps_v5(&mut queue, sub_queue_idx, group_key, market_index);
        }
    }

    let total_accounts = ctx.remaining_accounts.len();
    let mut accounts_cursor = 0usize;
    let mut reveal_cursor = 0usize;
    let mut promotion_budget = EXECUTION_QUEUE_V5_MAX_DIRECT_PROMOTIONS_PER_EXECUTE;

    loop {
        if promotion_budget > 0 {
            let promoted = {
                let mut queue = ctx.accounts.queue.load_mut()?;
                let mut direct_pool = ctx.accounts.direct_pool.load_mut()?;
                promote_eligible_direct_items_v5(
                    &mut queue,
                    &mut direct_pool,
                    group_key,
                    market_index,
                    sub_queue_idx,
                    clock.slot,
                    promotion_budget,
                )?
            };
            promotion_budget = promotion_budget.saturating_sub(promoted);
        }

        let (next_sequence, gap_wait_slots, head_item, direct_meta) = {
            let queue = ctx.accounts.queue.load()?;
            let sqh = &queue.sub_queue_headers[sub_queue_idx];
            if sqh.live_count == 0 {
                break;
            }
            let next_sequence = sqh.next_sequence_to_execute;
            let gap_wait_slots = sqh.gap_wait_slots as u64;
            let head_item = queue.get_item(sub_queue_idx, next_sequence).cloned();
            let direct_meta = if let Some(ref item) = head_item {
                if let Some(direct_item_id) = commit_item_direct_item_id(item) {
                    let direct_pool = ctx.accounts.direct_pool.load()?;
                    let staged = direct_pool
                        .get_item(direct_item_id)
                        .ok_or_else(|| error!(FermiError::ExecutionQueueV5DirectItemMismatch))?;
                    require!(
                        staged.status == DirectItemStatusV5::Promoted as u8
                            && staged.promoted_sequence == item.sequence,
                        FermiError::ExecutionQueueV5DirectItemMismatch
                    );
                    Some((
                        direct_item_id,
                        staged.kind,
                        staged.payload_len as usize,
                        staged.dispatch_accounts_count as usize,
                        staged.fermi_account,
                        staged.user_owner,
                        staged.accounts_hash,
                        staged.payload[..staged.payload_len as usize].to_vec(),
                    ))
                } else {
                    None
                }
            } else {
                None
            };
            (next_sequence, gap_wait_slots, head_item, direct_meta)
        };

        let Some(head_item) = head_item else {
            let should_break = {
                let mut queue = ctx.accounts.queue.load_mut()?;
                let sqh = &mut queue.sub_queue_headers[sub_queue_idx];
                if sqh.max_seen_sequence < next_sequence {
                    true
                } else {
                    if sqh.gap_observed_slot == 0 {
                        sqh.gap_observed_slot = clock.slot;
                    }
                    if clock.slot >= sqh.gap_observed_slot.saturating_add(gap_wait_slots) {
                        skip_sub_queue_head_gaps_v5(
                            &mut queue,
                            sub_queue_idx,
                            group_key,
                            market_index,
                        );
                        false
                    } else {
                        true
                    }
                }
            };
            if should_break {
                break;
            }
            continue;
        };

        let direct_item_id = direct_meta.as_ref().map(|meta| meta.0);
        let head_kind = direct_meta
            .as_ref()
            .map(|meta| meta.1)
            .unwrap_or(QueueItemKind::CtmWrapped as u8);

        if clock.slot < head_item.min_execute_slot {
            break;
        }
        if head_item.expires_at_slot != 0 && clock.slot > head_item.expires_at_slot {
            msg!(
                "v5 execute seq={} market_index={} expired: clock.slot={} expires_at_slot={}",
                head_item.sequence,
                market_index,
                clock.slot,
                head_item.expires_at_slot
            );
            ctx.accounts.queue.load_mut()?.clear_head(sub_queue_idx)?;
            if direct_item_id.is_some() {
                finalize_direct_head_v5(
                    &ctx.accounts.direct_pool,
                    &head_item,
                    DirectItemStatusV5::Expired,
                )?;
            }
            emit!(QueueItemProcessed {
                group: group_key,
                market_index,
                sequence: head_item.sequence,
                kind: head_kind,
                status: CommitStatusV5::Failed as u8,
                failure_code: QueueFailureCode::Expired as u8,
            });
            continue;
        }

        let (
            payload,
            kind,
            dispatch_accounts_count,
            fermi_account,
            user_owner,
            mut expected_commit_hash,
            direct_accounts_hash,
            ctm_client_order_id,
        ) = if let Some(meta) = direct_meta {
            (
                meta.7,
                meta.1,
                meta.3,
                meta.4,
                meta.5,
                None,
                Some(meta.6),
                None,
            )
        } else {
            if reveal_cursor >= reveals.len() {
                break;
            }
            let reveal = &reveals[reveal_cursor];
            reveal_cursor = reveal_cursor.saturating_add(1);

            (
                reveal.payload.clone(),
                reveal.kind,
                reveal.dispatch_accounts_count as usize,
                reveal.fermi_account,
                reveal.user_owner,
                None,
                None,
                Some(reveal.client_order_id),
            )
        };
        let mut ctm_replay_intent_hash: Option<[u8; 32]> = None;

        require!(
            dispatch_accounts_count > 0
                && accounts_cursor + dispatch_accounts_count <= total_accounts,
            FermiError::ExecutionQueueDispatchAccountLayoutInvalid
        );
        let dispatch_accounts =
            &ctx.remaining_accounts[accounts_cursor..accounts_cursor + dispatch_accounts_count];
        accounts_cursor += dispatch_accounts_count;
        let dispatch_account_metas = account_metas_from_infos(dispatch_accounts);

        if let Some(client_order_id) = ctm_client_order_id {
            let payload_hash = hashv(&[&payload]).to_bytes();
            let accounts_hash = hash_accounts_for_v5_ctm_reveal(
                group_key,
                ctx.accounts.queue.key(),
                &dispatch_account_metas,
            );
            let recomputed_commit_hash = canonical_user_intent_message_v3(
                group_key,
                fermi_account,
                user_owner,
                kind,
                UserIntentTargetKind::PerpMarket,
                market_index,
                &payload_hash,
                &accounts_hash,
                head_item.min_execute_slot,
                head_item.expires_at_slot,
                client_order_id,
            );
            if recomputed_commit_hash != head_item.commit_hash {
                let expected_prefix = u64::from_le_bytes([
                    recomputed_commit_hash[0],
                    recomputed_commit_hash[1],
                    recomputed_commit_hash[2],
                    recomputed_commit_hash[3],
                    recomputed_commit_hash[4],
                    recomputed_commit_hash[5],
                    recomputed_commit_hash[6],
                    recomputed_commit_hash[7],
                ]);
                let stored_prefix = u64::from_le_bytes([
                    head_item.commit_hash[0],
                    head_item.commit_hash[1],
                    head_item.commit_hash[2],
                    head_item.commit_hash[3],
                    head_item.commit_hash[4],
                    head_item.commit_hash[5],
                    head_item.commit_hash[6],
                    head_item.commit_hash[7],
                ]);
                let payload_prefix = u64::from_le_bytes([
                    payload_hash[0],
                    payload_hash[1],
                    payload_hash[2],
                    payload_hash[3],
                    payload_hash[4],
                    payload_hash[5],
                    payload_hash[6],
                    payload_hash[7],
                ]);
                msg!(
                    "v5 execute commit_hash mismatch: seq={} market={} kind={} expected_prefix={:#x} stored_prefix={:#x} payload_prefix={:#x} payload_len={} fermi_account={} user_owner={}",
                    head_item.sequence,
                    market_index,
                    kind,
                    expected_prefix,
                    stored_prefix,
                    payload_prefix,
                    payload.len(),
                    fermi_account,
                    user_owner,
                );
                return err!(FermiError::ExecutionQueueV5CommitRevealMismatch);
            }
            expected_commit_hash = Some(recomputed_commit_hash);
            ctm_replay_intent_hash = Some(recomputed_commit_hash);
        }

        if let Some(expected_accounts_hash) = direct_accounts_hash {
            let provided_accounts_hash =
                hash_accounts(&canonical_v5_direct_dispatch_account_metas(
                    group_key,
                    ctx.accounts.queue.key(),
                    &dispatch_account_metas,
                    user_owner,
                ));
            if provided_accounts_hash != expected_accounts_hash {
                msg!(
                    "v5 execute direct accounts mismatch: seq={} market={} item_id={:?}",
                    head_item.sequence,
                    market_index,
                    direct_item_id,
                );
                break;
            }
        }

        let decoded_payload = match decode_queue_payload(&payload) {
            Ok(p) if p.flags == 0 => p,
            Ok(p) => {
                msg!(
                    "v5 execute seq={} market_index={} rejected: non-zero payload flags={}",
                    head_item.sequence,
                    market_index,
                    p.flags
                );
                ctx.accounts.queue.load_mut()?.clear_head(sub_queue_idx)?;
                if direct_item_id.is_some() {
                    finalize_direct_head_v5(
                        &ctx.accounts.direct_pool,
                        &head_item,
                        DirectItemStatusV5::Failed,
                    )?;
                }
                emit!(QueueItemProcessed {
                    group: group_key,
                    market_index,
                    sequence: head_item.sequence,
                    kind,
                    status: CommitStatusV5::Failed as u8,
                    failure_code: QueueFailureCode::PayloadVariantInvalid as u8,
                });
                continue;
            }
            Err(err) => {
                msg!(
                    "v5 execute seq={} market_index={} payload decode failed: {:?}",
                    head_item.sequence,
                    market_index,
                    err
                );
                ctx.accounts.queue.load_mut()?.clear_head(sub_queue_idx)?;
                if direct_item_id.is_some() {
                    finalize_direct_head_v5(
                        &ctx.accounts.direct_pool,
                        &head_item,
                        DirectItemStatusV5::Failed,
                    )?;
                }
                emit!(QueueItemProcessed {
                    group: group_key,
                    market_index,
                    sequence: head_item.sequence,
                    kind,
                    status: CommitStatusV5::Failed as u8,
                    failure_code: QueueFailureCode::DecodeFailed as u8,
                });
                continue;
            }
        };

        let expected_kind = queue_item_kind_for_payload_variant(decoded_payload.variant);
        if expected_kind != kind {
                                                                           
                                                                     
            msg!(
                "v5 execute seq={} market_index={} kind mismatch: stored={} decoded={}; clearing head",
                head_item.sequence,
                market_index,
                kind,
                expected_kind,
            );
            fail_head_v5(
                &ctx.accounts.queue,
                Some(&ctx.accounts.direct_pool),
                sub_queue_idx,
                &head_item,
                group_key,
                market_index,
                kind,
                QueueFailureCode::PayloadKindMismatch,
            )?;
            continue;
        }

        if let Some(reason) = prevalidate_terminal_ctm_payload(&decoded_payload, now_ts) {
            ctx.accounts.queue.load_mut()?.clear_head(sub_queue_idx)?;
            if direct_item_id.is_some() {
                finalize_direct_head_v5(
                    &ctx.accounts.direct_pool,
                    &head_item,
                    if matches!(reason, TerminalCtmFailureReason::Expired) {
                        DirectItemStatusV5::Expired
                    } else {
                        DirectItemStatusV5::Failed
                    },
                )?;
            }
            emit!(QueueItemProcessed {
                group: group_key,
                market_index,
                sequence: head_item.sequence,
                kind,
                status: CommitStatusV5::Failed as u8,
                failure_code: QueueFailureCode::Expired as u8,
            });
            msg!(
                "{} seq={} reason={:?}",
                terminal_ctm_failure_msg(reason),
                head_item.sequence,
                reason
            );
            continue;
        }

                                                                            
                                                                           
        if let Err(err) =
            require_dispatch_market_index(dispatch_accounts, market_index).and_then(|_| {
                validate_queue_payload_dispatch_accounts(
                    group_key,
                    &decoded_payload,
                    dispatch_accounts,
                )
            })
        {
            let failure_code = classify_predispatch_validation_failure(&err);
            msg!(
                "v5 execute seq={} market_index={} dispatch accounts invalid ({:?}); clearing head: {:?}",
                head_item.sequence,
                market_index,
                failure_code,
                err
            );
            fail_head_v5(
                &ctx.accounts.queue,
                Some(&ctx.accounts.direct_pool),
                sub_queue_idx,
                &head_item,
                group_key,
                market_index,
                kind,
                failure_code,
            )?;
            continue;
        }

                                                                     
        let mut pending_replay_hash: Option<[u8; 32]> = None;
        if variant_uses_user_signature(decoded_payload.variant) {
                                                                             
                                                                       
            let (fermi_account_key, signer_authorized) = match authorize_v5_intent_signer(
                group_key,
                dispatch_accounts,
                user_owner,
                now_ts,
                delegate_intents_enabled,
            ) {
                Ok(authorized) => authorized,
                Err(err) => {
                    let failure_code = classify_predispatch_validation_failure(&err);
                    msg!(
                        "v5 execute seq={} market_index={} user account invalid ({:?}); clearing head: {:?}",
                        head_item.sequence,
                        market_index,
                        failure_code,
                        err
                    );
                    fail_head_v5(
                        &ctx.accounts.queue,
                        Some(&ctx.accounts.direct_pool),
                        sub_queue_idx,
                        &head_item,
                        group_key,
                        market_index,
                        kind,
                        failure_code,
                    )?;
                    continue;
                }
            };
                                                                             
                                                                               
                                                                     
            if fermi_account_key != fermi_account {
                msg!(
                    "v5 execute seq={} market_index={} fermi_account {} disagrees with signed dispatch account {}; clearing head",
                    head_item.sequence,
                    market_index,
                    fermi_account,
                    fermi_account_key
                );
                fail_head_v5(
                    &ctx.accounts.queue,
                    Some(&ctx.accounts.direct_pool),
                    sub_queue_idx,
                    &head_item,
                    group_key,
                    market_index,
                    kind,
                    QueueFailureCode::UserAccountInvalid,
                )?;
                continue;
            }
            if direct_item_id.is_some() {
                                                                             
                                                                                  
                if !signer_authorized {
                    msg!(
                        "v5 execute direct seq={} market_index={} signer {} not authorized for {}",
                        head_item.sequence,
                        market_index,
                        user_owner,
                        fermi_account_key
                    );
                    ctx.accounts.queue.load_mut()?.clear_head(sub_queue_idx)?;
                    finalize_direct_head_v5(
                        &ctx.accounts.direct_pool,
                        &head_item,
                        DirectItemStatusV5::Failed,
                    )?;
                    emit!(QueueItemProcessed {
                        group: group_key,
                        market_index,
                        sequence: head_item.sequence,
                        kind,
                        status: CommitStatusV5::Failed as u8,
                        failure_code: QueueFailureCode::IntentSignerNotAuthorized as u8,
                    });
                    continue;
                }
            } else {
                let intent_hash =
                    expected_commit_hash.ok_or_else(|| error!(FermiError::SomeError))?;
                verify_user_ed25519_preinstruction(
                    ctx.accounts.instructions.as_ref(),
                    user_owner,
                    &[intent_hash],
                )?;
                if !signer_authorized {
                    msg!(
                        "v5 execute seq={} market_index={} signer {} not authorized for {}",
                        head_item.sequence,
                        market_index,
                        user_owner,
                        fermi_account_key
                    );
                    ctx.accounts.queue.load_mut()?.clear_head(sub_queue_idx)?;
                    emit!(QueueItemProcessed {
                        group: group_key,
                        market_index,
                        sequence: head_item.sequence,
                        kind,
                        status: CommitStatusV5::Failed as u8,
                        failure_code: QueueFailureCode::IntentSignerNotAuthorized as u8,
                    });
                    continue;
                }
                let replay_intent_hash =
                    ctm_replay_intent_hash.ok_or_else(|| error!(FermiError::SomeError))?;
                let mut queue = ctx.accounts.queue.load_mut()?;
                if queue.has_recent_consumed_intent_hash(&replay_intent_hash, clock.slot) {
                    msg!(
                        "v5 execute seq={} market_index={} duplicate exact user intent",
                        head_item.sequence,
                        market_index,
                    );
                    queue.clear_head(sub_queue_idx)?;
                    drop(queue);
                    emit!(QueueItemProcessed {
                        group: group_key,
                        market_index,
                        sequence: head_item.sequence,
                        kind,
                        status: CommitStatusV5::Failed as u8,
                        failure_code: QueueFailureCode::IntentReplay as u8,
                    });
                    continue;
                }
                pending_replay_hash = Some(replay_intent_hash);
            }
        }

        let item_health_region = queue_health_region_spec(&decoded_payload);
        if let Some(spec) = item_health_region {
            if let Err(err) = queue_health_region_begin(dispatch_accounts, spec) {
                let (terminal, mut failure_code) = classify_dispatch_failure(&err);
                if failure_code == QueueFailureCode::Other {
                    failure_code = QueueFailureCode::HealthRegionBeginFailed;
                }
                msg!(
                    "v5 execute seq={} market_index={} health_region_begin failed (terminal={}): {:?}",
                    head_item.sequence,
                    market_index,
                    terminal,
                    err
                );
                let (retries, max_retries) = {
                    let mut queue = ctx.accounts.queue.load_mut()?;
                    let max = queue.header.max_retries_effective();
                    let r = queue.increment_retry(sub_queue_idx, head_item.sequence, clock.slot)?;
                    (r, max)
                };
                if terminal || retries >= max_retries {
                    if !terminal {
                        failure_code = QueueFailureCode::RetriesExhausted;
                    }
                    resolve_head_v5(
                        &ctx.accounts.queue,
                        sub_queue_idx,
                        pending_replay_hash,
                        clock.slot,
                    )?;
                    if direct_item_id.is_some() {
                        finalize_direct_head_v5(
                            &ctx.accounts.direct_pool,
                            &head_item,
                            DirectItemStatusV5::Failed,
                        )?;
                    }
                    emit!(QueueItemProcessed {
                        group: group_key,
                        market_index,
                        sequence: head_item.sequence,
                        kind,
                        status: CommitStatusV5::Failed as u8,
                        failure_code: failure_code as u8,
                    });
                    continue;
                }
                break;
            }
        }

        let queue_key = ctx.accounts.queue.key();
        let queue_bump = { ctx.accounts.queue.load()?.header.bump };
        let dispatch_result = dispatch_queue_payload(
            &decoded_payload,
            dispatch_accounts,
            &[],
            group_key,
            queue_key,
            queue_bump,
            market_index,
            head_item.sequence,
        );
        let dispatch_result = if let Some(spec) = item_health_region {
            match dispatch_result {
                Ok(()) => match queue_health_region_end(dispatch_accounts, spec) {
                    Ok(()) => Ok(()),
                    Err(err) => {
                        msg!(
                            "v5 execute seq={} market_index={} health_region_end failed; returning error so dispatch state rolls back: {:?}",
                            head_item.sequence,
                            market_index,
                            err
                        );
                        return Err(err);
                    }
                },
                Err(err) => {
                    msg!(
                        "v5 execute seq={} market_index={} health-region dispatch failed; returning error so partial dispatch state rolls back: {:?}",
                        head_item.sequence,
                        market_index,
                        err
                    );
                    return Err(err);
                }
            }
        } else {
            dispatch_result
        };

        match dispatch_result {
            Ok(_) => {
                resolve_head_v5(
                    &ctx.accounts.queue,
                    sub_queue_idx,
                    pending_replay_hash,
                    clock.slot,
                )?;
                if direct_item_id.is_some() {
                    finalize_direct_head_v5(
                        &ctx.accounts.direct_pool,
                        &head_item,
                        DirectItemStatusV5::Executed,
                    )?;
                }
                emit!(QueueItemProcessed {
                    group: group_key,
                    market_index,
                    sequence: head_item.sequence,
                    kind,
                    status: CommitStatusV5::Revealed as u8,
                    failure_code: QueueFailureCode::None as u8,
                });
            }
            Err(err) => {
                let (terminal, mut failure_code) = classify_dispatch_failure(&err);
                msg!(
                    "v5 execute seq={} market_index={} dispatch failed (terminal={}): {:?}",
                    head_item.sequence,
                    market_index,
                    terminal,
                    err
                );
                let (retries, max_retries) = {
                    let mut queue = ctx.accounts.queue.load_mut()?;
                    let max = queue.header.max_retries_effective();
                    let r = queue.increment_retry(sub_queue_idx, head_item.sequence, clock.slot)?;
                    (r, max)
                };
                if terminal || retries >= max_retries {
                    if !terminal {
                        failure_code = QueueFailureCode::RetriesExhausted;
                    }
                    resolve_head_v5(
                        &ctx.accounts.queue,
                        sub_queue_idx,
                        pending_replay_hash,
                        clock.slot,
                    )?;
                    if direct_item_id.is_some() {
                        finalize_direct_head_v5(
                            &ctx.accounts.direct_pool,
                            &head_item,
                            DirectItemStatusV5::Failed,
                        )?;
                    }
                    emit!(QueueItemProcessed {
                        group: group_key,
                        market_index,
                        sequence: head_item.sequence,
                        kind,
                        status: CommitStatusV5::Failed as u8,
                        failure_code: failure_code as u8,
                    });
                    continue;
                }
                break;
            }
        }
    }

    Ok(())
}

                                                                          
                                                                      
                                                                      
                                                                   
                                                                      
                                                    
   
                                                                      
                                                                    
                                                                       
                                                        
fn classify_dispatch_failure(err: &anchor_lang::error::Error) -> (bool, QueueFailureCode) {
    use anchor_lang::error::Error::*;
    let code = match err {
        AnchorError(ae) => ae.error_code_number,
        _ => return (false, QueueFailureCode::Other),
    };
                                                                      
                               
    let mapped = match code.checked_sub(6000) {
        Some(c)
            if c == FermiError::HealthMustBePositive as u32
                || c == FermiError::HealthMustBePositiveOrIncrease as u32
                || c == FermiError::HealthMustBeNegative as u32
                || c == FermiError::InvalidHealth as u32 =>
        {
            Some(QueueFailureCode::InsufficientMargin)
        }
        Some(c) if c == FermiError::BeingLiquidated as u32 => {
            Some(QueueFailureCode::BeingLiquidated)
        }
        Some(c) if c == FermiError::IsBankrupt as u32 => Some(QueueFailureCode::Bankrupt),
        Some(c) if c == FermiError::OracleStale as u32 => Some(QueueFailureCode::OracleStale),
        Some(c) if c == FermiError::OracleConfidence as u32 => {
            Some(QueueFailureCode::OracleConfidence)
        }
        Some(c) if c == FermiError::SpotPriceBandExceeded as u32 => {
            Some(QueueFailureCode::PriceBandExceeded)
        }
        Some(c) if c == FermiError::MarketInReduceOnlyMode as u32 => {
            Some(QueueFailureCode::MarketReduceOnly)
        }
        Some(c) if c == FermiError::TokenInReduceOnlyMode as u32 => {
            Some(QueueFailureCode::TokenReduceOnly)
        }
        Some(c) if c == FermiError::TokenInForceClose as u32 => {
            Some(QueueFailureCode::TokenForceClose)
        }
        Some(c) if c == FermiError::AccountIsFrozen as u32 => Some(QueueFailureCode::AccountFrozen),
        Some(c) if c == FermiError::GroupIsHalted as u32 => Some(QueueFailureCode::GroupHalted),
        Some(c) if c == FermiError::BankBorrowLimitReached as u32 => {
            Some(QueueFailureCode::BankBorrowLimit)
        }
        Some(c) if c == FermiError::BankNetBorrowsLimitReached as u32 => {
            Some(QueueFailureCode::BankNetBorrowsLimit)
        }
        Some(c) if c == FermiError::BankDepositLimit as u32 => {
            Some(QueueFailureCode::BankDepositLimit)
        }
        Some(c) if c == FermiError::DepositLimit as u32 => {
            Some(QueueFailureCode::GroupDepositLimit)
        }
        Some(c) if c == FermiError::WouldSelfTrade as u32 => Some(QueueFailureCode::WouldSelfTrade),
                                                                         
                                                                        
                                                    
                                                                          
                                                                       
                                            
        Some(c) if c == FermiError::PerpOrderIdNotFound as u32 => {
            Some(QueueFailureCode::PerpOrderIdNotFound)
        }
        _ => None,
    };
    match mapped {
        Some(fc) => (true, fc),
        None => (false, QueueFailureCode::Other),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::instructions::execution_queue::v5_intent_signer_authorized;

    #[test]
    fn v5_intent_signer_authorization() {
        let owner = Pubkey::new_unique();
        let delegate = Pubkey::new_unique();
        let temporary_delegate = Pubkey::new_unique();
        let stranger = Pubkey::new_unique();
        let mut account: FermiAccountFixed = bytemuck::Zeroable::zeroed();
        account.owner = owner;

                                                                                 
        for allow in [false, true] {
            assert!(v5_intent_signer_authorized(&account, owner, 100, allow));
            assert!(!v5_intent_signer_authorized(
                &account,
                Pubkey::default(),
                100,
                allow
            ));
            assert!(!v5_intent_signer_authorized(&account, stranger, 100, allow));
        }

        account.delegate = delegate;
        account.temporary_delegate = temporary_delegate;
        account.temporary_delegate_expiry = 100;

                                                                         
        assert!(v5_intent_signer_authorized(&account, owner, 100, true));
        assert!(v5_intent_signer_authorized(&account, delegate, 100, true));
        assert!(v5_intent_signer_authorized(
            &account,
            temporary_delegate,
            100,
            true
        ));
        assert!(!v5_intent_signer_authorized(
            &account,
            temporary_delegate,
            101,
            true
        ));
        assert!(!v5_intent_signer_authorized(&account, stranger, 100, true));

                                    
        assert!(v5_intent_signer_authorized(&account, owner, 100, false));
        assert!(!v5_intent_signer_authorized(&account, delegate, 100, false));
        assert!(!v5_intent_signer_authorized(
            &account,
            temporary_delegate,
            100,
            false
        ));

                           
        account.delegate = Pubkey::default();
        assert!(!v5_intent_signer_authorized(&account, delegate, 100, true));
    }

    #[test]
    fn direct_envelope_timing_requires_bounded_expiry() {
        let now = 1_000u64;
        let bump = EXECUTION_QUEUE_V5_DIRECT_DEFAULT_SPEEDBUMP_SLOTS;
        let bump_slots = bump as u64;
        let window = EXECUTION_QUEUE_V5_REPLAY_RETENTION_SLOTS;

                                                              
        assert!(validate_direct_envelope_timing(now, bump, 0).is_err());
                           
        assert!(validate_direct_envelope_timing(now, bump, now - 1).is_err());
                                                                      
        assert!(validate_direct_envelope_timing(now, bump, now + bump_slots - 1).is_err());
                                              
        assert_eq!(
            validate_direct_envelope_timing(now, bump, now + bump_slots).unwrap(),
            now + bump_slots
        );
        assert_eq!(
            validate_direct_envelope_timing(now, bump, now + window).unwrap(),
            now + bump_slots
        );
                                                                        
        assert!(validate_direct_envelope_timing(now, (window + 1) as u16, now + window).is_err());
    }

    #[test]
    fn canonical_fixed_prefix_rejects_short_dispatch_accounts() {
                                                                         
                                                                             
                                                                          
                                                                    
                                                                         
                                             
        let mut payload_bytes = Vec::new();
                                                                    
                        
        payload_bytes.extend_from_slice(&[1, 1, 0, 0]);
                                                          
        payload_bytes.extend_from_slice(&0u128.to_le_bytes());
        let decoded = decode_queue_payload(&payload_bytes).expect("payload decodes");
        let err = validate_v5_direct_canonical_fixed_prefix(
            Pubkey::new_unique(),
            Pubkey::new_unique(),
            Pubkey::new_unique(),
            &decoded,
            &[],
        )
        .unwrap_err();
        assert!(format!("{:?}", err).contains("ExecutionQueueDispatchAccountLayoutInvalid"));
    }

    #[test]
    fn user_intent_hash_allows_same_client_order_id_for_distinct_payloads() {
        let group = Pubkey::new_unique();
        let fermi_account = Pubkey::new_unique();
        let user_owner = Pubkey::new_unique();
        let accounts_hash = [9; 32];
        let first_payload = [1; 32];
        let second_payload = [2; 32];
        let client_order_id = 0;

        let first = canonical_user_intent_message_v3(
            group,
            fermi_account,
            user_owner,
            QueueItemKind::CtmWrapped as u8,
            UserIntentTargetKind::PerpMarket,
            5,
            &first_payload,
            &accounts_hash,
            0,
            0,
            client_order_id,
        );
        let second = canonical_user_intent_message_v3(
            group,
            fermi_account,
            user_owner,
            QueueItemKind::CtmWrapped as u8,
            UserIntentTargetKind::PerpMarket,
            5,
            &second_payload,
            &accounts_hash,
            0,
            0,
            client_order_id,
        );

        assert_ne!(first, second);
    }
}
