use crate::accounts_ix::*;
use crate::error::*;
use crate::state::*;
use anchor_lang::prelude::*;
use anchor_lang::solana_program::entrypoint::MAX_PERMITTED_DATA_INCREASE;
use anchor_lang::solana_program::hash::hashv;
use anchor_lang::solana_program::program::invoke;
use anchor_lang::solana_program::system_instruction;

use super::execution_queue::{
    account_metas_from_infos, canonical_envelope_message, canonical_user_intent_message_v1,
    canonical_user_intent_message_v2, decode_queue_payload, dispatch_queue_payload,
    extract_user_owner_for_ctm_payload, hash_accounts, prevalidate_terminal_ctm_payload,
    queue_health_region_begin, queue_health_region_end, queue_health_region_spec,
    queue_item_kind_for_payload_variant, require_dispatch_market_index, split_dispatch_accounts,
    terminal_ctm_failure_msg, validate_queue_payload_dispatch_accounts,
    variant_uses_queue_owner_signer, variant_uses_user_signature, verify_ed25519_preinstruction,
    verify_user_ed25519_preinstruction, CtmEnvelope, QueueFailureCode, QueueItemEnqueued,
    QueueItemProcessed, TerminalCtmFailureReason, UserIntentTargetKind,
    EXECUTION_QUEUE_GAP_SKIP_LIMIT_PER_EXECUTE,
};

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug)]
pub struct ExecutionQueueV2ConfigureParams {
    pub gap_wait_slots: u16,
    pub max_retries: u8,
    pub pause_ingress: bool,
    pub pause_execute: bool,
}

#[derive(Clone, Debug)]
struct ExecutableCandidateV2 {
    sequence: u64,
    kind: u8,
    payload: Vec<u8>,
    accounts_hash: [u8; 32],
    retries: u8,
}

enum HeadSelectionV2 {
    Candidate(ExecutableCandidateV2),
    Blocked,
    AdvancedGap,
    Empty,
}

enum ExecuteProgressV2 {
    Continue,
    Break,
}

fn require_v2_layout(queue: &ExecutionQueueV2, market_index: u16) -> Result<()> {
    require!(
        queue.header.layout_version == EXECUTION_QUEUE_V2_LAYOUT_VERSION,
        FermiError::ExecutionQueueSubQueueLayoutVersionMismatch
    );
    require!(
        queue.header.market_index == market_index,
        FermiError::ExecutionQueueSubQueueMarketIndexMismatch
    );
    Ok(())
}

fn terminal_failure_code_v2(reason: TerminalCtmFailureReason) -> QueueFailureCode {
    match reason {
        TerminalCtmFailureReason::Expired => QueueFailureCode::Expired,
        TerminalCtmFailureReason::InvalidNumericInput => QueueFailureCode::Other,
    }
}

fn skip_head_gaps_v2(queue: &mut ExecutionQueueV2, group: Pubkey, market_index: u16) {
    let mut skipped = 0u16;
    loop {
        if skipped >= EXECUTION_QUEUE_GAP_SKIP_LIMIT_PER_EXECUTE {
            break;
        }
        let next_sequence = queue.header.next_sequence_to_execute;
        if queue.header.max_seen_sequence < next_sequence {
            break;
        }
        let phys = ExecutionQueueV2::physical_index(next_sequence);
        let item_present = {
            let item = &queue.items[phys];
            item.status != QueueItemStatus::Empty as u8 && item.sequence == next_sequence
        };
        if item_present && queue.header.live_count > 0 {
            break;
        }
        emit!(QueueItemProcessed {
            group,
            market_index,
            sequence: next_sequence,
            kind: QueueItemKind::CtmWrapped as u8,
            status: QueueItemStatus::Skipped as u8,
            failure_code: QueueFailureCode::GapSkipped as u8,
        });
        queue.items[phys] = QueueItem::default();
        queue.header.next_sequence_to_execute = next_sequence.saturating_add(1);
        queue.header.gap_observed_slot = 0;
        skipped = skipped.saturating_add(1);
    }
}

fn select_head_candidate_v2(
    queue: &mut ExecutionQueueV2,
    clock: &Clock,
    group: Pubkey,
    market_index: u16,
) -> HeadSelectionV2 {
    let next_sequence = queue.header.next_sequence_to_execute;
    if queue.header.live_count > 0 {
        if let Some(item) = queue.get_item(next_sequence).copied() {
            if clock.slot < item.min_execute_slot {
                return HeadSelectionV2::Blocked;
            }
            let payload_len = item.payload_len as usize;
            return HeadSelectionV2::Candidate(ExecutableCandidateV2 {
                sequence: item.sequence,
                kind: item.kind,
                payload: item.payload[..payload_len].to_vec(),
                accounts_hash: item.accounts_hash,
                retries: item.retries,
            });
        }
    }

    if queue.header.max_seen_sequence >= next_sequence {
        if queue.header.gap_observed_slot == 0 {
            queue.header.gap_observed_slot = clock.slot;
        }
        if clock.slot
            >= queue
                .header
                .gap_observed_slot
                .saturating_add(queue.header.gap_wait_slots as u64)
        {
            skip_head_gaps_v2(queue, group, market_index);
            HeadSelectionV2::AdvancedGap
        } else {
            HeadSelectionV2::Blocked
        }
    } else {
        HeadSelectionV2::Empty
    }
}

fn classify_dispatch_failure_v2(err: &anchor_lang::error::Error) -> (bool, QueueFailureCode) {
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

fn process_candidate_v2<'info>(
    queue_loader: &AccountLoader<'info, ExecutionQueueV2>,
    candidate: ExecutableCandidateV2,
    dispatch_accounts: &[AccountInfo<'info>],
    invoke_accounts: &[AccountInfo<'info>],
    group_key: Pubkey,
    queue_key: Pubkey,
    queue_bump: u8,
    market_index: u16,
    current_slot: u64,
    now_ts: u64,
    max_retries: u8,
) -> Result<ExecuteProgressV2> {
    let decoded_payload = match decode_queue_payload(&candidate.payload) {
        Ok(payload) if payload.flags == 0 => payload,
        Ok(_) => {
            queue_loader.load_mut()?.clear_item_at(candidate.sequence)?;
            emit!(QueueItemProcessed {
                group: group_key,
                market_index,
                sequence: candidate.sequence,
                kind: candidate.kind,
                status: QueueItemStatus::Failed as u8,
                failure_code: QueueFailureCode::PayloadVariantInvalid as u8,
            });
            return Ok(ExecuteProgressV2::Continue);
        }
        Err(_) => {
            queue_loader.load_mut()?.clear_item_at(candidate.sequence)?;
            emit!(QueueItemProcessed {
                group: group_key,
                market_index,
                sequence: candidate.sequence,
                kind: candidate.kind,
                status: QueueItemStatus::Failed as u8,
                failure_code: QueueFailureCode::DecodeFailed as u8,
            });
            return Ok(ExecuteProgressV2::Continue);
        }
    };

    let payload_kind = queue_item_kind_for_payload_variant(decoded_payload.variant);
    if payload_kind != candidate.kind {
        queue_loader.load_mut()?.clear_item_at(candidate.sequence)?;
        emit!(QueueItemProcessed {
            group: group_key,
            market_index,
            sequence: candidate.sequence,
            kind: candidate.kind,
            status: QueueItemStatus::Failed as u8,
            failure_code: QueueFailureCode::PayloadKindMismatch as u8,
        });
        return Ok(ExecuteProgressV2::Continue);
    }

    if let Some(reason) = prevalidate_terminal_ctm_payload(&decoded_payload, now_ts) {
        queue_loader.load_mut()?.clear_item_at(candidate.sequence)?;
        emit!(QueueItemProcessed {
            group: group_key,
            market_index,
            sequence: candidate.sequence,
            kind: candidate.kind,
            status: QueueItemStatus::Failed as u8,
            failure_code: terminal_failure_code_v2(reason) as u8,
        });
        msg!(
            "{} seq={} reason={:?}",
            terminal_ctm_failure_msg(reason),
            candidate.sequence,
            reason
        );
        return Ok(ExecuteProgressV2::Continue);
    }

    let item_health_region = queue_health_region_spec(&decoded_payload);
    if item_health_region.is_some() && candidate.retries >= max_retries {
        queue_loader.load_mut()?.clear_item_at(candidate.sequence)?;
        emit!(QueueItemProcessed {
            group: group_key,
            market_index,
            sequence: candidate.sequence,
            kind: candidate.kind,
            status: QueueItemStatus::Failed as u8,
            failure_code: QueueFailureCode::RetriesExhausted as u8,
        });
        return Ok(ExecuteProgressV2::Continue);
    }

    if let Some(spec) = item_health_region {
        if let Err(err) = queue_health_region_begin(dispatch_accounts, spec) {
            let (terminal, mut failure_code) = classify_dispatch_failure_v2(&err);
            if failure_code == QueueFailureCode::Other {
                failure_code = QueueFailureCode::HealthRegionBeginFailed;
            }
            let retries = queue_loader
                .load_mut()?
                .increment_retry(candidate.sequence, current_slot)?;
            if terminal || retries >= max_retries {
                if !terminal {
                    failure_code = QueueFailureCode::RetriesExhausted;
                }
                queue_loader.load_mut()?.clear_item_at(candidate.sequence)?;
                emit!(QueueItemProcessed {
                    group: group_key,
                    market_index,
                    sequence: candidate.sequence,
                    kind: candidate.kind,
                    status: QueueItemStatus::Failed as u8,
                    failure_code: failure_code as u8,
                });
                return Ok(ExecuteProgressV2::Continue);
            }
            return Ok(ExecuteProgressV2::Break);
        }
    }

    let dispatch_result = dispatch_queue_payload(
        &decoded_payload,
        dispatch_accounts,
        invoke_accounts,
        group_key,
        queue_key,
        queue_bump,
        market_index,
        candidate.sequence,
    );
    let dispatch_result = if let Some(spec) = item_health_region {
        match dispatch_result {
            Ok(()) => match queue_health_region_end(dispatch_accounts, spec) {
                Ok(()) => Ok(()),
                Err(err) => {
                    msg!(
                        "v2 execute seq={} health_region_end failed; returning error so dispatch state rolls back: {:?}",
                        candidate.sequence,
                        err
                    );
                    return Err(err);
                }
            },
            Err(err) => {
                msg!(
                    "v2 execute seq={} health-region dispatch failed; returning error so partial dispatch state rolls back: {:?}",
                    candidate.sequence,
                    err
                );
                return Err(err);
            }
        }
    } else {
        dispatch_result
    };

    match dispatch_result {
        Ok(()) => {
            queue_loader.load_mut()?.clear_item_at(candidate.sequence)?;
            emit!(QueueItemProcessed {
                group: group_key,
                market_index,
                sequence: candidate.sequence,
                kind: candidate.kind,
                status: QueueItemStatus::Executed as u8,
                failure_code: QueueFailureCode::None as u8,
            });
            Ok(ExecuteProgressV2::Continue)
        }
        Err(err) => {
            let (terminal, mut failure_code) = classify_dispatch_failure_v2(&err);
            let retries = queue_loader
                .load_mut()?
                .increment_retry(candidate.sequence, current_slot)?;
            if terminal || retries >= max_retries {
                if !terminal {
                    failure_code = QueueFailureCode::RetriesExhausted;
                }
                queue_loader.load_mut()?.clear_item_at(candidate.sequence)?;
                emit!(QueueItemProcessed {
                    group: group_key,
                    market_index,
                    sequence: candidate.sequence,
                    kind: candidate.kind,
                    status: QueueItemStatus::Failed as u8,
                    failure_code: failure_code as u8,
                });
                Ok(ExecuteProgressV2::Continue)
            } else {
                Ok(ExecuteProgressV2::Break)
            }
        }
    }
}

pub fn execution_queue_v2_init_authority_state(
    ctx: Context<ExecutionQueueV2InitAuthorityState>,
    ctm_signer: Pubkey,
) -> Result<()> {
    let bump = *ctx
        .bumps
        .get("authority_state")
        .ok_or_else(|| error!(FermiError::SomeError))?;
    ctx.accounts.authority_state.init(
        ctx.accounts.group.key(),
        ctx.accounts.admin.key(),
        ctm_signer,
        bump,
    );
    Ok(())
}

pub fn execution_queue_v2_set_ctm_pending(
    ctx: Context<ExecutionQueueV2AuthorityAdmin>,
    pending_ctm_signer: Pubkey,
    activate_at_slot: u64,
) -> Result<()> {
    ctx.accounts
        .authority_state
        .set_pending_ctm_signer(pending_ctm_signer, activate_at_slot);
    Ok(())
}

pub fn execution_queue_v2_close_authority_state(
    _ctx: Context<ExecutionQueueV2CloseAuthorityState>,
) -> Result<()> {
                                                                                         
    Ok(())
}

pub fn execution_queue_v2_create(
    _ctx: Context<ExecutionQueueV2Create>,
    _market_index: u16,
) -> Result<()> {
    Ok(())
}

pub fn execution_queue_v2_resize(
    ctx: Context<ExecutionQueueV2Resize>,
    _market_index: u16,
) -> Result<()> {
    let queue_ai = ctx.accounts.queue.to_account_info();
    let current_len = queue_ai.data_len();
    let target_len = EXECUTION_QUEUE_V2_ACCOUNT_SPACE;

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

pub fn execution_queue_v2_init(
    ctx: Context<ExecutionQueueV2Init>,
    market_index: u16,
) -> Result<()> {
    require!(
        ctx.accounts.queue.to_account_info().data_len() == EXECUTION_QUEUE_V2_ACCOUNT_SPACE,
        FermiError::ExecutionQueueV2LayoutNotReady
    );
    let bump = *ctx
        .bumps
        .get("queue")
        .ok_or_else(|| error!(FermiError::SomeError))?;
    let mut queue = ctx.accounts.queue.load_init()?;
    queue.init(
        ctx.accounts.group.key(),
        ctx.accounts.authority_state.key(),
        market_index,
        bump,
    );
    Ok(())
}

pub fn execution_queue_v2_configure(
    ctx: Context<ExecutionQueueV2Admin>,
    params: ExecutionQueueV2ConfigureParams,
) -> Result<()> {
    let mut queue = ctx.accounts.queue.load_mut()?;
    require_v2_layout(&queue, queue.header.market_index)?;
    if params.gap_wait_slots != 0 {
        queue.header.gap_wait_slots = params.gap_wait_slots;
    }
    queue.header.max_retries = if params.max_retries == 0 {
        EXECUTION_QUEUE_V2_DEFAULT_MAX_RETRIES
    } else {
        params.max_retries
    };
    queue.header.paused_ingress = u8::from(params.pause_ingress);
    queue.header.paused_execute = u8::from(params.pause_execute);
    Ok(())
}

pub fn execution_queue_v2_drop_head(ctx: Context<ExecutionQueueV2Admin>, count: u16) -> Result<()> {
    let mut queue = ctx.accounts.queue.load_mut()?;
    let market_index = queue.header.market_index;
    require_v2_layout(&queue, market_index)?;
    require!(
        queue.header.paused_execute != 0,
        FermiError::ExecutionQueueAdminActionRequiresPause
    );

    for _ in 0..count {
        let next_sequence = queue.header.next_sequence_to_execute;
        if queue.header.max_seen_sequence < next_sequence {
            break;
        }
        if let Some(item) = queue.get_item(next_sequence).copied() {
            queue.clear_item_at(next_sequence)?;
            emit!(QueueItemProcessed {
                group: ctx.accounts.group.key(),
                market_index,
                sequence: item.sequence,
                kind: item.kind,
                status: QueueItemStatus::Failed as u8,
                failure_code: QueueFailureCode::None as u8,
            });
        } else {
            let phys = ExecutionQueueV2::physical_index(next_sequence);
            queue.items[phys] = QueueItem::default();
            queue.header.next_sequence_to_execute = next_sequence.saturating_add(1);
            queue.header.gap_observed_slot = 0;
            emit!(QueueItemProcessed {
                group: ctx.accounts.group.key(),
                market_index,
                sequence: next_sequence,
                kind: QueueItemKind::CtmWrapped as u8,
                status: QueueItemStatus::Skipped as u8,
                failure_code: QueueFailureCode::GapSkipped as u8,
            });
        }
    }
    Ok(())
}

pub fn execution_queue_v2_close(ctx: Context<ExecutionQueueV2Close>) -> Result<()> {
    let queue = ctx.accounts.queue.load()?;
    let market_index = queue.header.market_index;
    require_v2_layout(&queue, market_index)?;
    require!(
        queue.header.paused_ingress != 0 && queue.header.paused_execute != 0,
        FermiError::ExecutionQueueAdminActionRequiresPause
    );
    require!(
        queue.header.live_count == 0
            && queue.header.max_seen_sequence < queue.header.next_sequence_to_execute,
        FermiError::ExecutionQueueCloseRequiresEmpty
    );
    Ok(())
}

pub fn execution_queue_v2_enqueue_ctm(
    ctx: Context<ExecutionQueueV2EnqueueCtm>,
    market_index: u16,
    envelope: CtmEnvelope,
    payload: Vec<u8>,
) -> Result<()> {
    require!(
        envelope.kind == QueueItemKind::CtmWrapped as u8,
        FermiError::ExecutionQueueInvalidItemKind
    );
    require!(
        payload.len() <= EXECUTION_QUEUE_PAYLOAD_MAX,
        FermiError::ExecutionQueuePayloadTooLarge
    );
    require!(
        !ctx.remaining_accounts.is_empty(),
        FermiError::ExecutionQueueDispatchAccountLayoutInvalid
    );

    let (dispatch_accounts, _) =
        split_dispatch_accounts(ctx.remaining_accounts, ctx.accounts.queue.key())?;
    require_dispatch_market_index(dispatch_accounts, market_index)?;

    let clock = Clock::get()?;
    let now_ts: u64 = clock.unix_timestamp.try_into().unwrap_or(0);
    ctx.accounts
        .authority_state
        .maybe_activate_pending_ctm(clock.slot);

    {
        let queue = ctx.accounts.queue.load()?;
        require_v2_layout(&queue, market_index)?;
        require!(
            queue.header.paused_ingress == 0,
            FermiError::ExecutionQueueIngressPaused
        );
        queue.validate_sequence(envelope.sequence)?;
    }

    let payload_hash = hashv(&[&payload]).to_bytes();
    require!(
        payload_hash == envelope.payload_hash,
        FermiError::ExecutionQueuePayloadHashMismatch
    );

    let decoded_payload = decode_queue_payload(&payload)?;
    require!(
        decoded_payload.flags == 0,
        FermiError::ExecutionQueuePayloadDecodeFailed
    );
    if let Some(reason) = prevalidate_terminal_ctm_payload(&decoded_payload, now_ts) {
        msg!(
            "{} market={} reason={:?}",
            terminal_ctm_failure_msg(reason),
            market_index,
            reason
        );
        return match reason {
            TerminalCtmFailureReason::Expired => err!(FermiError::ExecutionQueueEnvelopeExpired),
            TerminalCtmFailureReason::InvalidNumericInput => err!(FermiError::SomeError),
        };
    }

    require!(
        queue_item_kind_for_payload_variant(decoded_payload.variant)
            == QueueItemKind::CtmWrapped as u8,
        FermiError::ExecutionQueuePayloadKindMismatch
    );
    validate_queue_payload_dispatch_accounts(
        ctx.accounts.group.key(),
        &decoded_payload,
        dispatch_accounts,
    )?;
    if variant_uses_queue_owner_signer(decoded_payload.variant) {
        require!(
            dispatch_accounts.len() >= 3,
            FermiError::ExecutionQueueDispatchAccountLayoutInvalid
        );
        require!(
            *dispatch_accounts[2].key == ctx.accounts.queue.key(),
            FermiError::ExecutionQueueOwnerMustBeQueueAuthority
        );
    }

    let accounts_hash = hash_accounts(&account_metas_from_infos(dispatch_accounts));
    require!(
        accounts_hash == envelope.accounts_hash,
        FermiError::ExecutionQueueAccountsHashMismatch
    );
    require!(
        envelope.expires_at_slot == 0 || clock.slot <= envelope.expires_at_slot,
        FermiError::ExecutionQueueEnvelopeExpired
    );

    let envelope_hash = canonical_envelope_message(ctx.accounts.group.key(), &envelope);
    verify_ed25519_preinstruction(
        ctx.accounts.instructions.as_ref(),
        ctx.accounts.authority_state.ctm_signer,
        envelope_hash.as_ref(),
    )?;

    if variant_uses_user_signature(decoded_payload.variant) {
        let (fermi_account, user_owner) =
            extract_user_owner_for_ctm_payload(ctx.accounts.group.key(), dispatch_accounts)?;
        let user_hash_v2 = canonical_user_intent_message_v2(
            ctx.accounts.group.key(),
            fermi_account,
            user_owner,
            envelope.kind,
            UserIntentTargetKind::PerpMarket,
            market_index,
            &envelope.payload_hash,
        );
        let user_hash_v1 = canonical_user_intent_message_v1(
            ctx.accounts.group.key(),
            fermi_account,
            user_owner,
            &envelope,
        );
        verify_user_ed25519_preinstruction(
            ctx.accounts.instructions.as_ref(),
            user_owner,
            &[user_hash_v2, user_hash_v1],
        )?;
    }

    let mut item = QueueItem::default();
    item.sequence = envelope.sequence;
    item.min_execute_slot = envelope.min_execute_slot;
    item.ingress_slot = clock.slot;
    item.kind = envelope.kind;
    item.status = QueueItemStatus::Pending as u8;
    item.payload_len = payload.len() as u16;
    item.payload_hash = envelope.payload_hash;
    item.accounts_hash = envelope.accounts_hash;
    item.payload[..payload.len()].copy_from_slice(&payload);

    ctx.accounts.queue.load_mut()?.push(item)?;

    emit!(QueueItemEnqueued {
        group: ctx.accounts.group.key(),
        market_index,
        sequence: envelope.sequence,
        kind: envelope.kind,
        min_execute_slot: envelope.min_execute_slot,
    });
    Ok(())
}

pub fn execution_queue_v2_execute<'info>(
    ctx: Context<'_, '_, '_, 'info, ExecutionQueueV2Execute<'info>>,
    market_index: u16,
    max_items: u16,
) -> Result<()> {
    require!(
        !ctx.remaining_accounts.is_empty(),
        FermiError::ExecutionQueueDispatchAccountLayoutInvalid
    );

    let (dispatch_accounts, invoke_accounts) =
        split_dispatch_accounts(ctx.remaining_accounts, ctx.accounts.queue.key())?;
    require_dispatch_market_index(dispatch_accounts, market_index)?;

    let clock = Clock::get()?;
    let now_ts: u64 = clock.unix_timestamp.try_into().unwrap_or(0);
    let group_key = ctx.accounts.group.key();
    let queue_key = ctx.accounts.queue.key();
    ctx.accounts
        .authority_state
        .maybe_activate_pending_ctm(clock.slot);

    let (queue_bump, max_retries) = {
        let queue = ctx.accounts.queue.load()?;
        require_v2_layout(&queue, market_index)?;
        require!(
            queue.header.paused_execute == 0,
            FermiError::ExecutionQueueExecutePaused
        );
        (queue.header.bump, queue.header.max_retries_effective())
    };

    let provided_accounts_hash = hash_accounts(&account_metas_from_infos(dispatch_accounts));
    for _ in 0..max_items {
        let selection = {
            let mut queue = ctx.accounts.queue.load_mut()?;
            select_head_candidate_v2(&mut queue, &clock, group_key, market_index)
        };
        let candidate = match selection {
            HeadSelectionV2::Candidate(candidate) => candidate,
            HeadSelectionV2::Blocked | HeadSelectionV2::Empty => break,
            HeadSelectionV2::AdvancedGap => continue,
        };

        if candidate.accounts_hash != [0; 32] && candidate.accounts_hash != provided_accounts_hash {
            break;
        }

        match process_candidate_v2(
            &ctx.accounts.queue,
            candidate,
            dispatch_accounts,
            invoke_accounts,
            group_key,
            queue_key,
            queue_bump,
            market_index,
            clock.slot,
            now_ts,
            max_retries,
        )? {
            ExecuteProgressV2::Continue => continue,
            ExecuteProgressV2::Break => break,
        }
    }

    Ok(())
}

pub fn execution_queue_v2_execute_multi<'info>(
    ctx: Context<'_, '_, '_, 'info, ExecutionQueueV2Execute<'info>>,
    market_index: u16,
    max_items: u16,
    lane_count: u8,
    accounts_per_lane: u16,
    lane_hashes: Vec<[u8; 32]>,
) -> Result<()> {
    let lane_count = lane_count as usize;
    let accounts_per_lane = accounts_per_lane as usize;
    require!(
        lane_count >= 1 && lane_count <= 20,
        FermiError::ExecutionQueueDispatchAccountLayoutInvalid
    );
    require!(
        ctx.remaining_accounts.len() >= lane_count * accounts_per_lane,
        FermiError::ExecutionQueueDispatchAccountLayoutInvalid
    );

    let lane_slices: Vec<&[AccountInfo]> = (0..lane_count)
        .map(|i| {
            let start = i * accounts_per_lane;
            let end = start + accounts_per_lane;
            &ctx.remaining_accounts[start..end]
        })
        .collect();

    require!(
        lane_hashes.len() == lane_count,
        FermiError::ExecutionQueueDispatchAccountLayoutInvalid
    );
    require_dispatch_market_index(lane_slices[0], market_index)?;
    for lane in lane_slices.iter().skip(1) {
        require_dispatch_market_index(lane, market_index)?;
    }

    let precomputed_lane_hashes: Vec<[u8; 32]> = lane_slices
        .iter()
        .map(|lane| hash_accounts(&account_metas_from_infos(lane)))
        .collect();
    for (i, declared_hash) in lane_hashes.iter().enumerate() {
        require!(
            *declared_hash == precomputed_lane_hashes[i],
            FermiError::ExecutionQueueAccountsHashMismatch
        );
    }

    let clock = Clock::get()?;
    let now_ts: u64 = clock.unix_timestamp.try_into().unwrap_or(0);
    let group_key = ctx.accounts.group.key();
    let queue_key = ctx.accounts.queue.key();
    ctx.accounts
        .authority_state
        .maybe_activate_pending_ctm(clock.slot);

    let (queue_bump, max_retries) = {
        let queue = ctx.accounts.queue.load()?;
        require_v2_layout(&queue, market_index)?;
        require!(
            queue.header.paused_execute == 0,
            FermiError::ExecutionQueueExecutePaused
        );
        (queue.header.bump, queue.header.max_retries_effective())
    };

    for _ in 0..max_items {
        let selection = {
            let mut queue = ctx.accounts.queue.load_mut()?;
            select_head_candidate_v2(&mut queue, &clock, group_key, market_index)
        };
        let candidate = match selection {
            HeadSelectionV2::Candidate(candidate) => candidate,
            HeadSelectionV2::Blocked | HeadSelectionV2::Empty => break,
            HeadSelectionV2::AdvancedGap => continue,
        };

        let lane_idx = if candidate.accounts_hash == [0; 32] {
            Some(0)
        } else {
            precomputed_lane_hashes
                .iter()
                .position(|hash| *hash == candidate.accounts_hash)
        };
        let Some(lane_idx) = lane_idx else {
            break;
        };
        let dispatch_accounts = lane_slices[lane_idx];

        match process_candidate_v2(
            &ctx.accounts.queue,
            candidate,
            dispatch_accounts,
            dispatch_accounts,
            group_key,
            queue_key,
            queue_bump,
            market_index,
            clock.slot,
            now_ts,
            max_retries,
        )? {
            ExecuteProgressV2::Continue => continue,
            ExecuteProgressV2::Break => break,
        }
    }

    Ok(())
}
