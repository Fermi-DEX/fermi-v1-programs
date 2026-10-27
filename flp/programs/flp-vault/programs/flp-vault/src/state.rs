use anchor_lang::prelude::*;

pub const VAULT_SEED: &[u8] = b"flp-vault";
pub const AUTHORITY_SEED: &[u8] = b"flp-authority";
pub const SHARE_MINT_SEED: &[u8] = b"flp-shares";
pub const ESCROW_SEED: &[u8] = b"flp-escrow";
pub const DEPOSIT_ESCROW_TAG: &[u8] = b"dep";
pub const REDEMPTION_ESCROW_TAG: &[u8] = b"red";
pub const EPOCH_SEED: &[u8] = b"flp-epoch";
pub const DEPOSITOR_SEED: &[u8] = b"flp-user";

                                                           
pub const NONE_EPOCH: u64 = u64::MAX;

                                                                                 
pub const PENDING_SLOTS: usize = 4;
pub const VAULT_VERSION: u8 = 4;
                                                   
pub const HEALTH_UNAVAILABLE: i64 = i64::MIN;
                                                                  
pub const MAX_VAULT_PERPS: usize = 13;
pub const RECOVERY_TARGETS_SEED: &[u8] = b"flp-recovery";

                                                                              
                                                                     
                                                                           
#[account]
#[derive(InitSpace)]
pub struct RecoveryTargets {
    pub vault: Pubkey,
    pub mode: u8,
    pub batch_epoch: u64,
    pub started_at: i64,
    pub market_indexes: [u16; 13],
    pub target_base_lots: [i64; 13],
}

impl RecoveryTargets {
    pub fn check_generation(&self, vault: &Vault) -> Result<()> {
        require!(self.mode == vault.unwind.mode
            && self.batch_epoch == vault.unwind.batch_epoch
            && self.started_at == vault.unwind.started_at,
            crate::error::FlpError::WrongRecoveryMarket);
        Ok(())
    }
}

pub mod status {
    pub const DEPOSITS_PAUSED: u8 = 1 << 0;
    pub const WITHDRAWALS_PAUSED: u8 = 1 << 1;
                                                                            
                           
    pub const BREAKER: u8 = 1 << 2;
    pub const PRE_DEPOSIT: u8 = 1 << 3;
    pub const UNWIND: u8 = 1 << 4;
    pub const CLOSED: u8 = 1 << 5;
                                                                               
                                                                                 
    pub const PUBLIC_DEPOSITS_PAUSED: u8 = 1 << 6;
                                                                              
    pub const SETTABLE: u8 = DEPOSITS_PAUSED | WITHDRAWALS_PAUSED | BREAKER | PUBLIC_DEPOSITS_PAUSED;
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Debug, Default, InitSpace, PartialEq)]
pub struct RecoveryParams {
    pub grace_proportional_s: u32,
    pub grace_full_s: u32,
    pub oracle_band_bps: u16,
                                                                             
    pub max_step_quote: u64,
}
impl RecoveryParams {
    pub fn initial() -> Self {
        Self {
            grace_proportional_s: 43_200,
            grace_full_s: 259_200,
            oracle_band_bps: 75,
            max_step_quote: 50_000_000_000,
        }
    }
    pub fn validate(&self) -> Result<()> {
        require!(
            self.grace_proportional_s > 0
                && self.grace_full_s >= self.grace_proportional_s
                && self.oracle_band_bps > 0
                && self.oracle_band_bps <= 500
                && self.max_step_quote > 0
                && self.max_step_quote <= i64::MAX as u64,
            crate::error::FlpError::InvalidParams
        );
        Ok(())
    }
                                                                                 
                                                                                    
                                                                                
    pub fn validate_staged(&self) -> Result<()> {
        self.validate()?;
        require!(
            self.grace_full_s <= MAX_RECOVERY_GRACE_S && self.max_step_quote >= MIN_STEP_QUOTE,
            crate::error::FlpError::InvalidParams
        );
        Ok(())
    }
                                                                                  
                                                                               
    pub fn only_widens(&self, current: &RecoveryParams) -> bool {
        self.oracle_band_bps >= current.oracle_band_bps
            && self.max_step_quote >= current.max_step_quote
            && self.grace_proportional_s == current.grace_proportional_s
            && self.grace_full_s == current.grace_full_s
    }
                                                                             
                                                                               
    pub fn initial_for(epoch_duration_s: u32, equity_window_s: u32) -> Self {
        let mut p = Self::initial();
        let cycle = epoch_duration_s.saturating_add(equity_window_s);
        p.grace_proportional_s = p.grace_proportional_s.max(cycle);
        p.grace_full_s = p.grace_full_s.max(p.grace_proportional_s.saturating_mul(2));
        p
    }
}

                                                                        
pub const MAX_RECOVERY_GRACE_S: u32 = 2_592_000;
                                                                  
pub const MIN_STEP_QUOTE: u64 = 1_000_000_000;

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Debug, Default, InitSpace, PartialEq)]
pub struct UnwindState {
                                                                                
    pub mode: u8,
    pub batch_epoch: u64,
    pub started_at: i64,
                                                                         
    pub market_indexes: [u16; 4],
                                                                       
    pub target_base_lots: [i64; 4],
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Debug, Default, InitSpace, PartialEq)]
pub struct VaultParams {
                                                    
    pub epoch_duration_s: u32,
                                                                             
                            
    pub request_cutoff_s: u32,
                                                                             
                                                                             
    pub timelock_s: u32,
    pub mgmt_fee_bps: u16,
    pub perf_fee_bps: u16,
    pub deposit_fee_bps: u16,
    pub withdraw_fee_bps: u16,
                                                                  
    pub upnl_haircut_bps: u16,
                                                                            
                                                                      
    pub min_post_withdraw_init_ratio_bps: u16,
    pub breaker_drawdown_bps: u16,
    pub breaker_min_init_ratio_bps: u16,
    pub breaker_window_s: u32,
                     
    pub tvl_cap: u64,
    pub min_deposit: u64,
    pub min_withdraw_shares: u64,
                                                                             
                                                                               
                                                                             
    pub equity_window_s: u32,
}

                                                                             
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Debug, Default, InitSpace, PartialEq)]
pub struct LegacyVaultParams {
                                                    
    pub epoch_duration_s: u32,
                                                                             
                            
    pub request_cutoff_s: u32,
                                                                             
                                                                             
    pub timelock_s: u32,
    pub mgmt_fee_bps: u16,
    pub perf_fee_bps: u16,
    pub deposit_fee_bps: u16,
    pub withdraw_fee_bps: u16,
                                                                  
    pub upnl_haircut_bps: u16,
                                                                            
                                                                      
    pub min_post_withdraw_init_ratio_bps: u16,
    pub breaker_drawdown_bps: u16,
    pub breaker_min_init_ratio_bps: u16,
    pub breaker_window_s: u32,
                     
    pub tvl_cap: u64,
    pub min_deposit: u64,
    pub min_withdraw_shares: u64,
}
impl From<VaultParams> for LegacyVaultParams {
    fn from(p: VaultParams) -> Self {
        Self {
            epoch_duration_s: p.epoch_duration_s,
            request_cutoff_s: p.request_cutoff_s,
            timelock_s: p.timelock_s,
            mgmt_fee_bps: p.mgmt_fee_bps,
            perf_fee_bps: p.perf_fee_bps,
            deposit_fee_bps: p.deposit_fee_bps,
            withdraw_fee_bps: p.withdraw_fee_bps,
            upnl_haircut_bps: p.upnl_haircut_bps,
            min_post_withdraw_init_ratio_bps: p.min_post_withdraw_init_ratio_bps,
            breaker_drawdown_bps: p.breaker_drawdown_bps,
            breaker_min_init_ratio_bps: p.breaker_min_init_ratio_bps,
            breaker_window_s: p.breaker_window_s,
            tvl_cap: p.tvl_cap,
            min_deposit: p.min_deposit,
            min_withdraw_shares: p.min_withdraw_shares,
        }
    }
}
impl From<LegacyVaultParams> for VaultParams {
    fn from(p: LegacyVaultParams) -> Self {
        Self {
            epoch_duration_s: p.epoch_duration_s,
            request_cutoff_s: p.request_cutoff_s,
            timelock_s: p.timelock_s,
            mgmt_fee_bps: p.mgmt_fee_bps,
            perf_fee_bps: p.perf_fee_bps,
            deposit_fee_bps: p.deposit_fee_bps,
            withdraw_fee_bps: p.withdraw_fee_bps,
            upnl_haircut_bps: p.upnl_haircut_bps,
            min_post_withdraw_init_ratio_bps: p.min_post_withdraw_init_ratio_bps,
            breaker_drawdown_bps: p.breaker_drawdown_bps,
            breaker_min_init_ratio_bps: p.breaker_min_init_ratio_bps,
            breaker_window_s: p.breaker_window_s,
            tvl_cap: p.tvl_cap,
            min_deposit: p.min_deposit,
            min_withdraw_shares: p.min_withdraw_shares,
            equity_window_s: MIN_EQUITY_WINDOW_S,
        }
    }
}
impl LegacyVaultParams {
    pub fn delegate_delay_s(&self) -> i64 {
        VaultParams::from(*self).delegate_delay_s()
    }
    pub fn parameter_delay_s(&self) -> i64 {
        VaultParams::from(*self).parameter_delay_s()
    }
}

pub const MAX_MGMT_FEE_BPS: u16 = 300;
pub const MAX_PERF_FEE_BPS: u16 = 3000;
pub const MAX_FLOW_FEE_BPS: u16 = 100;
                                                                       
pub const MIN_DELEGATE_TIMELOCK_S: u32 = 0;
pub const MIN_EPOCH_DURATION_S: u32 = 60;
                                                                              
                                              
pub const MAX_EPOCH_DURATION_S: u32 = 2_592_000;
                                                                            
                                                                             
pub const MIN_EQUITY_WINDOW_S: u32 = 300;
                                                                        
                                                                         
pub const MAX_EQUITY_WINDOW_S: u32 = 7_200;
                                                                             
                                                                    
pub const MAX_EQUITY_WINDOW_EPOCH_DIVISOR: u32 = 4;

                                                                      
pub fn max_equity_window_s(epoch_duration_s: u32) -> u32 {
    MAX_EQUITY_WINDOW_S.max(epoch_duration_s / MAX_EQUITY_WINDOW_EPOCH_DIVISOR)
}

impl VaultParams {
    pub fn delegate_delay_s(&self) -> i64 {
        i64::from(self.timelock_s)
    }
                                                                              
                                                                              
    pub fn parameter_delay_s(&self) -> i64 {
        (self.timelock_s as i64).max(self.epoch_duration_s as i64 + self.request_cutoff_s as i64)
    }

    pub fn validate(&self) -> Result<()> {
        require!(self.breaker_min_init_ratio_bps == 0, crate::error::FlpError::UnsupportedHealthBreaker);
        use crate::error::FlpError;
        require!(
            self.mgmt_fee_bps <= MAX_MGMT_FEE_BPS
                && self.perf_fee_bps <= MAX_PERF_FEE_BPS
                && self.deposit_fee_bps <= MAX_FLOW_FEE_BPS
                && self.withdraw_fee_bps <= MAX_FLOW_FEE_BPS
                && self.upnl_haircut_bps <= 10_000
                && self.breaker_drawdown_bps <= 10_000,
            FlpError::InvalidParams
        );
        require!(
            self.epoch_duration_s >= MIN_EPOCH_DURATION_S
                && self.epoch_duration_s <= MAX_EPOCH_DURATION_S
                && self.request_cutoff_s < self.epoch_duration_s,
            FlpError::InvalidParams
        );
        require!(
            self.equity_window_s >= MIN_EQUITY_WINDOW_S
                && self.equity_window_s <= max_equity_window_s(self.epoch_duration_s),
            FlpError::InvalidParams
        );
        Ok(())
    }
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Debug, Default, InitSpace, PartialEq)]
pub struct PendingTotals {
    pub epoch: u64,
    pub deposits_gross: u64,
                                                                              
                                                                 
    pub deposits_net: u64,
    pub withdraw_shares: u64,
    pub n_deposits: u32,
    pub n_withdraws: u32,
}

impl PendingTotals {
    pub fn empty() -> Self {
        Self {
            epoch: NONE_EPOCH,
            ..Default::default()
        }
    }
}

                                                                  
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Debug, Default, InitSpace, PartialEq)]
pub struct NavSnapshot {
    pub slot: u64,
    pub nav_mid: i64,
    pub nav_low: i64,
    pub nav_high: i64,
                                                                                 
                                   
    pub init_ratio_bps: i64,
                                                                       
                                                 
    pub spot_quote: i64,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Debug, Default, InitSpace, PartialEq)]
pub struct VaultStats {
    pub inception_ts: i64,
    pub total_deposited_gross: u64,
    pub total_withdrawn_net: u64,
                                                                             
    pub net_deposits: i64,
    pub cum_mgmt_fee_shares: u64,
    pub cum_perf_fee_shares: u64,
    pub active_depositors: u32,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Debug, Default, InitSpace, PartialEq)]
pub struct EpochTiming {
    pub enabled: bool,
    pub epoch_duration_s: u32,
    pub request_cutoff_s: u32,
}

#[account]
#[derive(InitSpace, Debug)]
pub struct Vault {
    pub version: u8,
    pub bump: u8,
    pub authority_bump: u8,
    pub share_mint_bump: u8,
    pub status: u8,
    pub vault_id: u16,
    pub quote_token_index: u16,

    pub group: Pubkey,
    pub fermi_account: Pubkey,
    pub quote_bank: Pubkey,
    pub quote_mint: Pubkey,
    pub share_mint: Pubkey,
    pub deposit_escrow: Pubkey,
    pub redemption_escrow: Pubkey,

    pub admin: Pubkey,
    pub pending_admin: Pubkey,
    pub guardian: Pubkey,
    pub fee_recipient: Pubkey,
    pub delegate: Pubkey,
    pub pending_delegate: Pubkey,
    pub pending_delegate_eta: i64,

    pub params: LegacyVaultParams,
    pub pending_params: LegacyVaultParams,
    pub pending_params_eta: i64,

                                 
    pub epoch: u64,
    pub epoch_started_at: i64,
    pub pending: [PendingTotals; PENDING_SLOTS],

                                                                      
                                                      
    pub owed_unminted_shares: u64,
                                                                           
                                                                     
    pub locked_shares: u64,
                                                                              
    pub dead_shares: u64,

                                                                         
    pub deferred_head: u64,
    pub deferred_tail: u64,

                                                               
    pub hwm_price_e12: u128,
    pub last_fee_accrual_ts: i64,
    pub last_nav: NavSnapshot,
    pub breaker_ref_price_e12: u128,
    pub breaker_ref_ts: i64,

    pub stats: VaultStats,
    pub next_request_id: u64,
    pub pre_deposit_total: u64,
    pub live_started_at: i64,
    pub recovery_params: RecoveryParams,
    pub pending_recovery_params: RecoveryParams,
    pub pending_recovery_eta: i64,
    pub unwind: UnwindState,
    pub terminal_assets: u64,
    pub terminal_supply: u64,
                                                                                
                                                                              
    pub held_deposits: u64,
                                                                             
                                                                             
    pub graduation_num: u128,
    pub graduation_den: u128,
    pub graduation_remaining_quote: u64,
    pub graduation_remaining_shares: u64,
                                                                                          
                                                                              
                                                                   
    pub reserved: [u8; 67],
}

impl Vault {
    pub fn has_expanded_recovery(&self) -> bool {
        self.reserved[18] == 1
    }
                                                                          
                                                                                 
                                                               
    pub fn head_skip(&self) -> (u8, i64) {
        (
            self.reserved[19],
            i64::from_le_bytes(self.reserved[20..28].try_into().unwrap()),
        )
    }
                                                                            
    pub fn set_head_skip(&mut self, reason: u8, now: i64) {
        self.reserved[19] = reason;
        if reason == 2 && self.head_skip().1 == 0 {
            self.reserved[20..28].copy_from_slice(&now.to_le_bytes());
        }
    }
                                                     
    pub fn clear_head_skip(&mut self) {
        self.reserved[19..28].fill(0);
    }
                                                                            
                                                                              
                                                                               
                                                                                                    
                                                                                 
                                                                              
                                                                
    pub fn permissionless_trigger_at(&self, full: bool) -> Option<i64> {
        let (reason, since) = self.head_skip();
        if since == 0 || reason != 2 {
            return None;
        }
        let p = self.recovery_params;
        let grace = if full { p.grace_full_s } else { p.grace_proportional_s };
        Some(since.saturating_add(grace as i64))
    }
                                                                              
                                                                               
                                                                                
    pub fn delegate_restore(&self) -> (Pubkey, bool) {
        (
            Pubkey::new_from_array(self.reserved[28..60].try_into().unwrap()),
            self.reserved[60] & 1 != 0,
        )
    }
    pub fn set_delegate_restore(&mut self, delegate: Pubkey, deposits_open: bool) {
        self.reserved[28..60].copy_from_slice(delegate.as_ref());
        self.reserved[60] = u8::from(deposits_open);
    }
    pub fn clear_delegate_restore(&mut self) {
        self.reserved[28..61].fill(0);
    }
                                                                        
    pub fn staged_equity_window_s(&self) -> u32 {
        let window = u32::from_le_bytes(self.reserved[14..18].try_into().unwrap());
        if window == 0 {
            MIN_EQUITY_WINDOW_S
        } else {
            window
        }
    }
                                                                          
                                                                             
                                                                           
    pub fn longest_close_cycle_s(&self) -> u64 {
        let window = self.equity_window_s() as u64;
        let mut cycle = self.params.epoch_duration_s as u64 + window;
        let timing = self.scheduled_timing();
        if timing.enabled {
            cycle = cycle.max(timing.epoch_duration_s as u64 + window);
        }
        if self.pending_params_eta != 0 {
            cycle = cycle.max(
                self.pending_params.epoch_duration_s as u64 + self.staged_equity_window_s() as u64,
            );
        }
        cycle
    }
    pub fn parameter_delay_s(&self) -> i64 {
        let timing = self.scheduled_timing();
        let pending = if timing.enabled {
            timing.epoch_duration_s as i64 + timing.request_cutoff_s as i64
        } else {
            0
        };
        self.params.parameter_delay_s().max(pending)
    }

    pub fn scheduled_timing(&self) -> EpochTiming {
        EpochTiming {
            enabled: self.reserved[0] != 0,
            epoch_duration_s: u32::from_le_bytes(self.reserved[1..5].try_into().unwrap()),
            request_cutoff_s: u32::from_le_bytes(self.reserved[5..9].try_into().unwrap()),
        }
    }
    pub fn equity_window_s(&self) -> u32 {
        let window = u32::from_le_bytes(self.reserved[10..14].try_into().unwrap());
        if window == 0 {
            MIN_EQUITY_WINDOW_S
        } else {
            window
        }
    }
    pub fn set_equity_window(&mut self, window: u32) {
        self.reserved[10..14].copy_from_slice(&window.to_le_bytes());
    }
    pub fn stage_equity_window(&mut self, window: u32) {
        self.reserved[14..18].copy_from_slice(&window.to_le_bytes());
    }
    pub fn apply_equity_window(&mut self) {
        let window = u32::from_le_bytes(self.reserved[14..18].try_into().unwrap());
        self.set_equity_window(if window == 0 {
            MIN_EQUITY_WINDOW_S
        } else {
            window
        });
    }
    pub fn schedule_epoch_timing(&mut self, duration: u32, cutoff: u32) {
        self.reserved[0] = 1;
        self.reserved[1..5].copy_from_slice(&duration.to_le_bytes());
        self.reserved[5..9].copy_from_slice(&cutoff.to_le_bytes());
    }
    pub fn activate_epoch_timing(&mut self) {
        let timing = self.scheduled_timing();
        if timing.enabled {
            self.params.epoch_duration_s = timing.epoch_duration_s;
            self.params.request_cutoff_s = timing.request_cutoff_s;
            self.reserved[..9].fill(0);
        }
    }
    pub fn is_pre_deposit(&self) -> bool {
        self.status & status::PRE_DEPOSIT != 0
    }
    pub fn is_unwinding(&self) -> bool {
        self.status & status::UNWIND != 0
    }
    pub fn is_closed(&self) -> bool {
        self.status & status::CLOSED != 0
    }
                                                                       
    pub fn require_trading_open(&self) -> Result<()> {
        require!(!self.is_closed(), crate::error::FlpError::WrongVaultMode);
        Ok(())
    }
    pub fn require_deposits_enabled(&self) -> Result<()> {
        require!(
            self.status
                & (status::DEPOSITS_PAUSED | status::BREAKER | status::UNWIND | status::CLOSED)
                == 0,
            crate::error::FlpError::DepositsPaused
        );
        Ok(())
    }
                                                      
    pub fn require_public_deposits_enabled(&self) -> Result<()> {
        self.require_deposits_enabled()?;
        require!(
            self.status & status::PUBLIC_DEPOSITS_PAUSED == 0,
            crate::error::FlpError::DepositsPaused
        );
        Ok(())
    }
    pub fn perf_fee_bps_now(&self) -> u16 {
        if self.is_pre_deposit() {
            0
        } else {
            self.params.perf_fee_bps
        }
    }

    pub fn is_seeded(&self) -> bool {
        self.dead_shares > 0
    }

    pub fn epoch_end(&self) -> i64 {
        self.epoch_started_at + self.params.epoch_duration_s as i64
    }

                                                     
    pub fn target_epoch(&self, now: i64) -> u64 {
        let cutoff_at = self.epoch_end() - self.params.request_cutoff_s as i64;
        if now < cutoff_at {
            self.epoch
        } else {
            self.epoch + 1
        }
    }

                                                                         
    pub fn can_cancel(&self, ticket_epoch: u64, now: i64) -> bool {
        ticket_epoch > self.epoch
            || (ticket_epoch == self.epoch
                && now < self.epoch_end() - self.params.request_cutoff_s as i64)
    }

    pub fn pending_slot_mut(&mut self, epoch: u64) -> Result<&mut PendingTotals> {
        let slot = &mut self.pending[(epoch % PENDING_SLOTS as u64) as usize];
        if slot.epoch != epoch {
                                                                                
            require!(
                slot.epoch == NONE_EPOCH,
                crate::error::FlpError::PendingSlotBusy
            );
            *slot = PendingTotals::empty();
            slot.epoch = epoch;
        }
        Ok(slot)
    }

    pub fn pending_for(&self, epoch: u64) -> PendingTotals {
        let slot = self.pending[(epoch % PENDING_SLOTS as u64) as usize];
        if slot.epoch == epoch {
            slot
        } else {
            PendingTotals::empty()
        }
    }

    pub fn clear_pending(&mut self, epoch: u64) {
        let slot = &mut self.pending[(epoch % PENDING_SLOTS as u64) as usize];
        if slot.epoch == epoch {
            *slot = PendingTotals::empty();
        }
    }

    pub fn total_pending_deposits(&self) -> u64 {
        self.pending
            .iter()
            .filter(|p| p.epoch != NONE_EPOCH)
            .map(|p| p.deposits_gross)
            .sum()
    }
}

#[repr(u8)]
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum WithdrawStatus {
    None = 0,
    Settled = 1,
    Deferred = 2,
}

#[account]
#[derive(InitSpace, Debug)]
pub struct EpochRecord {
    pub vault: Pubkey,
    pub epoch: u64,
    pub bump: u8,
    pub closed_at: i64,
    pub closed_slot: u64,
    pub nav: NavSnapshot,
                                                                       
    pub supply: u64,
    pub mgmt_fee_shares: u64,
    pub perf_fee_shares: u64,

    pub dep_gross: u64,
    pub dep_net: u64,
                                                                                  
                                                                                   
    pub dep_num: u128,
    pub dep_den: u128,
    pub dep_shares: u64,
    pub dep_claimed_net: u64,
    pub dep_claimed_shares: u64,

    pub wd_shares: u64,
    pub wd_status: u8,
    pub wd_fee_bps: u16,
                                                                    
    pub wd_close_num: u128,
    pub wd_close_den: u128,
                                                         
    pub wd_num: u128,
    pub wd_den: u128,
    pub wd_gross: u64,
    pub wd_fee: u64,
    pub wd_settled_at: i64,
    pub wd_claimed_shares: u64,
    pub wd_claimed_usdc: u64,
                                                                       
    pub netted: u64,
                                                                
    pub next_deferred: u64,
}

impl EpochRecord {
                                                                                
                                         
    pub fn deposits_held(&self) -> bool {
        self.dep_den == 0 && self.dep_gross > 0
    }
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Debug, Default, InitSpace, PartialEq)]
pub struct Ticket {
    pub request_id: u64,
    pub epoch: u64,
                                              
    pub amount: u64,
                                                                   
    pub aux: u64,
}

impl Ticket {
    pub fn empty() -> Self {
        Self {
            epoch: NONE_EPOCH,
            ..Default::default()
        }
    }
    pub fn is_empty(&self) -> bool {
        self.amount == 0
    }
}

#[account]
#[derive(InitSpace, Debug)]
pub struct Depositor {
    pub vault: Pubkey,
    pub user: Pubkey,
    pub bump: u8,
    pub dep: Ticket,
    pub wd: Ticket,
                                                                            
    pub shares_held: u64,
                                                             
    pub cost_basis: u64,
    pub realized_pnl: i64,
    pub total_deposited: u64,
    pub total_withdrawn: u64,
    pub first_deposit_ts: i64,
    pub last_action_ts: i64,
                                                                         
    pub pre_deposit_amount: u64,
    pub deposit_approved: bool,
    pub reserved: [u8; 55],
}

                                                                               
                                                                            
pub const FUNDING_WINDOW_SEED: &[u8] = b"funding_window";
#[account]
#[derive(InitSpace)]
pub struct FundingWindow {
    pub vault: Pubkey,
    pub version: u8,
    pub closed_epoch: u64,
    pub scheduled_start: i64,
    pub deadline: i64,
    pub eligible: bool,
}
impl FundingWindow {
    pub fn require_current(&self, vault: &Vault, vault_key: Pubkey) -> Result<()> {
        require!(self.version == 1 && self.vault == vault_key
            && self.closed_epoch.checked_add(1) == Some(vault.epoch), crate::error::FlpError::InvalidFundingWindow);
        Ok(())
    }
    pub fn is_open(&self, now: i64) -> bool {
        self.eligible && now >= self.scheduled_start && now <= self.deadline
    }
}
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum FundingStatusV2 {
    Fundable = 0,
    NonpositiveNav = 1,
    CashShortage = 2,
    BankLiquidity = 4,
    CoreFundingRequired = 6,
}
