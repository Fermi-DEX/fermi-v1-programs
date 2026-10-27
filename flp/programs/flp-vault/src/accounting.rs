                                                                           
                                                            
use crate::{
    error::FlpError,
    math,
    nav::{self, NavComputation},
    state::*,
};
use anchor_lang::prelude::*;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FeeAccrual {
    pub management: u64,
    pub performance: u64,
    pub supply: u64,
    pub hwm: u128,
}
impl FeeAccrual {
    pub fn calculate(vault: &Vault, nav_low: i64, supply: u64, now: i64) -> Result<Self> {
        let management = math::mgmt_fee_shares(
            supply,
            vault.params.mgmt_fee_bps,
            now.checked_sub(vault.last_fee_accrual_ts)
                .ok_or(FlpError::MathOverflow)?,
        )?;
        let supply = supply
            .checked_add(management)
            .ok_or(FlpError::MathOverflow)?;
        let (performance, hwm) = if vault.is_unwinding() {
            (0, vault.hwm_price_e12)
        } else {
            math::perf_fee(
                nav_low,
                supply,
                vault.hwm_price_e12,
                vault.perf_fee_bps_now(),
            )?
        };
        Ok(Self {
            management,
            performance,
            supply: supply
                .checked_add(performance)
                .ok_or(FlpError::MathOverflow)?,
            hwm,
        })
    }
    pub fn total(&self) -> Result<u64> {
        self.management
            .checked_add(self.performance)
            .ok_or_else(|| error!(FlpError::MathOverflow))
    }
}

#[derive(Clone, Copy, Debug)]
pub struct PricingSnapshot {
    pub nav: NavSnapshot,
    pub supply: u64,
    pub withdrawal_ratio: (u128, u128),
}
impl PricingSnapshot {
    pub fn new(nav: NavSnapshot, supply: u64) -> Result<Self> {
        Ok(Self {
            withdrawal_ratio: math::withdraw_ratio(supply, nav.nav_low)?,
            nav,
            supply,
        })
    }
    pub fn deposit_ratio(&self) -> Result<(u128, u128)> {
        math::deposit_ratio(self.supply, self.nav.nav_high)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct BatchPayout {
    pub shares: u64,
    pub gross: u64,
    pub fee: u64,
    pub net: u64,
}
impl BatchPayout {
    pub fn new(shares: u64, ratio: (u128, u128), fee_bps: u16) -> Result<Self> {
        let gross = math::apply_ratio_floor(shares, ratio)?;
        let fee = math::fee_floor(gross, fee_bps)?;
        Ok(Self {
            shares,
            gross,
            fee,
            net: gross - fee,
        })
    }
}

pub struct FundingCheck {
    pub idle: u64,
    pub core: u64,
    pub ok: bool,
                                                                         
                                                                      
    pub reason: u8,
}
impl FundingCheck {
    pub fn calculate(
        nav: &NavComputation,
        idle: u64,
        payout: &BatchPayout,
        bank_vault: &AccountInfo,
        full_recovery_ready: bool,
    ) -> Result<Self> {
        let idle = idle.min(payout.net);
        let core = payout.net - idle;
        let reason = if nav.snapshot.nav_low < 0
            || (nav.snapshot.nav_low == 0 && !full_recovery_ready) { 1 }
            else if core == 0 { 0 }
            else if core > nav.snapshot.spot_quote.max(0) as u64 { 2 }
            else if core > nav::token_account_amount(bank_vault)? { 4 }
            else { 0 };
        let ok = reason == 0;
        Ok(Self {
            idle,
            core,
            ok,
            reason,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anchor_lang::{AccountDeserialize, Discriminator, Space};

    fn vault() -> Vault {
        let mut bytes = vec![0; 8 + Vault::INIT_SPACE];
        bytes[..8].copy_from_slice(&Vault::discriminator());
        Vault::try_deserialize(&mut bytes.as_slice()).unwrap()
    }

    #[test]
    fn settlement_dependency_preserves_but_never_starts_failure_clock() {
        let mut v = vault();
        v.set_head_skip(5, 100);
        assert_eq!(v.head_skip(), (5, 0));
        assert_eq!(v.permissionless_trigger_at(false), None);
        v.set_head_skip(2, 200);
        assert_eq!(v.head_skip(), (2, 200));
        v.set_head_skip(5, 300);
        assert_eq!(v.head_skip(), (5, 200));
        assert_eq!(v.permissionless_trigger_at(false), None);
        v.set_head_skip(3, 400);
        assert_eq!(v.head_skip(), (3, 200));
        v.clear_head_skip();
        v.set_head_skip(5, 500);
        assert_eq!(v.head_skip(), (5, 0));
    }

    #[test]
    fn fee_policy_preserves_pre_graduation_and_recovery_marks() {
        let mut v = vault();
        v.params.mgmt_fee_bps = 100;
        v.params.perf_fee_bps = 2000;
        v.hwm_price_e12 = math::PRICE_SCALE;
        let now = math::SECONDS_PER_YEAR as i64;
        v.status = status::PRE_DEPOSIT;
        let pre = FeeAccrual::calculate(&v, 220_000_000, 99_000_000, now).unwrap();
        assert_eq!(
            (pre.management, pre.performance, pre.supply),
            (1_000_000, 0, 100_000_000)
        );
        assert_eq!(pre.hwm, 221_000_000u128 * math::PRICE_SCALE / 101_000_000);
        v.status = status::UNWIND;
        let recovery = FeeAccrual::calculate(&v, 220_000_000, 99_000_000, now).unwrap();
        assert_eq!(
            (recovery.management, recovery.performance, recovery.hwm),
            (1_000_000, 0, v.hwm_price_e12)
        );
        v.status = 0;
        let live = FeeAccrual::calculate(&v, 220_000_000, 99_000_000, now).unwrap();
        assert!(live.performance > 0 && live.hwm > v.hwm_price_e12);
        assert_eq!(live.supply, 99_000_000 + live.total().unwrap());
                                                                         
        v.params.perf_fee_bps = 0;
        assert_eq!(
            FeeAccrual::calculate(&v, 220_000_000, 99_000_000, now).unwrap(),
            pre
        );
    }

    #[test]
    fn one_snapshot_pays_each_batch_at_the_same_conservative_price() {
        let pricing = PricingSnapshot::new(
            NavSnapshot {
                nav_low: 100_000_000,
                nav_high: 120_000_000,
                ..NavSnapshot::default()
            },
            99_000_000,
        )
        .unwrap();
        let a = BatchPayout::new(30_000_000, pricing.withdrawal_ratio, 10).unwrap();
        let b = BatchPayout::new(20_000_000, pricing.withdrawal_ratio, 10).unwrap();
        assert_eq!((a.gross, a.fee, a.net), (30_300_000, 30_300, 30_269_700));
        assert_eq!((b.gross, b.fee, b.net), (20_200_000, 20_200, 20_179_800));
        assert_eq!(
            math::apply_ratio_floor(121_000_000, pricing.deposit_ratio().unwrap()).unwrap(),
            100_000_000
        );
    }

    #[test]
    fn equity_window_is_bounded_like_every_other_parameter() {
        let mut params = VaultParams {
            epoch_duration_s: 259_200,
            request_cutoff_s: 43_200,
            equity_window_s: MIN_EQUITY_WINDOW_S,
            ..VaultParams::default()
        };
        assert!(params.validate().is_ok());
        params.equity_window_s = MAX_EQUITY_WINDOW_S;
        assert!(params.validate().is_ok());
        params.equity_window_s = MIN_EQUITY_WINDOW_S - 1;
        assert!(params.validate().is_err(), "a window shorter than five minutes widens nothing");
                                                                 
        params.equity_window_s = 64_800;
        assert!(params.validate().is_ok());
        params.equity_window_s = 64_801;
        assert!(params.validate().is_err(), "more than a quarter of the epoch is a timing oracle");
                                                  
        params.epoch_duration_s = 3_600;
        params.request_cutoff_s = 600;
        params.equity_window_s = MAX_EQUITY_WINDOW_S;
        assert!(params.validate().is_ok());
        params.equity_window_s = MAX_EQUITY_WINDOW_S + 1;
        assert!(params.validate().is_err(), "a window longer than two hours is a timing oracle");
    }

    #[test]
    fn delegate_delay_matches_configured_parameters() {
        let mut params = VaultParams::default();
        for delay in [0, 3, 100, 86_399, 86_400, 172_800] {
            params.timelock_s = delay;
            assert_eq!(params.delegate_delay_s(), i64::from(delay));
        }
    }

    #[test]
    fn wire_pending_slots_preserve_successor_requests_across_many_closes() {
        let mut v = vault();
        for slot in &mut v.pending {
            slot.epoch = NONE_EPOCH;
        }
        v.params.epoch_duration_s = 3600;
        v.params.request_cutoff_s = 600;
        for e in 0..20 {
            assert_eq!(v.target_epoch(v.epoch_end() - 601), e);
            assert_eq!(v.target_epoch(v.epoch_end() - 600), e + 1);
            v.pending_slot_mut(e).unwrap().deposits_gross += 10;
            v.pending_slot_mut(e + 1).unwrap().deposits_gross = 20;
            assert_eq!(v.total_pending_deposits(), if e == 0 { 30 } else { 50 });
            v.clear_pending(e);
            v.epoch += 1;
            v.epoch_started_at += 3600;
            assert_eq!(v.pending_for(e + 1).deposits_gross, 20);
        }
    }
}


                                                                                       
pub fn record_immediate_redemption(v: &mut Vault, d: &mut Depositor,
    shares: u64, net: u64, now: i64) -> Result<()> {
    let held = d.shares_held;
    let basis = if held == 0 {
        0
    } else {
        math::to_u64(math::mul_div_floor(d.cost_basis as u128, shares.min(held) as u128, held as u128)?)?
    };
    d.cost_basis -= basis;
    d.shares_held = held.saturating_sub(shares);
    if held > 0 && d.shares_held == 0 && d.pre_deposit_amount == 0 {
        v.stats.active_depositors = v.stats.active_depositors.saturating_sub(1);
    }
    d.total_withdrawn = d.total_withdrawn.checked_add(net).ok_or(FlpError::MathOverflow)?;
    d.realized_pnl = d
        .realized_pnl
        .checked_add(i64::try_from(net).map_err(|_| FlpError::MathOverflow)?)
        .and_then(|x| x.checked_sub(i64::try_from(basis).ok()?))
        .ok_or(FlpError::MathOverflow)?;
    d.last_action_ts = now;
    v.stats.total_withdrawn_net = v
        .stats
        .total_withdrawn_net
        .checked_add(net)
        .ok_or(FlpError::MathOverflow)?;
    v.stats.net_deposits = v
        .stats
        .net_deposits
        .checked_sub(i64::try_from(net).map_err(|_| FlpError::MathOverflow)?)
        .ok_or(FlpError::MathOverflow)?;
    Ok(())
}
