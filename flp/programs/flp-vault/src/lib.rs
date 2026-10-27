                                                                     
   
                                                                           
                                                                            
                                                                        
                                                                             
                                                                        
   
                                                               
   
                                                                          
                                                                             
                                                                     
                                                                        
                                                                          

use anchor_lang::prelude::*;

                                                                               
                                                                            
                                                                              
                                                                          
                                                
macro_rules! emit_event {
    ($event:expr) => {
        $crate::events::log_event(&$event)
    };
}

pub mod accounting;
pub mod allocator;
pub mod error;
pub mod events;
pub mod fermi_cpi;
pub mod instructions;
pub mod math;
pub mod nav;
pub mod valuation;
pub mod projection;
pub mod state;

use instructions::*;
use state::{RecoveryParams, VaultParams};

declare_id!("FLPRrDPN5tJpLx6wY6FUiGFN5Kws8gHUCYMCcs6Goa9Q");

#[program]
pub mod flp_vault {
    use super::*;

    pub fn expand_market_capacity(ctx: Context<ExpandMarketCapacity>, perp_count: u8, perp_oo_count: u8) -> Result<()> {
        instructions::expand_market_capacity(ctx, perp_count, perp_oo_count)
    }

                                                                              

    pub fn initialize_vault_v4(
        ctx: Context<InitializeVault>,
        vault_id: u16,
        params: VaultParams,
        guardian: Pubkey,
        fee_recipient: Pubkey,
    ) -> Result<()> {
        instructions::initialize_vault(ctx, vault_id, params, guardian, fee_recipient)
    }

    pub fn seed_vault_v4<'info>(
        ctx: Context<'_, '_, '_, 'info, SeedVault<'info>>,
        amount: u64,
    ) -> Result<()> {
        instructions::seed_vault(ctx, amount)
    }

                                                                              

    pub fn pre_deposit_v3(ctx: Context<PreDepositFunds>, amount: u64) -> Result<()> {
        instructions::pre_deposit(ctx, amount)
    }

    pub fn pre_withdraw_v3(ctx: Context<PreDepositFunds>, amount: u64) -> Result<()> {
        instructions::pre_withdraw(ctx, amount)
    }

    pub fn set_deposit_permission(
        ctx: Context<SetDepositPermission>,
        approved: bool,
    ) -> Result<()> {
        instructions::set_deposit_permission(ctx, approved)
    }

    pub fn permissioned_deposit<'info>(
        ctx: Context<'_, '_, '_, 'info, PermissionedDeposit<'info>>,
        amount: u64,
        min_shares: u64,
    ) -> Result<()> {
        instructions::permissioned_deposit(ctx, amount, min_shares)
    }

    pub fn graduate<'info>(ctx: Context<'_, '_, '_, 'info, Graduate<'info>>) -> Result<()> {
        instructions::graduate(ctx)
    }

    pub fn claim_pre_deposit(ctx: Context<ClaimPreDeposit>) -> Result<()> {
        instructions::claim_pre_deposit(ctx)
    }

    pub fn request_deposit(ctx: Context<RequestDeposit>, amount: u64) -> Result<()> {
        instructions::request_deposit(ctx, amount)
    }

    pub fn cancel_deposit(ctx: Context<CancelDeposit>) -> Result<()> {
        instructions::cancel_deposit(ctx)
    }
    pub fn refund_pending_deposit(ctx: Context<RefundPendingDeposit>) -> Result<()> {
        instructions::refund_pending_deposit(ctx)
    }

    pub fn claim_deposit(ctx: Context<ClaimDeposit>) -> Result<()> {
        instructions::claim_deposit(ctx)
    }

    pub fn request_withdraw(ctx: Context<RequestWithdraw>, shares: u64) -> Result<()> {
        instructions::request_withdraw(ctx, shares)
    }

    pub fn cancel_withdraw(ctx: Context<CancelWithdraw>) -> Result<()> {
        instructions::cancel_withdraw(ctx)
    }

    pub fn claim_withdrawal(ctx: Context<ClaimWithdrawal>) -> Result<()> {
        instructions::claim_withdrawal(ctx)
    }

                                                                            

                                                                         

    pub fn close_epoch_v4<'info>(
        ctx: Context<'_, '_, '_, 'info, CloseEpoch<'info>>,
        deferred_batch_count: u8,
    ) -> Result<()> {
        instructions::close_epoch(ctx, deferred_batch_count)
    }

    pub fn extend_partial_unwind_v2<'info>(ctx: Context<'_, '_, '_, 'info, ExtendPartialUnwindV2<'info>>, reduction_bps: u16) -> Result<()> {
        instructions::extend_partial_unwind_v2(ctx, reduction_bps)
    }

    pub fn fund_head_v2<'info>(ctx: Context<'_, '_, '_, 'info, FundHeadV2<'info>>) -> Result<()> {
        instructions::fund_head_v2(ctx)
    }

    pub fn sweep_idle_v2<'info>(ctx: Context<'_, '_, '_, 'info, SweepIdle<'info>>, max_amount: u64) -> Result<()> {
        instructions::sweep_idle(ctx, max_amount)
    }

                                                                   
    pub fn deactivate_perp_position<'info>(
        ctx: Context<'_, '_, '_, 'info, DeactivatePerpPosition<'info>>,
    ) -> Result<()> {
        instructions::deactivate_perp_position(ctx)
    }

    pub fn check_breaker<'info>(
        ctx: Context<'_, '_, '_, 'info, CheckBreaker<'info>>,
    ) -> Result<()> {
        instructions::check_breaker(ctx)
    }

    pub fn reset_breaker<'info>(
        ctx: Context<'_, '_, '_, 'info, ResetBreaker<'info>>,
    ) -> Result<()> {
        instructions::reset_breaker(ctx)
    }

    pub fn cancel_orders<'info>(
        ctx: Context<'_, '_, '_, 'info, CancelOrders<'info>>,
    ) -> Result<()> {
        instructions::cancel_orders(ctx)
    }

    pub fn trigger_unwind<'info>(
        ctx: Context<'_, '_, '_, 'info, TriggerUnwind<'info>>,
        full: bool,
    ) -> Result<()> {
        instructions::trigger_unwind(ctx, full)
    }
    pub fn unwind_step_v2<'info>(ctx: Context<'_, '_, '_, 'info, UnwindStep<'info>>, max_base_lots: i64) -> Result<()> {
        instructions::unwind_step(ctx, max_base_lots)
    }
    pub fn rescue_foreign_token(ctx: Context<RescueForeignToken>, amount: u64) -> Result<()> {
        instructions::rescue_foreign_token(ctx, amount)
    }

    pub fn clear_foreign_core_token<'info>(ctx: Context<'_, '_, '_, 'info, ClearForeignCoreToken<'info>>) -> Result<()> {
        instructions::clear_foreign_core_token(ctx)
    }
    pub fn finish_unwind<'info>(
        ctx: Context<'_, '_, '_, 'info, FinishUnwind<'info>>,
    ) -> Result<()> {
        instructions::finish_unwind(ctx)
    }
    pub fn terminal_redeem(ctx: Context<TerminalRedeem>, shares: u64) -> Result<()> {
        instructions::terminal_redeem(ctx, shares)
    }
    pub fn stage_recovery_params(ctx: Context<AdminOnly>, params: RecoveryParams) -> Result<()> {
        instructions::stage_recovery_params(ctx, params)
    }
    pub fn apply_recovery_params(ctx: Context<AdminOnly>) -> Result<()> {
        instructions::apply_recovery_params(ctx)
    }

                                                                            

    pub fn propose_delegate(ctx: Context<AdminOnly>, delegate: Pubkey) -> Result<()> {
        instructions::propose_delegate(ctx, delegate)
    }

    pub fn activate_delegate(ctx: Context<EditDelegate>) -> Result<()> {
        instructions::activate_delegate(ctx)
    }

    pub fn remove_delegate(ctx: Context<EditDelegate>) -> Result<()> {
        instructions::remove_delegate(ctx)
    }

    pub fn set_status(ctx: Context<SetStatus>, set_flags: u8, clear_flags: u8) -> Result<()> {
        instructions::set_status(ctx, set_flags, clear_flags)
    }

    pub fn stage_params_v3(ctx: Context<AdminOnly>, params: VaultParams) -> Result<()> {
        instructions::stage_params(ctx, params)
    }

                                                                               
                                                                               
    pub fn apply_params<'info>(ctx: Context<'_, '_, '_, 'info, AdminOnly<'info>>) -> Result<()> {
        instructions::apply_params(ctx)
    }

    pub fn set_roles(
        ctx: Context<AdminOnly>,
        guardian: Pubkey,
        fee_recipient: Pubkey,
    ) -> Result<()> {
        instructions::set_roles(ctx, guardian, fee_recipient)
    }

    pub fn propose_admin(ctx: Context<AdminOnly>, new_admin: Pubkey) -> Result<()> {
        instructions::propose_admin(ctx, new_admin)
    }

    pub fn accept_admin(ctx: Context<AcceptAdmin>) -> Result<()> {
        instructions::accept_admin(ctx)
    }

                                                                             
                                                                               
    pub fn abort_unwind<'info>(ctx: Context<'_, '_, '_, 'info, AdminOnly<'info>>) -> Result<()> {
        instructions::abort_unwind(ctx)
    }

                                                                              
    pub fn init_fermi_account_v4(
        ctx: Context<InitFermiAccountV4>,
        perp_count: u8,
        perp_oo_count: u8,
        token_count: u8,
    ) -> Result<()> {
        instructions::init_fermi_account_v4(ctx, perp_count, perp_oo_count, token_count)
    }

                                                                                                
    pub fn cure_deferred_v2<'info>(ctx: Context<'_, '_, '_, 'info, CureDeferred<'info>>) -> Result<()> {
        instructions::cure_deferred(ctx)
    }

                                                                                    
    pub fn trigger_partial_unwind<'info>(
        ctx: Context<'_, '_, '_, 'info, TriggerUnwind<'info>>,
        fraction_bps: u16,
    ) -> Result<()> {
        instructions::trigger_partial_unwind(ctx, fraction_bps)
    }

                                                                             
    pub fn permissioned_withdraw<'info>(
        ctx: Context<'_, '_, '_, 'info, PermissionedWithdraw<'info>>,
        shares: u64,
        min_quote: u64,
    ) -> Result<()> {
        instructions::permissioned_withdraw(ctx, shares, min_quote)
    }

                                                                         
    pub fn init_depositor(ctx: Context<InitDepositor>) -> Result<()> {
        instructions::init_depositor(ctx)
    }
}
