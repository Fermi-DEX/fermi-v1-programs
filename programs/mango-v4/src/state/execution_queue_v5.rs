// ── Execution queue v5 ─────────────────────────────────────────────────────
//
// Single-account, fixed-size ring buffer design. Inspired by the v2 sub-queue
// layout but stores only commit hashes (not payloads) so per-item cost is
// ~4× smaller, and the whole queue for a group fits in one Solana account.
//
// Layout:
//   Header           32+32+8+72 = 144 bytes
//   SubQueueHeader × 16            1,024 bytes
//   CommitItemV5   × 4096        393,216 bytes
//   Total                       ~394,384 bytes (+8 disc)
//
// Sub-queue indexing:
//   Each market_index claims one of the 16 SubQueueHeader slots on first
//   configure. Its 256-item ring lives at items[sub_queue_idx * 256 ..
//   sub_queue_idx * 256 + 256]. The physical slot for a sequence is
//   `sub_queue_idx * 256 + (sequence % 256)`. Head/tail are tracked in the
//   SubQueueHeader as `next_sequence_to_execute` and `max_seen_sequence`.
//
// Why this design:
// - No pagination, no abs_page_no, no page rotation. The class of
//   "orphan page" stalls that plagued v4 is structurally impossible here.
// - `drop_head_market` is a one-branch loop: clear item at head, advance,
//   decrement live_count. Always makes progress.
// - Same commit-reveal semantics as v4 (hash at commit, verify+execute
//   at reveal). The executor pipeline becomes significantly simpler.

use crate::error::FermiError;
use anchor_lang::prelude::*;
use static_assertions::const_assert_eq;
use std::mem::size_of;

pub const EXECUTION_QUEUE_V5_LAYOUT_VERSION: u8 = 7;

/// Default per-commit retry budget used when `ExecutionQueueV5Header.max_retries`
/// is 0 (fresh init, or a queue that pre-dates the layout-v6 field). Effective
/// value is also what `execution_queue_v5_set_max_retries` resets to when
/// called with 0.
pub const EXECUTION_QUEUE_V5_DEFAULT_MAX_RETRIES: u8 = 3;

/// Upper bound on admin-configured max_retries. Bounds the worst-case CU
/// wasted on a genuinely-undispatchable head item before autodrop takes over.
pub const EXECUTION_QUEUE_V5_MAX_RETRIES_CAP: u8 = 20;
/// One queue account per (group, market_index) PDA — each account holds a
/// single sub-queue. Prior layout packed all markets into one account, which
/// forced Solana to serialize every reveal across markets on the same
/// account lock. Splitting to per-market PDAs unlocks true on-chain
/// parallelism. N=1 reflects that each queue account is single-market.
pub const EXECUTION_QUEUE_V5_N_MAX_MARKETS: usize = 1;
/// 1024 ring-buffer slots per market — 4× the prior 256 so the relayer's
/// admission window is large enough to absorb bursts without tripping
/// `ExecutionQueueFull` (6076) during reveal stalls / autodrop windows.
/// 1024 × 96B item + 64B header ≈ 99KB, well under the account-size limit.
pub const EXECUTION_QUEUE_V5_PER_MARKET_CAPACITY: usize = 1024;
pub const EXECUTION_QUEUE_V5_TOTAL_CAPACITY: usize =
    EXECUTION_QUEUE_V5_N_MAX_MARKETS * EXECUTION_QUEUE_V5_PER_MARKET_CAPACITY;

/// Rolling on-chain replay cache for user-signed v5 intents. This is separate
/// from the live queue ring so replay protection survives normal head clears.
/// Mainnet r6 keeps this intentionally small to reduce upfront rent while the
/// relayer is trusted to avoid accidental duplicate admissions.
pub const EXECUTION_QUEUE_V5_REPLAY_CACHE_CAPACITY: usize = 512;

/// Maximum slots between admission and the earliest legal execution slot. Bounds
/// `min_execute_slot - clock.slot` on both CTM commits and direct enqueues. A
/// far-future `min_execute_slot` would otherwise pin the head behind a slot the
/// autodrop watchdog deliberately refuses to reclaim, halting the per-market
/// FIFO until manual admin recovery. Wall-clock duration depends on cluster slot
/// time; 2400 slots is roughly 16 minutes at 400ms/slot.
pub const EXECUTION_QUEUE_V5_MAX_DELAY_SLOTS: u64 = 2400;

/// Maximum slots between admission and a non-zero envelope expiry. Production
/// FIFO traffic may still use `expires_at_slot=0`. This bounds the admission
/// envelope only; replay-cache retention is intentionally decoupled below.
/// ~36 minutes at 400ms/slot.
pub const EXECUTION_QUEUE_V5_MAX_TTL_SLOTS: u64 = 5400;

/// Rolling duplicate guard for exact signed user intents: a hash consumed within
/// this many slots is rejected, older entries are ignored and eventually
/// overwritten. Entries never block admission, so the window must stay bounded.
///
/// Direct-pool admission requires a non-zero `expires_at_slot` no later than
/// `now + EXECUTION_QUEUE_V5_MAX_TTL_SLOTS` (see
/// `validate_direct_envelope_timing`). A direct intent re-submitted within this
/// window hits the cache; one after its expiry is rejected. An expiry beyond
/// this window leaves the gap between the two open to re-submission, so the
/// recommended off-chain policy is to sign
/// `expires_at_slot = current_slot + RETENTION` (the TS client's default); the
/// program does not enforce it. The window should also exceed the direct speed
/// bump (default 10) by enough slots to absorb transaction landing delay: 20
/// tolerates a 10-slot (~4 s) delay between signing and landing.
/// Capacity check: 512 entries over 20 slots leaves room for ~25 admissions per
/// slot per market before a live entry could be overwritten early.
///
/// For CTM intents this is only a short duplicate guard; the relayer is trusted
/// for broader replay discipline.
pub const EXECUTION_QUEUE_V5_REPLAY_RETENTION_SLOTS: u64 = 20;

pub const EXECUTION_QUEUE_V5_DEFAULT_GAP_WAIT_SLOTS: u16 = 4;
pub const EXECUTION_QUEUE_V5_DEFAULT_SOFT_LIMIT: u16 = 0; // 0 = full capacity

// Batch caps. Kept similar to v4 so the relayer's batching logic does not
// need rethinking. Real tx-size limits will usually bite first.
pub const EXECUTION_QUEUE_V5_MAX_COMMIT_BATCH: usize = 64;
pub const EXECUTION_QUEUE_V5_MAX_REVEAL_BATCH: usize = 32;

pub const EXECUTION_QUEUE_V5_ACCOUNT_SPACE: usize = 8 + size_of::<ExecutionQueueV5>();

#[repr(u8)]
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum CommitStatusV5 {
    Empty = 0,
    Committed = 1,
    Revealed = 2,
    Failed = 3,
}

/// Per-entry commit record. Holds the small commit hash plus the timing
/// envelope. Payloads are recomputed off-chain and verified at reveal time
/// against `commit_hash`.
#[zero_copy]
#[derive(Debug)]
pub struct CommitItemV5 {
    pub sequence: u64,
    pub min_execute_slot: u64,
    pub expires_at_slot: u64,
    pub ingress_slot: u64,
    /// First slot at which a reveal for this item failed. Zero until the
    /// first failure. Used for audit / debugging only — retry policy lives
    /// in the executor.
    pub first_failure_slot: u64,
    pub commit_hash: [u8; 32],
    pub status: u8,
    pub retries: u8,
    pub flags: u8,
    pub _padding0: [u8; 5],
    pub reserved: [u8; 16],
}
const_assert_eq!(size_of::<CommitItemV5>(), 96);
const_assert_eq!(size_of::<CommitItemV5>() % 8, 0);

impl Default for CommitItemV5 {
    fn default() -> Self {
        Self {
            sequence: 0,
            min_execute_slot: 0,
            expires_at_slot: 0,
            ingress_slot: 0,
            first_failure_slot: 0,
            commit_hash: [0; 32],
            status: CommitStatusV5::Empty as u8,
            retries: 0,
            flags: 0,
            _padding0: [0; 5],
            reserved: [0; 16],
        }
    }
}

/// Durable marker that an exact signed user intent has already been consumed.
/// This is keyed by the full commit hash, not by client_order_id alone:
/// legacy clients may use client_order_id=0 for many live intents, and queue
/// execution latency must not serialize or reject distinct orders.
#[zero_copy]
#[derive(Debug)]
pub struct ReplayEntryV5 {
    pub intent_hash: [u8; 32],
    pub consumed_slot: u64,
}
const_assert_eq!(size_of::<ReplayEntryV5>(), 40);
const_assert_eq!(size_of::<ReplayEntryV5>() % 8, 0);

impl Default for ReplayEntryV5 {
    fn default() -> Self {
        Self {
            intent_hash: [0; 32],
            consumed_slot: 0,
        }
    }
}

/// Per-market state. 16 of these live in the queue. First-use allocation:
/// the first commit for a given `market_index` claims an inactive slot and
/// binds it for the lifetime of the queue.
#[zero_copy]
#[derive(Debug)]
pub struct SubQueueHeaderV5 {
    pub market_index: u16,
    pub active: u8,
    pub paused_ingress: u8,
    pub paused_execute: u8,
    pub shard_id: u8,
    pub _padding0: [u8; 2],
    pub live_count: u32,
    pub gap_wait_slots: u16,
    pub soft_limit: u16,
    pub next_sequence_to_execute: u64,
    pub max_seen_sequence: u64,
    pub gap_observed_slot: u64,
    pub first_failure_slot: u64,
    pub reserved: [u8; 16],
}
const_assert_eq!(size_of::<SubQueueHeaderV5>(), 64);
const_assert_eq!(size_of::<SubQueueHeaderV5>() % 8, 0);

impl SubQueueHeaderV5 {
    pub fn init(&mut self, market_index: u16, shard_id: u8, soft_limit: u16, gap_wait_slots: u16) {
        self.market_index = market_index;
        self.active = 1;
        self.paused_ingress = 0;
        self.paused_execute = 0;
        self.shard_id = shard_id;
        self._padding0 = [0; 2];
        self.live_count = 0;
        self.gap_wait_slots = gap_wait_slots;
        self.soft_limit = soft_limit;
        self.next_sequence_to_execute = 0;
        self.max_seen_sequence = 0;
        self.gap_observed_slot = 0;
        self.first_failure_slot = 0;
        self.reserved = [0; 16];
    }

    pub fn capacity(&self) -> u32 {
        EXECUTION_QUEUE_V5_PER_MARKET_CAPACITY as u32
    }

    pub fn admission_limit(&self) -> u64 {
        let cap = self.capacity() as u64;
        if self.soft_limit == 0 {
            cap
        } else {
            cap.min(self.soft_limit as u64)
        }
    }

    pub fn next_enqueue_sequence(&self) -> u64 {
        if self.live_count == 0 {
            self.next_sequence_to_execute
        } else {
            self.max_seen_sequence.saturating_add(1)
        }
    }

    /// The head slot's position inside the per-market window (0..256).
    pub fn head_ring_offset(&self) -> u16 {
        (self.next_sequence_to_execute % EXECUTION_QUEUE_V5_PER_MARKET_CAPACITY as u64) as u16
    }

    /// The ring-window offset for a given sequence.
    pub fn ring_offset_for_sequence(sequence: u64) -> u16 {
        (sequence % EXECUTION_QUEUE_V5_PER_MARKET_CAPACITY as u64) as u16
    }

    pub fn validate_commit_sequence(&self, sequence: u64) -> Result<()> {
        require!(
            sequence >= self.next_sequence_to_execute,
            FermiError::InvalidSequenceNumber
        );
        require!(
            sequence
                < self
                    .next_sequence_to_execute
                    .saturating_add(self.admission_limit()),
            FermiError::ExecutionQueueFull
        );
        Ok(())
    }

    pub fn note_commit(&mut self, sequence: u64) {
        self.live_count = self.live_count.saturating_add(1);
        if self.live_count == 1 || sequence > self.max_seen_sequence {
            self.max_seen_sequence = sequence;
        }
    }

    pub fn note_head_advanced(&mut self) {
        self.live_count = self.live_count.saturating_sub(1);
        self.next_sequence_to_execute = self.next_sequence_to_execute.saturating_add(1);
    }
}

/// Queue-wide header. Shared metadata, group binding, admin pause flags.
#[zero_copy]
#[derive(Debug)]
pub struct ExecutionQueueV5Header {
    pub group: Pubkey,
    pub authority_state: Pubkey,
    pub bump: u8,
    pub paused_ingress: u8,
    pub paused_execute: u8,
    pub layout_version: u8,
    /// Per-commit retry budget inside `reveal_execute_market`. 0 means
    /// "use default" so queues created under layout_version < 6 keep working
    /// without migration. Configure via `execution_queue_v5_set_max_retries`.
    pub max_retries: u8,
    pub _padding0: [u8; 3],
    pub total_count: u64,
    pub replay_write_cursor: u32,
    pub replay_cache_len: u32,
    pub reserved: [u8; 56],
}
const_assert_eq!(size_of::<ExecutionQueueV5Header>() % 8, 0);

impl ExecutionQueueV5Header {
    /// Retry budget actually used by the reveal handler. Falls back to the
    /// default when the admin hasn't set a value (or the queue pre-dates
    /// the max_retries field).
    pub fn max_retries_effective(&self) -> u8 {
        if self.max_retries == 0 {
            EXECUTION_QUEUE_V5_DEFAULT_MAX_RETRIES
        } else {
            self.max_retries
        }
    }
}

#[account(zero_copy)]
#[derive(Debug)]
pub struct ExecutionQueueV5 {
    pub header: ExecutionQueueV5Header,
    pub sub_queue_headers: [SubQueueHeaderV5; EXECUTION_QUEUE_V5_N_MAX_MARKETS],
    pub items: [CommitItemV5; EXECUTION_QUEUE_V5_TOTAL_CAPACITY],
    pub replay_cache: [ReplayEntryV5; EXECUTION_QUEUE_V5_REPLAY_CACHE_CAPACITY],
}
const_assert_eq!(size_of::<ExecutionQueueV5>() % 8, 0);

impl ExecutionQueueV5 {
    pub fn init(&mut self, group: Pubkey, authority_state: Pubkey, bump: u8) {
        self.header.group = group;
        self.header.authority_state = authority_state;
        self.header.bump = bump;
        self.header.paused_ingress = 0;
        self.header.paused_execute = 0;
        self.header.layout_version = EXECUTION_QUEUE_V5_LAYOUT_VERSION;
        self.header.max_retries = EXECUTION_QUEUE_V5_DEFAULT_MAX_RETRIES;
        self.header._padding0 = [0; 3];
        self.header.total_count = 0;
        self.header.replay_write_cursor = 0;
        self.header.replay_cache_len = 0;
        self.header.reserved = [0; 56];
        for sqh in self.sub_queue_headers.iter_mut() {
            *sqh = SubQueueHeaderV5 {
                market_index: 0,
                active: 0,
                paused_ingress: 0,
                paused_execute: 0,
                shard_id: 0,
                _padding0: [0; 2],
                live_count: 0,
                gap_wait_slots: EXECUTION_QUEUE_V5_DEFAULT_GAP_WAIT_SLOTS,
                soft_limit: EXECUTION_QUEUE_V5_DEFAULT_SOFT_LIMIT,
                next_sequence_to_execute: 0,
                max_seen_sequence: 0,
                gap_observed_slot: 0,
                first_failure_slot: 0,
                reserved: [0; 16],
            };
        }
        for item in self.items.iter_mut() {
            *item = CommitItemV5::default();
        }
        for entry in self.replay_cache.iter_mut() {
            *entry = ReplayEntryV5::default();
        }
    }

    /// Returns the sub-queue index for `market_index`, or None if not yet
    /// configured.
    pub fn find_sub_queue(&self, market_index: u16) -> Option<usize> {
        self.sub_queue_headers
            .iter()
            .position(|h| h.active == 1 && h.market_index == market_index)
    }

    /// Allocates a new sub-queue slot for `market_index`. Admin-only caller
    /// is enforced at the instruction layer. Fails if no free slot or if
    /// `market_index` is already configured.
    pub fn allocate_sub_queue(
        &mut self,
        market_index: u16,
        shard_id: u8,
        soft_limit: u16,
        gap_wait_slots: u16,
    ) -> Result<usize> {
        require!(
            self.find_sub_queue(market_index).is_none(),
            FermiError::ExecutionQueueSubQueueMarketIndexMismatch
        );
        let idx = self
            .sub_queue_headers
            .iter()
            .position(|h| h.active == 0)
            .ok_or_else(|| error!(FermiError::ExecutionQueueFull))?;
        self.sub_queue_headers[idx].init(market_index, shard_id, soft_limit, gap_wait_slots);
        Ok(idx)
    }

    /// Physical index into `items` for a given sub-queue and ring offset.
    pub fn physical_index(sub_queue_idx: usize, ring_offset: u16) -> usize {
        sub_queue_idx * EXECUTION_QUEUE_V5_PER_MARKET_CAPACITY + ring_offset as usize
    }

    /// Writes a commit item at the slot derived from `item.sequence`.
    pub fn write_commit(&mut self, sub_queue_idx: usize, item: CommitItemV5) -> Result<()> {
        self.sub_queue_headers[sub_queue_idx].validate_commit_sequence(item.sequence)?;
        let offset = SubQueueHeaderV5::ring_offset_for_sequence(item.sequence);
        let phys = Self::physical_index(sub_queue_idx, offset);
        let existing = &self.items[phys];
        require!(
            existing.status != CommitStatusV5::Committed as u8,
            FermiError::ExecutionQueueFull
        );
        self.items[phys] = item;
        self.sub_queue_headers[sub_queue_idx].note_commit(self.items[phys].sequence);
        self.header.total_count = self.header.total_count.saturating_add(1);
        Ok(())
    }

    /// Clears the head of a sub-queue (used by reveal success, admin drop,
    /// and expiry pruning) and advances `next_sequence_to_execute`. Returns
    /// the sequence that was cleared, or an error if the sub-queue is empty.
    pub fn clear_head(&mut self, sub_queue_idx: usize) -> Result<u64> {
        let sqh = &mut self.sub_queue_headers[sub_queue_idx];
        require!(sqh.live_count > 0, FermiError::ExecutionQueueEmpty);
        let seq = sqh.next_sequence_to_execute;
        let offset = sqh.head_ring_offset();
        let phys = Self::physical_index(sub_queue_idx, offset);
        let item = &self.items[phys];
        require!(
            item.sequence == seq && item.status != CommitStatusV5::Empty as u8,
            FermiError::ExecutionQueueSequenceNotPending
        );
        self.items[phys] = CommitItemV5::default();
        self.sub_queue_headers[sub_queue_idx].note_head_advanced();
        self.header.total_count = self.header.total_count.saturating_sub(1);
        Ok(seq)
    }

    /// Read-only lookup of the item at a given sequence within a sub-queue.
    /// Returns None if the slot is Empty or holds a stale entry (sequence
    /// mismatch due to wrap-around — should not happen with correct caller
    /// usage, but defended against).
    pub fn get_item(&self, sub_queue_idx: usize, sequence: u64) -> Option<&CommitItemV5> {
        let offset = SubQueueHeaderV5::ring_offset_for_sequence(sequence);
        let phys = Self::physical_index(sub_queue_idx, offset);
        let item = &self.items[phys];
        if item.status == CommitStatusV5::Empty as u8 || item.sequence != sequence {
            None
        } else {
            Some(item)
        }
    }

    pub fn get_head_item(&self, sub_queue_idx: usize) -> Option<&CommitItemV5> {
        let sqh = &self.sub_queue_headers[sub_queue_idx];
        if sqh.live_count == 0 {
            return None;
        }
        self.get_item(sub_queue_idx, sqh.next_sequence_to_execute)
    }

    pub fn increment_retry(
        &mut self,
        sub_queue_idx: usize,
        sequence: u64,
        current_slot: u64,
    ) -> Result<u8> {
        let offset = SubQueueHeaderV5::ring_offset_for_sequence(sequence);
        let phys = Self::physical_index(sub_queue_idx, offset);
        let item = &mut self.items[phys];
        require!(
            item.sequence == sequence && item.status == CommitStatusV5::Committed as u8,
            FermiError::ExecutionQueueSubQueueMarketIndexMismatch
        );
        if item.first_failure_slot == 0 {
            item.first_failure_slot = current_slot;
        }
        item.retries = item.retries.saturating_add(1);
        Ok(item.retries)
    }

    pub fn has_recent_consumed_intent_hash(
        &self,
        intent_hash: &[u8; 32],
        current_slot: u64,
    ) -> bool {
        let len =
            (self.header.replay_cache_len as usize).min(EXECUTION_QUEUE_V5_REPLAY_CACHE_CAPACITY);
        // Compare the cheap slot age first: most entries are older than the
        // retention window, so their hashes never need comparing.
        for entry in self.replay_cache.iter().take(len) {
            if current_slot.saturating_sub(entry.consumed_slot)
                <= EXECUTION_QUEUE_V5_REPLAY_RETENTION_SLOTS
                && entry.intent_hash == *intent_hash
            {
                return true;
            }
        }
        false
    }

    /// Records `intent_hash` unless it was consumed within the retention
    /// window. Returns false, recording nothing, for such a duplicate. One
    /// cache scan, where a check followed by `record_consumed_intent_hash`
    /// takes two.
    pub fn try_record_consumed_intent_hash(
        &mut self,
        intent_hash: [u8; 32],
        consumed_slot: u64,
    ) -> bool {
        if self.has_recent_consumed_intent_hash(&intent_hash, consumed_slot) {
            return false;
        }
        self.push_consumed_intent_hash(intent_hash, consumed_slot);
        true
    }

    /// Records `intent_hash` without the duplicate check. Only for callers
    /// that already checked this hash and have not since released the queue
    /// to other intents.
    pub fn push_consumed_intent_hash(&mut self, intent_hash: [u8; 32], consumed_slot: u64) {
        let cap = EXECUTION_QUEUE_V5_REPLAY_CACHE_CAPACITY;
        let cursor = (self.header.replay_write_cursor as usize) % cap;
        // Always overwrite the cursor. Replay protection is a short duplicate
        // guard, not a liveness blocker.
        self.replay_cache[cursor] = ReplayEntryV5 {
            intent_hash,
            consumed_slot,
        };
        self.header.replay_write_cursor = ((cursor + 1) % cap) as u32;
        if (self.header.replay_cache_len as usize) < cap {
            self.header.replay_cache_len = self.header.replay_cache_len.saturating_add(1);
        }
    }

    pub fn record_consumed_intent_hash(
        &mut self,
        intent_hash: [u8; 32],
        consumed_slot: u64,
    ) -> Result<()> {
        require!(
            self.try_record_consumed_intent_hash(intent_hash, consumed_slot),
            FermiError::ExecutionQueueV5IntentReplay
        );
        Ok(())
    }
}

// ── Layout offsets (consumed by off-chain raw parsers in the engine) ──
//
// All offsets include the 8-byte Anchor discriminator.
const DISC: usize = 8;
pub const EXECUTION_QUEUE_V5_HEADER_OFFSET: usize = DISC;
pub const EXECUTION_QUEUE_V5_HEADER_SIZE: usize = size_of::<ExecutionQueueV5Header>();
pub const EXECUTION_QUEUE_V5_SUB_QUEUE_HEADERS_OFFSET: usize =
    EXECUTION_QUEUE_V5_HEADER_OFFSET + EXECUTION_QUEUE_V5_HEADER_SIZE;
pub const EXECUTION_QUEUE_V5_SUB_QUEUE_HEADER_STRIDE: usize = size_of::<SubQueueHeaderV5>();
pub const EXECUTION_QUEUE_V5_ITEMS_OFFSET: usize = EXECUTION_QUEUE_V5_SUB_QUEUE_HEADERS_OFFSET
    + EXECUTION_QUEUE_V5_N_MAX_MARKETS * EXECUTION_QUEUE_V5_SUB_QUEUE_HEADER_STRIDE;
pub const EXECUTION_QUEUE_V5_ITEM_STRIDE: usize = size_of::<CommitItemV5>();
pub const EXECUTION_QUEUE_V5_REPLAY_CACHE_OFFSET: usize = EXECUTION_QUEUE_V5_ITEMS_OFFSET
    + EXECUTION_QUEUE_V5_TOTAL_CAPACITY * EXECUTION_QUEUE_V5_ITEM_STRIDE;
pub const EXECUTION_QUEUE_V5_REPLAY_ENTRY_STRIDE: usize = size_of::<ReplayEntryV5>();

// SubQueueHeader internal offsets (relative to one header).
pub const SQH_V5_MARKET_INDEX_OFFSET: usize = 0;
pub const SQH_V5_ACTIVE_OFFSET: usize = 2;
pub const SQH_V5_LIVE_COUNT_OFFSET: usize = 8;
pub const SQH_V5_NEXT_SEQ_OFFSET: usize = 16;
pub const SQH_V5_MAX_SEEN_SEQ_OFFSET: usize = 24;

// CommitItem internal offsets (relative to one item).
pub const CI_V5_SEQUENCE_OFFSET: usize = 0;
pub const CI_V5_MIN_EXECUTE_SLOT_OFFSET: usize = 8;
pub const CI_V5_EXPIRES_AT_SLOT_OFFSET: usize = 16;
pub const CI_V5_INGRESS_SLOT_OFFSET: usize = 24;
pub const CI_V5_FIRST_FAILURE_SLOT_OFFSET: usize = 32;
pub const CI_V5_COMMIT_HASH_OFFSET: usize = 40;
pub const CI_V5_STATUS_OFFSET: usize = 72;
pub const CI_V5_RETRIES_OFFSET: usize = 73;

#[cfg(all(test, feature = "legacy-unit-tests"))]
mod tests {
    use super::*;

    #[test]
    fn commit_item_v5_is_96_bytes() {
        assert_eq!(size_of::<CommitItemV5>(), 96);
    }

    #[test]
    fn sub_queue_header_v5_is_64_bytes() {
        assert_eq!(size_of::<SubQueueHeaderV5>(), 64);
    }

    #[test]
    fn queue_layout_matches_offsets() {
        assert_eq!(
            EXECUTION_QUEUE_V5_SUB_QUEUE_HEADERS_OFFSET,
            DISC + EXECUTION_QUEUE_V5_HEADER_SIZE
        );
        assert_eq!(
            EXECUTION_QUEUE_V5_ITEMS_OFFSET,
            EXECUTION_QUEUE_V5_SUB_QUEUE_HEADERS_OFFSET
                + EXECUTION_QUEUE_V5_N_MAX_MARKETS * EXECUTION_QUEUE_V5_SUB_QUEUE_HEADER_STRIDE
        );
    }

    #[test]
    fn allocate_sub_queue_binds_slot() {
        let mut q: Box<ExecutionQueueV5> = unsafe { Box::new(std::mem::zeroed()) };
        q.init(Pubkey::new_unique(), Pubkey::new_unique(), 1);
        let idx = q.allocate_sub_queue(7, 0, 0, 4).unwrap();
        assert_eq!(idx, 0);
        assert_eq!(q.sub_queue_headers[0].market_index, 7);
        assert_eq!(q.sub_queue_headers[0].active, 1);
        // Re-allocating the same market_index fails.
        assert!(q.allocate_sub_queue(7, 0, 0, 4).is_err());
        // This layout is one market per queue account, so a second market
        // must use its own PDA instead of claiming another local slot.
        assert!(q.allocate_sub_queue(11, 0, 0, 4).is_err());
    }

    #[test]
    fn commit_reveal_cycle_advances_head() {
        let mut q: Box<ExecutionQueueV5> = unsafe { Box::new(std::mem::zeroed()) };
        q.init(Pubkey::new_unique(), Pubkey::new_unique(), 1);
        let idx = q.allocate_sub_queue(2, 0, 0, 4).unwrap();
        for seq in 0..5u64 {
            q.sub_queue_headers[idx]
                .validate_commit_sequence(seq)
                .unwrap();
            let mut item = CommitItemV5::default();
            item.sequence = seq;
            item.status = CommitStatusV5::Committed as u8;
            q.write_commit(idx, item).unwrap();
        }
        assert_eq!(q.sub_queue_headers[idx].live_count, 5);
        for expected in 0..5u64 {
            let cleared = q.clear_head(idx).unwrap();
            assert_eq!(cleared, expected);
        }
        assert_eq!(q.sub_queue_headers[idx].live_count, 0);
        assert_eq!(q.sub_queue_headers[idx].next_sequence_to_execute, 5);
    }

    #[test]
    fn ring_wraps_after_capacity() {
        // After clearing and re-committing beyond capacity, offsets should wrap.
        let mut q: Box<ExecutionQueueV5> = unsafe { Box::new(std::mem::zeroed()) };
        q.init(Pubkey::new_unique(), Pubkey::new_unique(), 1);
        let idx = q.allocate_sub_queue(0, 0, 0, 4).unwrap();
        let cap = EXECUTION_QUEUE_V5_PER_MARKET_CAPACITY as u64;
        // Commit and reveal `cap` entries to fill the ring exactly once.
        for seq in 0..cap {
            let mut item = CommitItemV5::default();
            item.sequence = seq;
            item.status = CommitStatusV5::Committed as u8;
            q.write_commit(idx, item).unwrap();
        }
        for _ in 0..cap {
            q.clear_head(idx).unwrap();
        }
        // Sequence == cap should land at ring offset 0 again.
        let mut item = CommitItemV5::default();
        item.sequence = cap;
        item.status = CommitStatusV5::Committed as u8;
        q.write_commit(idx, item).unwrap();
        assert_eq!(q.sub_queue_headers[idx].head_ring_offset(), 0);
        assert_eq!(
            q.items[ExecutionQueueV5::physical_index(idx, 0)].sequence,
            cap
        );
    }

    #[test]
    fn commit_rejects_out_of_window_sequence() {
        let mut q: Box<ExecutionQueueV5> = unsafe { Box::new(std::mem::zeroed()) };
        q.init(Pubkey::new_unique(), Pubkey::new_unique(), 1);
        let idx = q.allocate_sub_queue(0, 0, 0, 4).unwrap();
        let cap = EXECUTION_QUEUE_V5_PER_MARKET_CAPACITY as u64;
        // Sequence = capacity is out of the admission window (head=0).
        assert!(q.sub_queue_headers[idx]
            .validate_commit_sequence(cap)
            .is_err());
        // Sequence below head is rejected once head advances.
        q.sub_queue_headers[idx].next_sequence_to_execute = 10;
        assert!(q.sub_queue_headers[idx]
            .validate_commit_sequence(5)
            .is_err());
    }

    #[test]
    fn write_commit_enforces_window_and_live_slot_uniqueness() {
        let mut q: Box<ExecutionQueueV5> = unsafe { Box::new(std::mem::zeroed()) };
        q.init(Pubkey::new_unique(), Pubkey::new_unique(), 1);
        let idx = q.allocate_sub_queue(0, 0, 0, 4).unwrap();
        let cap = EXECUTION_QUEUE_V5_PER_MARKET_CAPACITY as u64;

        let mut item = CommitItemV5::default();
        item.sequence = cap;
        item.status = CommitStatusV5::Committed as u8;
        assert!(q.write_commit(idx, item).is_err());

        let mut item = CommitItemV5::default();
        item.sequence = 0;
        item.status = CommitStatusV5::Committed as u8;
        q.write_commit(idx, item).unwrap();
        let mut duplicate = CommitItemV5::default();
        duplicate.sequence = 0;
        duplicate.status = CommitStatusV5::Committed as u8;
        assert!(q.write_commit(idx, duplicate).is_err());
    }

    #[test]
    fn clear_head_requires_a_real_head_item() {
        let mut q: Box<ExecutionQueueV5> = unsafe { Box::new(std::mem::zeroed()) };
        q.init(Pubkey::new_unique(), Pubkey::new_unique(), 1);
        let idx = q.allocate_sub_queue(0, 0, 0, 4).unwrap();
        q.sub_queue_headers[idx].live_count = 1;
        q.sub_queue_headers[idx].max_seen_sequence = 1;
        q.sub_queue_headers[idx].next_sequence_to_execute = 0;
        assert!(q.clear_head(idx).is_err());
        assert_eq!(q.sub_queue_headers[idx].live_count, 1);
        assert_eq!(q.sub_queue_headers[idx].next_sequence_to_execute, 0);
    }

    #[test]
    fn replay_cache_rejects_recent_consumed_intent_hash() {
        let mut q: Box<ExecutionQueueV5> = unsafe { Box::new(std::mem::zeroed()) };
        q.init(Pubkey::new_unique(), Pubkey::new_unique(), 1);
        let intent_hash = [7u8; 32];
        let baseline_slot = 123;

        assert!(!q.has_recent_consumed_intent_hash(&intent_hash, baseline_slot));
        q.record_consumed_intent_hash(intent_hash, baseline_slot)
            .unwrap();
        assert!(q.has_recent_consumed_intent_hash(&intent_hash, baseline_slot));
        assert!(q.has_recent_consumed_intent_hash(
            &intent_hash,
            baseline_slot + EXECUTION_QUEUE_V5_REPLAY_RETENTION_SLOTS
        ));
        assert!(q
            .record_consumed_intent_hash(intent_hash, baseline_slot + 1)
            .is_err());
        assert!(!q.has_recent_consumed_intent_hash(
            &intent_hash,
            baseline_slot + EXECUTION_QUEUE_V5_REPLAY_RETENTION_SLOTS + 1
        ));
        q.record_consumed_intent_hash(
            intent_hash,
            baseline_slot + EXECUTION_QUEUE_V5_REPLAY_RETENTION_SLOTS + 1,
        )
        .unwrap();
        assert_eq!(q.header.replay_cache_len, 2);
        assert_eq!(q.header.replay_write_cursor, 2);
    }

    #[test]
    fn replay_cache_overwrites_full_cache_without_blocking() {
        let mut q: Box<ExecutionQueueV5> = unsafe { Box::new(std::mem::zeroed()) };
        q.init(Pubkey::new_unique(), Pubkey::new_unique(), 1);

        // Fill the cache: distinct hashes, all consumed at the same slot.
        let baseline_slot: u64 = 1_000_000;
        for i in 0..EXECUTION_QUEUE_V5_REPLAY_CACHE_CAPACITY {
            let mut h = [0u8; 32];
            h[0..8].copy_from_slice(&(i as u64).to_le_bytes());
            q.record_consumed_intent_hash(h, baseline_slot).unwrap();
        }
        assert_eq!(
            q.header.replay_cache_len as usize,
            EXECUTION_QUEUE_V5_REPLAY_CACHE_CAPACITY
        );

        // A duplicate inside the short retention window is still rejected.
        let mut duplicate = [0u8; 32];
        duplicate[0..8].copy_from_slice(&1u64.to_le_bytes());
        assert!(q
            .record_consumed_intent_hash(duplicate, baseline_slot)
            .is_err());

        // A distinct admission at the same slot overwrites the cursor instead
        // of failing with `ExecutionQueueV5ReplayCacheFull`.
        let mut new_hash = [0u8; 32];
        new_hash[0..8].copy_from_slice(&(u64::MAX - 1).to_le_bytes());
        q.record_consumed_intent_hash(new_hash, baseline_slot)
            .expect("full replay cache must not block distinct intents");
        assert_eq!(
            q.header.replay_cache_len as usize,
            EXECUTION_QUEUE_V5_REPLAY_CACHE_CAPACITY
        );
        assert_eq!(q.header.replay_write_cursor, 1);

        for i in 0..EXECUTION_QUEUE_V5_REPLAY_CACHE_CAPACITY {
            let mut h = [0u8; 32];
            h[0..8].copy_from_slice(&((10_000 + i) as u64).to_le_bytes());
            q.record_consumed_intent_hash(h, baseline_slot)
                .expect("continued overwrites must stay non-blocking");
        }
    }
}

#[cfg(test)]
mod replay_cache_tests {
    use super::*;

    #[test]
    fn try_record_matches_check_then_record() {
        let mut q: Box<ExecutionQueueV5> = unsafe { Box::new(std::mem::zeroed()) };
        q.init(Pubkey::new_unique(), Pubkey::new_unique(), 1);
        let hash = [7u8; 32];
        let slot = 1_000;

        assert!(q.try_record_consumed_intent_hash(hash, slot));
        assert_eq!(q.header.replay_cache_len, 1);

        // Duplicate inside the retention window: rejected, nothing recorded.
        assert!(!q.try_record_consumed_intent_hash(
            hash,
            slot + EXECUTION_QUEUE_V5_REPLAY_RETENTION_SLOTS
        ));
        assert_eq!(q.header.replay_cache_len, 1);
        assert!(q.record_consumed_intent_hash(hash, slot + 1).is_err());

        // Past the window the same hash is accepted again.
        assert!(q.try_record_consumed_intent_hash(
            hash,
            slot + EXECUTION_QUEUE_V5_REPLAY_RETENTION_SLOTS + 1
        ));
        assert_eq!(q.header.replay_cache_len, 2);

        // A distinct hash at the same slot is never a duplicate.
        assert!(q.try_record_consumed_intent_hash([8u8; 32], slot));
        assert_eq!(q.header.replay_write_cursor, 3);
    }
}
