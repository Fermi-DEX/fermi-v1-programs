use std::cell::{Ref, RefMut};
use std::mem::size_of;

use anchor_lang::prelude::*;
use anchor_lang::Discriminator;
use arrayref::array_ref;
use derivative::Derivative;

use fixed::types::I80F48;

use solana_program::program_memory::sol_memmove;
use static_assertions::const_assert_eq;

use crate::error::*;
use crate::health::{HealthCache, HealthType};
use crate::logs::{emit_stack, DeactivatePerpPositionLog, DeactivateTokenPositionLog};
use crate::util;

use super::FillEvent;
use super::LeafNode;
use super::PerpMarket;
use super::PerpMarketIndex;
use super::PerpOpenOrder;
use super::Serum3MarketIndex;
use super::TokenConditionalSwap;
use super::TokenIndex;
use super::FREE_ORDER_SLOT;
use super::{dynamic_account::*, Group};
use super::{BookSideOrderTree, OpenbookV2MarketIndex, OpenbookV2Orders};
use super::{PerpPosition, Serum3Orders, TokenPosition};
use super::{Side, SideAndOrderTree};

type BorshVecLength = u32;
const BORSH_VEC_PADDING_BYTES: usize = 4;
const BORSH_VEC_SIZE_BYTES: usize = 4;
const DEFAULT_FERMI_ACCOUNT_VERSION: u8 = 1;
const DYNAMIC_RESERVED_BYTES: usize = 56;

                                                                               
                                                 
#[derive(PartialEq)]
pub enum CheckLiquidatable {
    NotLiquidatable,
    Liquidatable,
    BecameNotLiquidatable,
}

pub struct FermiAccountPdaSeeds {
    pub group: Pubkey,
    pub owner: Pubkey,
    pub account_num_bytes: [u8; 4],
    pub bump_bytes: [u8; 1],
}

impl FermiAccountPdaSeeds {
    pub fn signer_seeds(&self) -> [&[u8]; 5] {
        [
            b"FermiAccount".as_ref(),
            self.group.as_ref(),
            self.owner.as_ref(),
            &self.account_num_bytes,
            &self.bump_bytes,
        ]
    }
}

                
                                                                                                         
                                                            
                                                                 
  
                                                      
                                                                            
                                                                                                  
                  
                                                                                             
                                                               
                                                                                    
                                                                                  
                                                                           
                                                 
  
                                                                                                      
              
  
                                                                                                            
                                                                                         
#[account]
#[derive(Derivative, PartialEq)]
#[derivative(Debug)]
pub struct FermiAccount {
            
                                                             
                                                  
    pub group: Pubkey,

                                                   
    pub owner: Pubkey,

    #[derivative(Debug(format_with = "util::format_zero_terminated_utf8_bytes"))]
    pub name: [u8; 32],

                                                                       
    pub delegate: Pubkey,

    pub account_num: u32,

                                                                             
       
                                                                                            
                                                                                             
                                                                                
       
                                                                                      
                                                                                                
                            
    pub being_liquidated: u8,

                                                                                                      
       
                                                   
    pub in_health_region: u8,

    pub bump: u8,

    pub sequence_number: u8,

                     
                                        
                                                           
                                   
    pub net_deposits: i64,
                     
                                                       
    pub perp_spot_transfers: i64,

                                                                      
    pub health_region_begin_init_health: i64,

    pub frozen_until: u64,

                                                    
                                                                         
    pub buyback_fees_accrued_current: u64,
                                                              
    pub buyback_fees_accrued_previous: u64,
                                                                                
    pub buyback_fees_expiry_timestamp: u64,

                                                         
    pub next_token_conditional_swap_id: u64,

    pub temporary_delegate: Pubkey,
    pub temporary_delegate_expiry: u64,

                                                         
    pub last_collateral_fee_charge: u64,

    #[derivative(Debug = "ignore")]
    pub reserved: [u8; 152],

              
    pub header_version: u8,
    #[derivative(Debug = "ignore")]
    pub padding3: [u8; 7],
                                                                      
    #[derivative(Debug = "ignore")]
    pub padding4: u32,
                                                                
                                           
    pub tokens: Vec<TokenPosition>,
    #[derivative(Debug = "ignore")]
    pub padding5: u32,
                                                                   
                                           
    pub serum3: Vec<Serum3Orders>,
    #[derivative(Debug = "ignore")]
    pub padding6: u32,
    pub perps: Vec<PerpPosition>,
    #[derivative(Debug = "ignore")]
    pub padding7: u32,
    pub perp_open_orders: Vec<PerpOpenOrder>,
    #[derivative(Debug = "ignore")]
    pub padding8: u32,
    pub token_conditional_swaps: Vec<TokenConditionalSwap>,
    #[derivative(Debug = "ignore")]
    pub padding9: u32,
    pub openbook_v2: Vec<OpenbookV2Orders>,

    #[derivative(Debug = "ignore")]
    pub reserved_dynamic: [u8; 56],
}

impl FermiAccount {
    pub fn default_for_tests() -> Self {
        Self {
            name: Default::default(),
            group: Pubkey::default(),
            owner: Pubkey::default(),
            delegate: Pubkey::default(),
            being_liquidated: 0,
            in_health_region: 0,
            account_num: 0,
            bump: 0,
            sequence_number: 0,
            net_deposits: 0,
            perp_spot_transfers: 0,
            health_region_begin_init_health: 0,
            frozen_until: 0,
            buyback_fees_accrued_current: 0,
            buyback_fees_accrued_previous: 0,
            buyback_fees_expiry_timestamp: 0,
            next_token_conditional_swap_id: 0,
            temporary_delegate: Pubkey::default(),
            temporary_delegate_expiry: 0,
            last_collateral_fee_charge: 0,
            reserved: [0; 152],
            header_version: DEFAULT_FERMI_ACCOUNT_VERSION,
            padding3: Default::default(),
            padding4: Default::default(),
            tokens: vec![TokenPosition::default(); 3],
            padding5: Default::default(),
            serum3: vec![Serum3Orders::default(); 5],
            padding6: Default::default(),
            perps: vec![PerpPosition::default(); 4],
            padding7: Default::default(),
            perp_open_orders: vec![PerpOpenOrder::default(); 6],
            padding8: Default::default(),
            token_conditional_swaps: vec![TokenConditionalSwap::default(); 2],
            padding9: Default::default(),
            openbook_v2: vec![OpenbookV2Orders::default(); 5],
            reserved_dynamic: [0; 56],
        }
    }

                                                                                
    pub fn space(
        token_count: u8,
        serum3_count: u8,
        perp_count: u8,
        perp_oo_count: u8,
        token_conditional_swap_count: u8,
        openbook_v2_count: u8,
    ) -> usize {
        8 + size_of::<FermiAccountFixed>()
            + Self::dynamic_size(
                token_count,
                serum3_count,
                perp_count,
                perp_oo_count,
                token_conditional_swap_count,
                openbook_v2_count,
            )
    }

    pub fn dynamic_token_vec_offset() -> usize {
        8                            
            + BORSH_VEC_PADDING_BYTES
    }

    pub fn dynamic_serum3_vec_offset(token_count: u8) -> usize {
        Self::dynamic_token_vec_offset()
            + (BORSH_VEC_SIZE_BYTES + size_of::<TokenPosition>() * usize::from(token_count))
            + BORSH_VEC_PADDING_BYTES
    }

    pub fn dynamic_perp_vec_offset(token_count: u8, serum3_count: u8) -> usize {
        Self::dynamic_serum3_vec_offset(token_count)
            + (BORSH_VEC_SIZE_BYTES + size_of::<Serum3Orders>() * usize::from(serum3_count))
            + BORSH_VEC_PADDING_BYTES
    }

    pub fn dynamic_perp_oo_vec_offset(token_count: u8, serum3_count: u8, perp_count: u8) -> usize {
        Self::dynamic_perp_vec_offset(token_count, serum3_count)
            + (BORSH_VEC_SIZE_BYTES + size_of::<PerpPosition>() * usize::from(perp_count))
            + BORSH_VEC_PADDING_BYTES
    }

    pub fn dynamic_token_conditional_swap_vec_offset(
        token_count: u8,
        serum3_count: u8,
        perp_count: u8,
        perp_oo_count: u8,
    ) -> usize {
        Self::dynamic_perp_oo_vec_offset(token_count, serum3_count, perp_count)
            + (BORSH_VEC_SIZE_BYTES + size_of::<PerpOpenOrder>() * usize::from(perp_oo_count))
            + BORSH_VEC_PADDING_BYTES
    }

    pub fn dynamic_openbook_v2_vec_offset(
        token_count: u8,
        serum3_count: u8,
        perp_count: u8,
        perp_oo_count: u8,
        token_conditional_swap_count: u8,
    ) -> usize {
        Self::dynamic_token_conditional_swap_vec_offset(
            token_count,
            serum3_count,
            perp_count,
            perp_oo_count,
        ) + (BORSH_VEC_SIZE_BYTES
            + size_of::<TokenConditionalSwap>() * usize::from(token_conditional_swap_count))
            + BORSH_VEC_PADDING_BYTES
    }

    pub fn dynamic_reserved_bytes_offset(
        token_count: u8,
        serum3_count: u8,
        perp_count: u8,
        perp_oo_count: u8,
        token_conditional_swap_count: u8,
        openbook_v2_count: u8,
    ) -> usize {
        Self::dynamic_openbook_v2_vec_offset(
            token_count,
            serum3_count,
            perp_count,
            perp_oo_count,
            token_conditional_swap_count,
        ) + (BORSH_VEC_SIZE_BYTES + size_of::<OpenbookV2Orders>() * usize::from(openbook_v2_count))
    }

    pub fn dynamic_size(
        token_count: u8,
        serum3_count: u8,
        perp_count: u8,
        perp_oo_count: u8,
        token_conditional_swap_count: u8,
        openbook_v2_count: u8,
    ) -> usize {
        Self::dynamic_reserved_bytes_offset(
            token_count,
            serum3_count,
            perp_count,
            perp_oo_count,
            token_conditional_swap_count,
            openbook_v2_count,
        ) + DYNAMIC_RESERVED_BYTES
    }
}

                                                              
#[zero_copy]
pub struct FermiAccountFixed {
    pub group: Pubkey,
    pub owner: Pubkey,
    pub name: [u8; 32],
    pub delegate: Pubkey,
    pub account_num: u32,
    being_liquidated: u8,
    in_health_region: u8,
    pub bump: u8,
    pub sequence_number: u8,
    pub net_deposits: i64,
    pub perp_spot_transfers: i64,
    pub health_region_begin_init_health: i64,
    pub frozen_until: u64,
    pub buyback_fees_accrued_current: u64,
    pub buyback_fees_accrued_previous: u64,
    pub buyback_fees_expiry_timestamp: u64,
    pub next_token_conditional_swap_id: u64,
    pub temporary_delegate: Pubkey,
    pub temporary_delegate_expiry: u64,
    pub last_collateral_fee_charge: u64,
    pub reserved: [u8; 152],
}
const_assert_eq!(
    size_of::<FermiAccountFixed>(),
    32 * 4 + 8 + 8 * 8 + 32 + 8 + 8 + 152
);
const_assert_eq!(size_of::<FermiAccountFixed>(), 400);
const_assert_eq!(size_of::<FermiAccountFixed>() % 8, 0);

impl FermiAccountFixed {
    pub fn name(&self) -> &str {
        std::str::from_utf8(&self.name)
            .unwrap()
            .trim_matches(char::from(0))
    }

    pub fn is_operational(&self) -> bool {
        let now_ts: u64 = Clock::get().unwrap().unix_timestamp.try_into().unwrap();
        self.frozen_until < now_ts
    }

    pub fn is_owner_or_delegate(&self, ix_signer: Pubkey) -> bool {
        self.owner == ix_signer || self.is_delegate(ix_signer)
    }

    pub fn is_delegate(&self, ix_signer: Pubkey) -> bool {
        if self.delegate == ix_signer {
            return true;
        }

        let now_ts: u64 = Clock::get().unwrap().unix_timestamp.try_into().unwrap();
        if now_ts > self.temporary_delegate_expiry {
            return false;
        }

        self.temporary_delegate == ix_signer
    }

    pub fn being_liquidated(&self) -> bool {
        self.being_liquidated == 1
    }

    pub fn set_being_liquidated(&mut self, b: bool) {
        self.being_liquidated = u8::from(b);
    }

    pub fn is_in_health_region(&self) -> bool {
        self.in_health_region == 1
    }

    pub fn set_in_health_region(&mut self, b: bool) {
        self.in_health_region = u8::from(b);
    }

    pub fn maybe_recover_from_being_liquidated(&mut self, liq_end_health: I80F48) -> bool {
                                                                                     
        let one_native_usdc = I80F48::ONE;
        if self.being_liquidated() && liq_end_health > -one_native_usdc {
            self.set_being_liquidated(false);
            true
        } else {
            false
        }
    }

                                                                                    
    pub fn expire_buyback_fees(&mut self, now_ts: u64, interval: u64) {
        if interval == 0 || now_ts < self.buyback_fees_expiry_timestamp {
            return;
        } else if now_ts < self.buyback_fees_expiry_timestamp + interval {
            self.buyback_fees_accrued_previous = self.buyback_fees_accrued_current;
        } else {
            self.buyback_fees_accrued_previous = 0;
        }
        self.buyback_fees_accrued_current = 0;
        self.buyback_fees_expiry_timestamp = (now_ts / interval + 1) * interval;
    }

                                                                       
    pub fn buyback_fees_accrued(&self) -> u64 {
        self.buyback_fees_accrued_current
            .saturating_add(self.buyback_fees_accrued_previous)
    }

                                                                   
       
                                                                                     
                                
    pub fn accrue_buyback_fees(&mut self, amount: u64) {
        self.buyback_fees_accrued_current =
            self.buyback_fees_accrued_current.saturating_add(amount);
    }

                                                                        
       
                                                               
    pub fn reduce_buyback_fees_accrued(&mut self, amount: u64) {
        if amount > self.buyback_fees_accrued_previous {
            let remaining_amount = amount - self.buyback_fees_accrued_previous;
            assert!(remaining_amount <= self.buyback_fees_accrued_current);
            self.buyback_fees_accrued_current -= remaining_amount;
            self.buyback_fees_accrued_previous = 0;
        } else {
            self.buyback_fees_accrued_previous -= amount;
        }
    }

    pub fn pda_seeds(&self) -> FermiAccountPdaSeeds {
        FermiAccountPdaSeeds {
            group: self.group,
            owner: self.owner,
            account_num_bytes: self.account_num.to_le_bytes(),
            bump_bytes: [self.bump],
        }
    }
}

impl Owner for FermiAccountFixed {
    fn owner() -> Pubkey {
        FermiAccount::owner()
    }
}

impl Discriminator for FermiAccountFixed {
    const DISCRIMINATOR: [u8; 8] = FermiAccount::DISCRIMINATOR;
}

impl anchor_lang::ZeroCopy for FermiAccountFixed {}

#[derive(Clone, Debug)]
pub struct FermiAccountDynamicHeader {
    pub token_count: u8,
    pub serum3_count: u8,
    pub perp_count: u8,
    pub perp_oo_count: u8,
    pub token_conditional_swap_count: u8,
    pub openbook_v2_count: u8,
}

impl DynamicHeader for FermiAccountDynamicHeader {
    fn from_bytes(dynamic_data: &[u8]) -> Result<Self> {
        let header_version = u8::from_le_bytes(*array_ref![dynamic_data, 0, size_of::<u8>()]);

        match header_version {
            1 => {
                let token_count = u8::try_from(BorshVecLength::from_le_bytes(*array_ref![
                    dynamic_data,
                    FermiAccount::dynamic_token_vec_offset(),
                    BORSH_VEC_SIZE_BYTES
                ]))
                .unwrap();

                let serum3_count = u8::try_from(BorshVecLength::from_le_bytes(*array_ref![
                    dynamic_data,
                    FermiAccount::dynamic_serum3_vec_offset(token_count),
                    BORSH_VEC_SIZE_BYTES
                ]))
                .unwrap();

                let perp_count = u8::try_from(BorshVecLength::from_le_bytes(*array_ref![
                    dynamic_data,
                    FermiAccount::dynamic_perp_vec_offset(token_count, serum3_count),
                    BORSH_VEC_SIZE_BYTES
                ]))
                .unwrap();

                let perp_oo_count = u8::try_from(BorshVecLength::from_le_bytes(*array_ref![
                    dynamic_data,
                    FermiAccount::dynamic_perp_oo_vec_offset(token_count, serum3_count, perp_count),
                    BORSH_VEC_SIZE_BYTES
                ]))
                .unwrap();

                let token_conditional_swap_vec_offset =
                    FermiAccount::dynamic_token_conditional_swap_vec_offset(
                        token_count,
                        serum3_count,
                        perp_count,
                        perp_oo_count,
                    );
                let token_conditional_swap_count =
                    u8::try_from(BorshVecLength::from_le_bytes(*array_ref![
                        dynamic_data,
                        token_conditional_swap_vec_offset,
                        BORSH_VEC_SIZE_BYTES
                    ]))
                    .unwrap();

                let openbook_v2_vec_offset = FermiAccount::dynamic_openbook_v2_vec_offset(
                    token_count,
                    serum3_count,
                    perp_count,
                    perp_oo_count,
                    token_conditional_swap_count,
                );
                let openbook_v2_count = u8::try_from(BorshVecLength::from_le_bytes(*array_ref![
                    dynamic_data,
                    openbook_v2_vec_offset,
                    BORSH_VEC_SIZE_BYTES
                ]))
                .unwrap();

                Ok(Self {
                    token_count,
                    serum3_count,
                    perp_count,
                    perp_oo_count,
                    token_conditional_swap_count,
                    openbook_v2_count,
                })
            }
            _ => err!(FermiError::NotImplementedError).context("unexpected header version number"),
        }
    }

    fn initialize(dynamic_data: &mut [u8]) -> Result<()> {
        let dst: &mut [u8] = &mut dynamic_data[0..1];
        dst.copy_from_slice(&DEFAULT_FERMI_ACCOUNT_VERSION.to_le_bytes());
        Ok(())
    }
}

fn get_helper<T: bytemuck::Pod>(data: &[u8], index: usize) -> &T {
    bytemuck::from_bytes(&data[index..index + size_of::<T>()])
}

fn get_helper_mut<T: bytemuck::Pod>(data: &mut [u8], index: usize) -> &mut T {
    bytemuck::from_bytes_mut(&mut data[index..index + size_of::<T>()])
}

impl FermiAccountDynamicHeader {
    pub fn account_size(&self) -> usize {
        FermiAccount::space(
            self.token_count,
            self.serum3_count,
            self.perp_count,
            self.perp_oo_count,
            self.token_conditional_swap_count,
            self.openbook_v2_count,
        )
    }

                                                                      
                           
    pub fn token_offset(&self, raw_index: usize) -> usize {
        FermiAccount::dynamic_token_vec_offset()
            + BORSH_VEC_SIZE_BYTES
            + raw_index * size_of::<TokenPosition>()
    }

                                                                     
                           
    pub fn serum3_offset(&self, raw_index: usize) -> usize {
        FermiAccount::dynamic_serum3_vec_offset(self.token_count)
            + BORSH_VEC_SIZE_BYTES
            + raw_index * size_of::<Serum3Orders>()
    }

                                                                     
    pub fn perp_offset(&self, raw_index: usize) -> usize {
        FermiAccount::dynamic_perp_vec_offset(self.token_count, self.serum3_count)
            + BORSH_VEC_SIZE_BYTES
            + raw_index * size_of::<PerpPosition>()
    }

    fn perp_oo_offset(&self, raw_index: usize) -> usize {
        FermiAccount::dynamic_perp_oo_vec_offset(
            self.token_count,
            self.serum3_count,
            self.perp_count,
        ) + BORSH_VEC_SIZE_BYTES
            + raw_index * size_of::<PerpOpenOrder>()
    }

    fn token_conditional_swap_offset(&self, raw_index: usize) -> usize {
        FermiAccount::dynamic_token_conditional_swap_vec_offset(
            self.token_count,
            self.serum3_count,
            self.perp_count,
            self.perp_oo_count,
        ) + BORSH_VEC_SIZE_BYTES
            + raw_index * size_of::<TokenConditionalSwap>()
    }

    fn openbook_v2_offset(&self, raw_index: usize) -> usize {
        FermiAccount::dynamic_openbook_v2_vec_offset(
            self.token_count,
            self.serum3_count,
            self.perp_count,
            self.perp_oo_count,
            self.token_conditional_swap_count,
        ) + BORSH_VEC_SIZE_BYTES
            + raw_index * size_of::<OpenbookV2Orders>()
    }

    fn reserved_bytes_offset(&self) -> usize {
        FermiAccount::dynamic_reserved_bytes_offset(
            self.token_count,
            self.serum3_count,
            self.perp_count,
            self.perp_oo_count,
            self.token_conditional_swap_count,
            self.openbook_v2_count,
        )
    }

    pub fn token_count(&self) -> usize {
        self.token_count.into()
    }
    pub fn serum3_count(&self) -> usize {
        self.serum3_count.into()
    }
    pub fn perp_count(&self) -> usize {
        self.perp_count.into()
    }
    pub fn perp_oo_count(&self) -> usize {
        self.perp_oo_count.into()
    }
    pub fn token_conditional_swap_count(&self) -> usize {
        self.token_conditional_swap_count.into()
    }
    pub fn openbook_v2_count(&self) -> usize {
        self.openbook_v2_count.into()
    }

    pub fn zero() -> Self {
        Self {
            token_count: 0,
            serum3_count: 0,
            perp_count: 0,
            perp_oo_count: 0,
            token_conditional_swap_count: 0,
            openbook_v2_count: 0,
        }
    }

    pub fn expected_health_accounts(&self) -> usize {
        self.token_count() * 2
            + self.serum3_count()
            + self.perp_count() * 2
            + self.openbook_v2_count()
    }

    pub fn max_health_accounts() -> usize {
        28
    }

                                                             
       
                                                           
                                                            
                                                                               
                                                                         
                                                                    
    pub fn check_resize_from(&self, prev: &Self) -> Result<()> {
        #[cfg(not(feature = "legacy-spot"))]
        {
            require_msg_typed!(
                self.serum3_count == 0,
                FermiError::SomeError,
                "serum3 spot account slots are disabled in this build"
            );
            require_msg_typed!(
                self.openbook_v2_count == 0,
                FermiError::SomeError,
                "openbook v2 spot account slots are disabled in this build"
            );
        }
        #[cfg(not(feature = "token-conditional-swaps"))]
        require_msg_typed!(
            self.token_conditional_swap_count == 0,
            FermiError::SomeError,
            "token conditional swap account slots are disabled in this build"
        );

        let new_health_accounts = self.expected_health_accounts();
        let prev_health_accounts = prev.expected_health_accounts();
        if new_health_accounts > prev_health_accounts {
            require_gte!(Self::max_health_accounts(), new_health_accounts);
        }

        if self.perp_oo_count > prev.perp_oo_count {
            require_gte!(64, self.perp_oo_count);
        }

        if self.token_conditional_swap_count > prev.token_conditional_swap_count {
            require_gte!(64, self.token_conditional_swap_count);
        }

        Ok(())
    }
}

                                              
pub type FermiAccountValue = DynamicAccount<FermiAccountDynamicHeader, FermiAccountFixed, Vec<u8>>;

                                           
pub type FermiAccountRef<'a> =
    DynamicAccount<&'a FermiAccountDynamicHeader, &'a FermiAccountFixed, &'a [u8]>;
                                           
pub type FermiAccountRefMut<'a> =
    DynamicAccount<&'a mut FermiAccountDynamicHeader, &'a mut FermiAccountFixed, &'a mut [u8]>;

                                  
pub type FermiAccountLoadedRef<'a> =
    DynamicAccount<FermiAccountDynamicHeader, &'a FermiAccountFixed, &'a [u8]>;
                                                           
pub type FermiAccountLoadedRefCell<'a> =
    DynamicAccount<FermiAccountDynamicHeader, Ref<'a, FermiAccountFixed>, Ref<'a, [u8]>>;
                                                           
pub type FermiAccountLoadedRefCellMut<'a> =
    DynamicAccount<FermiAccountDynamicHeader, RefMut<'a, FermiAccountFixed>, RefMut<'a, [u8]>>;

impl FermiAccountValue {
                                  
    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        let (fixed, dynamic) = bytes.split_at(size_of::<FermiAccountFixed>());
        Ok(Self {
            fixed: *bytemuck::from_bytes(fixed),
            header: FermiAccountDynamicHeader::from_bytes(dynamic)?,
            dynamic: dynamic.to_vec(),
        })
    }
}

impl<'a> FermiAccountLoadedRef<'a> {
                                  
    pub fn from_bytes(bytes: &'a [u8]) -> Result<Self> {
        let (fixed, dynamic) = bytes.split_at(size_of::<FermiAccountFixed>());
        Ok(Self {
            fixed: bytemuck::from_bytes(fixed),
            header: FermiAccountDynamicHeader::from_bytes(dynamic)?,
            dynamic,
        })
    }
}

                                                                         
                                                  
impl<
        Header: DerefOrBorrow<FermiAccountDynamicHeader>,
        Fixed: DerefOrBorrow<FermiAccountFixed>,
        Dynamic: DerefOrBorrow<[u8]>,
    > DynamicAccount<Header, Fixed, Dynamic>
{
    fn header(&self) -> &FermiAccountDynamicHeader {
        self.header.deref_or_borrow()
    }

    pub fn header_version(&self) -> &u8 {
        get_helper(self.dynamic(), 0)
    }

    fn fixed(&self) -> &FermiAccountFixed {
        self.fixed.deref_or_borrow()
    }

    fn dynamic(&self) -> &[u8] {
        self.dynamic.deref_or_borrow()
    }

    #[allow(dead_code)]
    fn dynamic_reserved_bytes(&self) -> &[u8] {
        let reserved_offset = self.header().reserved_bytes_offset();
        &self.dynamic()[reserved_offset..reserved_offset + DYNAMIC_RESERVED_BYTES]
    }

               
                      
                                                                                       
    pub fn token_position_and_raw_index(
        &self,
        token_index: TokenIndex,
    ) -> Result<(&TokenPosition, usize)> {
        self.all_token_positions()
            .enumerate()
            .find_map(|(raw_index, p)| p.is_active_for_token(token_index).then_some((p, raw_index)))
            .ok_or_else(|| {
                error_msg_typed!(
                    FermiError::TokenPositionDoesNotExist,
                    "position for token index {} not found",
                    token_index
                )
            })
    }

    pub fn token_position(&self, token_index: TokenIndex) -> Result<&TokenPosition> {
        self.token_position_and_raw_index(token_index)
            .map(|(p, _)| p)
    }

    pub(crate) fn token_position_by_raw_index_unchecked(&self, raw_index: usize) -> &TokenPosition {
        get_helper(self.dynamic(), self.header().token_offset(raw_index))
    }

    pub fn token_position_by_raw_index(&self, raw_index: usize) -> Result<&TokenPosition> {
        require_gt!(self.header().token_count(), raw_index);
        Ok(self.token_position_by_raw_index_unchecked(raw_index))
    }

                                                            
    pub fn all_token_positions(&self) -> impl Iterator<Item = &TokenPosition> + '_ {
        (0..self.header().token_count()).map(|i| self.token_position_by_raw_index_unchecked(i))
    }

                                              
    pub fn active_token_positions(&self) -> impl Iterator<Item = &TokenPosition> + '_ {
        self.all_token_positions().filter(|token| token.is_active())
    }

    pub fn serum3_orders(&self, market_index: Serum3MarketIndex) -> Result<&Serum3Orders> {
        self.all_serum3_orders()
            .find(|p| p.is_active_for_market(market_index))
            .ok_or_else(|| error_msg!("serum3 orders for market index {} not found", market_index))
    }

    pub(crate) fn serum3_orders_by_raw_index_unchecked(&self, raw_index: usize) -> &Serum3Orders {
        get_helper(self.dynamic(), self.header().serum3_offset(raw_index))
    }

    pub fn serum3_orders_by_raw_index(&self, raw_index: usize) -> Result<&Serum3Orders> {
        require_gt!(self.header().serum3_count(), raw_index);
        Ok(self.serum3_orders_by_raw_index_unchecked(raw_index))
    }

    pub fn all_serum3_orders(&self) -> impl Iterator<Item = &Serum3Orders> + '_ {
        (0..self.header().serum3_count()).map(|i| self.serum3_orders_by_raw_index_unchecked(i))
    }

    pub fn active_serum3_orders(&self) -> impl Iterator<Item = &Serum3Orders> + '_ {
        self.all_serum3_orders()
            .filter(|serum3_order| serum3_order.is_active())
    }

    pub fn perp_position(&self, market_index: PerpMarketIndex) -> Result<&PerpPosition> {
        self.all_perp_positions()
            .find(|p| p.is_active_for_market(market_index))
            .ok_or_else(|| error!(FermiError::PerpPositionDoesNotExist))
    }

    pub(crate) fn perp_position_by_raw_index_unchecked(&self, raw_index: usize) -> &PerpPosition {
        get_helper(self.dynamic(), self.header().perp_offset(raw_index))
    }

    pub fn perp_position_by_raw_index(&self, raw_index: usize) -> Result<&PerpPosition> {
        require_gt!(self.header().perp_count(), raw_index);
        Ok(self.perp_position_by_raw_index_unchecked(raw_index))
    }

    pub fn all_perp_positions(&self) -> impl Iterator<Item = &PerpPosition> {
        (0..self.header().perp_count()).map(|i| self.perp_position_by_raw_index_unchecked(i))
    }

    pub fn active_perp_positions(&self) -> impl Iterator<Item = &PerpPosition> {
        self.all_perp_positions().filter(|p| p.is_active())
    }

    pub(crate) fn perp_order_by_raw_index_unchecked(&self, raw_index: usize) -> &PerpOpenOrder {
        get_helper(self.dynamic(), self.header().perp_oo_offset(raw_index))
    }

    pub fn perp_order_by_raw_index(&self, raw_index: usize) -> Result<&PerpOpenOrder> {
        require_gt!(self.header().perp_oo_count(), raw_index);
        Ok(self.perp_order_by_raw_index_unchecked(raw_index))
    }

    pub fn all_perp_orders(&self) -> impl Iterator<Item = &PerpOpenOrder> {
        (0..self.header().perp_oo_count()).map(|i| self.perp_order_by_raw_index_unchecked(i))
    }

    pub fn perp_next_order_slot(&self) -> Result<usize> {
        self.all_perp_orders()
            .position(|&oo| oo.market == FREE_ORDER_SLOT)
            .ok_or_else(|| error_msg!("no free perp order index"))
    }

    pub fn perp_find_order_with_client_order_id(
        &self,
        market_index: PerpMarketIndex,
        client_order_id: u64,
    ) -> Option<(usize, &PerpOpenOrder)> {
        self.all_perp_orders().enumerate().find(|(_, &oo)| {
            oo.is_active_for_market(market_index) && oo.client_id == client_order_id
        })
    }

    pub fn perp_find_order_with_order_id(
        &self,
        market_index: PerpMarketIndex,
        order_id: u128,
    ) -> Option<(usize, &PerpOpenOrder)> {
        self.all_perp_orders()
            .enumerate()
            .find(|(_, &oo)| oo.is_active_for_market(market_index) && oo.id == order_id)
    }

    pub fn being_liquidated(&self) -> bool {
        self.fixed().being_liquidated()
    }

    fn token_conditional_swap_by_index_unchecked(&self, index: usize) -> &TokenConditionalSwap {
        get_helper(
            self.dynamic(),
            self.header().token_conditional_swap_offset(index),
        )
    }

    pub fn token_conditional_swap_by_index(&self, index: usize) -> Result<&TokenConditionalSwap> {
        require_gt!(self.header().token_conditional_swap_count(), index);
        Ok(self.token_conditional_swap_by_index_unchecked(index))
    }

    pub fn token_conditional_swap_by_id(&self, id: u64) -> Result<(usize, &TokenConditionalSwap)> {
        let index = self
            .all_token_conditional_swaps()
            .position(|tcs| tcs.is_configured() && tcs.id == id)
            .ok_or_else(|| error_msg!("token conditional swap with id {} not found", id))?;
        Ok((index, self.token_conditional_swap_by_index_unchecked(index)))
    }

    pub fn all_token_conditional_swaps(&self) -> impl Iterator<Item = &TokenConditionalSwap> {
        (0..self.header().token_conditional_swap_count())
            .map(|i| self.token_conditional_swap_by_index_unchecked(i))
    }

    pub fn active_token_conditional_swaps(&self) -> impl Iterator<Item = &TokenConditionalSwap> {
        self.all_token_conditional_swaps()
            .filter(|p| p.is_configured())
    }

    pub fn token_conditional_swap_free_index(&self) -> Result<usize> {
        self.all_token_conditional_swaps()
            .position(|&v| !v.is_configured())
            .ok_or_else(|| error_msg!("no free token conditional swap index"))
    }

    pub fn openbook_v2_orders(
        &self,
        market_index: OpenbookV2MarketIndex,
    ) -> Result<&OpenbookV2Orders> {
        self.all_openbook_v2_orders()
            .find(|p| p.is_active_for_market(market_index))
            .ok_or_else(|| {
                error_msg!(
                    "openbook v2 orders for market index {} not found",
                    market_index
                )
            })
    }

    pub(crate) fn openbook_v2_orders_by_raw_index_unchecked(
        &self,
        raw_index: usize,
    ) -> &OpenbookV2Orders {
        get_helper(self.dynamic(), self.header().openbook_v2_offset(raw_index))
    }

    pub fn openbook_v2_orders_by_raw_index(&self, raw_index: usize) -> Result<&OpenbookV2Orders> {
        require_gt!(self.header().openbook_v2_count(), raw_index);
        Ok(self.openbook_v2_orders_by_raw_index_unchecked(raw_index))
    }

    pub fn all_openbook_v2_orders(&self) -> impl Iterator<Item = &OpenbookV2Orders> + '_ {
        (0..self.header().openbook_v2_count())
            .map(|i| self.openbook_v2_orders_by_raw_index_unchecked(i))
    }

    pub fn active_openbook_v2_orders(&self) -> impl Iterator<Item = &OpenbookV2Orders> + '_ {
        self.all_openbook_v2_orders()
            .filter(|openbook_v2_order| openbook_v2_order.is_active())
    }

    pub fn borrow(&self) -> FermiAccountRef {
        FermiAccountRef {
            header: self.header(),
            fixed: self.fixed(),
            dynamic: self.dynamic(),
        }
    }
}

impl<
        Header: DerefOrBorrowMut<FermiAccountDynamicHeader> + DerefOrBorrow<FermiAccountDynamicHeader>,
        Fixed: DerefOrBorrowMut<FermiAccountFixed> + DerefOrBorrow<FermiAccountFixed>,
        Dynamic: DerefOrBorrowMut<[u8]> + DerefOrBorrow<[u8]>,
    > DynamicAccount<Header, Fixed, Dynamic>
{
    fn header_mut(&mut self) -> &mut FermiAccountDynamicHeader {
        self.header.deref_or_borrow_mut()
    }
    fn fixed_mut(&mut self) -> &mut FermiAccountFixed {
        self.fixed.deref_or_borrow_mut()
    }
    fn dynamic_mut(&mut self) -> &mut [u8] {
        self.dynamic.deref_or_borrow_mut()
    }

    pub fn borrow_mut(&mut self) -> FermiAccountRefMut {
        FermiAccountRefMut {
            header: self.header.deref_or_borrow_mut(),
            fixed: self.fixed.deref_or_borrow_mut(),
            dynamic: self.dynamic.deref_or_borrow_mut(),
        }
    }

               
                      
                                                                                       
    pub fn token_position_mut(
        &mut self,
        token_index: TokenIndex,
    ) -> Result<(&mut TokenPosition, usize)> {
        let raw_index = self
            .all_token_positions()
            .enumerate()
            .find_map(|(raw_index, p)| p.is_active_for_token(token_index).then_some(raw_index))
            .ok_or_else(|| {
                error_msg_typed!(
                    FermiError::TokenPositionDoesNotExist,
                    "position for token index {} not found",
                    token_index
                )
            })?;
        Ok((self.token_position_mut_by_raw_index(raw_index), raw_index))
    }

                                         
    pub fn token_position_mut_by_raw_index(&mut self, raw_index: usize) -> &mut TokenPosition {
        let offset = self.header().token_offset(raw_index);
        get_helper_mut(self.dynamic_mut(), offset)
    }

                                                                 
                
                      
                                                                            
                                                                   
    pub fn ensure_token_position(
        &mut self,
        token_index: TokenIndex,
    ) -> Result<(&mut TokenPosition, usize, usize)> {
        let mut active_index = 0;
        let mut match_or_free = None;
        for (raw_index, position) in self.all_token_positions().enumerate() {
            if position.is_active_for_token(token_index) {
                                                          
                match_or_free = Some((raw_index, active_index));
                break;
            }
            if position.is_active() {
                active_index += 1;
            } else if match_or_free.is_none() {
                match_or_free = Some((raw_index, active_index));
            }
        }
        if let Some((raw_index, bank_index)) = match_or_free {
            let v = self.token_position_mut_by_raw_index(raw_index);
            if !v.is_active_for_token(token_index) {
                *v = TokenPosition {
                    indexed_position: I80F48::ZERO,
                    token_index,
                    in_use_count: 0,
                    cumulative_deposit_interest: 0.0,
                    cumulative_borrow_interest: 0.0,
                    previous_index: I80F48::ZERO,
                    padding: Default::default(),
                    reserved: [0; 128],
                };
            }
            Ok((v, raw_index, bank_index))
        } else {
            err!(FermiError::NoFreeTokenPositionIndex)
                .context(format!("when looking for token index {}", token_index))
        }
    }

    pub fn deactivate_token_position(&mut self, raw_index: usize) {
        assert!(self.token_position_mut_by_raw_index(raw_index).in_use_count == 0);
        self.token_position_mut_by_raw_index(raw_index).token_index = TokenIndex::MAX;
    }

    pub fn deactivate_token_position_and_log(
        &mut self,
        raw_index: usize,
        fermi_account_pubkey: Pubkey,
    ) {
        let fermi_group = self.fixed().group;
        let token_position = self.token_position_mut_by_raw_index(raw_index);
        assert!(token_position.in_use_count == 0);
        emit_stack(DeactivateTokenPositionLog {
            fermi_group,
            fermi_account: fermi_account_pubkey,
            token_index: token_position.token_index,
            cumulative_deposit_interest: token_position.cumulative_deposit_interest,
            cumulative_borrow_interest: token_position.cumulative_borrow_interest,
        });
        self.token_position_mut_by_raw_index(raw_index).token_index = TokenIndex::MAX;
    }

                                                                        
       
                                                                                      
                   
    pub fn token_decrement_dust_deactivate(
        &mut self,
        bank: &mut crate::state::Bank,
        now_ts: u64,
        fermi_account_pubkey: Pubkey,
    ) -> Result<()> {
        let token_result = self.token_position_mut(bank.token_index);
        if token_result.is_anchor_error_with_code(FermiError::TokenPositionDoesNotExist.into()) {
                                        
            return Ok(());
        }
        let (position, raw_index) = token_result?;

        position.decrement_in_use();
        let active = bank.dust_if_possible(position, now_ts)?;
        if !active {
            self.deactivate_token_position_and_log(raw_index, fermi_account_pubkey);
        }

        Ok(())
    }

                                        
    pub fn serum3_orders_mut_by_raw_index(&mut self, raw_index: usize) -> &mut Serum3Orders {
        let offset = self.header().serum3_offset(raw_index);
        get_helper_mut(self.dynamic_mut(), offset)
    }

    pub fn create_serum3_orders(
        &mut self,
        market_index: Serum3MarketIndex,
    ) -> Result<&mut Serum3Orders> {
        if self.serum3_orders(market_index).is_ok() {
            return err!(FermiError::Serum3OpenOrdersExistAlready);
        }

        let raw_index_opt = self.all_serum3_orders().position(|p| !p.is_active());
        if let Some(raw_index) = raw_index_opt {
            *(self.serum3_orders_mut_by_raw_index(raw_index)) = Serum3Orders {
                market_index: market_index as Serum3MarketIndex,
                ..Serum3Orders::default()
            };
            Ok(self.serum3_orders_mut_by_raw_index(raw_index))
        } else {
            err!(FermiError::NoFreeSerum3OpenOrdersIndex)
        }
    }

    pub fn deactivate_serum3_orders(&mut self, market_index: Serum3MarketIndex) -> Result<()> {
        let raw_index = self
            .all_serum3_orders()
            .position(|p| p.is_active_for_market(market_index))
            .ok_or_else(|| error_msg!("serum3 open orders index {} not found", market_index))?;
        self.serum3_orders_mut_by_raw_index(raw_index).market_index = Serum3MarketIndex::MAX;
        Ok(())
    }

    pub fn serum3_orders_mut(
        &mut self,
        market_index: Serum3MarketIndex,
    ) -> Result<&mut Serum3Orders> {
        let raw_index_opt = self
            .all_serum3_orders()
            .position(|p| p.is_active_for_market(market_index));
        raw_index_opt
            .map(|raw_index| self.serum3_orders_mut_by_raw_index(raw_index))
            .ok_or_else(|| error_msg!("serum3 orders for market index {} not found", market_index))
    }

                                            
    pub fn openbook_v2_orders_mut_by_raw_index(
        &mut self,
        raw_index: usize,
    ) -> &mut OpenbookV2Orders {
        let offset = self.header().openbook_v2_offset(raw_index);
        get_helper_mut(self.dynamic_mut(), offset)
    }

    pub fn create_openbook_v2_orders(
        &mut self,
        market_index: OpenbookV2MarketIndex,
    ) -> Result<&mut OpenbookV2Orders> {
        if self.openbook_v2_orders(market_index).is_ok() {
            return err!(FermiError::OpenbookV2OpenOrdersExistAlready);
        }

        let raw_index_opt = self.all_openbook_v2_orders().position(|p| !p.is_active());
        if let Some(raw_index) = raw_index_opt {
            *(self.openbook_v2_orders_mut_by_raw_index(raw_index)) = OpenbookV2Orders {
                market_index: market_index as OpenbookV2MarketIndex,
                ..OpenbookV2Orders::default()
            };
            Ok(self.openbook_v2_orders_mut_by_raw_index(raw_index))
        } else {
            err!(FermiError::NoFreeOpenbookV2OpenOrdersIndex)
        }
    }

    pub fn deactivate_openbook_v2_orders(
        &mut self,
        market_index: OpenbookV2MarketIndex,
    ) -> Result<()> {
        let raw_index = self
            .all_openbook_v2_orders()
            .position(|p| p.is_active_for_market(market_index))
            .ok_or_else(|| {
                error_msg!("openbook v2 open orders index {} not found", market_index)
            })?;
        self.openbook_v2_orders_mut_by_raw_index(raw_index)
            .market_index = OpenbookV2MarketIndex::MAX;
        Ok(())
    }

    pub fn openbook_v2_orders_mut(
        &mut self,
        market_index: OpenbookV2MarketIndex,
    ) -> Result<&mut OpenbookV2Orders> {
        let raw_index_opt = self
            .all_openbook_v2_orders()
            .position(|p| p.is_active_for_market(market_index));
        raw_index_opt
            .map(|raw_index| self.openbook_v2_orders_mut_by_raw_index(raw_index))
            .ok_or_else(|| {
                error_msg!(
                    "openbook v2 orders for market index {} not found",
                    market_index
                )
            })
    }

                                        
    pub fn perp_position_mut_by_raw_index(&mut self, raw_index: usize) -> &mut PerpPosition {
        let offset = self.header().perp_offset(raw_index);
        get_helper_mut(self.dynamic_mut(), offset)
    }

    pub fn perp_order_mut_by_raw_index(&mut self, raw_index: usize) -> &mut PerpOpenOrder {
        let offset = self.header().perp_oo_offset(raw_index);
        get_helper_mut(self.dynamic_mut(), offset)
    }

    pub fn perp_position_mut(
        &mut self,
        market_index: PerpMarketIndex,
    ) -> Result<&mut PerpPosition> {
        let raw_index_opt = self
            .all_perp_positions()
            .position(|p| p.is_active_for_market(market_index));
        raw_index_opt
            .map(|raw_index| self.perp_position_mut_by_raw_index(raw_index))
            .ok_or_else(|| error!(FermiError::PerpPositionDoesNotExist))
    }

    pub fn ensure_perp_position(
        &mut self,
        perp_market_index: PerpMarketIndex,
        settle_token_index: TokenIndex,
    ) -> Result<(&mut PerpPosition, usize)> {
        let mut raw_index_opt = self
            .all_perp_positions()
            .position(|p| p.is_active_for_market(perp_market_index));
        if raw_index_opt.is_none() {
            raw_index_opt = self.all_perp_positions().position(|p| !p.is_active());
            if let Some(raw_index) = raw_index_opt {
                let perp_position = self.perp_position_mut_by_raw_index(raw_index);
                *perp_position = PerpPosition::default();
                perp_position.market_index = perp_market_index;

                let settle_token_position = self.ensure_token_position(settle_token_index)?.0;
                settle_token_position.increment_in_use();
            }
        }
        if let Some(raw_index) = raw_index_opt {
            Ok((self.perp_position_mut_by_raw_index(raw_index), raw_index))
        } else {
            err!(FermiError::NoFreePerpPositionIndex)
        }
    }

                              
    pub fn deactivate_perp_position(
        &mut self,
        perp_market_index: PerpMarketIndex,
        settle_token_index: TokenIndex,
    ) -> Result<()> {
        self.perp_position_mut(perp_market_index)?.market_index = PerpMarketIndex::MAX;

        let settle_token_position = self.token_position_mut(settle_token_index)?.0;
        settle_token_position.decrement_in_use();

        Ok(())
    }

    pub fn deactivate_perp_position_and_log(
        &mut self,
        perp_market_index: PerpMarketIndex,
        settle_token_index: TokenIndex,
        fermi_account_pubkey: Pubkey,
    ) -> Result<()> {
        let fermi_group = self.fixed().group;
        let perp_position = self.perp_position_mut(perp_market_index)?;

        emit_stack(DeactivatePerpPositionLog {
            fermi_group,
            fermi_account: fermi_account_pubkey,
            market_index: perp_market_index,
            cumulative_long_funding: perp_position.cumulative_long_funding,
            cumulative_short_funding: perp_position.cumulative_short_funding,
            maker_volume: perp_position.maker_volume,
            taker_volume: perp_position.taker_volume,
            perp_spot_transfers: perp_position.perp_spot_transfers,
        });

        perp_position.market_index = PerpMarketIndex::MAX;

        let settle_token_position = self.token_position_mut(settle_token_index)?.0;
        settle_token_position.decrement_in_use();

        Ok(())
    }

    pub fn find_first_active_unused_perp_position(&self) -> Option<&PerpPosition> {
        let first_unused_position_opt = self.all_perp_positions().find(|p| {
            p.is_active()
                && p.base_position_lots == 0
                && { p.quote_position_native } == 0
                && p.bids_base_lots == 0
                && p.asks_base_lots == 0
                && p.taker_base_lots == 0
                && p.taker_quote_lots == 0
        });
        first_unused_position_opt
    }

    pub fn add_perp_order(
        &mut self,
        perp_market_index: PerpMarketIndex,
        side: Side,
        order_tree: BookSideOrderTree,
        order: &LeafNode,
    ) -> Result<()> {
        let perp_account = self.perp_position_mut(perp_market_index)?;
        perp_account.adjust_maker_lots(side, order.quantity);
        let slot = order.owner_slot as usize;

        let oo = self.perp_order_mut_by_raw_index(slot);
        oo.market = perp_market_index;
        oo.side_and_tree = SideAndOrderTree::new(side, order_tree).into();
        oo.id = order.key;
        oo.client_id = order.client_order_id;
        oo.quantity = order.quantity;
        Ok(())
    }

                                                                       
       
                                                                           
                                                                          
                                                                            
                    
    pub fn remove_perp_order(&mut self, slot: usize, quantity: i64) -> Result<()> {
        let oo = self.perp_order_by_raw_index(slot)?;
        require_neq!(oo.market, FREE_ORDER_SLOT);
        let perp_market_index = oo.market;
        let order_side = oo.side_and_tree().side();

        let perp_account = self.perp_position_mut(perp_market_index)?;
        perp_account.adjust_maker_lots(order_side, -quantity);

        let oo = self.perp_order_mut_by_raw_index(slot);
        oo.clear();

        Ok(())
    }

                                                          
    pub fn execute_perp_maker(
        &mut self,
        perp_market_index: PerpMarketIndex,
        perp_market: &mut PerpMarket,
        fill: &FillEvent,
        group: &Group,
    ) -> Result<I80F48> {
        let side = fill.taker_side().invert_side();
        let (base_change, quote_change) = fill.base_quote_change(side);
        let quote = I80F48::from(perp_market.quote_lot_size) * I80F48::from(quote_change);
        let fees = quote.abs() * I80F48::from_num(fill.maker_fee);
        if fees.is_positive() {
            let f = self.fixed_mut();
            let now_ts = Clock::get().unwrap().unix_timestamp.try_into().unwrap();
            f.expire_buyback_fees(now_ts, group.buyback_fees_expiry_interval);
            f.accrue_buyback_fees(fees.floor().to_num::<u64>());
        }
        let pa = self.perp_position_mut(perp_market_index)?;
        pa.settle_funding(perp_market);
        pa.record_trading_fee(fees);
        let realized_pnl = pa.record_trade(perp_market, base_change, quote);

        pa.maker_volume += quote.abs().to_num::<u64>();

        let quantity_filled = base_change.abs();
        let maker_slot = fill.maker_slot as usize;

                                                                       
                                                                                        
                                                                                     
                                  
        pa.adjust_maker_lots(side, -quantity_filled);

        let oo = self.perp_order_mut_by_raw_index(maker_slot);
        let is_active = oo.is_active_for_market(perp_market_index);

                                                                              
                                                                            
                                                                            
        let is_old_fill = fill.maker_order_id == 0;
        let order_id_match = is_old_fill || oo.id == fill.maker_order_id;

        if is_active && order_id_match {
                                         
            oo.quantity = (oo.quantity - quantity_filled).max(0);

            if fill.maker_out() {
                oo.clear();
            }
        }

        Ok(realized_pnl)
    }

                                                          
    pub fn execute_perp_taker(
        &mut self,
        perp_market_index: PerpMarketIndex,
        perp_market: &mut PerpMarket,
        fill: &FillEvent,
    ) -> Result<I80F48> {
        let pa = self.perp_position_mut(perp_market_index)?;
        pa.settle_funding(perp_market);

        let (base_change, quote_change) = fill.base_quote_change(fill.taker_side());
        pa.remove_taker_trade(base_change, quote_change);
                                                                          
        let quote_change_native =
            I80F48::from(perp_market.quote_lot_size) * I80F48::from(quote_change);
        let realized_pnl = pa.record_trade(perp_market, base_change, quote_change_native);

        pa.taker_volume += quote_change_native.abs().to_num::<u64>();

        Ok(realized_pnl)
    }

    pub fn execute_perp_out_event(
        &mut self,
        perp_market_index: PerpMarketIndex,
        side: Side,
        slot: usize,
        quantity: i64,
        order_id: u128,
    ) -> Result<()> {
                                                                            
                                                    
        let pa = self.perp_position_mut(perp_market_index)?;
        pa.adjust_maker_lots(side, -quantity);

        let oo = self.perp_order_mut_by_raw_index(slot);
        let is_active = oo.is_active_for_market(perp_market_index);

                                                                   
                                                                            
                                                                       
        let is_old_event = order_id == 0;
        let order_id_match = is_old_event || oo.id == order_id;

                                                                                 
                                                    
        if is_active && order_id_match {
            oo.clear();
        }

        Ok(())
    }

    pub fn token_conditional_swap_mut_by_index(
        &mut self,
        index: usize,
    ) -> Result<&mut TokenConditionalSwap> {
        let count: usize = self.header().token_conditional_swap_count.into();
        require_gt!(count, index);
        let offset = self.header().token_conditional_swap_offset(index);
        Ok(get_helper_mut(self.dynamic_mut(), offset))
    }

    pub fn free_token_conditional_swap_mut(&mut self) -> Result<&mut TokenConditionalSwap> {
        let index = self.token_conditional_swap_free_index()?;
        let tcs = self.token_conditional_swap_mut_by_index(index)?;
        Ok(tcs)
    }

    pub fn check_health_pre(&mut self, health_cache: &HealthCache) -> Result<I80F48> {
        let pre_init_health = health_cache.health(HealthType::Init);
        self.check_health_pre_checks(health_cache, pre_init_health)?;
        Ok(pre_init_health)
    }

    pub fn check_health_pre_checks(
        &mut self,
        health_cache: &HealthCache,
        pre_init_health: I80F48,
    ) -> Result<()> {
                                                                                  
                                                
        self.fixed_mut()
            .maybe_recover_from_being_liquidated(pre_init_health);
        if self.fixed().being_liquidated() {
            let liq_end_health = health_cache.health(HealthType::LiquidationEnd);
            self.fixed_mut()
                .maybe_recover_from_being_liquidated(liq_end_health);
        }
        require!(
            !self.fixed().being_liquidated(),
            FermiError::BeingLiquidated
        );
        Ok(())
    }

    pub fn check_health_post(
        &mut self,
        health_cache: &HealthCache,
        pre_init_health: I80F48,
    ) -> Result<I80F48> {
        let post_init_health = health_cache.health(HealthType::Init);
        self.check_health_post_checks(pre_init_health, post_init_health)?;
        Ok(post_init_health)
    }

    pub fn check_health_post_checks(
        &mut self,
        pre_init_health: I80F48,
        post_init_health: I80F48,
    ) -> Result<()> {
                                                                                           
                                 
                                                                                             
                                                                                                   
                                                                                            
                       
                                                                                                
                                                                                                 
                                                                                                  
                                                             
        let health_does_not_decrease = if post_init_health < -1000 {
            post_init_health.ceil() >= pre_init_health.ceil()
        } else {
            post_init_health >= pre_init_health
        };

        require!(
            post_init_health >= 0 || health_does_not_decrease,
            FermiError::HealthMustBePositiveOrIncrease
        );
        Ok(())
    }

                                                                                                 
                               
    pub fn check_health_post_checks_strict(&mut self, post_init_health: I80F48) -> Result<()> {
        require!(post_init_health >= 0, FermiError::HealthMustBePositive);
        Ok(())
    }

    pub fn check_liquidatable(&mut self, health_cache: &HealthCache) -> Result<CheckLiquidatable> {
                                                                         
                                                                                  
                                                                 
        if self.being_liquidated() {
            let liq_end_health = health_cache.health(HealthType::LiquidationEnd);
            if self
                .fixed_mut()
                .maybe_recover_from_being_liquidated(liq_end_health)
            {
                msg!("Liqee init_health above zero");
                return Ok(CheckLiquidatable::BecameNotLiquidatable);
            }
        } else {
            let maint_health = health_cache.health(HealthType::Maint);
            if maint_health >= I80F48::ZERO {
                msg!("Liqee is not liquidatable");
                return Ok(CheckLiquidatable::NotLiquidatable);
            }
            self.fixed_mut().set_being_liquidated(true);
        }
        return Ok(CheckLiquidatable::Liquidatable);
    }

    fn write_borsh_vec_length_and_padding(&mut self, offset: usize, count: u8) {
        let dst: &mut [u8] =
            &mut self.dynamic_mut()[offset - BORSH_VEC_SIZE_BYTES - BORSH_VEC_PADDING_BYTES
                ..offset - BORSH_VEC_SIZE_BYTES];
        dst.copy_from_slice(&[0u8; BORSH_VEC_PADDING_BYTES]);
        let dst: &mut [u8] = &mut self.dynamic_mut()[offset - BORSH_VEC_SIZE_BYTES..offset];
        dst.copy_from_slice(&BorshVecLength::from(count).to_le_bytes());
    }

                                                                                                  
                                                
    fn write_token_length(&mut self) {
        let offset = self.header().token_offset(0);
        let count = self.header().token_count;
        self.write_borsh_vec_length_and_padding(offset, count)
    }

    fn write_serum3_length(&mut self) {
        let offset = self.header().serum3_offset(0);
        let count = self.header().serum3_count;
        self.write_borsh_vec_length_and_padding(offset, count)
    }

    fn write_perp_length(&mut self) {
        let offset = self.header().perp_offset(0);
        let count = self.header().perp_count;
        self.write_borsh_vec_length_and_padding(offset, count)
    }

    fn write_perp_oo_length(&mut self) {
        let offset = self.header().perp_oo_offset(0);
        let count = self.header().perp_oo_count;
        self.write_borsh_vec_length_and_padding(offset, count)
    }

    fn write_token_conditional_swap_length(&mut self) {
        let offset = self.header().token_conditional_swap_offset(0);
        let count = self.header().token_conditional_swap_count;
        self.write_borsh_vec_length_and_padding(offset, count)
    }

    fn write_openbook_v2_length(&mut self) {
        let offset = self.header().openbook_v2_offset(0);
        let count = self.header().openbook_v2_count;
        self.write_borsh_vec_length_and_padding(offset, count)
    }

    pub fn resize_dynamic_content(
        &mut self,
        new_token_count: u8,
        new_serum3_count: u8,
        new_perp_count: u8,
        new_perp_oo_count: u8,
        new_token_conditional_swap_count: u8,
        new_openbook_v2_count: u8,
    ) -> Result<()> {
        let new_header = FermiAccountDynamicHeader {
            token_count: new_token_count,
            serum3_count: new_serum3_count,
            perp_count: new_perp_count,
            perp_oo_count: new_perp_oo_count,
            token_conditional_swap_count: new_token_conditional_swap_count,
            openbook_v2_count: new_openbook_v2_count,
        };
        let old_header = self.header().clone();

        new_header.check_resize_from(&old_header)?;

        let dynamic = self.dynamic_mut();

                                                                                    
                                                                                 
          
                                        
                                                                                     
                                                                             
                              
                                                                                
                                                                                 
                                          
                                                                                              
                                                                                               
                                                           

                                                                                      
          
                                                                        
                                                                                
          
                                                                                                 
        let mut active_token_positions = 0;
        for i in 0..old_header.token_count() {
            let src = old_header.token_offset(i);
            let pos: &TokenPosition = get_helper(dynamic, src);
            if !pos.is_active() {
                continue;
            }
            if i != active_token_positions {
                let dst = old_header.token_offset(active_token_positions);
                unsafe {
                    sol_memmove(
                        &mut dynamic[dst],
                        &mut dynamic[src],
                        size_of::<TokenPosition>(),
                    );
                }
            }
            active_token_positions += 1;
        }

        let mut active_serum3_orders = 0;
        for i in 0..old_header.serum3_count() {
            let src = old_header.serum3_offset(i);
            let pos: &Serum3Orders = get_helper(dynamic, src);
            if !pos.is_active() {
                continue;
            }
            if i != active_serum3_orders {
                let dst = old_header.serum3_offset(active_serum3_orders);
                unsafe {
                    sol_memmove(
                        &mut dynamic[dst],
                        &mut dynamic[src],
                        size_of::<Serum3Orders>(),
                    );
                }
            }
            active_serum3_orders += 1;
        }

        let mut active_perp_positions = 0;
        for i in 0..old_header.perp_count() {
            let src = old_header.perp_offset(i);
            let pos: &PerpPosition = get_helper(dynamic, src);
            if !pos.is_active() {
                continue;
            }
            if i != active_perp_positions {
                let dst = old_header.perp_offset(active_perp_positions);
                unsafe {
                    sol_memmove(
                        &mut dynamic[dst],
                        &mut dynamic[src],
                        size_of::<PerpPosition>(),
                    );
                }
            }
            active_perp_positions += 1;
        }

                                                                                     
                                                                                  
        let mut blocked_perp_oo = 0;
        for i in 0..old_header.perp_oo_count() {
            let idx = old_header.perp_oo_count() - 1 - i;
            let src = old_header.perp_oo_offset(idx);
            let pos: &PerpOpenOrder = get_helper(dynamic, src);
            if pos.is_active() {
                blocked_perp_oo = idx + 1;
                break;
            }
        }

        let mut active_tcs = 0;
        for i in 0..old_header.token_conditional_swap_count() {
            let src = old_header.token_conditional_swap_offset(i);
            let pos: &TokenConditionalSwap = get_helper(dynamic, src);
            if !pos.is_configured() {
                continue;
            }
            if i != active_tcs {
                let dst = old_header.token_conditional_swap_offset(active_tcs);
                unsafe {
                    sol_memmove(
                        &mut dynamic[dst],
                        &mut dynamic[src],
                        size_of::<TokenConditionalSwap>(),
                    );
                }
            }
            active_tcs += 1;
        }

        let mut active_openbook_v2_orders = 0;
        for i in 0..old_header.openbook_v2_count() {
            let src = old_header.openbook_v2_offset(i);
            let pos: &OpenbookV2Orders = get_helper(dynamic, src);
            if !pos.is_active() {
                continue;
            }
            if i != active_openbook_v2_orders {
                let dst = old_header.openbook_v2_offset(active_openbook_v2_orders);
                unsafe {
                    sol_memmove(
                        &mut dynamic[dst],
                        &mut dynamic[src],
                        size_of::<OpenbookV2Orders>(),
                    );
                }
            }
            active_openbook_v2_orders += 1;
        }

                                                                   
        require_gte!(new_header.token_count(), active_token_positions);
        require_gte!(new_header.serum3_count(), active_serum3_orders);
        require_gte!(new_header.perp_count(), active_perp_positions);
        require_gte!(new_header.perp_oo_count(), blocked_perp_oo);
        require_gte!(new_header.token_conditional_swap_count(), active_tcs);
        require_gte!(new_header.openbook_v2_count(), active_openbook_v2_orders);

                                                                                      
                                                                      
                                                                   
                                                                                          
                                                                                      
                                                                                              
        {
                                         

            let old_serum3_start = old_header.serum3_offset(0);
            let new_serum3_start = new_header.serum3_offset(0);
            if new_serum3_start < old_serum3_start && active_serum3_orders > 0 {
                unsafe {
                    sol_memmove(
                        &mut dynamic[new_serum3_start],
                        &mut dynamic[old_serum3_start],
                        size_of::<Serum3Orders>() * active_serum3_orders,
                    );
                }
            }

            let old_perp_start = old_header.perp_offset(0);
            let new_perp_start = new_header.perp_offset(0);
            if new_perp_start < old_perp_start && active_perp_positions > 0 {
                unsafe {
                    sol_memmove(
                        &mut dynamic[new_perp_start],
                        &mut dynamic[old_perp_start],
                        size_of::<PerpPosition>() * active_perp_positions,
                    );
                }
            }

            let old_perp_oo_start = old_header.perp_oo_offset(0);
            let new_perp_oo_start = new_header.perp_oo_offset(0);
            if new_perp_oo_start < old_perp_oo_start && blocked_perp_oo > 0 {
                unsafe {
                    sol_memmove(
                        &mut dynamic[new_perp_oo_start],
                        &mut dynamic[old_perp_oo_start],
                        size_of::<PerpOpenOrder>() * blocked_perp_oo,
                    );
                }
            }

            let old_tcs_start = old_header.token_conditional_swap_offset(0);
            let new_tcs_start = new_header.token_conditional_swap_offset(0);
            if new_tcs_start < old_tcs_start && active_tcs > 0 {
                unsafe {
                    sol_memmove(
                        &mut dynamic[new_tcs_start],
                        &mut dynamic[old_tcs_start],
                        size_of::<TokenConditionalSwap>() * active_tcs,
                    );
                }
            }

            let old_openbook_v2_start = old_header.openbook_v2_offset(0);
            let new_openbook_v2_start = new_header.openbook_v2_offset(0);
            if new_openbook_v2_start < old_openbook_v2_start && active_openbook_v2_orders > 0 {
                unsafe {
                    sol_memmove(
                        &mut dynamic[new_openbook_v2_start],
                        &mut dynamic[old_openbook_v2_start],
                        size_of::<OpenbookV2Orders>() * active_openbook_v2_orders,
                    );
                }
            }
        }

                                                                                         
                                                
                                                                                  
                                                                                        
                                                                                         
                                  
        {
            let old_openbook_v2_start = old_header.openbook_v2_offset(0);
            let new_openbook_v2_start = new_header.openbook_v2_offset(0);
            if new_openbook_v2_start > old_openbook_v2_start && active_openbook_v2_orders > 0 {
                unsafe {
                    sol_memmove(
                        &mut dynamic[new_openbook_v2_start],
                        &mut dynamic[old_openbook_v2_start],
                        size_of::<OpenbookV2Orders>() * active_openbook_v2_orders,
                    );
                }
            }

            let old_tcs_start = old_header.token_conditional_swap_offset(0);
            let new_tcs_start = new_header.token_conditional_swap_offset(0);
            if new_tcs_start > old_tcs_start && active_tcs > 0 {
                unsafe {
                    sol_memmove(
                        &mut dynamic[new_tcs_start],
                        &mut dynamic[old_tcs_start],
                        size_of::<TokenConditionalSwap>() * active_tcs,
                    );
                }
            }

            let old_perp_oo_start = old_header.perp_oo_offset(0);
            let new_perp_oo_start = new_header.perp_oo_offset(0);
            if new_perp_oo_start > old_perp_oo_start && blocked_perp_oo > 0 {
                unsafe {
                    sol_memmove(
                        &mut dynamic[new_perp_oo_start],
                        &mut dynamic[old_perp_oo_start],
                        size_of::<PerpOpenOrder>() * blocked_perp_oo,
                    );
                }
            }

            let old_perp_start = old_header.perp_offset(0);
            let new_perp_start = new_header.perp_offset(0);
            if new_perp_start > old_perp_start && active_perp_positions > 0 {
                unsafe {
                    sol_memmove(
                        &mut dynamic[new_perp_start],
                        &mut dynamic[old_perp_start],
                        size_of::<PerpPosition>() * active_perp_positions,
                    );
                }
            }

            let old_serum3_start = old_header.serum3_offset(0);
            let new_serum3_start = new_header.serum3_offset(0);
            if new_serum3_start > old_serum3_start && active_serum3_orders > 0 {
                unsafe {
                    sol_memmove(
                        &mut dynamic[new_serum3_start],
                        &mut dynamic[old_serum3_start],
                        size_of::<Serum3Orders>() * active_serum3_orders,
                    );
                }
            }

                                         
        }

                                                                                               
        {
            for i in active_token_positions..new_header.token_count() {
                *get_helper_mut(dynamic, new_header.token_offset(i)) = TokenPosition::default();
            }
            for i in active_serum3_orders..new_header.serum3_count() {
                *get_helper_mut(dynamic, new_header.serum3_offset(i)) = Serum3Orders::default();
            }
            for i in active_perp_positions..new_header.perp_count() {
                *get_helper_mut(dynamic, new_header.perp_offset(i)) = PerpPosition::default();
            }
            for i in blocked_perp_oo..new_header.perp_oo_count() {
                *get_helper_mut(dynamic, new_header.perp_oo_offset(i)) = PerpOpenOrder::default();
            }
            for i in active_tcs..new_header.token_conditional_swap_count() {
                *get_helper_mut(dynamic, new_header.token_conditional_swap_offset(i)) =
                    TokenConditionalSwap::default();
            }
            for i in active_openbook_v2_orders..new_header.openbook_v2_count() {
                *get_helper_mut(dynamic, new_header.openbook_v2_offset(i)) =
                    OpenbookV2Orders::default();
            }
        }
        {
            let offset = new_header.reserved_bytes_offset();
            dynamic[offset..offset + DYNAMIC_RESERVED_BYTES]
                .copy_from_slice(&[0u8; DYNAMIC_RESERVED_BYTES]);
        }

                                           
        *self.header_mut() = new_header;

                                                              
        self.write_token_length();
        self.write_serum3_length();
        self.write_perp_length();
        self.write_perp_oo_length();
        self.write_token_conditional_swap_length();
        self.write_openbook_v2_length();

        Ok(())
    }
}

                                                                                                 
pub trait FermiAccountLoader<'a> {
    fn load_full(self) -> Result<FermiAccountLoadedRefCell<'a>>;
    fn load_full_mut(self) -> Result<FermiAccountLoadedRefCellMut<'a>>;
    fn load_full_init(self) -> Result<FermiAccountLoadedRefCellMut<'a>>;
}

impl<'a, 'info: 'a> FermiAccountLoader<'a> for &'a AccountLoader<'info, FermiAccountFixed> {
    fn load_full(self) -> Result<FermiAccountLoadedRefCell<'a>> {
                         
        self.load()?;

        let data = self.as_ref().try_borrow_data()?;
        let header =
            FermiAccountDynamicHeader::from_bytes(&data[8 + size_of::<FermiAccountFixed>()..])?;
        let (_, data) = Ref::map_split(data, |d| d.split_at(8));
        let (fixed_bytes, dynamic) =
            Ref::map_split(data, |d| d.split_at(size_of::<FermiAccountFixed>()));
        Ok(FermiAccountLoadedRefCell {
            header,
            fixed: Ref::map(fixed_bytes, |b| bytemuck::from_bytes(b)),
            dynamic,
        })
    }

    fn load_full_mut(self) -> Result<FermiAccountLoadedRefCellMut<'a>> {
                         
        self.load_mut()?;

        let data = self.as_ref().try_borrow_mut_data()?;
        let header =
            FermiAccountDynamicHeader::from_bytes(&data[8 + size_of::<FermiAccountFixed>()..])?;
        let (_, data) = RefMut::map_split(data, |d| d.split_at_mut(8));
        let (fixed_bytes, dynamic) =
            RefMut::map_split(data, |d| d.split_at_mut(size_of::<FermiAccountFixed>()));
        Ok(FermiAccountLoadedRefCellMut {
            header,
            fixed: RefMut::map(fixed_bytes, |b| bytemuck::from_bytes_mut(b)),
            dynamic,
        })
    }

    fn load_full_init(self) -> Result<FermiAccountLoadedRefCellMut<'a>> {
                         
        self.load_init()?;

        {
            let mut data = self.as_ref().try_borrow_mut_data()?;

            let disc_bytes: &mut [u8] = &mut data[0..8];
            disc_bytes.copy_from_slice(bytemuck::bytes_of(&(FermiAccount::discriminator())));

            FermiAccountDynamicHeader::initialize(&mut data[8 + size_of::<FermiAccountFixed>()..])?;
        }

        self.load_full_mut()
    }
}

#[cfg(all(test, feature = "legacy-unit-tests"))]
mod tests {
    use bytemuck::Zeroable;
    use itertools::Itertools;
    use std::path::PathBuf;

    use crate::state::PostOrderType;

    use super::*;

    fn make_test_account() -> FermiAccountValue {
        let account = FermiAccount::default_for_tests();
        let bytes = AnchorSerialize::try_to_vec(&account).unwrap();

                                              
        let expected_space = FermiAccount::space(
            account.tokens.len() as u8,
            account.serum3.len() as u8,
            account.perps.len() as u8,
            account.perp_open_orders.len() as u8,
            account.token_conditional_swaps.len() as u8,
            account.openbook_v2.len() as u8,
        );

        assert_eq!(expected_space, 8 + bytes.len());

        FermiAccountValue::from_bytes(&bytes).unwrap()
    }

    #[test]
    fn test_serialization_match() {
        let mut account = FermiAccount::default_for_tests();
        account.group = Pubkey::new_unique();
        account.owner = Pubkey::new_unique();
        account.name = crate::util::fill_from_str("abcdef").unwrap();
        account.delegate = Pubkey::new_unique();
        account.account_num = 1;
        account.being_liquidated = 2;
        account.in_health_region = 3;
        account.bump = 4;
        account.net_deposits = 5;
        account.health_region_begin_init_health = 7;
        account.buyback_fees_accrued_current = 10;
        account.buyback_fees_accrued_previous = 11;
        account.buyback_fees_expiry_timestamp = 12;
        account.tokens.resize(8, TokenPosition::default());
        account.tokens[0].token_index = 8;
        account.serum3.resize(8, Serum3Orders::default());
        account.perps.resize(4, PerpPosition::default());
        account.perps[0].market_index = 9;
        account.perp_open_orders.resize(8, PerpOpenOrder::default());
        account.next_token_conditional_swap_id = 13;
        account
            .token_conditional_swaps
            .resize(12, TokenConditionalSwap::default());
        account.token_conditional_swaps[0].buy_token_index = 14;

        let account_bytes = AnchorSerialize::try_to_vec(&account).unwrap();
        assert_eq!(
            8 + account_bytes.len(),
            FermiAccount::space(8, 8, 4, 8, 12, 5)
        );

        let account2 = FermiAccountValue::from_bytes(&account_bytes).unwrap();
        assert_eq!(account.group, account2.fixed.group);
        assert_eq!(account.owner, account2.fixed.owner);
        assert_eq!(account.name, account2.fixed.name);
        assert_eq!(account.delegate, account2.fixed.delegate);
        assert_eq!(account.account_num, account2.fixed.account_num);
        assert_eq!(account.being_liquidated, account2.fixed.being_liquidated);
        assert_eq!(account.in_health_region, account2.fixed.in_health_region);
        assert_eq!(account.bump, account2.fixed.bump);
        assert_eq!(account.net_deposits, account2.fixed.net_deposits);
        assert_eq!(
            account.perp_spot_transfers,
            account2.fixed.perp_spot_transfers
        );
        assert_eq!(
            account.health_region_begin_init_health,
            account2.fixed.health_region_begin_init_health
        );
        assert_eq!(
            account.buyback_fees_accrued_current,
            account2.fixed.buyback_fees_accrued_current
        );
        assert_eq!(
            account.buyback_fees_accrued_previous,
            account2.fixed.buyback_fees_accrued_previous
        );
        assert_eq!(
            account.buyback_fees_expiry_timestamp,
            account2.fixed.buyback_fees_expiry_timestamp
        );
        assert_eq!(
            account.next_token_conditional_swap_id,
            account2.fixed.next_token_conditional_swap_id
        );
        assert_eq!(
            account.tokens[0].token_index,
            account2
                .token_position_by_raw_index_unchecked(0)
                .token_index
        );
        assert_eq!(
            account.serum3[0].open_orders,
            account2.serum3_orders_by_raw_index_unchecked(0).open_orders
        );
        assert_eq!(
            account.perps[0].market_index,
            account2
                .perp_position_by_raw_index_unchecked(0)
                .market_index
        );
        assert_eq!(
            account.token_conditional_swaps.len(),
            account2.all_token_conditional_swaps().count()
        );
        assert_eq!(
            account.token_conditional_swaps[0].buy_token_index,
            account2
                .token_conditional_swap_by_index(0)
                .unwrap()
                .buy_token_index
        );
    }

    #[test]
    fn test_token_positions() {
        let mut account = make_test_account();
        assert!(account.token_position(1).is_err());
        assert!(account.token_position_and_raw_index(2).is_err());
        assert!(account.token_position_mut(3).is_err());
        assert_eq!(
            account.token_position_by_raw_index_unchecked(0).token_index,
            TokenIndex::MAX
        );

        {
            let (pos, raw, active) = account.ensure_token_position(1).unwrap();
            assert_eq!(raw, 0);
            assert_eq!(active, 0);
            assert_eq!(pos.token_index, 1);
        }
        {
            let (pos, raw, active) = account.ensure_token_position(7).unwrap();
            assert_eq!(raw, 1);
            assert_eq!(active, 1);
            assert_eq!(pos.token_index, 7);
        }
        {
            let (pos, raw, active) = account.ensure_token_position(42).unwrap();
            assert_eq!(raw, 2);
            assert_eq!(active, 2);
            assert_eq!(pos.token_index, 42);
        }

        {
            account.deactivate_token_position(1);

            let (pos, raw, active) = account.ensure_token_position(42).unwrap();
            assert_eq!(raw, 2);
            assert_eq!(active, 1);
            assert_eq!(pos.token_index, 42);

            let (pos, raw, active) = account.ensure_token_position(8).unwrap();
            assert_eq!(raw, 1);
            assert_eq!(active, 1);
            assert_eq!(pos.token_index, 8);
        }

        assert_eq!(account.active_token_positions().count(), 3);
        account.deactivate_token_position(0);
        assert_eq!(
            account.token_position_by_raw_index_unchecked(0).token_index,
            TokenIndex::MAX
        );
        assert!(account.token_position(1).is_err());
        assert!(account.token_position_mut(1).is_err());
        assert!(account.token_position(8).is_ok());
        assert!(account.token_position(42).is_ok());
        assert_eq!(account.token_position_and_raw_index(42).unwrap().1, 2);
        assert_eq!(account.active_token_positions().count(), 2);

        {
            let (pos, raw) = account.token_position_mut(42).unwrap();
            assert_eq!(pos.token_index, 42);
            assert_eq!(raw, 2);
        }
        {
            let (pos, raw) = account.token_position_mut(8).unwrap();
            assert_eq!(pos.token_index, 8);
            assert_eq!(raw, 1);
        }
    }

    #[test]
    fn test_serum3_orders() {
        let mut account = make_test_account();
        assert!(account.serum3_orders(1).is_err());
        assert!(account.serum3_orders_mut(3).is_err());
        assert_eq!(
            account.serum3_orders_by_raw_index_unchecked(0).market_index,
            Serum3MarketIndex::MAX
        );

        assert_eq!(account.create_serum3_orders(1).unwrap().market_index, 1);
        assert_eq!(account.create_serum3_orders(7).unwrap().market_index, 7);
        assert_eq!(account.create_serum3_orders(42).unwrap().market_index, 42);
        assert!(account.create_serum3_orders(7).is_err());
        assert_eq!(account.active_serum3_orders().count(), 3);

        assert!(account.deactivate_serum3_orders(7).is_ok());
        assert_eq!(
            account.serum3_orders_by_raw_index_unchecked(1).market_index,
            Serum3MarketIndex::MAX
        );
        assert!(account.create_serum3_orders(8).is_ok());
        assert_eq!(
            account.serum3_orders_by_raw_index_unchecked(1).market_index,
            8
        );

        assert_eq!(account.active_serum3_orders().count(), 3);
        assert!(account.deactivate_serum3_orders(1).is_ok());
        assert!(account.serum3_orders(1).is_err());
        assert!(account.serum3_orders_mut(1).is_err());
        assert!(account.serum3_orders(8).is_ok());
        assert!(account.serum3_orders(42).is_ok());
        assert_eq!(account.active_serum3_orders().count(), 2);

        assert_eq!(account.serum3_orders_mut(42).unwrap().market_index, 42);
        assert_eq!(account.serum3_orders_mut(8).unwrap().market_index, 8);
        assert!(account.serum3_orders_mut(7).is_err());
    }

    #[test]
    fn test_perp_positions() {
        let mut account = make_test_account();
        assert!(account.perp_position(1).is_err());
        assert!(account.perp_position_mut(3).is_err());
        assert_eq!(
            account.perp_position_by_raw_index_unchecked(0).market_index,
            PerpMarketIndex::MAX
        );

        {
            let (pos, raw) = account.ensure_perp_position(1, 0).unwrap();
            assert_eq!(raw, 0);
            assert_eq!(pos.market_index, 1);
            assert_eq!(account.token_position_mut(0).unwrap().0.in_use_count, 1);
        }
        {
            let (pos, raw) = account.ensure_perp_position(7, 0).unwrap();
            assert_eq!(raw, 1);
            assert_eq!(pos.market_index, 7);
            assert_eq!(account.token_position_mut(0).unwrap().0.in_use_count, 2);
        }
        {
            let (pos, raw) = account.ensure_perp_position(42, 0).unwrap();
            assert_eq!(raw, 2);
            assert_eq!(pos.market_index, 42);
            assert_eq!(account.token_position_mut(0).unwrap().0.in_use_count, 3);
        }

        {
            let pos_res = account.perp_position_mut(1);
            assert!(pos_res.is_ok());
            assert_eq!(pos_res.unwrap().market_index, 1)
        }

        {
            let pos_res = account.perp_position_mut(99);
            assert!(pos_res.is_err());
        }

        {
            assert!(account.deactivate_perp_position(7, 0).is_ok());

            let (pos, raw) = account.ensure_perp_position(42, 0).unwrap();
            assert_eq!(raw, 2);
            assert_eq!(pos.market_index, 42);
            assert_eq!(account.token_position_mut(0).unwrap().0.in_use_count, 2);

            let (pos, raw) = account.ensure_perp_position(8, 0).unwrap();
            assert_eq!(raw, 1);
            assert_eq!(pos.market_index, 8);
            assert_eq!(account.token_position_mut(0).unwrap().0.in_use_count, 3);
        }

        assert_eq!(account.active_perp_positions().count(), 3);
        assert!(account.deactivate_perp_position(1, 0).is_ok());
        assert_eq!(
            account.perp_position_by_raw_index_unchecked(0).market_index,
            PerpMarketIndex::MAX
        );
        assert!(account.perp_position(1).is_err());
        assert!(account.perp_position_mut(1).is_err());
        assert!(account.perp_position(8).is_ok());
        assert!(account.perp_position(42).is_ok());
        assert_eq!(account.active_perp_positions().count(), 2);
    }

    #[test]
    fn test_buyback_fees() {
        let mut account = make_test_account();
        let fixed = account.fixed_mut();
        assert_eq!(fixed.buyback_fees_accrued(), 0);
        fixed.expire_buyback_fees(1000, 10);
        assert_eq!(fixed.buyback_fees_accrued(), 0);
        assert_eq!(fixed.buyback_fees_expiry_timestamp, 1010);

        fixed.accrue_buyback_fees(10);
        fixed.accrue_buyback_fees(5);
        assert_eq!(fixed.buyback_fees_accrued(), 15);
        fixed.reduce_buyback_fees_accrued(2);
        assert_eq!(fixed.buyback_fees_accrued(), 13);

        fixed.expire_buyback_fees(1009, 10);
        assert_eq!(fixed.buyback_fees_expiry_timestamp, 1010);
        assert_eq!(fixed.buyback_fees_accrued(), 13);
        assert_eq!(fixed.buyback_fees_accrued_current, 13);

        fixed.expire_buyback_fees(1010, 10);
        assert_eq!(fixed.buyback_fees_expiry_timestamp, 1020);
        assert_eq!(fixed.buyback_fees_accrued(), 13);
        assert_eq!(fixed.buyback_fees_accrued_previous, 13);
        assert_eq!(fixed.buyback_fees_accrued_current, 0);

        fixed.accrue_buyback_fees(5);
        assert_eq!(fixed.buyback_fees_accrued(), 18);

        fixed.reduce_buyback_fees_accrued(15);
        assert_eq!(fixed.buyback_fees_accrued(), 3);
        assert_eq!(fixed.buyback_fees_accrued_previous, 0);
        assert_eq!(fixed.buyback_fees_accrued_current, 3);

        fixed.expire_buyback_fees(1021, 10);
        fixed.accrue_buyback_fees(1);
        assert_eq!(fixed.buyback_fees_expiry_timestamp, 1030);
        assert_eq!(fixed.buyback_fees_accrued_previous, 3);
        assert_eq!(fixed.buyback_fees_accrued_current, 1);

        fixed.expire_buyback_fees(1051, 10);
        assert_eq!(fixed.buyback_fees_expiry_timestamp, 1060);
        assert_eq!(fixed.buyback_fees_accrued_previous, 0);
        assert_eq!(fixed.buyback_fees_accrued_current, 0);

        fixed.accrue_buyback_fees(7);
        fixed.expire_buyback_fees(1060, 10);
        fixed.accrue_buyback_fees(5);
        assert_eq!(fixed.buyback_fees_expiry_timestamp, 1070);
        assert_eq!(fixed.buyback_fees_accrued(), 12);

        fixed.reduce_buyback_fees_accrued(12);
        assert_eq!(fixed.buyback_fees_accrued(), 0);
    }

    #[test]
    fn test_token_conditional_swap() {
        let mut account = make_test_account();
        assert_eq!(account.all_token_conditional_swaps().count(), 2);
        assert_eq!(account.active_token_conditional_swaps().count(), 0);
        assert_eq!(account.token_conditional_swap_free_index().unwrap(), 0);

        let tcs = account.free_token_conditional_swap_mut().unwrap();
        tcs.id = 123;
        tcs.is_configured = 1;
        assert_eq!(account.all_token_conditional_swaps().count(), 2);
        assert_eq!(account.active_token_conditional_swaps().count(), 1);
        assert_eq!(account.token_conditional_swap_free_index().unwrap(), 1);

        let tcs = account.free_token_conditional_swap_mut().unwrap();
        tcs.id = 234;
        tcs.is_configured = 1;
        assert_eq!(account.all_token_conditional_swaps().count(), 2);
        assert_eq!(account.active_token_conditional_swaps().count(), 2);

        let (index, tcs) = account.token_conditional_swap_by_id(123).unwrap();
        assert_eq!(index, 0);
        assert_eq!(tcs.id, 123);
        let tcs = account.token_conditional_swap_by_index(0).unwrap();
        assert_eq!(tcs.id, 123);

        let (index, tcs) = account.token_conditional_swap_by_id(234).unwrap();
        assert_eq!(index, 1);
        assert_eq!(tcs.id, 234);
        let tcs = account.token_conditional_swap_by_index(1).unwrap();
        assert_eq!(tcs.id, 234);

        assert!(account.free_token_conditional_swap_mut().is_err());
        assert!(account.token_conditional_swap_free_index().is_err());

        let tcs = account.token_conditional_swap_mut_by_index(0).unwrap();
        tcs.is_configured = 0;
        assert_eq!(account.all_token_conditional_swaps().count(), 2);
        assert_eq!(account.active_token_conditional_swaps().count(), 1);
        assert_eq!(
            account.active_token_conditional_swaps().next().unwrap().id,
            234
        );
        assert!(account.token_conditional_swap_by_id(123).is_err());

        assert_eq!(account.token_conditional_swap_free_index().unwrap(), 0);
        let tcs = account.free_token_conditional_swap_mut().unwrap();
        assert_eq!(tcs.id, 123);            
    }

    #[test]
    fn test_openbook_v2_orders() {
        let mut account = make_test_account();
        assert!(account.openbook_v2_orders(1).is_err());
        assert!(account.openbook_v2_orders_mut(3).is_err());

                                                                          
                                                                                                                    
                      
                                                                                 
                                         
             

        assert_eq!(
            account.create_openbook_v2_orders(1).unwrap().market_index,
            1
        );
        assert_eq!(
            account.create_openbook_v2_orders(7).unwrap().market_index,
            7
        );
        assert_eq!(
            account.create_openbook_v2_orders(42).unwrap().market_index,
            42
        );
        assert!(account.create_openbook_v2_orders(7).is_err());
        assert_eq!(account.active_openbook_v2_orders().count(), 3);

        assert!(account.deactivate_openbook_v2_orders(7).is_ok());
        assert_eq!(
            account
                .openbook_v2_orders_by_raw_index_unchecked(1)
                .market_index,
            OpenbookV2MarketIndex::MAX
        );
        assert!(account.create_openbook_v2_orders(8).is_ok());
        assert_eq!(
            account
                .openbook_v2_orders_by_raw_index_unchecked(1)
                .market_index,
            8
        );

        assert_eq!(account.active_openbook_v2_orders().count(), 3);
        assert!(account.deactivate_openbook_v2_orders(1).is_ok());
        assert!(account.openbook_v2_orders(1).is_err());
        assert!(account.openbook_v2_orders_mut(1).is_err());
        assert!(account.openbook_v2_orders(8).is_ok());
        assert!(account.openbook_v2_orders(42).is_ok());
        assert_eq!(account.active_openbook_v2_orders().count(), 2);

        assert_eq!(account.openbook_v2_orders_mut(42).unwrap().market_index, 42);
        assert_eq!(account.openbook_v2_orders_mut(8).unwrap().market_index, 8);
        assert!(account.openbook_v2_orders_mut(7).is_err());
    }

    fn make_resize_test_account(header: &FermiAccountDynamicHeader) -> FermiAccountValue {
        let mut account = FermiAccount::default_for_tests();
        account
            .tokens
            .resize(header.token_count(), TokenPosition::default());
        account
            .serum3
            .resize(header.serum3_count(), Serum3Orders::default());
        account
            .perps
            .resize(header.perp_count(), PerpPosition::default());
        account
            .perp_open_orders
            .resize(header.perp_oo_count(), PerpOpenOrder::default());
        account
            .openbook_v2
            .resize(header.openbook_v2_count(), OpenbookV2Orders::default());
        let mut bytes = AnchorSerialize::try_to_vec(&account).unwrap();

                                                                                     
        let expected_space = header.account_size();
        bytes.extend(vec![0u8; expected_space - bytes.len()]);

                                                
        let (fixed, dynamic) = bytes.split_at_mut(size_of::<FermiAccountFixed>());
        let mut out_header = FermiAccountDynamicHeader::from_bytes(dynamic).unwrap();
        out_header.token_conditional_swap_count = header.token_conditional_swap_count;
        out_header.openbook_v2_count = header.openbook_v2_count;
        let mut account = FermiAccountRefMut {
            header: &mut out_header,
            fixed: bytemuck::from_bytes_mut(fixed),
            dynamic,
        };
        account.write_token_conditional_swap_length();
        account.write_openbook_v2_length();

        let mut account = FermiAccountValue::from_bytes(&bytes).unwrap();

                                                                                       
        for i in 0..header.openbook_v2_count() {
            *account.openbook_v2_orders_mut_by_raw_index(i) = OpenbookV2Orders::default();
        }

        account
    }

    fn check_account_active_and_order(
        account: &FermiAccountValue,
        active: &FermiAccountDynamicHeader,
    ) -> Result<()> {
        let header = account.header();

        assert_eq!(account.all_token_positions().count(), header.token_count());
        assert_eq!(
            account.active_token_positions().count(),
            active.token_count()
        );
        for i in 0..active.token_count() {
            assert_eq!(
                account.token_position_by_raw_index(i)?.token_index,
                i as TokenIndex
            );
        }
        for i in active.token_count()..header.token_count() {
            let def = TokenPosition::default().try_to_vec().unwrap();
            assert_eq!(
                account
                    .token_position_by_raw_index(i)?
                    .try_to_vec()
                    .unwrap(),
                def
            );
        }

        assert_eq!(account.all_serum3_orders().count(), header.serum3_count());
        assert_eq!(
            account.active_serum3_orders().count(),
            active.serum3_count()
        );
        for i in 0..active.serum3_count() {
            assert_eq!(
                account.serum3_orders_by_raw_index(i)?.market_index,
                i as Serum3MarketIndex
            );
        }
        for i in active.serum3_count()..header.serum3_count() {
            let def = Serum3Orders::default().try_to_vec().unwrap();
            assert_eq!(
                account.serum3_orders_by_raw_index(i)?.try_to_vec().unwrap(),
                def
            );
        }

        assert_eq!(account.all_perp_positions().count(), header.perp_count());
        assert_eq!(account.active_perp_positions().count(), active.perp_count());
        for i in 0..active.perp_count() {
            assert_eq!(
                account.perp_position_by_raw_index(i)?.market_index,
                i as PerpMarketIndex
            );
        }
        for i in active.perp_count()..header.perp_count() {
            let def = PerpPosition::default().try_to_vec().unwrap();
            assert_eq!(
                account.perp_position_by_raw_index(i)?.try_to_vec().unwrap(),
                def
            );
        }

        for i in 0..header.perp_oo_count() {
            let perp_oo = account.perp_order_by_raw_index(i)?;
            if i + 1 == active.perp_oo_count() {
                assert_eq!(perp_oo.market, 0);
            } else {
                let def = PerpOpenOrder::default().try_to_vec().unwrap();
                assert_eq!(perp_oo.try_to_vec().unwrap(), def);
            }
        }

        assert_eq!(
            account.all_token_conditional_swaps().count(),
            header.token_conditional_swap_count()
        );
        assert_eq!(
            account.active_token_conditional_swaps().count(),
            active.token_conditional_swap_count()
        );
        for i in 0..active.token_conditional_swap_count() {
            assert_eq!(account.token_conditional_swap_by_index(i)?.id, i as u64);
        }
        for i in active.token_conditional_swap_count()..header.token_conditional_swap_count() {
            let def = TokenConditionalSwap::default().try_to_vec().unwrap();
            assert_eq!(
                account
                    .token_conditional_swap_by_index(i)?
                    .try_to_vec()
                    .unwrap(),
                def
            );
        }

        assert!(account.dynamic_reserved_bytes().iter().all(|&v| v == 0));

        Ok(())
    }

    #[test]
    fn test_account_resize_fixed() -> Result<()> {
        let header = FermiAccountDynamicHeader {
            token_count: 4,
            serum3_count: 5,
            perp_count: 6,
            perp_oo_count: 7,
            token_conditional_swap_count: 8,
            openbook_v2_count: 5,
        };
        let mut account = make_resize_test_account(&header);

                                         
        account.ensure_token_position(7)?;
        account.ensure_token_position(0)?;
        account.ensure_token_position(8)?;
        account.ensure_token_position(1)?;
        account.deactivate_token_position(0);
        account.deactivate_token_position(2);

        account.create_serum3_orders(0)?;
        account.create_serum3_orders(7)?;
        account.create_serum3_orders(1)?;
        *account.serum3_orders_mut_by_raw_index(1) = Serum3Orders::default();

        account.ensure_perp_position(0, 0)?;
        account.ensure_perp_position(1, 0)?;
        account.ensure_perp_position(2, 0)?;
        account.ensure_perp_position(7, 0)?;
        account.ensure_perp_position(8, 0)?;
        account.ensure_perp_position(3, 0)?;
        account.deactivate_perp_position(7, 0)?;
        account.deactivate_perp_position(8, 0)?;

        let mut perp_oo = account.perp_order_mut_by_raw_index(4);
        perp_oo.market = 0;

        let mut make_tcs = |raw_index: usize, id| {
            let mut tcs = account
                .token_conditional_swap_mut_by_index(raw_index)
                .unwrap();
            tcs.set_is_configured(true);
            tcs.id = id;
        };
        make_tcs(2, 0);
        make_tcs(4, 1);

        account.create_openbook_v2_orders(0)?;
        account.create_openbook_v2_orders(7)?;
        account.create_openbook_v2_orders(1)?;
        *account.openbook_v2_orders_mut_by_raw_index(1) = OpenbookV2Orders::default();

        let active = FermiAccountDynamicHeader {
            token_count: 2,
            serum3_count: 2,
            perp_count: 4,
            perp_oo_count: 5,
            token_conditional_swap_count: 2,
            openbook_v2_count: 2,
        };

                                                                  
        {
            let mut ta = account.clone();
            ta.resize_dynamic_content(
                header.token_count,
                header.serum3_count,
                header.perp_count,
                header.perp_oo_count,
                header.token_conditional_swap_count,
                header.openbook_v2_count,
            )?;
            check_account_active_and_order(&ta, &active)?;
        }

                                               
        {
            let mut ta = account.clone();
            ta.resize_dynamic_content(
                active.token_count,
                active.serum3_count,
                active.perp_count,
                active.perp_oo_count,
                active.token_conditional_swap_count,
                active.openbook_v2_count,
            )?;
            check_account_active_and_order(&ta, &active)?;
        }

                                                            
        {
            let mut ta = account.clone();
            ta.resize_dynamic_content(
                active.token_count - 1,
                active.serum3_count,
                active.perp_count,
                active.perp_oo_count,
                active.token_conditional_swap_count,
                active.openbook_v2_count,
            )
            .unwrap_err();
            ta.resize_dynamic_content(
                active.token_count,
                active.serum3_count - 1,
                active.perp_count,
                active.perp_oo_count,
                active.token_conditional_swap_count,
                active.openbook_v2_count,
            )
            .unwrap_err();
            ta.resize_dynamic_content(
                active.token_count,
                active.serum3_count,
                active.perp_count - 1,
                active.perp_oo_count,
                active.token_conditional_swap_count,
                active.openbook_v2_count,
            )
            .unwrap_err();
            ta.resize_dynamic_content(
                active.token_count,
                active.serum3_count,
                active.perp_count,
                active.perp_oo_count - 1,
                active.token_conditional_swap_count,
                active.openbook_v2_count,
            )
            .unwrap_err();
            ta.resize_dynamic_content(
                active.token_count,
                active.serum3_count,
                active.perp_count,
                active.perp_oo_count,
                active.token_conditional_swap_count - 1,
                active.openbook_v2_count,
            )
            .unwrap_err();
            ta.resize_dynamic_content(
                active.token_count,
                active.serum3_count,
                active.perp_count,
                active.perp_oo_count,
                active.token_conditional_swap_count,
                active.openbook_v2_count - 1,
            )
            .unwrap_err();
        }

        Ok(())
    }

    #[test]
    fn test_account_resize_random() -> Result<()> {
        use rand::{seq::SliceRandom, Rng};
        let mut rng = rand::thread_rng();
        for _ in 0..1000 {
            let header = FermiAccountDynamicHeader {
                token_count: 4,
                serum3_count: 4,
                perp_count: 4,
                perp_oo_count: 8,
                token_conditional_swap_count: 4,
                openbook_v2_count: 2,
            };
            let mut account = make_resize_test_account(&header);

            let active = FermiAccountDynamicHeader {
                token_count: rng.gen_range(0..header.token_count + 1),
                serum3_count: rng.gen_range(0..header.serum3_count + 1),
                perp_count: rng.gen_range(0..header.perp_count + 1),
                perp_oo_count: rng.gen_range(0..header.perp_oo_count + 1),
                token_conditional_swap_count: rng
                    .gen_range(0..header.token_conditional_swap_count + 1),
                openbook_v2_count: rng.gen_range(0..header.openbook_v2_count + 1),
            };

            let options = (0..header.token_count()).collect_vec();
            let selected = options.choose_multiple(&mut rng, active.token_count());
            for (i, index) in selected.sorted().enumerate() {
                account.token_position_mut_by_raw_index(*index).token_index = i as TokenIndex;
            }

            let options = (0..header.serum3_count()).collect_vec();
            let selected = options.choose_multiple(&mut rng, active.serum3_count());
            for (i, index) in selected.sorted().enumerate() {
                account.serum3_orders_mut_by_raw_index(*index).market_index =
                    i as Serum3MarketIndex;
            }

            let options = (0..header.perp_count()).collect_vec();
            let selected = options.choose_multiple(&mut rng, active.perp_count());
            for (i, index) in selected.sorted().enumerate() {
                account.perp_position_mut_by_raw_index(*index).market_index = i as PerpMarketIndex;
            }

            if active.perp_oo_count() > 0 {
                let mut perp_oo = account.perp_order_mut_by_raw_index(active.perp_oo_count() - 1);
                perp_oo.market = 0;
            }

            let options = (0..header.token_conditional_swap_count()).collect_vec();
            let selected = options.choose_multiple(&mut rng, active.token_conditional_swap_count());
            for (i, index) in selected.sorted().enumerate() {
                let tcs = account.token_conditional_swap_mut_by_index(*index).unwrap();
                tcs.set_is_configured(true);
                tcs.id = i as u64;
            }

            let options = (0..header.openbook_v2_count()).collect_vec();
            let selected = options.choose_multiple(&mut rng, active.openbook_v2_count());
            for (i, index) in selected.sorted().enumerate() {
                account
                    .openbook_v2_orders_mut_by_raw_index(*index)
                    .market_index = i as OpenbookV2MarketIndex;
            }

            let target = FermiAccountDynamicHeader {
                token_count: rng.gen_range(active.token_count..6),
                serum3_count: rng.gen_range(active.serum3_count..6),
                perp_count: rng.gen_range(active.perp_count..6),
                perp_oo_count: rng.gen_range(active.perp_oo_count..16),
                token_conditional_swap_count: rng.gen_range(active.token_conditional_swap_count..6),
                openbook_v2_count: rng.gen_range(active.openbook_v2_count..4),
            };

            let target_size = target.account_size();
            if target_size > account.dynamic.len() {
                account
                    .dynamic
                    .extend(vec![0u8; target_size - account.dynamic.len()]);
            }

            account
                .resize_dynamic_content(
                    target.token_count,
                    target.serum3_count,
                    target.perp_count,
                    target.perp_oo_count,
                    target.token_conditional_swap_count,
                    target.openbook_v2_count,
                )
                .unwrap();

            check_account_active_and_order(&account, &active).unwrap();
        }
        Ok(())
    }

    #[test]
    fn test_perp_order_events() -> Result<()> {
        let group = Group::zeroed();

        let perp_market_index = 0;
        let mut perp_market = PerpMarket::zeroed();

        let mut account = make_test_account();
        account.ensure_token_position(0)?;
        account.ensure_perp_position(perp_market_index, 0)?;

        let owner = Pubkey::new_unique();
        let slot = account.perp_next_order_slot()?;
        let order_id = 127;
        let quantity = 42;
        let order = LeafNode::new(
            slot as u8,
            order_id,
            owner,
            quantity,
            1,
            PostOrderType::Limit,
            0,
            0,
            0,
        );
        let side = Side::Bid;
        account.add_perp_order(0, side, BookSideOrderTree::Fixed, &order)?;

        let make_fill = |quantity, out, order_id| {
            FillEvent::new(
                side.invert_side(),
                out,
                slot as u8,
                0,
                0,
                owner,
                order_id,
                0,
                I80F48::ZERO,
                0,
                owner,
                0,
                I80F48::ZERO,
                1,
                quantity,
            )
        };

        let pp = |a: &FermiAccountValue| a.perp_position(perp_market_index).unwrap().clone();

        {
                        
            let mut account = account.clone();

            let fill = make_fill(quantity, true, order_id);
            account.execute_perp_maker(perp_market_index, &mut perp_market, &fill, &group)?;
            assert_eq!(pp(&account).bids_base_lots, 0);
            assert_eq!(pp(&account).asks_base_lots, 0);
            assert!(!account.perp_order_by_raw_index(0)?.is_active());
        }

        {
                                     
            let mut account = account.clone();

            let fill = make_fill(quantity, true, 0);
            account.execute_perp_maker(perp_market_index, &mut perp_market, &fill, &group)?;
            assert_eq!(pp(&account).bids_base_lots, 0);
            assert_eq!(pp(&account).asks_base_lots, 0);
            assert!(!account.perp_order_by_raw_index(0)?.is_active());
        }

        {
                        
            let mut account = account.clone();

            account.execute_perp_out_event(perp_market_index, side, slot, quantity, order_id)?;
            assert_eq!(pp(&account).bids_base_lots, 0);
            assert_eq!(pp(&account).asks_base_lots, 0);
            assert!(!account.perp_order_by_raw_index(0)?.is_active());
        }

        {
                                     
            let mut account = account.clone();

            account.execute_perp_out_event(perp_market_index, side, slot, quantity, 0)?;
            assert_eq!(pp(&account).bids_base_lots, 0);
            assert_eq!(pp(&account).asks_base_lots, 0);
            assert!(!account.perp_order_by_raw_index(0)?.is_active());
        }

        {
                     
            let mut account = account.clone();

            account.remove_perp_order(slot, quantity)?;
            assert_eq!(pp(&account).bids_base_lots, 0);
            assert_eq!(pp(&account).asks_base_lots, 0);
            assert!(!account.perp_order_by_raw_index(0)?.is_active());
        }

        {
                                                                                      
            let mut account = account.clone();

            let fill = make_fill(quantity - 10, false, order_id);
            account.execute_perp_maker(perp_market_index, &mut perp_market, &fill, &group)?;
            assert_eq!(pp(&account).bids_base_lots, 10);
            assert_eq!(pp(&account).asks_base_lots, 0);
            assert_eq!(account.perp_order_by_raw_index(slot)?.quantity, 10);

                                               

            account.remove_perp_order(slot, 0)?;
            assert_eq!(pp(&account).bids_base_lots, 10);
            assert_eq!(pp(&account).asks_base_lots, 0);
            assert!(!account.perp_order_by_raw_index(0)?.is_active());

            account.execute_perp_out_event(perp_market_index, side, slot, 10, order_id)?;
            assert_eq!(pp(&account).bids_base_lots, 0);
            assert_eq!(pp(&account).asks_base_lots, 0);
        }

        {
                                                                  
            let mut account = account.clone();

            account.remove_perp_order(slot, 0)?;
            assert_eq!(pp(&account).bids_base_lots, quantity);
            assert_eq!(pp(&account).asks_base_lots, 0);
            assert!(!account.perp_order_by_raw_index(0)?.is_active());

            let fill = make_fill(quantity - 10, false, order_id);
            account.execute_perp_maker(perp_market_index, &mut perp_market, &fill, &group)?;
            assert_eq!(pp(&account).bids_base_lots, 10);
            assert_eq!(pp(&account).asks_base_lots, 0);

            account.execute_perp_out_event(perp_market_index, side, slot, 10, order_id)?;
            assert_eq!(pp(&account).bids_base_lots, 0);
            assert_eq!(pp(&account).asks_base_lots, 0);
        }

        {
                                                              
            let mut account = account.clone();

            account.remove_perp_order(slot, 10)?;
            assert_eq!(pp(&account).bids_base_lots, quantity - 10);
            assert_eq!(pp(&account).asks_base_lots, 0);
            assert!(!account.perp_order_by_raw_index(0)?.is_active());

            let fill = make_fill(quantity - 10, false, order_id);
            account.execute_perp_maker(perp_market_index, &mut perp_market, &fill, &group)?;
            assert_eq!(pp(&account).bids_base_lots, 0);
            assert_eq!(pp(&account).asks_base_lots, 0);
        }

        {
                            
            let mut account = account.clone();

            let fill = make_fill(10, false, order_id);
            account.execute_perp_maker(perp_market_index, &mut perp_market, &fill, &group)?;
            assert_eq!(pp(&account).bids_base_lots, quantity - 10);
            assert_eq!(pp(&account).asks_base_lots, 0);
            assert_eq!(
                account.perp_order_by_raw_index(slot)?.quantity,
                quantity - 10
            );

            let fill = make_fill(10, false, order_id);
            account.execute_perp_maker(perp_market_index, &mut perp_market, &fill, &group)?;
            assert_eq!(pp(&account).bids_base_lots, quantity - 20);
            assert_eq!(pp(&account).asks_base_lots, 0);
            assert_eq!(
                account.perp_order_by_raw_index(slot)?.quantity,
                quantity - 20
            );

            let fill = make_fill(quantity - 20, true, order_id);
            account.execute_perp_maker(perp_market_index, &mut perp_market, &fill, &group)?;
            assert_eq!(pp(&account).bids_base_lots, 0);
            assert_eq!(pp(&account).asks_base_lots, 0);
            assert!(!account.perp_order_by_raw_index(0)?.is_active());
        }

        {
                                      
            let mut account = account.clone();

            let mut fill = make_fill(10, false, order_id);
            fill.maker_order_id = 1;
            account.execute_perp_maker(perp_market_index, &mut perp_market, &fill, &group)?;
            assert_eq!(pp(&account).bids_base_lots, quantity - 10);
            assert_eq!(pp(&account).asks_base_lots, 0);
            assert_eq!(account.perp_order_by_raw_index(slot)?.quantity, quantity);

            account.execute_perp_out_event(perp_market_index, side, slot, 10, 1)?;
            assert_eq!(pp(&account).bids_base_lots, quantity - 20);
            assert_eq!(pp(&account).asks_base_lots, 0);
            assert_eq!(account.perp_order_by_raw_index(slot)?.quantity, quantity);
        }

        Ok(())
    }

    #[test]
    fn test_perp_auto_close_first_unused() {
        let mut account = make_test_account();

                              
        assert_eq!(account.header.perp_count, 4);
        account.ensure_perp_position(1, 0).unwrap();
        account.ensure_perp_position(2, 0).unwrap();
        account.ensure_perp_position(3, 0).unwrap();
        account.ensure_perp_position(4, 0).unwrap();
        assert_eq!(account.active_perp_positions().count(), 4);

                                                          
        account.perp_position_mut(1).unwrap().taker_base_lots = 10;
        account.perp_position_mut(2).unwrap().base_position_lots = 10;
        account.perp_position_mut(4).unwrap().quote_position_native = I80F48::from_num(10);
        assert!(account.perp_position(3).ok().is_some());

                                     
        {
            let e = account.ensure_perp_position(5, 0);
            assert!(e.is_anchor_error_with_code(FermiError::NoFreePerpPositionIndex.error_code()));
        }

              
        let to_be_closed_account_opt = account.find_first_active_unused_perp_position();

        assert_eq!(to_be_closed_account_opt.unwrap().market_index, 3)
    }

                                                                           
    #[test]
    fn test_fermi_account_backwards_compatibility() -> Result<()> {
        use solana_program_test::{find_file, read_file};

        let mut d = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        d.push("resources/test");

                                  
                                                                                                                                              
        let fixtures = vec!["mangoaccount-v0.21.3", "mangoaccount-v0.23.0"];

        for fixture in fixtures {
            let filename = format!("resources/test/{}.bin", fixture);
            let mut account_bytes = read_file(find_file(&filename).unwrap());

                                                                             
                                                                                
                                                                           
            let legacy_discriminator: [u8; 8] = [243, 228, 247, 3, 169, 52, 175, 31];
            assert_eq!(&account_bytes[..8], &legacy_discriminator[..]);
            account_bytes[..8].copy_from_slice(&FermiAccount::DISCRIMINATOR);

                              
            let mut account_bytes_slice: &[u8] = &account_bytes;
            let borsh_account = FermiAccount::try_deserialize(&mut account_bytes_slice)?;

                                 
            let zerocopy_reader = FermiAccountValue::from_bytes(&account_bytes[8..])?;
            let fixed = &zerocopy_reader.fixed;
            let zerocopy_account = FermiAccount {
                group: fixed.group,
                owner: fixed.owner,
                name: fixed.name,
                delegate: fixed.delegate,
                account_num: fixed.account_num,
                being_liquidated: fixed.being_liquidated,
                in_health_region: fixed.in_health_region,
                bump: fixed.bump,
                sequence_number: 0,
                net_deposits: fixed.net_deposits,
                perp_spot_transfers: fixed.perp_spot_transfers,
                health_region_begin_init_health: fixed.health_region_begin_init_health,
                frozen_until: fixed.frozen_until,
                buyback_fees_accrued_current: fixed.buyback_fees_accrued_current,
                buyback_fees_accrued_previous: fixed.buyback_fees_accrued_previous,
                buyback_fees_expiry_timestamp: fixed.buyback_fees_expiry_timestamp,
                next_token_conditional_swap_id: fixed.next_token_conditional_swap_id,
                temporary_delegate: fixed.temporary_delegate,
                temporary_delegate_expiry: fixed.temporary_delegate_expiry,
                last_collateral_fee_charge: fixed.last_collateral_fee_charge,
                reserved: [0u8; 152],

                header_version: *zerocopy_reader.header_version(),
                padding3: Default::default(),

                padding4: Default::default(),
                tokens: zerocopy_reader.all_token_positions().cloned().collect_vec(),

                padding5: Default::default(),
                serum3: zerocopy_reader.all_serum3_orders().cloned().collect_vec(),

                padding6: Default::default(),
                perps: zerocopy_reader.all_perp_positions().cloned().collect_vec(),

                padding7: Default::default(),
                perp_open_orders: zerocopy_reader.all_perp_orders().cloned().collect_vec(),

                padding8: Default::default(),
                token_conditional_swaps: zerocopy_reader
                    .all_token_conditional_swaps()
                    .cloned()
                    .collect_vec(),

                padding9: Default::default(),
                openbook_v2: zerocopy_reader
                    .all_openbook_v2_orders()
                    .cloned()
                    .collect_vec(),

                reserved_dynamic: zerocopy_reader.dynamic_reserved_bytes().try_into().unwrap(),
            };

                                  
            assert!(borsh_account == zerocopy_account);

                                                                    
            let mut borsh_bytes = Vec::new();
            borsh_account.try_serialize(&mut borsh_bytes)?;
            let mut slice: &[u8] = &borsh_bytes;
            let roundtrip_account = FermiAccount::try_deserialize(&mut slice)?;
            assert!(borsh_account == roundtrip_account);
        }

        Ok(())
    }
}

                                                                         
                                                                             
                                                          
#[cfg(test)]
mod wire_identity_tests {
    use bytemuck::Zeroable;

    use super::*;

    #[test]
    fn test_fermi_account_wire_identity() {
        assert_eq!(
            FermiAccount::DISCRIMINATOR,
            [245, 36, 51, 8, 90, 127, 231, 244]
        );
        assert_eq!(
            FermiAccountFixed::DISCRIMINATOR,
            FermiAccount::DISCRIMINATOR
        );
        assert_eq!(
            crate::events::FermiAccountData::DISCRIMINATOR,
            [234, 19, 135, 213, 10, 131, 107, 216]
        );

                                                                          
                                                                       
        let fixed = FermiAccountFixed::zeroed();
        let pda_seeds = fixed.pda_seeds();
        let signer_seeds = pda_seeds.signer_seeds();
        let macro_seeds = fermi_account_seeds!(fixed);
        let component_seeds = crate::account_seeds!(fixed);
        assert_eq!(signer_seeds[0], b"FermiAccount");
        assert_eq!(macro_seeds[0], b"FermiAccount");
        assert_eq!(component_seeds[0], b"FermiAccount");
    }
}

#[macro_export]
macro_rules! fermi_account_seeds {
    ( $account:expr ) => {
        &[
            b"FermiAccount".as_ref(),
            $account.group.as_ref(),
            $account.owner.as_ref(),
            &$account.account_num.to_le_bytes(),
            &[$account.bump],
        ]
    };
}
pub use fermi_account_seeds;
