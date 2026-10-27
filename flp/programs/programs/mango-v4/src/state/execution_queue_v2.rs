use super::execution_queue::{QueueItem, QueueItemStatus};
use crate::error::FermiError;
use anchor_lang::prelude::*;
use static_assertions::const_assert_eq;
use std::mem::size_of;

pub const EXECUTION_QUEUE_AUTHORITY_STATE_SPACE: usize =
    8 + size_of::<ExecutionQueueAuthorityState>();
pub const EXECUTION_QUEUE_V2_LAYOUT_VERSION: u8 = 1;
pub const EXECUTION_QUEUE_V2_DEFAULT_GAP_WAIT_SLOTS: u16 = 4;
pub const EXECUTION_QUEUE_V2_DEFAULT_MAX_RETRIES: u8 = 1;
pub const EXECUTION_QUEUE_V2_PER_MARKET_CAPACITY: usize = 1024;
pub const EXECUTION_QUEUE_V2_ACCOUNT_SPACE: usize = 8 + size_of::<ExecutionQueueV2>();

#[account]
#[derive(Debug)]
pub struct ExecutionQueueAuthorityState {
    pub group: Pubkey,
    pub admin: Pubkey,
    pub ctm_signer: Pubkey,
    pub pending_ctm_signer: Pubkey,
    pub pending_ctm_activation_slot: u64,
    pub bump: u8,
    pub _padding: [u8; 7],
    pub reserved: [u8; 16],
}
const_assert_eq!(size_of::<ExecutionQueueAuthorityState>() % 8, 0);

impl ExecutionQueueAuthorityState {
    pub fn init(&mut self, group: Pubkey, admin: Pubkey, ctm_signer: Pubkey, bump: u8) {
        self.group = group;
        self.admin = admin;
        self.ctm_signer = ctm_signer;
        self.pending_ctm_signer = Pubkey::default();
        self.pending_ctm_activation_slot = 0;
        self.bump = bump;
        self._padding = [0; 7];
        self.reserved = [0; 16];
    }

    pub fn set_pending_ctm_signer(&mut self, pending_ctm_signer: Pubkey, activate_at_slot: u64) {
        self.pending_ctm_signer = pending_ctm_signer;
        self.pending_ctm_activation_slot = activate_at_slot;
    }

    pub fn maybe_activate_pending_ctm(&mut self, current_slot: u64) -> bool {
        if self.pending_ctm_signer == Pubkey::default()
            || current_slot < self.pending_ctm_activation_slot
        {
            return false;
        }
        self.ctm_signer = self.pending_ctm_signer;
        self.pending_ctm_signer = Pubkey::default();
        self.pending_ctm_activation_slot = 0;
        true
    }
}

#[zero_copy]
#[derive(Debug)]
pub struct ExecutionQueueV2Header {
    pub group: Pubkey,
    pub authority_state: Pubkey,
    pub market_index: u16,
    pub bump: u8,
    pub paused_ingress: u8,
    pub paused_execute: u8,
    pub layout_version: u8,
    pub _padding0: [u8; 2],
    pub gap_wait_slots: u16,
    pub max_retries: u8,
    pub _padding1: [u8; 5],
    pub live_count: u32,
    pub _padding2: [u8; 4],
    pub next_sequence_to_execute: u64,
    pub max_seen_sequence: u64,
    pub gap_observed_slot: u64,
    pub first_failure_slot: u64,
    pub reserved: [u8; 72],
}
const_assert_eq!(size_of::<ExecutionQueueV2Header>(), 192);
const_assert_eq!(size_of::<ExecutionQueueV2Header>() % 8, 0);

#[account(zero_copy)]
#[derive(Debug)]
pub struct ExecutionQueueV2 {
    pub header: ExecutionQueueV2Header,
    pub items: [QueueItem; EXECUTION_QUEUE_V2_PER_MARKET_CAPACITY],
}
const_assert_eq!(size_of::<ExecutionQueueV2>() % 8, 0);

impl ExecutionQueueV2Header {
    pub fn max_retries_effective(&self) -> u8 {
        if self.max_retries == 0 {
            EXECUTION_QUEUE_V2_DEFAULT_MAX_RETRIES
        } else {
            self.max_retries
        }
    }
}

impl ExecutionQueueV2 {
    pub fn init(&mut self, group: Pubkey, authority_state: Pubkey, market_index: u16, bump: u8) {
        self.header.group = group;
        self.header.authority_state = authority_state;
        self.header.market_index = market_index;
        self.header.bump = bump;
        self.header.paused_ingress = 0;
        self.header.paused_execute = 0;
        self.header.layout_version = EXECUTION_QUEUE_V2_LAYOUT_VERSION;
        self.header._padding0 = [0; 2];
        self.header.gap_wait_slots = EXECUTION_QUEUE_V2_DEFAULT_GAP_WAIT_SLOTS;
        self.header.max_retries = EXECUTION_QUEUE_V2_DEFAULT_MAX_RETRIES;
        self.header._padding1 = [0; 5];
        self.header.live_count = 0;
        self.header._padding2 = [0; 4];
        self.header.next_sequence_to_execute = 0;
        self.header.max_seen_sequence = 0;
        self.header.gap_observed_slot = 0;
        self.header.first_failure_slot = 0;
        self.header.reserved = [0; 72];
        for item in self.items.iter_mut() {
            *item = QueueItem::default();
        }
    }

    pub fn next_enqueue_sequence(&self) -> u64 {
        if self.header.live_count == 0 {
            self.header.next_sequence_to_execute
        } else {
            self.header.max_seen_sequence.saturating_add(1)
        }
    }

    pub fn validate_sequence(&self, sequence: u64) -> Result<()> {
        require!(
            sequence >= self.header.next_sequence_to_execute,
            FermiError::InvalidSequenceNumber
        );
        require!(
            sequence
                < self
                    .header
                    .next_sequence_to_execute
                    .saturating_add(EXECUTION_QUEUE_V2_PER_MARKET_CAPACITY as u64),
            FermiError::ExecutionQueueFull
        );
        Ok(())
    }

    pub fn physical_index(sequence: u64) -> usize {
        (sequence % EXECUTION_QUEUE_V2_PER_MARKET_CAPACITY as u64) as usize
    }

    pub fn get_item(&self, sequence: u64) -> Option<&QueueItem> {
        let phys = Self::physical_index(sequence);
        let item = &self.items[phys];
        if item.status == QueueItemStatus::Empty as u8 || item.sequence != sequence {
            None
        } else {
            Some(item)
        }
    }

    pub fn get_item_mut(&mut self, sequence: u64) -> Option<&mut QueueItem> {
        let phys = Self::physical_index(sequence);
        let item = &mut self.items[phys];
        if item.status == QueueItemStatus::Empty as u8 || item.sequence != sequence {
            None
        } else {
            Some(item)
        }
    }

    pub fn get_head_item(&self) -> Option<&QueueItem> {
        if self.header.live_count == 0 {
            return None;
        }
        self.get_item(self.header.next_sequence_to_execute)
    }

    pub fn push(&mut self, item: QueueItem) -> Result<()> {
        self.validate_sequence(item.sequence)?;
        let phys = Self::physical_index(item.sequence);
        let existing = &self.items[phys];
        require!(
            existing.status != QueueItemStatus::Pending as u8 || existing.sequence != item.sequence,
            FermiError::ExecutionQueueDuplicateSequence
        );
        require!(
            existing.status != QueueItemStatus::Pending as u8,
            FermiError::ExecutionQueueFull
        );
        self.items[phys] = item;
        self.header.live_count = self.header.live_count.saturating_add(1);
        if self.header.live_count == 1 || item.sequence > self.header.max_seen_sequence {
            self.header.max_seen_sequence = item.sequence;
        }
        Ok(())
    }

    pub fn clear_head_or_gap(&mut self) -> Result<(u64, Option<QueueItem>)> {
        require!(
            self.header.live_count > 0
                || self.header.max_seen_sequence >= self.header.next_sequence_to_execute,
            FermiError::ExecutionQueueEmpty
        );
        let sequence = self.header.next_sequence_to_execute;
        let phys = Self::physical_index(sequence);
        let item = if self.items[phys].status != QueueItemStatus::Empty as u8
            && self.items[phys].sequence == sequence
        {
            let item = self.items[phys];
            self.items[phys] = QueueItem::default();
            self.header.live_count = self.header.live_count.saturating_sub(1);
            Some(item)
        } else {
            self.items[phys] = QueueItem::default();
            None
        };
        self.header.next_sequence_to_execute = sequence.saturating_add(1);
        self.header.gap_observed_slot = 0;
        Ok((sequence, item))
    }

    pub fn clear_item_at(&mut self, sequence: u64) -> Result<()> {
        let phys = Self::physical_index(sequence);
        let item = &self.items[phys];
        require!(
            item.status != QueueItemStatus::Empty as u8 && item.sequence == sequence,
            FermiError::ExecutionQueueSequenceNotPending
        );
        self.items[phys] = QueueItem::default();
        self.header.live_count = self.header.live_count.saturating_sub(1);
        if sequence == self.header.next_sequence_to_execute {
            self.header.next_sequence_to_execute =
                self.header.next_sequence_to_execute.saturating_add(1);
            self.header.gap_observed_slot = 0;
        }
        Ok(())
    }

    pub fn increment_retry(&mut self, sequence: u64, current_slot: u64) -> Result<u8> {
        let item = self
            .get_item_mut(sequence)
            .ok_or_else(|| error!(FermiError::ExecutionQueueSequenceNotPending))?;
        if item.first_failure_slot == 0 {
            item.first_failure_slot = current_slot;
        }
        item.retries = item.retries.saturating_add(1);
        Ok(item.retries)
    }
}

#[cfg(all(test, feature = "legacy-unit-tests"))]
mod tests {
    use super::super::execution_queue::QueueItemKind;
    use super::*;

    #[test]
    fn header_size_matches() {
        assert_eq!(size_of::<ExecutionQueueV2Header>(), 192);
    }

    #[test]
    fn push_and_clear_head() {
        let mut q: Box<ExecutionQueueV2> = unsafe { Box::new(std::mem::zeroed()) };
        q.init(Pubkey::new_unique(), Pubkey::new_unique(), 7, 1);
        let mut item = QueueItem::default();
        item.sequence = 0;
        item.kind = QueueItemKind::CtmWrapped as u8;
        item.status = QueueItemStatus::Pending as u8;
        q.push(item).unwrap();
        let (seq, cleared) = q.clear_head_or_gap().unwrap();
        assert_eq!(seq, 0);
        assert!(cleared.is_some());
        assert_eq!(q.header.live_count, 0);
        assert_eq!(q.header.next_sequence_to_execute, 1);
    }

    #[test]
    fn next_enqueue_tracks_max_seen() {
        let mut q: Box<ExecutionQueueV2> = unsafe { Box::new(std::mem::zeroed()) };
        q.init(Pubkey::new_unique(), Pubkey::new_unique(), 7, 1);
        assert_eq!(q.next_enqueue_sequence(), 0);
        let mut item = QueueItem::default();
        item.sequence = 3;
        item.status = QueueItemStatus::Pending as u8;
        q.push(item).unwrap();
        assert_eq!(q.next_enqueue_sequence(), 4);
    }

    #[test]
    fn clear_head_gap_advances_one_sequence() {
        let mut q: Box<ExecutionQueueV2> = unsafe { Box::new(std::mem::zeroed()) };
        q.init(Pubkey::new_unique(), Pubkey::new_unique(), 7, 1);
        q.header.next_sequence_to_execute = 1;
        q.header.max_seen_sequence = 3;

        let (seq, cleared) = q.clear_head_or_gap().unwrap();
        assert_eq!(seq, 1);
        assert!(cleared.is_none());
        assert_eq!(q.header.next_sequence_to_execute, 2);
    }
}
