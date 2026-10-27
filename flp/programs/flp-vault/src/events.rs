                                                                             
                                                                          
                                                                            
use anchor_lang::prelude::*;

                                                                              
                                                                                
pub const EVENT_LOG_BYTES: usize = 256;

                                                                               
pub fn log_event<E: anchor_lang::Discriminator + AnchorSerialize>(event: &E) {
    let mut buffer = [0u8; EVENT_LOG_BYTES];
    buffer[..8].copy_from_slice(&E::DISCRIMINATOR);
    let mut rest = &mut buffer[8..];
    let room = rest.len();
    event.serialize(&mut rest).expect("event exceeds EVENT_LOG_BYTES");
    let len = 8 + room - rest.len();
    anchor_lang::solana_program::log::sol_log_data(&[&buffer[..len]]);
}

#[event]
pub struct FlpMarketCapacityExpanded {
    pub vault: Pubkey,
    pub perp_count: u8,
    pub perp_oo_count: u8,
}

#[event]
pub struct FlpExpandedRecoveryTargets {
    pub vault: Pubkey,
    pub mode: u8,
    pub batch_epoch: u64,
    pub started_at: i64,
    pub market_indexes: [u16; 13],
    pub target_base_lots: [i64; 13],
}

#[event]
pub struct FlpUnwindTargetsExtended {
    pub vault: Pubkey,
    pub batch_epoch: u64,
                                                                            
    pub reason: u8,
    pub target_base_lots: [i64; 4],
}

#[event]
pub struct FlpPreDepositFlow {
    pub vault: Pubkey,
    pub user: Pubkey,
    pub deposit: bool,
    pub amount: u64,
    pub escrow_total: u64,
}

#[event]
pub struct FlpUnwindStarted {
    pub vault: Pubkey,
    pub full: bool,
    pub batch_epoch: u64,
    pub started_at: i64,
}
#[event]
pub struct FlpUnwindStep {
    pub vault: Pubkey,
    pub market_index: u16,
    pub base_lots_before: i64,
    pub target_base_lots: i64,
    pub max_base_lots: i64,
    pub price_lots: i64,
}
#[event]
pub struct FlpUnwindFinished {
    pub vault: Pubkey,
    pub closed: bool,
    pub terminal_assets: u64,
    pub terminal_supply: u64,
}
#[event]
pub struct FlpTerminalRedeemed {
    pub vault: Pubkey,
    pub user: Pubkey,
    pub shares: u64,
    pub payout: u64,
}
#[event]
pub struct FlpRecoveryParamsChanged {
    pub vault: Pubkey,
    pub action: u8,
    pub eta: i64,
    pub params: RecoveryParams,
}
                                                                         
#[event]
pub struct FlpDepositsHeld {
    pub vault: Pubkey,
    pub epoch: u64,
    pub gross: u64,
    pub count: u32,
}
#[event]
pub struct FlpBreakerReset {
    pub vault: Pubkey,
                                                           
    pub price_low_e12: u128,
    pub init_ratio_bps: i64,
    pub by: Pubkey,
}
#[event]
pub struct FlpOrdersCancelled {
    pub vault: Pubkey,
    pub by: Pubkey,
                                    
    pub market_indexes: [u16; 4],
}

use crate::state::{NavSnapshot, RecoveryParams};

#[event]
pub struct FlpVaultInitialized {
    pub vault: Pubkey,
    pub group: Pubkey,
    pub vault_id: u16,
    pub share_mint: Pubkey,
}

#[event]
pub struct FlpFermiAccountCreated {
    pub vault: Pubkey,
    pub fermi_account: Pubkey,
    pub delegate: Pubkey,
}

#[event]
pub struct FlpSeeded {
    pub vault: Pubkey,
    pub amount: u64,
    pub shares: u64,
}

#[event]
pub struct FlpDepositRequested {
    pub vault: Pubkey,
    pub user: Pubkey,
    pub request_id: u64,
    pub epoch: u64,
    pub gross: u64,
    pub net: u64,
}

#[event]
pub struct FlpDepositCancelled {
    pub vault: Pubkey,
    pub user: Pubkey,
    pub request_id: u64,
    pub epoch: u64,
    pub gross: u64,
}

#[event]
pub struct FlpWithdrawRequested {
    pub vault: Pubkey,
    pub user: Pubkey,
    pub request_id: u64,
    pub epoch: u64,
    pub shares: u64,
    pub basis_released: u64,
}

#[event]
pub struct FlpWithdrawCancelled {
    pub vault: Pubkey,
    pub user: Pubkey,
    pub request_id: u64,
    pub epoch: u64,
    pub shares: u64,
}

#[event]
pub struct FlpNav {
    pub vault: Pubkey,
    pub nav: NavSnapshot,
    pub supply: u64,
    pub price_low_e12: u128,
}

#[event]
pub struct FlpFeesAccrued {
    pub vault: Pubkey,
    pub epoch: u64,
    pub mgmt_shares: u64,
    pub perf_shares: u64,
    pub hwm_price_e12: u128,
}

#[event]
pub struct FlpEpochClosed {
    pub vault: Pubkey,
    pub epoch: u64,
    pub nav: NavSnapshot,
    pub supply: u64,
    pub dep_gross: u64,
    pub dep_net: u64,
    pub dep_shares: u64,
    pub wd_shares: u64,
    pub wd_status: u8,
    pub wd_gross: u64,
                                                         
    pub netted: u64,
    pub fermi_deposit: u64,
                                                                          
    pub fermi_withdraw: u64,
}

#[event]
pub struct FlpWithdrawDeferred {
    pub vault: Pubkey,
    pub epoch: u64,
    pub shares: u64,
    pub required: u64,
    pub spot_quote: i64,
    pub init_ratio_after_bps: i64,
                                                                            
                                                                               
                                                                               
    pub reason: u8,
}

#[event]
pub struct FlpWithdrawSettled {
    pub vault: Pubkey,
    pub epoch: u64,
    pub shares: u64,
    pub gross: u64,
    pub fee: u64,
    pub wd_num: u128,
    pub wd_den: u128,
    pub deferred: bool,
}

#[event]
pub struct FlpClaimed {
    pub vault: Pubkey,
    pub user: Pubkey,
    pub request_id: u64,
    pub epoch: u64,
                                                            
    pub kind: u8,
    pub amount_in: u64,
    pub amount_out: u64,
}

#[event]
pub struct FlpDelegateChanged {
    pub vault: Pubkey,
    pub old: Pubkey,
    pub new: Pubkey,
                                                 
    pub action: u8,
    pub eta: i64,
}

#[event]
pub struct FlpStatusChanged {
    pub vault: Pubkey,
    pub old: u8,
    pub new: u8,
    pub by: Pubkey,
}

#[event]
pub struct FlpBreakerTripped {
    pub vault: Pubkey,
    pub price_low_e12: u128,
    pub ref_price_e12: u128,
    pub init_ratio_bps: i64,
}

#[event]
pub struct FlpParamsChanged {
    pub vault: Pubkey,
                                
    pub action: u8,
    pub eta: i64,
}

#[event]
pub struct FlpIdleSwept {
    pub vault: Pubkey,
    pub amount: u64,
}

                                                                               
                                                                                    
                                                                                 
                                                         


#[event]
pub struct FlpDepositPermission {
    pub vault: Pubkey,
    pub user: Pubkey,
    pub approved: bool,
}
#[event]
pub struct FlpPermissionedDeposit {
    pub vault: Pubkey,
    pub user: Pubkey,
    pub amount: u64,
    pub shares: u64,
}
#[event]
pub struct FlpGraduated {
    pub vault: Pubkey,
    pub amount: u64,
    pub shares: u64,
    pub num: u128,
    pub den: u128,
                                                                                            
    pub positive_upnl: u64,
                                                                                         
    pub public_share_bps: u16,
}
#[event]
pub struct FlpPreDepositClaimed {
    pub vault: Pubkey,
    pub user: Pubkey,
    pub amount: u64,
    pub shares: u64,
}

#[event]
pub struct FlpForeignTokenRescued {
    pub vault: Pubkey,
    pub admin: Pubkey,
    pub mint: Pubkey,
    pub amount: u64,
    pub source: Pubkey,
    pub destination: Pubkey,
}

#[event]
pub struct FlpForeignCoreTokenCleared {
    pub vault: Pubkey,
    pub admin: Pubkey,
    pub mint: Pubkey,
    pub token_index: u16,
    pub amount: u64,
    pub destination: Pubkey,
}

#[event]
pub struct FlpEquityClose {
    pub vault: Pubkey,
    pub epoch: u64,
    pub scheduled_end: i64,
    pub eligible: bool,
    pub batches_funded: u8,
    pub remaining_head: u64,
    pub wd_num: u128,
    pub wd_den: u128,
                                                           
    pub ineligible_reason: u8,
}

                                                                            
                                                                                 
                                                                               
                                                                                          
                                                         
#[event]
pub struct FlpEquityFundingSkipped {
    pub vault: Pubkey,
    pub epoch: u64,
    pub pricing_epoch: u64,
    pub reason: u8,
    pub required: u64,
    pub idle: u64,
    pub spot_quote: i64,
    pub init_ratio_after_bps: i64,
}

                                                                            
                                                                
#[event]
pub struct FlpPerpPositionDeactivated {
    pub vault: Pubkey,
    pub market_index: u16,
}










                                                                     
#[event]
pub struct FlpOrdersCancelledExpanded {
    pub vault: Pubkey,
    pub by: Pubkey,
                                    
    pub market_indexes: [u16; 13],
}
                                                                              
                                                                            
                                                            
#[event]
pub struct FlpTerminalResidual {
    pub vault: Pubkey,
    pub reserved_quote: u64,
    pub forfeited_quote: u64,
}
#[event]
pub struct FlpUnwindAborted {
    pub vault: Pubkey,
    pub mode: u8,
    pub batch_epoch: u64,
}
                                                                               
                                                              
#[event]
pub struct FlpBreakerConditionAtClose {
    pub vault: Pubkey,
    pub price_low_e12: u128,
    pub ref_price_e12: u128,
    pub init_ratio_bps: i64,
}

                                                                 
#[event]
pub struct FlpPermissionedWithdrawal {
    pub vault: Pubkey,
    pub user: Pubkey,
    pub shares: u64,
    pub gross: u64,
    pub fee: u64,
    pub net: u64,
}
                                                                          
#[event]
pub struct FlpDeferredCured {
    pub vault: Pubkey,
    pub epoch: u64,
    pub by: Pubkey,
    pub wd_num: u128,
    pub wd_den: u128,
}

#[cfg(test)]
mod tests {
    use super::*;
    use anchor_lang::Event;

    #[test]
    fn event_records_fit_the_log_buffer() {
        let nav = NavSnapshot::default();
        let graduated = FlpGraduated {
            vault: Pubkey::default(), amount: 0, shares: 0, num: 0, den: 0,
            positive_upnl: 0, public_share_bps: 0,
        }.data();
        assert_eq!(graduated.len(), 8 + 90);
        let largest = [
            graduated,
            FlpExpandedRecoveryTargets {
                vault: Pubkey::default(), mode: 0, batch_epoch: 0, started_at: 0,
                market_indexes: [0; 13], target_base_lots: [0; 13],
            }.data(),
            FlpEpochClosed {
                vault: Pubkey::default(), epoch: 0, nav, supply: 0, dep_gross: 0, dep_net: 0,
                dep_shares: 0, wd_shares: 0, wd_status: 0, wd_gross: 0, netted: 0,
                fermi_deposit: 0, fermi_withdraw: 0,
            }.data(),
            FlpOrdersCancelledExpanded { vault: Pubkey::default(), by: Pubkey::default(), market_indexes: [0; 13] }.data(),
        ];
        assert_eq!(largest.iter().map(Vec::len).max(), Some(8 + 179));
        assert!(largest.iter().all(|d| d.len() <= EVENT_LOG_BYTES));
    }
}

                                                                         
#[event]
pub struct FlpBreakerObservation {
    pub vault: Pubkey,
    pub pending_events: u32,
    pub being_liquidated: bool,
}

#[event]
pub struct FlpOperatorUnwindTargetsExtendedV2 {
    pub vault: Pubkey,
    pub batch_epoch: u64,
    pub by: Pubkey,
    pub reduction_bps: u16,
}
                                                         
#[event]
pub struct FlpHealthAssertionV2 {
    pub account: Pubkey,
                                                                                  
    pub metric: u8,
    pub threshold_bps: u16,
    pub passed: bool,
}
