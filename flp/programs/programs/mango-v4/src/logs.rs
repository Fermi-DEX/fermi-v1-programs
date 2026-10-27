use crate::{
    accounts_ix::FlashLoanType,
    state::{OracleType, PerpMarket, PerpPosition},
};
use anchor_lang::prelude::*;
use borsh::BorshSerialize;

#[inline(never)]                            
pub fn emit_stack<T: anchor_lang::Event>(e: T) {
    use std::io::{Cursor, Write};

                                         
    let mut buffer = [0u8; 3000];

    let mut cursor = Cursor::new(&mut buffer[..]);
    cursor.write_all(&T::DISCRIMINATOR).unwrap();
    e.serialize(&mut cursor)
        .expect("event must fit into stack buffer");

    let pos = cursor.position() as usize;
    anchor_lang::solana_program::log::sol_log_data(&[&buffer[..pos]]);
}

pub fn emit_perp_balances(
    fermi_group: Pubkey,
    fermi_account: Pubkey,
    pp: &PerpPosition,
    pm: &PerpMarket,
) {
    emit_stack(PerpBalanceLog {
        fermi_group,
        fermi_account,
        market_index: pm.perp_market_index,
        base_position: pp.base_position_lots(),
        quote_position: pp.quote_position_native().to_bits(),
        long_settled_funding: pp.long_settled_funding.to_bits(),
        short_settled_funding: pp.short_settled_funding.to_bits(),
        long_funding: pm.long_funding.to_bits(),
        short_funding: pm.short_funding.to_bits(),
    });
}

#[event]
pub struct PerpBalanceLog {
    pub fermi_group: Pubkey,
    pub fermi_account: Pubkey,
    pub market_index: u16,
    pub base_position: i64,
    pub quote_position: i128,                 
    pub long_settled_funding: i128,           
    pub short_settled_funding: i128,          
    pub long_funding: i128,                   
    pub short_funding: i128,                  
}

#[event]
pub struct TokenBalanceLog {
    pub fermi_group: Pubkey,
    pub fermi_account: Pubkey,
    pub token_index: u16,                                   
    pub indexed_position: i128,                                                                               
    pub deposit_index: i128,             
    pub borrow_index: i128,              
}

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct FlashLoanTokenDetail {
    pub token_index: u16,
    pub change_amount: i128,
    pub loan: i128,
    pub loan_origination_fee: i128,
    pub deposit_index: i128,
    pub borrow_index: i128,
    pub price: i128,
}

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct FlashLoanTokenDetailV2 {
    pub token_index: u16,

                                                                        
       
                                                                     
       
                                                                             
                                                                       
    pub change_amount: i128,

                                                                                   
    pub loan: i128,

                                                                           
    pub loan_origination_fee: i128,

    pub deposit_index: i128,
    pub borrow_index: i128,
    pub price: i128,

                                                    
       
                                        
    pub deposit_fee: i128,

                                                      
    pub approved_amount: u64,
}

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct FlashLoanTokenDetailV3 {
    pub token_index: u16,

                                                                        
       
                                                                     
       
                                                                             
                                                                       
    pub change_amount: i128,

                                                                                   
    pub loan: i128,

                                                                           
    pub loan_origination_fee: i128,

    pub deposit_index: i128,
    pub borrow_index: i128,
    pub price: i128,

                                                
       
                                        
    pub swap_fee: i128,

                                                      
    pub approved_amount: u64,
}

#[event]
pub struct FlashLoanLog {
    pub fermi_group: Pubkey,
    pub fermi_account: Pubkey,
    pub token_loan_details: Vec<FlashLoanTokenDetail>,
    pub flash_loan_type: FlashLoanType,
}

#[event]
pub struct FlashLoanLogV2 {
    pub fermi_group: Pubkey,
    pub fermi_account: Pubkey,
    pub token_loan_details: Vec<FlashLoanTokenDetailV2>,
    pub flash_loan_type: FlashLoanType,
}

#[event]
pub struct FlashLoanLogV3 {
    pub fermi_group: Pubkey,
    pub fermi_account: Pubkey,
    pub token_loan_details: Vec<FlashLoanTokenDetailV3>,
    pub flash_loan_type: FlashLoanType,
}

#[event]
pub struct WithdrawLog {
    pub fermi_group: Pubkey,
    pub fermi_account: Pubkey,
    pub signer: Pubkey,
    pub token_index: u16,
    pub quantity: u64,
    pub price: i128,          
}

#[event]
pub struct DepositLog {
    pub fermi_group: Pubkey,
    pub fermi_account: Pubkey,
    pub signer: Pubkey,
    pub token_index: u16,
    pub quantity: u64,
    pub price: i128,          
}

#[event]
pub struct FillLog {
    pub fermi_group: Pubkey,
    pub market_index: u16,
    pub taker_side: u8,                             
    pub maker_slot: u8,
    pub maker_out: bool,                                     
    pub timestamp: u64,
    pub seq_num: u64,                           

    pub maker: Pubkey,
    pub maker_order_id: u128,
    pub maker_fee: i128,

                                                                                  
    pub maker_timestamp: u64,

    pub taker: Pubkey,
    pub taker_order_id: u128,
    pub taker_client_order_id: u64,
    pub taker_fee: i128,

    pub price: i64,
    pub quantity: i64,                       
}

#[event]
pub struct FillLogV2 {
    pub fermi_group: Pubkey,
    pub market_index: u16,
    pub taker_side: u8,                             
    pub maker_slot: u8,
    pub maker_out: bool,                                     
    pub timestamp: u64,
    pub seq_num: u64,                           

    pub maker: Pubkey,
    pub maker_client_order_id: u64,
    pub maker_fee: f32,

                                                                                  
    pub maker_timestamp: u64,

    pub taker: Pubkey,
    pub taker_client_order_id: u64,
    pub taker_fee: f32,

    pub price: i64,
    pub quantity: i64,                       
}

#[event]
pub struct FillLogV3 {
    pub fermi_group: Pubkey,
    pub market_index: u16,
    pub taker_side: u8,                             
    pub maker_slot: u8,
    pub maker_out: bool,                                     
    pub timestamp: u64,
    pub seq_num: u64,                           

    pub maker: Pubkey,
    pub maker_client_order_id: u64,
    pub maker_fee: f32,

                                                                                  
    pub maker_timestamp: u64,

    pub taker: Pubkey,
    pub taker_client_order_id: u64,
    pub taker_fee: f32,

    pub price: i64,
    pub quantity: i64,                               
    pub maker_closed_pnl: f64,                             
    pub taker_closed_pnl: f64,                             
}

#[event]
pub struct PerpUpdateFundingLog {
    pub fermi_group: Pubkey,
    pub market_index: u16,
    pub long_funding: i128,
    pub short_funding: i128,
    pub price: i128,
    pub oracle_slot: u64,
    pub stable_price: i128,
    pub fees_accrued: i128,
    pub fees_settled: i128,
    pub open_interest: i64,
    pub instantaneous_funding_rate: i128,
}

#[event]
pub struct PerpUpdateFundingLogV2 {
    pub fermi_group: Pubkey,
    pub market_index: u16,
    pub long_funding: i128,
    pub short_funding: i128,
    pub price: i128,
    pub oracle_slot: u64,
    pub oracle_confidence: i128,
    pub oracle_type: OracleType,
    pub stable_price: i128,
    pub fees_accrued: i128,
    pub fees_settled: i128,
    pub open_interest: i64,
    pub instantaneous_funding_rate: i128,
}

#[event]
pub struct UpdateIndexLog {
    pub fermi_group: Pubkey,
    pub token_index: u16,
    pub deposit_index: i128,            
    pub borrow_index: i128,             
    pub avg_utilization: i128,          
    pub price: i128,                    
    pub stable_price: i128,             
    pub collected_fees: i128,           
    pub loan_fee_rate: i128,            
    pub total_borrows: i128,
    pub total_deposits: i128,
    pub borrow_rate: i128,
    pub deposit_rate: i128,
}

#[event]
pub struct UpdateRateLog {
    pub fermi_group: Pubkey,
    pub token_index: u16,
    pub rate0: i128,             
    pub rate1: i128,             
    pub max_rate: i128,          
}

#[event]
pub struct UpdateRateLogV2 {
    pub fermi_group: Pubkey,
    pub token_index: u16,
                                                                  
    pub rate0: i128,             
    pub util0: i128,             
    pub rate1: i128,             
    pub util1: i128,             
    pub max_rate: i128,          
    pub curve_scaling: f64,
    pub target_utilization: f32,
}

#[event]
pub struct TokenLiqWithTokenLog {
    pub fermi_group: Pubkey,
    pub liqee: Pubkey,
    pub liqor: Pubkey,
    pub asset_token_index: u16,
    pub liab_token_index: u16,
    pub asset_transfer: i128,          
    pub liab_transfer: i128,           
    pub asset_price: i128,             
    pub liab_price: i128,              
    pub bankruptcy: bool,
}

#[event]
pub struct TokenLiqWithTokenLogV2 {
    pub fermi_group: Pubkey,
    pub liqee: Pubkey,
    pub liqor: Pubkey,
    pub asset_token_index: u16,
    pub liab_token_index: u16,
    pub asset_transfer_from_liqee: i128,          
    pub asset_transfer_to_liqor: i128,            
    pub asset_liquidation_fee: i128,              
    pub liab_transfer: i128,                      
    pub asset_price: i128,                        
    pub liab_price: i128,                         
    pub bankruptcy: bool,
}

#[event]
pub struct Serum3OpenOrdersBalanceLog {
    pub fermi_group: Pubkey,
    pub fermi_account: Pubkey,
    pub base_token_index: u16,
    pub quote_token_index: u16,
    pub base_total: u64,
    pub base_free: u64,
    pub quote_total: u64,
    pub quote_free: u64,
    pub referrer_rebates_accrued: u64,
}

#[event]
pub struct Serum3OpenOrdersBalanceLogV2 {
    pub fermi_group: Pubkey,
    pub fermi_account: Pubkey,
    pub market_index: u16,
    pub base_token_index: u16,
    pub quote_token_index: u16,
    pub base_total: u64,
    pub base_free: u64,
    pub quote_total: u64,
    pub quote_free: u64,
    pub referrer_rebates_accrued: u64,
}

#[event]
pub struct OpenbookV2OpenOrdersBalanceLog {
    pub fermi_group: Pubkey,
    pub fermi_account: Pubkey,
    pub market_index: u16,
    pub base_token_index: u16,
    pub quote_token_index: u16,
    pub base_total: u64,
    pub base_free: u64,
    pub quote_total: u64,
    pub quote_free: u64,
    pub referrer_rebates_accrued: u64,
}

#[derive(PartialEq, Copy, Clone, Debug, AnchorSerialize, AnchorDeserialize)]
#[repr(u8)]
pub enum LoanOriginationFeeInstruction {
    Unknown,
    LiqTokenBankruptcy,
    LiqTokenWithToken,
    Serum3LiqForceCancelOrders,
    Serum3PlaceOrder,
    Serum3SettleFunds,
    TokenWithdraw,
    TokenConditionalSwapTrigger,
    OpenbookV2LiqForceCancelOrders,
    OpenbookV2PlaceOrder,
    OpenbookV2SettleFunds,
}

#[event]
pub struct WithdrawLoanOriginationFeeLog {
    pub fermi_group: Pubkey,
    pub fermi_account: Pubkey,
    pub token_index: u16,
    pub loan_origination_fee: i128,          
    pub instruction: LoanOriginationFeeInstruction,
}

#[event]
pub struct WithdrawLoanLog {
    pub fermi_group: Pubkey,
    pub fermi_account: Pubkey,
    pub token_index: u16,
    pub loan_amount: i128,
    pub loan_origination_fee: i128,
    pub instruction: LoanOriginationFeeInstruction,
    pub price: Option<i128>,                                                                                                   
}

#[event]
pub struct TokenLiqBankruptcyLog {
    pub fermi_group: Pubkey,
    pub liqee: Pubkey,
    pub liqor: Pubkey,
    pub liab_token_index: u16,
    pub initial_liab_native: i128,
    pub liab_price: i128,
    pub insurance_token_index: u16,
    pub insurance_transfer: i128,
    pub socialized_loss: i128,
    pub starting_liab_deposit_index: i128,
    pub ending_liab_deposit_index: i128,
}

#[event]
pub struct DeactivateTokenPositionLog {
    pub fermi_group: Pubkey,
    pub fermi_account: Pubkey,
    pub token_index: u16,
    pub cumulative_deposit_interest: f64,
    pub cumulative_borrow_interest: f64,
}

#[event]
pub struct DeactivatePerpPositionLog {
    pub fermi_group: Pubkey,
    pub fermi_account: Pubkey,
    pub market_index: u16,
    pub cumulative_long_funding: f64,
    pub cumulative_short_funding: f64,
    pub maker_volume: u64,
    pub taker_volume: u64,
    pub perp_spot_transfers: i64,
}

#[event]
pub struct TokenMetaDataLog {
    pub fermi_group: Pubkey,
    pub mint: Pubkey,
    pub token_index: u16,
    pub mint_decimals: u8,
    pub oracle: Pubkey,
    pub mint_info: Pubkey,
}

#[event]
pub struct TokenMetaDataLogV2 {
    pub fermi_group: Pubkey,
    pub mint: Pubkey,
    pub token_index: u16,
    pub mint_decimals: u8,
    pub oracle: Pubkey,
    pub fallback_oracle: Pubkey,
    pub mint_info: Pubkey,
}

#[event]
pub struct PerpMarketMetaDataLog {
    pub fermi_group: Pubkey,
    pub perp_market: Pubkey,
    pub perp_market_index: u16,
    pub base_decimals: u8,
    pub base_lot_size: i64,
    pub quote_lot_size: i64,
    pub oracle: Pubkey,
}

#[event]
pub struct Serum3RegisterMarketLog {
    pub fermi_group: Pubkey,
    pub serum_market: Pubkey,
    pub market_index: u16,
    pub base_token_index: u16,
    pub quote_token_index: u16,
    pub serum_program: Pubkey,
    pub serum_program_external: Pubkey,
}

#[event]
pub struct OpenbookV2RegisterMarketLog {
    pub fermi_group: Pubkey,
    pub openbook_market: Pubkey,
    pub market_index: u16,
    pub base_token_index: u16,
    pub quote_token_index: u16,
    pub openbook_program: Pubkey,
    pub openbook_market_external: Pubkey,
}

#[event]
pub struct PerpLiqBaseOrPositivePnlLog {
    pub fermi_group: Pubkey,
    pub perp_market_index: u16,
    pub liqor: Pubkey,
    pub liqee: Pubkey,
    pub base_transfer: i64,
    pub quote_transfer: i128,
    pub pnl_transfer: i128,
    pub pnl_settle_limit_transfer: i128,
    pub price: i128,
}

#[event]
pub struct PerpLiqBaseOrPositivePnlLogV2 {
    pub fermi_group: Pubkey,
    pub perp_market_index: u16,
    pub liqor: Pubkey,
    pub liqee: Pubkey,
    pub base_transfer_liqee: i64,
    pub quote_transfer_liqee: i128,
    pub quote_transfer_liqor: i128,
    pub quote_platform_fee: i128,
    pub pnl_transfer: i128,
    pub pnl_settle_limit_transfer: i128,
    pub price: i128,
}

#[event]
pub struct PerpLiqBaseOrPositivePnlLogV3 {
    pub fermi_group: Pubkey,
    pub perp_market_index: u16,
    pub liqor: Pubkey,
    pub liqee: Pubkey,
    pub base_transfer_liqee: i64,
    pub quote_transfer_liqee: i128,
    pub quote_transfer_liqor: i128,
    pub quote_platform_fee: i128,
    pub pnl_transfer: i128,
    pub pnl_settle_limit_transfer_recurring: i64,
    pub pnl_settle_limit_transfer_oneshot: i64,
    pub price: i128,
}

#[event]
pub struct PerpLiqBankruptcyLog {
    pub fermi_group: Pubkey,
    pub liqee: Pubkey,
    pub liqor: Pubkey,
    pub perp_market_index: u16,
    pub insurance_transfer: i128,
    pub socialized_loss: i128,
    pub starting_long_funding: i128,
    pub starting_short_funding: i128,
    pub ending_long_funding: i128,
    pub ending_short_funding: i128,
}

#[event]
pub struct PerpLiqNegativePnlOrBankruptcyLog {
    pub fermi_group: Pubkey,
    pub liqee: Pubkey,
    pub liqor: Pubkey,
    pub perp_market_index: u16,
    pub settlement: i128,
}

#[event]
pub struct PerpSettlePnlLog {
    pub fermi_group: Pubkey,
    pub fermi_account_a: Pubkey,
    pub fermi_account_b: Pubkey,
    pub perp_market_index: u16,
    pub settlement: i128,
    pub settler: Pubkey,
    pub fee: i128,
}

#[event]
pub struct PerpSettleFeesLog {
    pub fermi_group: Pubkey,
    pub fermi_account: Pubkey,
    pub perp_market_index: u16,
    pub settlement: i128,
}

#[event]
pub struct AccountBuybackFeesWithMngoLog {
    pub fermi_group: Pubkey,
    pub fermi_account: Pubkey,
    pub buyback_fees: i128,
    pub buyback_mngo: i128,
    pub mngo_buyback_price: i128,
    pub oracle_price: i128,
}

#[event]
pub struct FilledPerpOrderLog {
    pub fermi_group: Pubkey,
    pub perp_market_index: u16,
    pub seq_num: u64,
}

#[event]
pub struct PerpTakerTradeLog {
    pub fermi_group: Pubkey,
    pub fermi_account: Pubkey,
    pub perp_market_index: u16,
    pub taker_side: u8,
    pub total_base_lots_taken: i64,                                  
    pub total_base_lots_decremented: i64,                                  
    pub total_quote_lots_taken: i64,                                                        
    pub total_quote_lots_decremented: i64,                                  
    pub taker_fees_paid: i128,                              
    pub fee_penalty: i128,                                  
}

#[event]
pub struct PerpForceClosePositionLog {
    pub fermi_group: Pubkey,
    pub perp_market_index: u16,
    pub account_a: Pubkey,
    pub account_b: Pubkey,
    pub base_transfer: i64,
    pub quote_transfer: i128,
    pub price: i128,
}

#[event]
pub struct TokenForceCloseBorrowsWithTokenLog {
    pub fermi_group: Pubkey,
    pub liqor: Pubkey,
    pub liqee: Pubkey,
    pub asset_token_index: u16,
    pub liab_token_index: u16,
    pub asset_transfer: i128,
    pub liab_transfer: i128,
    pub asset_price: i128,
    pub liab_price: i128,
    pub fee_factor: i128,
}

#[event]
pub struct TokenForceCloseBorrowsWithTokenLogV2 {
    pub fermi_group: Pubkey,
    pub liqor: Pubkey,
    pub liqee: Pubkey,
    pub asset_token_index: u16,
    pub liab_token_index: u16,
    pub asset_transfer_from_liqee: i128,          
    pub asset_transfer_to_liqor: i128,            
    pub asset_liquidation_fee: i128,              
    pub liab_transfer: i128,                      
    pub asset_price: i128,                        
    pub liab_price: i128,                         
                                                     
    pub fee_factor: i128,          
}

#[event]
pub struct TokenConditionalSwapCreateLog {
    pub fermi_group: Pubkey,
    pub fermi_account: Pubkey,
    pub id: u64,
    pub max_buy: u64,
    pub max_sell: u64,
    pub expiry_timestamp: u64,
    pub price_lower_limit: f64,
    pub price_upper_limit: f64,
    pub price_premium_rate: f64,
    pub taker_fee_rate: f32,
    pub maker_fee_rate: f32,
    pub buy_token_index: u16,
    pub sell_token_index: u16,
    pub allow_creating_deposits: bool,
    pub allow_creating_borrows: bool,
}

#[event]
pub struct TokenConditionalSwapCreateLogV2 {
    pub fermi_group: Pubkey,
    pub fermi_account: Pubkey,
    pub id: u64,
    pub max_buy: u64,
    pub max_sell: u64,
    pub expiry_timestamp: u64,
    pub price_lower_limit: f64,
    pub price_upper_limit: f64,
    pub price_premium_rate: f64,
    pub taker_fee_rate: f32,
    pub maker_fee_rate: f32,
    pub buy_token_index: u16,
    pub sell_token_index: u16,
    pub allow_creating_deposits: bool,
    pub allow_creating_borrows: bool,
    pub display_price_style: u8,
    pub intention: u8,
}

#[event]
pub struct TokenConditionalSwapCreateLogV3 {
    pub fermi_group: Pubkey,
    pub fermi_account: Pubkey,
    pub id: u64,
    pub max_buy: u64,
    pub max_sell: u64,
    pub expiry_timestamp: u64,
    pub price_lower_limit: f64,
    pub price_upper_limit: f64,
    pub price_premium_rate: f64,
    pub taker_fee_rate: f32,
    pub maker_fee_rate: f32,
    pub buy_token_index: u16,
    pub sell_token_index: u16,
    pub allow_creating_deposits: bool,
    pub allow_creating_borrows: bool,
    pub display_price_style: u8,
    pub intention: u8,
    pub tcs_type: u8,
    pub start_timestamp: u64,
    pub duration_seconds: u64,
}

#[event]
pub struct TokenConditionalSwapTriggerLog {
    pub fermi_group: Pubkey,
    pub liqee: Pubkey,
    pub liqor: Pubkey,
    pub token_conditional_swap_id: u64,
    pub buy_token_index: u16,
    pub sell_token_index: u16,
    pub buy_amount: u64,                               
    pub sell_amount: u64,                                                
    pub maker_fee: u64,                                                                   
    pub taker_fee: u64,                                                                                    
    pub buy_token_price: i128,          
    pub sell_token_price: i128,          
    pub closed: bool,
}

#[event]
pub struct TokenConditionalSwapTriggerLogV2 {
    pub fermi_group: Pubkey,
    pub liqee: Pubkey,
    pub liqor: Pubkey,
    pub token_conditional_swap_id: u64,
    pub buy_token_index: u16,
    pub sell_token_index: u16,
    pub buy_amount: u64,                               
    pub sell_amount: u64,                                                
    pub maker_fee: u64,                                                                   
    pub taker_fee: u64,                                                                                    
    pub buy_token_price: i128,          
    pub sell_token_price: i128,          
    pub closed: bool,
    pub display_price_style: u8,
    pub intention: u8,
}

#[event]
pub struct TokenConditionalSwapTriggerLogV3 {
    pub fermi_group: Pubkey,
    pub liqee: Pubkey,
    pub liqor: Pubkey,
    pub token_conditional_swap_id: u64,
    pub buy_token_index: u16,
    pub sell_token_index: u16,
    pub buy_amount: u64,                               
    pub sell_amount: u64,                                                
    pub maker_fee: u64,                                                                   
    pub taker_fee: u64,                                                                                    
    pub buy_token_price: i128,          
    pub sell_token_price: i128,          
    pub closed: bool,
    pub display_price_style: u8,
    pub intention: u8,
    pub tcs_type: u8,
    pub start_timestamp: u64,
}

#[event]
pub struct TokenConditionalSwapCancelLog {
    pub fermi_group: Pubkey,
    pub fermi_account: Pubkey,
    pub id: u64,
}

#[event]
pub struct TokenConditionalSwapStartLog {
    pub fermi_group: Pubkey,
    pub fermi_account: Pubkey,
    pub caller: Pubkey,
    pub token_conditional_swap_id: u64,
    pub incentive_token_index: u16,
    pub incentive_amount: u64,
}

#[event]
pub struct TokenCollateralFeeLog {
    pub fermi_group: Pubkey,
    pub fermi_account: Pubkey,
    pub token_index: u16,
    pub asset_usage_fraction: i128,
    pub fee: i128,
    pub price: i128,
}

#[event]
pub struct ForceWithdrawLog {
    pub fermi_group: Pubkey,
    pub fermi_account: Pubkey,
    pub token_index: u16,
    pub quantity: u64,
    pub price: i128,          
    pub to_token_account: Pubkey,
}
