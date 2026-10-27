use crate::error::FermiError;
use anchor_lang::prelude::*;
use static_assertions::const_assert_eq;
use std::mem::size_of;

                                                                          
  
                                                                        
                                                                        
                                                                          
                                                                           
                                          
  
                                                                           
                                                                          
                                                                         
pub const EXECUTION_QUEUE_N_MAX_MARKETS: usize = 16;
pub const EXECUTION_QUEUE_PER_MARKET_CTM_CAPACITY: usize = 64;
pub const EXECUTION_QUEUE_CTM_CAPACITY: usize =
    EXECUTION_QUEUE_N_MAX_MARKETS * EXECUTION_QUEUE_PER_MARKET_CTM_CAPACITY;
pub const EXECUTION_QUEUE_LIQUIDITY_CAPACITY: usize = 128;
pub const EXECUTION_QUEUE_PAYLOAD_MAX: usize = 256;
pub const EXECUTION_QUEUE_DISCRIMINATOR_BYTES: usize = 8;
pub const EXECUTION_QUEUE_ACCOUNT_SPACE: usize = 8 + size_of::<ExecutionQueue>();
pub const EXECUTION_QUEUE_CREATE_SPACE: usize = 8;

pub const EXECUTION_QUEUE_LAYOUT_VERSION_V1: u8 = 0;
pub const EXECUTION_QUEUE_LAYOUT_VERSION_V2: u8 = 2;

#[repr(u8)]
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum QueueItemKind {
    CtmWrapped = 0,
    LiquidityDeposit = 1,
    LiquidityWithdraw = 2,
}

#[repr(u8)]
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum QueueItemStatus {
    Empty = 0,
    Pending = 1,
    Executed = 2,
    Failed = 3,
    Skipped = 4,
}

#[zero_copy]
#[derive(Debug)]
pub struct QueueItem {
    pub sequence: u64,
    pub min_execute_slot: u64,
    pub ingress_slot: u64,
    pub first_failure_slot: u64,
    pub kind: u8,
    pub status: u8,
    pub retries: u8,
    pub _padding: [u8; 5],
    pub payload_len: u16,
    pub _padding2: [u8; 6],
    pub payload_hash: [u8; 32],
    pub accounts_hash: [u8; 32],
    pub payload: [u8; EXECUTION_QUEUE_PAYLOAD_MAX],
}

impl Default for QueueItem {
    fn default() -> Self {
        Self {
            sequence: 0,
            min_execute_slot: 0,
            ingress_slot: 0,
            first_failure_slot: 0,
            kind: QueueItemKind::CtmWrapped as u8,
            status: QueueItemStatus::Empty as u8,
            retries: 0,
            _padding: [0; 5],
            payload_len: 0,
            _padding2: [0; 6],
            payload_hash: [0; 32],
            accounts_hash: [0; 32],
            payload: [0; EXECUTION_QUEUE_PAYLOAD_MAX],
        }
    }
}
const_assert_eq!(size_of::<QueueItem>(), 368);
const_assert_eq!(size_of::<QueueItem>() % 8, 0);

                                                                        
                                                                        
                                                                    
                                                                             
#[zero_copy]
#[derive(Debug)]
pub struct SubQueueHeader {
    pub market_index: u16,
    pub active: u8,
    pub paused_ingress: u8,
    pub paused_execute: u8,
    pub _padding0: [u8; 3],
    pub ctm_count: u32,
    pub _padding1: u32,
    pub next_sequence_to_execute: u64,
    pub max_seen_sequence: u64,
    pub gap_observed_slot: u64,
    pub first_failure_slot: u64,
    pub reserved: [u8; 16],
}
const_assert_eq!(size_of::<SubQueueHeader>(), 64);
const_assert_eq!(size_of::<SubQueueHeader>() % 8, 0);

                                                                     
                                                                        
                                                    
#[zero_copy]
#[derive(Debug)]
pub struct ExecutionQueueGlobalHeader {
    pub gap_wait_slots: u64,
    pub liquidity_delay_slots: u64,
    pub liquidity_head: u32,
    pub liquidity_count: u32,
    pub total_count: u32,
    pub _padding: u32,
    pub reserved: [u8; 32],
}
const_assert_eq!(size_of::<ExecutionQueueGlobalHeader>(), 64);
const_assert_eq!(size_of::<ExecutionQueueGlobalHeader>() % 8, 0);

#[account(zero_copy)]
#[derive(Debug)]
pub struct ExecutionQueue {
    pub group: Pubkey,
    pub admin: Pubkey,
    pub ctm_signer: Pubkey,
    pub pending_ctm_signer: Pubkey,
    pub pending_ctm_activate_slot: u64,
    pub bump: u8,
    pub paused_ingress: u8,
    pub paused_execute: u8,
                                                                        
                                                                     
                        
    pub layout_version: u8,
    pub _padding: [u8; 4],
    pub global_header: ExecutionQueueGlobalHeader,
    pub reserved: [u8; 48],
    pub sub_queue_headers: [SubQueueHeader; EXECUTION_QUEUE_N_MAX_MARKETS],
                                                                           
                                                                                      
    pub ctm_items: [QueueItem; EXECUTION_QUEUE_CTM_CAPACITY],
    pub liquidity_items: [QueueItem; EXECUTION_QUEUE_LIQUIDITY_CAPACITY],
}
                                                                                     
                                                               
const_assert_eq!(size_of::<ExecutionQueue>(), 425216);
const_assert_eq!(size_of::<ExecutionQueue>() % 8, 0);
const_assert_eq!(EXECUTION_QUEUE_ACCOUNT_SPACE, 425224);

                                                                         
  
                                                                        
                                                                      
                                                                           
const DISCRIMINATOR_LEN: usize = EXECUTION_QUEUE_DISCRIMINATOR_BYTES;

                                                     
pub const EXECUTION_QUEUE_BUMP_OFFSET: usize = DISCRIMINATOR_LEN + 136;
pub const EXECUTION_QUEUE_PAUSED_INGRESS_OFFSET: usize = DISCRIMINATOR_LEN + 137;
pub const EXECUTION_QUEUE_PAUSED_EXECUTE_OFFSET: usize = DISCRIMINATOR_LEN + 138;
pub const EXECUTION_QUEUE_LAYOUT_VERSION_OFFSET: usize = DISCRIMINATOR_LEN + 139;
pub const EXECUTION_QUEUE_GLOBAL_HEADER_OFFSET: usize = DISCRIMINATOR_LEN + 144;

                                                                    
pub const EXECUTION_QUEUE_GAP_WAIT_SLOTS_OFFSET: usize = EXECUTION_QUEUE_GLOBAL_HEADER_OFFSET + 0;
pub const EXECUTION_QUEUE_LIQUIDITY_DELAY_SLOTS_OFFSET: usize =
    EXECUTION_QUEUE_GLOBAL_HEADER_OFFSET + 8;
pub const EXECUTION_QUEUE_LIQUIDITY_HEAD_OFFSET: usize = EXECUTION_QUEUE_GLOBAL_HEADER_OFFSET + 16;
pub const EXECUTION_QUEUE_LIQUIDITY_COUNT_OFFSET: usize = EXECUTION_QUEUE_GLOBAL_HEADER_OFFSET + 20;
pub const EXECUTION_QUEUE_TOTAL_COUNT_OFFSET: usize = EXECUTION_QUEUE_GLOBAL_HEADER_OFFSET + 24;

pub const EXECUTION_QUEUE_SUB_QUEUE_HEADERS_OFFSET: usize = DISCRIMINATOR_LEN + 256;
pub const EXECUTION_QUEUE_SUB_QUEUE_HEADER_STRIDE: usize = size_of::<SubQueueHeader>();
pub const EXECUTION_QUEUE_CTM_ITEMS_OFFSET: usize = EXECUTION_QUEUE_SUB_QUEUE_HEADERS_OFFSET
    + EXECUTION_QUEUE_N_MAX_MARKETS * EXECUTION_QUEUE_SUB_QUEUE_HEADER_STRIDE;
pub const EXECUTION_QUEUE_ITEM_SIZE: usize = size_of::<QueueItem>();
pub const EXECUTION_QUEUE_LIQUIDITY_ITEMS_OFFSET: usize =
    EXECUTION_QUEUE_CTM_ITEMS_OFFSET + EXECUTION_QUEUE_CTM_CAPACITY * EXECUTION_QUEUE_ITEM_SIZE;

                                                                                
pub const EXECUTION_QUEUE_ITEM_SEQUENCE_OFFSET: usize = 0;
pub const EXECUTION_QUEUE_ITEM_MIN_EXECUTE_SLOT_OFFSET: usize = 8;
pub const EXECUTION_QUEUE_ITEM_KIND_OFFSET: usize = 32;
pub const EXECUTION_QUEUE_ITEM_STATUS_OFFSET: usize = 33;
pub const EXECUTION_QUEUE_ITEM_RETRIES_OFFSET: usize = 34;
pub const EXECUTION_QUEUE_ITEM_PAYLOAD_LEN_OFFSET: usize = 40;
pub const EXECUTION_QUEUE_ITEM_PAYLOAD_HASH_OFFSET: usize = 48;
pub const EXECUTION_QUEUE_ITEM_ACCOUNTS_HASH_OFFSET: usize = 80;
pub const EXECUTION_QUEUE_ITEM_PAYLOAD_OFFSET: usize = 112;

                                                                              
pub const SUB_QUEUE_HEADER_MARKET_INDEX_OFFSET: usize = 0;
pub const SUB_QUEUE_HEADER_ACTIVE_OFFSET: usize = 2;
pub const SUB_QUEUE_HEADER_PAUSED_INGRESS_OFFSET: usize = 3;
pub const SUB_QUEUE_HEADER_PAUSED_EXECUTE_OFFSET: usize = 4;
pub const SUB_QUEUE_HEADER_CTM_COUNT_OFFSET: usize = 8;
pub const SUB_QUEUE_HEADER_NEXT_SEQUENCE_OFFSET: usize = 16;
pub const SUB_QUEUE_HEADER_MAX_SEEN_SEQUENCE_OFFSET: usize = 24;
pub const SUB_QUEUE_HEADER_GAP_OBSERVED_SLOT_OFFSET: usize = 32;
pub const SUB_QUEUE_HEADER_FIRST_FAILURE_SLOT_OFFSET: usize = 40;

                                                                           
const_assert_eq!(EXECUTION_QUEUE_LAYOUT_VERSION_OFFSET, 147);
const_assert_eq!(EXECUTION_QUEUE_GLOBAL_HEADER_OFFSET, 152);
const_assert_eq!(EXECUTION_QUEUE_SUB_QUEUE_HEADERS_OFFSET, 264);
const_assert_eq!(EXECUTION_QUEUE_CTM_ITEMS_OFFSET, 1288);
const_assert_eq!(EXECUTION_QUEUE_LIQUIDITY_ITEMS_OFFSET, 378120);

impl ExecutionQueue {
                                                                     
                                                            
    pub fn init(&mut self, group: Pubkey, admin: Pubkey, ctm_signer: Pubkey, bump: u8) {
        self.group = group;
        self.admin = admin;
        self.ctm_signer = ctm_signer;
        self.pending_ctm_signer = Pubkey::default();
        self.pending_ctm_activate_slot = 0;
        self.bump = bump;
        self.paused_ingress = 0;
        self.paused_execute = 0;
        self.layout_version = EXECUTION_QUEUE_LAYOUT_VERSION_V2;
        self._padding = [0; 4];
        self.global_header = ExecutionQueueGlobalHeader {
            gap_wait_slots: 4,
            liquidity_delay_slots: 25,
            liquidity_head: 0,
            liquidity_count: 0,
            total_count: 0,
            _padding: 0,
            reserved: [0; 32],
        };
        self.reserved = [0; 48];
        for header in self.sub_queue_headers.iter_mut() {
            *header = SubQueueHeader {
                market_index: 0,
                active: 0,
                paused_ingress: 0,
                paused_execute: 0,
                _padding0: [0; 3],
                ctm_count: 0,
                _padding1: 0,
                next_sequence_to_execute: 0,
                max_seen_sequence: 0,
                gap_observed_slot: 0,
                first_failure_slot: 0,
                reserved: [0; 16],
            };
        }
                                                                        
                                                                        
        for item in self.ctm_items.iter_mut() {
            *item = QueueItem::default();
        }
        for item in self.liquidity_items.iter_mut() {
            *item = QueueItem::default();
        }
    }

    pub fn maybe_activate_pending_ctm(&mut self, current_slot: u64) {
        if self.pending_ctm_signer != Pubkey::default()
            && current_slot >= self.pending_ctm_activate_slot
        {
            self.ctm_signer = self.pending_ctm_signer;
            self.pending_ctm_signer = Pubkey::default();
            self.pending_ctm_activate_slot = 0;
        }
    }

    pub fn is_v2(&self) -> bool {
        self.layout_version == EXECUTION_QUEUE_LAYOUT_VERSION_V2
    }

    pub fn require_v2(&self) -> Result<()> {
        require!(
            self.is_v2(),
            FermiError::ExecutionQueueSubQueueLayoutVersionMismatch
        );
        Ok(())
    }

    pub fn len(&self) -> usize {
        self.global_header.total_count as usize
    }

    pub fn is_empty(&self) -> bool {
        self.global_header.total_count == 0
    }

    pub fn is_full(&self) -> bool {
        let all_ctm_full = self
            .sub_queue_headers
            .iter()
            .filter(|h| h.active != 0)
            .all(|h| h.ctm_count >= EXECUTION_QUEUE_PER_MARKET_CTM_CAPACITY as u32);
        let liq_full =
            self.global_header.liquidity_count >= EXECUTION_QUEUE_LIQUIDITY_CAPACITY as u32;
        all_ctm_full && liq_full
    }

                                                         
    pub fn find_sub_queue_slot(&self, market_index: u16) -> Option<usize> {
        self.sub_queue_headers
            .iter()
            .position(|h| h.active != 0 && h.market_index == market_index)
    }

                                                                         
                                          
    pub fn sub_queue(&self, market_index: u16) -> Result<&SubQueueHeader> {
        let slot = self
            .find_sub_queue_slot(market_index)
            .ok_or_else(|| error!(FermiError::ExecutionQueueSubQueueInvalidMarketIndex))?;
        Ok(&self.sub_queue_headers[slot])
    }

    pub fn sub_queue_mut(&mut self, market_index: u16) -> Result<&mut SubQueueHeader> {
        let slot = self
            .find_sub_queue_slot(market_index)
            .ok_or_else(|| error!(FermiError::ExecutionQueueSubQueueInvalidMarketIndex))?;
        Ok(&mut self.sub_queue_headers[slot])
    }

                                                                         
                                                                          
                         
    pub fn get_or_allocate_sub_queue_slot(&mut self, market_index: u16) -> Result<usize> {
        if let Some(existing) = self.find_sub_queue_slot(market_index) {
            return Ok(existing);
        }
        let free_slot = self
            .sub_queue_headers
            .iter()
            .position(|h| h.active == 0)
            .ok_or_else(|| error!(FermiError::ExecutionQueueSubQueueSlotsExhausted))?;
        let header = &mut self.sub_queue_headers[free_slot];
        header.market_index = market_index;
        header.active = 1;
        header.paused_ingress = 0;
        header.paused_execute = 0;
        header.ctm_count = 0;
        header.next_sequence_to_execute = 0;
        header.max_seen_sequence = 0;
        header.gap_observed_slot = 0;
        header.first_failure_slot = 0;
        Ok(free_slot)
    }

                                                               
    fn item_index(slot_index: usize, sequence: u64) -> usize {
        slot_index * EXECUTION_QUEUE_PER_MARKET_CTM_CAPACITY
            + (sequence as usize) % EXECUTION_QUEUE_PER_MARKET_CTM_CAPACITY
    }

    pub fn ctm_item_for(&self, slot_index: usize, sequence: u64) -> &QueueItem {
        &self.ctm_items[Self::item_index(slot_index, sequence)]
    }

    pub fn ctm_item_for_mut(&mut self, slot_index: usize, sequence: u64) -> &mut QueueItem {
        &mut self.ctm_items[Self::item_index(slot_index, sequence)]
    }

    pub fn ctm_item(&self, market_index: u16, sequence: u64) -> Result<&QueueItem> {
        let slot = self
            .find_sub_queue_slot(market_index)
            .ok_or_else(|| error!(FermiError::ExecutionQueueSubQueueInvalidMarketIndex))?;
        Ok(self.ctm_item_for(slot, sequence))
    }

    pub fn can_enqueue_ctm_sequence(&self, market_index: u16, sequence: u64) -> bool {
        let Some(header) = self
            .find_sub_queue_slot(market_index)
            .map(|i| &self.sub_queue_headers[i])
        else {
                                                                                       
            return self.sub_queue_headers.iter().any(|h| h.active == 0);
        };
        let base = header.next_sequence_to_execute;
        sequence >= base
            && sequence < base.saturating_add(EXECUTION_QUEUE_PER_MARKET_CTM_CAPACITY as u64)
    }

    pub fn push_ctm(&mut self, market_index: u16, item: QueueItem) -> Result<()> {
        let slot = self.get_or_allocate_sub_queue_slot(market_index)?;
        let base = self.sub_queue_headers[slot].next_sequence_to_execute;
        require!(
            item.sequence >= base
                && item.sequence
                    < base.saturating_add(EXECUTION_QUEUE_PER_MARKET_CTM_CAPACITY as u64),
            FermiError::ExecutionQueueFull
        );
        let item_idx = Self::item_index(slot, item.sequence);
        let existing = &self.ctm_items[item_idx];
        require!(
            existing.status != QueueItemStatus::Pending as u8 || existing.sequence != item.sequence,
            FermiError::ExecutionQueueDuplicateSequence
        );
        require!(
            existing.status != QueueItemStatus::Pending as u8,
            FermiError::ExecutionQueueFull
        );
        self.ctm_items[item_idx] = item;
        let header = &mut self.sub_queue_headers[slot];
        header.ctm_count = header.ctm_count.saturating_add(1);
        if item.sequence > header.max_seen_sequence {
            header.max_seen_sequence = item.sequence;
        }
        self.global_header.total_count = self.global_header.total_count.saturating_add(1);
        Ok(())
    }

    pub fn current_ctm_head(&self, market_index: u16) -> Option<&QueueItem> {
        let slot = self.find_sub_queue_slot(market_index)?;
        let header = &self.sub_queue_headers[slot];
        if header.ctm_count == 0 {
            return None;
        }
        let next = header.next_sequence_to_execute;
        let item = self.ctm_item_for(slot, next);
        if item.status == QueueItemStatus::Pending as u8 && item.sequence == next {
            Some(item)
        } else {
            None
        }
    }

    pub fn clear_current_ctm_head_and_advance(&mut self, market_index: u16) -> Result<()> {
        let slot = self
            .find_sub_queue_slot(market_index)
            .ok_or_else(|| error!(FermiError::ExecutionQueueSubQueueInvalidMarketIndex))?;
        let next = self.sub_queue_headers[slot].next_sequence_to_execute;
        let item = self.ctm_item_for_mut(slot, next);
        *item = QueueItem::default();
        let header = &mut self.sub_queue_headers[slot];
        header.ctm_count = header.ctm_count.saturating_sub(1);
        header.next_sequence_to_execute = header.next_sequence_to_execute.saturating_add(1);
        header.gap_observed_slot = 0;
        self.global_header.total_count = self.global_header.total_count.saturating_sub(1);
        self.normalize_empty_ctm_head(market_index)?;
        Ok(())
    }

                                                                                 
                                                                          
    pub fn find_matching_ctm_item(
        &self,
        market_index: u16,
        accounts_hash: &[u8; 32],
        scan_limit: u16,
    ) -> Option<(u64, QueueItem)> {
        let slot = self.find_sub_queue_slot(market_index)?;
        let header = &self.sub_queue_headers[slot];
        let base = header.next_sequence_to_execute;
        let max = header.max_seen_sequence;
        let limit = (scan_limit as u64).min(EXECUTION_QUEUE_PER_MARKET_CTM_CAPACITY as u64);
        let mut seq = base;
        while seq <= max && seq < base.saturating_add(limit) {
            let item = self.ctm_item_for(slot, seq);
            if item.status == QueueItemStatus::Pending as u8
                && item.sequence == seq
                && &item.accounts_hash == accounts_hash
            {
                return Some((seq, *item));
            }
            seq = seq.saturating_add(1);
        }
        None
    }

                                                                         
                                                                       
                                                                  
    pub fn clear_ctm_item_at(&mut self, market_index: u16, sequence: u64) -> Result<()> {
        let slot = self
            .find_sub_queue_slot(market_index)
            .ok_or_else(|| error!(FermiError::ExecutionQueueSubQueueInvalidMarketIndex))?;
        let item = self.ctm_item_for_mut(slot, sequence);
        *item = QueueItem::default();
        let header = &mut self.sub_queue_headers[slot];
        header.ctm_count = header.ctm_count.saturating_sub(1);
        self.global_header.total_count = self.global_header.total_count.saturating_sub(1);
                                                        
        while self.sub_queue_headers[slot].ctm_count > 0
            && self.sub_queue_headers[slot].max_seen_sequence
                >= self.sub_queue_headers[slot].next_sequence_to_execute
        {
            let next_seq = self.sub_queue_headers[slot].next_sequence_to_execute;
            let head = self.ctm_item_for(slot, next_seq);
            if head.status == QueueItemStatus::Pending as u8 && head.sequence == next_seq {
                break;                                 
            }
            if head.status == QueueItemStatus::Empty as u8 || head.sequence != next_seq {
                let header = &mut self.sub_queue_headers[slot];
                header.next_sequence_to_execute = header.next_sequence_to_execute.saturating_add(1);
                header.gap_observed_slot = 0;
            } else {
                break;
            }
        }
        self.normalize_empty_ctm_head(market_index)?;
        Ok(())
    }

                                                                          
                                                                          
                                                                       
                                        
    pub fn normalize_empty_ctm_head(&mut self, market_index: u16) -> Result<()> {
        let slot = self
            .find_sub_queue_slot(market_index)
            .ok_or_else(|| error!(FermiError::ExecutionQueueSubQueueInvalidMarketIndex))?;
        let header = &mut self.sub_queue_headers[slot];
        if header.ctm_count == 0 && header.max_seen_sequence >= header.next_sequence_to_execute {
            header.next_sequence_to_execute = header.max_seen_sequence.saturating_add(1);
            header.gap_observed_slot = 0;
        }
        Ok(())
    }

                                                                           
                    
    pub fn increment_ctm_retry(
        &mut self,
        market_index: u16,
        sequence: u64,
        current_slot: u64,
    ) -> Result<u8> {
        let slot = self
            .find_sub_queue_slot(market_index)
            .ok_or_else(|| error!(FermiError::ExecutionQueueSubQueueInvalidMarketIndex))?;
        let item = self.ctm_item_for_mut(slot, sequence);
        if item.first_failure_slot == 0 {
            item.first_failure_slot = current_slot;
        }
        item.retries = item.retries.saturating_add(1);
        Ok(item.retries)
    }

    pub fn total_ctm_count(&self) -> u32 {
        self.sub_queue_headers
            .iter()
            .filter(|h| h.active != 0)
            .map(|h| h.ctm_count)
            .sum()
    }

                                                                            
    pub fn liquidity_tail_index(&self) -> usize {
        ((self.global_header.liquidity_head + self.global_header.liquidity_count) as usize)
            % EXECUTION_QUEUE_LIQUIDITY_CAPACITY
    }

    pub fn liquidity_head_item(&self) -> Option<&QueueItem> {
        if self.global_header.liquidity_count == 0 {
            return None;
        }
        Some(&self.liquidity_items[self.global_header.liquidity_head as usize])
    }

    pub fn liquidity_head_item_mut(&mut self) -> Option<&mut QueueItem> {
        if self.global_header.liquidity_count == 0 {
            return None;
        }
        Some(&mut self.liquidity_items[self.global_header.liquidity_head as usize])
    }

    pub fn push_liquidity(&mut self, item: QueueItem) -> Result<()> {
        require!(
            self.global_header.liquidity_count < EXECUTION_QUEUE_LIQUIDITY_CAPACITY as u32,
            FermiError::ExecutionQueueFull
        );
        let tail = self.liquidity_tail_index();
        self.liquidity_items[tail] = item;
        self.global_header.liquidity_count = self.global_header.liquidity_count.saturating_add(1);
        self.global_header.total_count = self.global_header.total_count.saturating_add(1);
        Ok(())
    }

    pub fn pop_liquidity_head(&mut self) -> Option<QueueItem> {
        if self.global_header.liquidity_count == 0 {
            return None;
        }
        let head = self.global_header.liquidity_head as usize;
        let item = self.liquidity_items[head];
        self.liquidity_items[head] = QueueItem::default();
        self.global_header.liquidity_head =
            ((head + 1) % EXECUTION_QUEUE_LIQUIDITY_CAPACITY) as u32;
        self.global_header.liquidity_count = self.global_header.liquidity_count.saturating_sub(1);
        self.global_header.total_count = self.global_header.total_count.saturating_sub(1);
        Some(item)
    }

    pub fn rotate_liquidity_head_with_retry(
        &mut self,
        current_slot: u64,
        next_retry: u8,
        backoff_slots: u64,
    ) -> Result<()> {
        let mut item = self
            .pop_liquidity_head()
            .ok_or_else(|| error!(FermiError::SomeError))?;
        if item.first_failure_slot == 0 {
            item.first_failure_slot = current_slot;
        }
        item.retries = next_retry;
        item.min_execute_slot = current_slot.saturating_add(backoff_slots);
        self.push_liquidity(item)
    }
}

#[cfg(all(test, feature = "legacy-unit-tests"))]
mod tests {
    use super::*;

    const TEST_MARKET: u16 = 0;

    fn test_queue() -> Box<ExecutionQueue> {
        let layout = std::alloc::Layout::new::<ExecutionQueue>();
        let ptr = unsafe { std::alloc::alloc_zeroed(layout) as *mut ExecutionQueue };
        assert!(!ptr.is_null());
        let mut queue = unsafe { Box::from_raw(ptr) };
                                                                            
                                                                          
                         
        queue.group = Pubkey::new_unique();
        queue.admin = Pubkey::new_unique();
        queue.ctm_signer = Pubkey::new_unique();
        queue.bump = 1;
        queue.layout_version = EXECUTION_QUEUE_LAYOUT_VERSION_V2;
        queue.global_header.gap_wait_slots = 4;
        queue.global_header.liquidity_delay_slots = 25;
        queue
    }

    fn pending_ctm_item(sequence: u64, accounts_hash_seed: u8) -> QueueItem {
        let mut item = QueueItem::default();
        item.sequence = sequence;
        item.kind = QueueItemKind::CtmWrapped as u8;
        item.status = QueueItemStatus::Pending as u8;
        item.accounts_hash = [accounts_hash_seed; 32];
        item
    }

    fn pending_liquidity_item(sequence: u64, kind: QueueItemKind) -> QueueItem {
        let mut item = QueueItem::default();
        item.sequence = sequence;
        item.kind = kind as u8;
        item.status = QueueItemStatus::Pending as u8;
        item
    }

    fn slot_header(queue: &ExecutionQueue, market: u16) -> &SubQueueHeader {
        let slot = queue.find_sub_queue_slot(market).unwrap();
        &queue.sub_queue_headers[slot]
    }

    #[test]
    fn push_ctm_rejects_duplicate_pending_sequence() {
        let mut queue = test_queue();
        queue.push_ctm(TEST_MARKET, pending_ctm_item(0, 1)).unwrap();

        let result = queue.push_ctm(TEST_MARKET, pending_ctm_item(0, 2));
        assert!(result.is_err());
        assert_eq!(slot_header(&queue, TEST_MARKET).ctm_count, 1);
        assert_eq!(queue.global_header.total_count, 1);
    }

    #[test]
    fn clear_ctm_item_at_advances_across_front_gaps() {
        let mut queue = test_queue();
        queue.push_ctm(TEST_MARKET, pending_ctm_item(0, 1)).unwrap();
        queue.push_ctm(TEST_MARKET, pending_ctm_item(1, 2)).unwrap();
        queue.push_ctm(TEST_MARKET, pending_ctm_item(2, 3)).unwrap();

        queue.clear_ctm_item_at(TEST_MARKET, 1).unwrap();
        assert_eq!(slot_header(&queue, TEST_MARKET).next_sequence_to_execute, 0);
        assert_eq!(slot_header(&queue, TEST_MARKET).ctm_count, 2);
        assert_eq!(queue.global_header.total_count, 2);

        queue.clear_ctm_item_at(TEST_MARKET, 0).unwrap();
        assert_eq!(slot_header(&queue, TEST_MARKET).next_sequence_to_execute, 2);
        assert_eq!(slot_header(&queue, TEST_MARKET).ctm_count, 1);
        assert_eq!(queue.global_header.total_count, 1);
        assert_eq!(queue.current_ctm_head(TEST_MARKET).unwrap().sequence, 2);
    }

    #[test]
    fn find_matching_ctm_item_respects_hash_and_scan_limit() {
        let mut queue = test_queue();
        queue.push_ctm(TEST_MARKET, pending_ctm_item(0, 1)).unwrap();
        queue.push_ctm(TEST_MARKET, pending_ctm_item(3, 9)).unwrap();

        assert!(queue
            .find_matching_ctm_item(TEST_MARKET, &[9; 32], 2)
            .is_none());

        let (sequence, item) = queue
            .find_matching_ctm_item(TEST_MARKET, &[9; 32], 4)
            .unwrap();
        assert_eq!(sequence, 3);
        assert_eq!(item.accounts_hash, [9; 32]);
    }

    #[test]
    fn rotate_liquidity_head_with_retry_preserves_fifo_order() {
        let mut queue = test_queue();
        queue
            .push_liquidity(pending_liquidity_item(7, QueueItemKind::LiquidityDeposit))
            .unwrap();
        queue
            .push_liquidity(pending_liquidity_item(8, QueueItemKind::LiquidityWithdraw))
            .unwrap();

        queue.rotate_liquidity_head_with_retry(50, 3, 7).unwrap();

        let head = queue.liquidity_head_item().unwrap();
        assert_eq!(head.sequence, 8);
        assert_eq!(head.kind, QueueItemKind::LiquidityWithdraw as u8);

        let first = queue.pop_liquidity_head().unwrap();
        let second = queue.pop_liquidity_head().unwrap();
        assert_eq!(first.sequence, 8);
        assert_eq!(second.sequence, 7);
        assert_eq!(second.retries, 3);
        assert_eq!(second.first_failure_slot, 50);
        assert_eq!(second.min_execute_slot, 57);
    }

    #[test]
    fn push_ctm_rejects_sequence_outside_enqueue_window() {
        let mut queue = test_queue();
                                                                           
        queue.push_ctm(TEST_MARKET, pending_ctm_item(0, 1)).unwrap();
        queue
            .sub_queue_mut(TEST_MARKET)
            .unwrap()
            .next_sequence_to_execute = 11;

        let result = queue.push_ctm(TEST_MARKET, pending_ctm_item(10, 1));
        assert!(result.is_err());
    }

    #[test]
    fn clear_current_ctm_head_and_advance_updates_counts_and_resets_gap_slot() {
        let mut queue = test_queue();
        queue.push_ctm(TEST_MARKET, pending_ctm_item(0, 1)).unwrap();
        queue.push_ctm(TEST_MARKET, pending_ctm_item(1, 2)).unwrap();
        queue.sub_queue_mut(TEST_MARKET).unwrap().gap_observed_slot = 99;

        queue
            .clear_current_ctm_head_and_advance(TEST_MARKET)
            .unwrap();

        let header = slot_header(&queue, TEST_MARKET);
        assert_eq!(header.next_sequence_to_execute, 1);
        assert_eq!(header.ctm_count, 1);
        assert_eq!(queue.global_header.total_count, 1);
        assert_eq!(header.gap_observed_slot, 0);
        assert_eq!(queue.current_ctm_head(TEST_MARKET).unwrap().sequence, 1);
    }

    #[test]
    fn clear_ctm_item_at_stops_advancing_once_real_pending_head_found() {
        let mut queue = test_queue();
        queue.push_ctm(TEST_MARKET, pending_ctm_item(0, 1)).unwrap();
        queue.push_ctm(TEST_MARKET, pending_ctm_item(2, 2)).unwrap();
        queue.push_ctm(TEST_MARKET, pending_ctm_item(3, 3)).unwrap();

        queue.clear_ctm_item_at(TEST_MARKET, 0).unwrap();

        let header = slot_header(&queue, TEST_MARKET);
        assert_eq!(header.next_sequence_to_execute, 2);
        assert_eq!(header.ctm_count, 2);
        assert_eq!(queue.global_header.total_count, 2);
        assert_eq!(queue.current_ctm_head(TEST_MARKET).unwrap().sequence, 2);
    }

    #[test]
    fn mixed_ctm_and_liquidity_operations_preserve_header_totals() {
        let mut queue = test_queue();
        queue.push_ctm(TEST_MARKET, pending_ctm_item(0, 1)).unwrap();
        queue.push_ctm(TEST_MARKET, pending_ctm_item(1, 2)).unwrap();
        queue
            .push_liquidity(pending_liquidity_item(7, QueueItemKind::LiquidityDeposit))
            .unwrap();

        assert_eq!(
            queue.global_header.total_count,
            queue.total_ctm_count() + queue.global_header.liquidity_count
        );

        queue
            .clear_current_ctm_head_and_advance(TEST_MARKET)
            .unwrap();
        assert_eq!(
            queue.global_header.total_count,
            queue.total_ctm_count() + queue.global_header.liquidity_count
        );

        queue.pop_liquidity_head().unwrap();
        assert_eq!(
            queue.global_header.total_count,
            queue.total_ctm_count() + queue.global_header.liquidity_count
        );

        queue
            .clear_current_ctm_head_and_advance(TEST_MARKET)
            .unwrap();
        assert_eq!(queue.global_header.total_count, 0);
        assert_eq!(queue.total_ctm_count(), 0);
        assert_eq!(queue.global_header.liquidity_count, 0);
    }

                                       

    #[test]
    fn allocates_first_slot_on_demand() {
        let mut queue = test_queue();
        queue.push_ctm(5, pending_ctm_item(0, 1)).unwrap();
        let slot = queue.find_sub_queue_slot(5).unwrap();
        assert_eq!(slot, 0);
        assert_eq!(queue.sub_queue_headers[0].market_index, 5);
        assert_eq!(queue.sub_queue_headers[0].active, 1);
    }

    #[test]
    fn reuses_existing_slot_for_same_market() {
        let mut queue = test_queue();
        queue.push_ctm(7, pending_ctm_item(0, 1)).unwrap();
        queue.push_ctm(7, pending_ctm_item(1, 2)).unwrap();
        let slot = queue.find_sub_queue_slot(7).unwrap();
        assert_eq!(slot, 0);
        assert_eq!(queue.sub_queue_headers[0].ctm_count, 2);
    }

    #[test]
    fn slot_exhaustion_errors_on_seventeenth_market() {
        let mut queue = test_queue();
        for m in 0..EXECUTION_QUEUE_N_MAX_MARKETS as u16 {
            queue.push_ctm(m, pending_ctm_item(0, 1)).unwrap();
        }
        let result = queue.push_ctm(99, pending_ctm_item(0, 1));
        assert!(result.is_err());
    }

    #[test]
    fn push_ctm_market_a_does_not_affect_market_b_counts() {
        let mut queue = test_queue();
        queue.push_ctm(0, pending_ctm_item(0, 1)).unwrap();
        queue.push_ctm(0, pending_ctm_item(1, 2)).unwrap();
        queue.push_ctm(1, pending_ctm_item(0, 3)).unwrap();

        assert_eq!(slot_header(&queue, 0).ctm_count, 2);
        assert_eq!(slot_header(&queue, 1).ctm_count, 1);
        assert_eq!(queue.global_header.total_count, 3);
    }

    #[test]
    fn clear_ctm_item_at_market_a_leaves_market_b_head_intact() {
        let mut queue = test_queue();
        queue.push_ctm(0, pending_ctm_item(0, 1)).unwrap();
        queue.push_ctm(1, pending_ctm_item(0, 1)).unwrap();
        queue.push_ctm(1, pending_ctm_item(1, 2)).unwrap();

        queue.clear_ctm_item_at(0, 0).unwrap();

        assert_eq!(slot_header(&queue, 0).ctm_count, 0);
        assert_eq!(slot_header(&queue, 1).ctm_count, 2);
        assert_eq!(queue.current_ctm_head(1).unwrap().sequence, 0);
    }

    #[test]
    fn full_one_market_ring_does_not_block_another_market() {
        let mut queue = test_queue();
                        
        for i in 0..EXECUTION_QUEUE_PER_MARKET_CTM_CAPACITY as u64 {
            queue.push_ctm(0, pending_ctm_item(i, 1)).unwrap();
        }
                                      
        let r = queue.push_ctm(
            0,
            pending_ctm_item(EXECUTION_QUEUE_PER_MARKET_CTM_CAPACITY as u64, 1),
        );
        assert!(r.is_err());
                                         
        queue.push_ctm(1, pending_ctm_item(0, 1)).unwrap();
        assert_eq!(slot_header(&queue, 1).ctm_count, 1);
    }

    #[test]
    fn sequence_allocators_independent_per_market() {
        let mut queue = test_queue();
        queue.push_ctm(0, pending_ctm_item(0, 1)).unwrap();
        queue.push_ctm(1, pending_ctm_item(0, 1)).unwrap();
                                                                           
        assert_eq!(slot_header(&queue, 0).ctm_count, 1);
        assert_eq!(slot_header(&queue, 1).ctm_count, 1);
    }

    #[test]
    fn maybe_activate_pending_ctm_at_exact_slot() {
        let mut queue = test_queue();
        let new_signer = Pubkey::new_unique();
        queue.pending_ctm_signer = new_signer;
        queue.pending_ctm_activate_slot = 100;
        queue.maybe_activate_pending_ctm(100);
        assert_eq!(queue.ctm_signer, new_signer);
        assert_eq!(queue.pending_ctm_signer, Pubkey::default());
        assert_eq!(queue.pending_ctm_activate_slot, 0);
    }

    #[test]
    fn maybe_activate_pending_ctm_before_slot() {
        let mut queue = test_queue();
        let old_signer = queue.ctm_signer;
        let new_signer = Pubkey::new_unique();
        queue.pending_ctm_signer = new_signer;
        queue.pending_ctm_activate_slot = 100;
        queue.maybe_activate_pending_ctm(99);
        assert_eq!(queue.ctm_signer, old_signer);
        assert_eq!(queue.pending_ctm_signer, new_signer);
        assert_eq!(queue.pending_ctm_activate_slot, 100);
    }

    #[test]
    fn push_liquidity_fills_to_128_then_rejects() {
        let mut queue = test_queue();
        for i in 0..EXECUTION_QUEUE_LIQUIDITY_CAPACITY as u64 {
            queue
                .push_liquidity(pending_liquidity_item(i, QueueItemKind::LiquidityDeposit))
                .unwrap();
        }
        assert_eq!(
            queue.global_header.liquidity_count,
            EXECUTION_QUEUE_LIQUIDITY_CAPACITY as u32
        );
        let result = queue.push_liquidity(pending_liquidity_item(
            EXECUTION_QUEUE_LIQUIDITY_CAPACITY as u64,
            QueueItemKind::LiquidityDeposit,
        ));
        assert!(result.is_err());
    }

    #[test]
    fn liquidity_wraparound_head_and_tail() {
        let mut queue = test_queue();
        for i in 0..3u64 {
            queue
                .push_liquidity(pending_liquidity_item(i, QueueItemKind::LiquidityDeposit))
                .unwrap();
        }
        let first = queue.pop_liquidity_head().unwrap();
        let second = queue.pop_liquidity_head().unwrap();
        assert_eq!(first.sequence, 0);
        assert_eq!(second.sequence, 1);
        assert_eq!(queue.global_header.liquidity_head, 2);

        for i in 3..EXECUTION_QUEUE_LIQUIDITY_CAPACITY as u64 + 1 {
            queue
                .push_liquidity(pending_liquidity_item(i, QueueItemKind::LiquidityDeposit))
                .unwrap();
        }
        let head = queue.liquidity_head_item().unwrap();
        assert_eq!(head.sequence, 2);

        let mut prev_seq = 1u64;
        while queue.global_header.liquidity_count > 0 {
            let item = queue.pop_liquidity_head().unwrap();
            assert!(item.sequence > prev_seq);
            prev_seq = item.sequence;
        }
    }

    #[test]
    fn liquidity_tail_index_wraps() {
        let mut queue = test_queue();
        queue.global_header.liquidity_head = 120;
        queue.global_header.liquidity_count = 10;
        assert_eq!(queue.liquidity_tail_index(), 2);
    }
}
