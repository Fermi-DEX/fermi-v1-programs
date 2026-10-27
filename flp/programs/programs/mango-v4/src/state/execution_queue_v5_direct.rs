use crate::error::FermiError;
use crate::state::EXECUTION_QUEUE_PAYLOAD_MAX;
use anchor_lang::prelude::*;
use static_assertions::const_assert_eq;
use std::mem::size_of;

pub const EXECUTION_QUEUE_V5_DIRECT_LAYOUT_VERSION: u8 = 1;
                                                                               
                                                                        
                                                                           
                                                                          
                                                                           
                                                       
                                                                                
                                                                               
                                                        
pub const EXECUTION_QUEUE_V5_DIRECT_DEFAULT_SPEEDBUMP_SLOTS: u16 = 10;
pub const EXECUTION_QUEUE_V5_DIRECT_CAPACITY: usize = 256;
pub const EXECUTION_QUEUE_V5_DIRECT_ACCOUNT_SPACE: usize =
    8 + size_of::<ExecutionQueueV5DirectPool>();

#[repr(u8)]
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum DirectItemStatusV5 {
    Empty = 0,
    Staged = 1,
    Promoted = 2,
    Executed = 3,
    Failed = 4,
    Expired = 5,
}

#[zero_copy]
#[derive(Debug)]
pub struct DirectItemV5 {
    pub item_id: u64,
    pub promoted_sequence: u64,
    pub eligible_at_slot: u64,
    pub expires_at_slot: u64,
    pub ingress_slot: u64,
    pub direct_intent_hash: [u8; 32],
    pub payload_hash: [u8; 32],
    pub accounts_hash: [u8; 32],
    pub user_owner: Pubkey,
    pub fermi_account: Pubkey,
    pub payload_len: u16,
    pub dispatch_accounts_count: u16,
    pub kind: u8,
    pub status: u8,
    pub _padding0: [u8; 2],
    pub payload: [u8; EXECUTION_QUEUE_PAYLOAD_MAX],
    pub reserved: [u8; 16],
}
const_assert_eq!(size_of::<DirectItemV5>(), 480);
const_assert_eq!(size_of::<DirectItemV5>() % 8, 0);

impl Default for DirectItemV5 {
    fn default() -> Self {
        Self {
            item_id: 0,
            promoted_sequence: 0,
            eligible_at_slot: 0,
            expires_at_slot: 0,
            ingress_slot: 0,
            direct_intent_hash: [0; 32],
            payload_hash: [0; 32],
            accounts_hash: [0; 32],
            user_owner: Pubkey::default(),
            fermi_account: Pubkey::default(),
            payload_len: 0,
            dispatch_accounts_count: 0,
            kind: 0,
            status: DirectItemStatusV5::Empty as u8,
            _padding0: [0; 2],
            payload: [0; EXECUTION_QUEUE_PAYLOAD_MAX],
            reserved: [0; 16],
        }
    }
}

#[zero_copy]
#[derive(Debug)]
pub struct ExecutionQueueV5DirectHeader {
    pub group: Pubkey,
    pub authority_state: Pubkey,
    pub queue: Pubkey,
    pub market_index: u16,
    pub bump: u8,
    pub paused_ingress: u8,
    pub layout_version: u8,
    pub _padding0: [u8; 1],
    pub speedbump_slots: u16,
    pub live_count: u32,
    pub _padding1: [u8; 4],
    pub next_item_id: u64,
    pub next_item_to_promote: u64,
    pub next_item_to_retire: u64,
    pub reserved: [u8; 56],
}
const_assert_eq!(size_of::<ExecutionQueueV5DirectHeader>(), 192);
const_assert_eq!(size_of::<ExecutionQueueV5DirectHeader>() % 8, 0);

#[account(zero_copy)]
#[derive(Debug)]
pub struct ExecutionQueueV5DirectPool {
    pub header: ExecutionQueueV5DirectHeader,
    pub items: [DirectItemV5; EXECUTION_QUEUE_V5_DIRECT_CAPACITY],
}
const_assert_eq!(size_of::<ExecutionQueueV5DirectPool>() % 8, 0);

impl ExecutionQueueV5DirectPool {
    pub fn init(
        &mut self,
        group: Pubkey,
        authority_state: Pubkey,
        queue: Pubkey,
        market_index: u16,
        bump: u8,
    ) {
        self.header.group = group;
        self.header.authority_state = authority_state;
        self.header.queue = queue;
        self.header.market_index = market_index;
        self.header.bump = bump;
        self.header.paused_ingress = 1;
        self.header.layout_version = EXECUTION_QUEUE_V5_DIRECT_LAYOUT_VERSION;
        self.header._padding0 = [0; 1];
        self.header.speedbump_slots = EXECUTION_QUEUE_V5_DIRECT_DEFAULT_SPEEDBUMP_SLOTS;
        self.header.live_count = 0;
        self.header._padding1 = [0; 4];
        self.header.next_item_id = 0;
        self.header.next_item_to_promote = 0;
        self.header.next_item_to_retire = 0;
        self.header.reserved = [0; 56];
        for item in self.items.iter_mut() {
            *item = DirectItemV5::default();
        }
    }

    pub fn capacity(&self) -> u64 {
        EXECUTION_QUEUE_V5_DIRECT_CAPACITY as u64
    }

    pub fn active_len(&self) -> u64 {
        self.header
            .next_item_id
            .saturating_sub(self.header.next_item_to_retire)
    }

    pub fn can_enqueue(&self) -> bool {
        self.active_len() < self.capacity()
    }

    pub fn physical_index(item_id: u64) -> usize {
        (item_id % EXECUTION_QUEUE_V5_DIRECT_CAPACITY as u64) as usize
    }

    pub fn get_item(&self, item_id: u64) -> Option<&DirectItemV5> {
        let phys = Self::physical_index(item_id);
        let item = &self.items[phys];
        if item.status == DirectItemStatusV5::Empty as u8 || item.item_id != item_id {
            None
        } else {
            Some(item)
        }
    }

    pub fn get_item_mut(&mut self, item_id: u64) -> Option<&mut DirectItemV5> {
        let phys = Self::physical_index(item_id);
        let item = &mut self.items[phys];
        if item.status == DirectItemStatusV5::Empty as u8 || item.item_id != item_id {
            None
        } else {
            Some(item)
        }
    }

    pub fn write_staged(&mut self, mut item: DirectItemV5) -> Result<u64> {
        require!(self.can_enqueue(), FermiError::ExecutionQueueFull);
        let item_id = self.header.next_item_id;
        let phys = Self::physical_index(item_id);
        require!(
            self.items[phys].status == DirectItemStatusV5::Empty as u8,
            FermiError::ExecutionQueueFull
        );
        item.item_id = item_id;
        item.promoted_sequence = 0;
        item.status = DirectItemStatusV5::Staged as u8;
        self.items[phys] = item;
        self.header.next_item_id = self.header.next_item_id.saturating_add(1);
        self.header.live_count = self.header.live_count.saturating_add(1);
        Ok(item_id)
    }

    pub fn staged_head(&self) -> Option<&DirectItemV5> {
        let item = self.get_item(self.header.next_item_to_promote)?;
        if item.status == DirectItemStatusV5::Staged as u8 {
            Some(item)
        } else {
            None
        }
    }

    pub fn mark_promoted(&mut self, item_id: u64, sequence: u64) -> Result<()> {
        let item = self
            .get_item_mut(item_id)
            .ok_or_else(|| error!(FermiError::ExecutionQueueSequenceNotPending))?;
        require!(
            item.status == DirectItemStatusV5::Staged as u8,
            FermiError::ExecutionQueueSequenceNotPending
        );
        item.promoted_sequence = sequence;
        item.status = DirectItemStatusV5::Promoted as u8;
        self.header.next_item_to_promote = item_id.saturating_add(1);
        Ok(())
    }

    pub fn mark_terminal(
        &mut self,
        item_id: u64,
        terminal_status: DirectItemStatusV5,
    ) -> Result<()> {
        require!(
            matches!(
                terminal_status,
                DirectItemStatusV5::Executed
                    | DirectItemStatusV5::Failed
                    | DirectItemStatusV5::Expired
            ),
            FermiError::SomeError
        );
        let item = self
            .get_item_mut(item_id)
            .ok_or_else(|| error!(FermiError::ExecutionQueueSequenceNotPending))?;
        require!(
            item.status != DirectItemStatusV5::Empty as u8,
            FermiError::ExecutionQueueSequenceNotPending
        );
        item.status = terminal_status as u8;
        self.retire_prefix();
        Ok(())
    }

    pub fn retire_prefix(&mut self) {
        loop {
            let item_id = self.header.next_item_to_retire;
            let Some(item) = self.get_item(item_id) else {
                break;
            };
            let terminal = matches!(
                item.status,
                x if x == DirectItemStatusV5::Executed as u8
                    || x == DirectItemStatusV5::Failed as u8
                    || x == DirectItemStatusV5::Expired as u8
            );
            if !terminal {
                break;
            }
            let phys = Self::physical_index(item_id);
            self.items[phys] = DirectItemV5::default();
            self.header.next_item_to_retire = item_id.saturating_add(1);
            self.header.live_count = self.header.live_count.saturating_sub(1);
        }
    }
}

pub const EXECUTION_QUEUE_V5_DIRECT_HEADER_OFFSET: usize = 8;
pub const EXECUTION_QUEUE_V5_DIRECT_ITEMS_OFFSET: usize =
    EXECUTION_QUEUE_V5_DIRECT_HEADER_OFFSET + size_of::<ExecutionQueueV5DirectHeader>();
pub const EXECUTION_QUEUE_V5_DIRECT_ITEM_STRIDE: usize = size_of::<DirectItemV5>();

#[cfg(all(test, feature = "legacy-unit-tests"))]
mod tests {
    use super::*;

    #[test]
    fn direct_item_v5_size() {
        assert_eq!(size_of::<DirectItemV5>(), 480);
    }

    #[test]
    fn direct_pool_header_size() {
        assert_eq!(size_of::<ExecutionQueueV5DirectHeader>(), 192);
    }

    #[test]
    fn direct_pool_default_speedbump_is_conservative() {
        let mut pool: Box<ExecutionQueueV5DirectPool> = unsafe { Box::new(std::mem::zeroed()) };
        pool.init(
            Pubkey::new_unique(),
            Pubkey::new_unique(),
            Pubkey::new_unique(),
            7,
            1,
        );
        assert_eq!(
            pool.header.speedbump_slots,
            EXECUTION_QUEUE_V5_DIRECT_DEFAULT_SPEEDBUMP_SLOTS
        );
        assert!(pool.header.speedbump_slots >= 10);
    }

    #[test]
    fn direct_pool_stage_promote_retire_cycle() {
        let mut pool: Box<ExecutionQueueV5DirectPool> = unsafe { Box::new(std::mem::zeroed()) };
        let group = Pubkey::new_unique();
        let authority_state = Pubkey::new_unique();
        let queue = Pubkey::new_unique();
        pool.init(group, authority_state, queue, 7, 1);
        pool.header.paused_ingress = 0;

        let mut item = DirectItemV5::default();
        item.kind = 1;
        item.payload_len = 3;
        item.payload[..3].copy_from_slice(&[1, 2, 3]);
        item.eligible_at_slot = 10;
        let item_id = pool.write_staged(item).unwrap();
        assert_eq!(item_id, 0);
        assert_eq!(pool.header.live_count, 1);

        pool.mark_promoted(item_id, 42).unwrap();
        let promoted = pool.get_item(item_id).unwrap();
        assert_eq!(promoted.promoted_sequence, 42);
        assert_eq!(promoted.status, DirectItemStatusV5::Promoted as u8);
        assert_eq!(pool.header.next_item_to_promote, 1);

        pool.mark_terminal(item_id, DirectItemStatusV5::Executed)
            .unwrap();
        assert_eq!(pool.header.live_count, 0);
        assert_eq!(pool.header.next_item_to_retire, 1);
        assert!(pool.get_item(item_id).is_none());
    }
}
