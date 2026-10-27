                                                                            
                                                                
   
                                                                        
                                                                              
                                                                      

use crate::error::FlpError;
use anchor_lang::prelude::*;

                                                                               
                                                                                
pub const VIRTUAL_SHARES: u128 = 1_000_000;
pub const VIRTUAL_ASSETS: u128 = 1_000_000;
pub const PRICE_SCALE: u128 = 1_000_000_000_000;
pub const BPS: u128 = 10_000;
pub const SECONDS_PER_YEAR: u128 = 365 * 24 * 60 * 60;

pub fn mul_div_floor(a: u128, b: u128, c: u128) -> Result<u128> {
    require!(c > 0, FlpError::MathOverflow);
    a.checked_mul(b)
        .map(|p| p / c)
        .ok_or_else(|| error!(FlpError::MathOverflow))
}

pub fn mul_div_ceil(a: u128, b: u128, c: u128) -> Result<u128> {
    require!(c > 0, FlpError::MathOverflow);
    let p = a.checked_mul(b).ok_or_else(|| error!(FlpError::MathOverflow))?;
    Ok(p / c + u128::from(p % c != 0))
}

pub fn to_u64(v: u128) -> Result<u64> {
    u64::try_from(v).map_err(|_| error!(FlpError::MathOverflow))
}

                                      
pub fn fee_ceil(amount: u64, bps: u16) -> Result<u64> {
    to_u64(mul_div_ceil(amount as u128, bps as u128, BPS)?)
}

                                                                               
                                                       
pub fn fee_floor(amount: u64, bps: u16) -> Result<u64> {
    to_u64(mul_div_floor(amount as u128, bps as u128, BPS)?)
}

                                                                          
pub fn priced_assets(nav: i64) -> Result<u128> {
    let a = nav as i128 + VIRTUAL_ASSETS as i128;
    require!(a > 0, FlpError::VaultInsolvent);
    Ok(a as u128)
}

pub fn priced_supply(supply: u64) -> u128 {
    supply as u128 + VIRTUAL_SHARES
}

                                                                  
pub fn deposit_ratio(supply: u64, nav_high: i64) -> Result<(u128, u128)> {
    require!(nav_high > 0 || supply == 0, FlpError::VaultInsolvent);
    Ok((priced_supply(supply), priced_assets(nav_high)?))
}

                                                                   
pub fn withdraw_ratio(supply: u64, nav_low: i64) -> Result<(u128, u128)> {
                                                                      
                                                                       
    if nav_low <= 0 { return Ok((0, priced_supply(supply))); }
    Ok((priced_assets(nav_low)?, priced_supply(supply)))
}

                                                                            
                                                                             
                                                                                 
pub fn recovery_withdraw_ratio(supply: u64, nav_low: i64) -> Result<(u128, u128)> {
    require!(supply > 0 && nav_low >= 0, FlpError::VaultInsolvent);
    Ok((nav_low as u128, supply as u128))
}

pub fn apply_ratio_floor(amount: u64, ratio: (u128, u128)) -> Result<u64> {
    to_u64(mul_div_floor(amount as u128, ratio.0, ratio.1)?)
}

                                                                      
pub fn price_e12(nav: i64, supply: u64) -> Result<u128> {
    let a = nav as i128 + VIRTUAL_ASSETS as i128;
    if a <= 0 {
        return Ok(0);
    }
    mul_div_floor(a as u128, PRICE_SCALE, priced_supply(supply))
}

                                                                          
                                                                               
                                                   
pub fn mgmt_fee_shares(supply: u64, bps: u16, dt: i64) -> Result<u64> {
    if bps == 0 || dt <= 0 || supply == 0 {
        return Ok(0);
    }
    let num = (bps as u128)
        .checked_mul(dt as u128)
        .ok_or_else(|| error!(FlpError::MathOverflow))?;
    let den = BPS * SECONDS_PER_YEAR;
    if num >= den {
                                                                               
        return err!(FlpError::MathOverflow);
    }
    to_u64(mul_div_floor(supply as u128, num, den - num)?)
}

                                                                             
                                              
pub fn perf_fee(nav_low: i64, supply: u64, hwm_e12: u128, bps: u16) -> Result<(u64, u128)> {
    let price = price_e12(nav_low, supply)?;
                                                                                
                                                                      
    if bps == 0 {
        return Ok((0, price.max(hwm_e12)));
    }
    if price <= hwm_e12 {
        return Ok((0, hwm_e12));
    }
    let assets = priced_assets(nav_low)?;
    let s = priced_supply(supply);
    let profit = mul_div_floor(price - hwm_e12, s, PRICE_SCALE)?;
    let fee_val = mul_div_floor(profit, bps as u128, BPS)?;
    if fee_val == 0 || fee_val >= assets {
        return Ok((0, price.max(hwm_e12)));
    }
    let shares = to_u64(mul_div_floor(fee_val, s, assets - fee_val)?)?;
    let new_hwm = mul_div_floor(assets, PRICE_SCALE, s + shares as u128)?;
    Ok((shares, new_hwm.max(hwm_e12)))
}

#[cfg(test)]
mod tests {
    use super::*;

                                                                            
    struct Rng(u64);
    impl Rng {
        fn next(&mut self) -> u64 {
            self.0 ^= self.0 << 13;
            self.0 ^= self.0 >> 7;
            self.0 ^= self.0 << 17;
            self.0
        }
        fn below(&mut self, n: u64) -> u64 {
            self.next() % n.max(1)
        }
    }

    #[test]
    fn first_deposit_prices_at_one() {
        let r = deposit_ratio(0, 0).unwrap();
        assert_eq!(apply_ratio_floor(1_000_000, r).unwrap(), 1_000_000);
        assert_eq!(price_e12(0, 0).unwrap(), PRICE_SCALE);
    }

    #[test]
    fn wiped_out_vault_has_no_phantom_payout_or_recapitalization() {
        for nav in [0, -1, -999_999, -1_000_000, i64::MIN] {
            assert_eq!(apply_ratio_floor(1_000_000, withdraw_ratio(1_000_000, nav).unwrap()).unwrap(), 0);
            assert!(deposit_ratio(1_000_000, nav).is_err());
        }
    }

    #[test]
    fn recovery_cash_conservation_across_losses_and_minimum_seed() {
        let mut rng = Rng(0x3141592653589793);
        for dead in [1_000_000u64, 10_000_000] {
            for _ in 0..10_000 {
                let s = dead + 1 + rng.below(1 << 40);
                let a = rng.below(s * 2) as i64;
                let ratio = recovery_withdraw_ratio(s, a).unwrap();
                let left = rng.below(s - dead);
                let right = s - dead - left;
                let paid = apply_ratio_floor(left, ratio).unwrap() + apply_ratio_floor(right, ratio).unwrap();
                assert!(paid <= a as u64);
                assert!(paid <= apply_ratio_floor(s - dead, ratio).unwrap());
                assert!(apply_ratio_floor(s - dead, ratio).unwrap() - paid <= 1);
            }
        }
        assert_eq!(apply_ratio_floor(990_000_000, recovery_withdraw_ratio(1_000_000_000, 0).unwrap()).unwrap(), 0);
        assert!(recovery_withdraw_ratio(1_000_000, -1).is_err());
        assert!(recovery_withdraw_ratio(0, 0).is_err());
    }

    #[test]
    fn deposit_then_withdraw_never_profits() {
                                                                               
                                                        
        let mut rng = Rng(0x9e3779b97f4a7c15);
        for _ in 0..20_000 {
            let supply = rng.below(1 << 50);
            let nav = rng.below(1 << 50) as i64;
            let d = 1 + rng.below(1 << 40);
            let p_before = price_e12(nav, supply).unwrap();
            let shares = apply_ratio_floor(d, deposit_ratio(supply, nav).unwrap()).unwrap();
            let s2 = supply + shares;
            let n2 = nav + d as i64;
            assert!(price_e12(n2, s2).unwrap() >= p_before);
            let back = apply_ratio_floor(shares, withdraw_ratio(s2, n2).unwrap()).unwrap();
            assert!(back <= d, "round trip profited: in {d} out {back}");
            assert!(price_e12(n2 - back as i64, s2 - shares).unwrap() >= p_before);
        }
    }

    #[test]
    fn per_user_floors_never_exceed_aggregate() {
                                                                               
        let mut rng = Rng(42);
        for _ in 0..2_000 {
            let supply = 1 + rng.below(1 << 45);
            let nav = 1 + rng.below(1 << 45) as i64;
            let r = deposit_ratio(supply, nav).unwrap();
            let users: Vec<u64> = (0..1 + rng.below(20)).map(|_| 1 + rng.below(1 << 35)).collect();
            let total: u64 = users.iter().sum();
            let agg = apply_ratio_floor(total, r).unwrap();
            let sum: u64 = users.iter().map(|u| apply_ratio_floor(*u, r).unwrap()).sum();
            assert!(sum <= agg);

            let w = withdraw_ratio(supply, nav).unwrap();
            let bps = rng.below(101) as u16;
            let agg_gross = apply_ratio_floor(total.min(supply), w).unwrap();
            let agg_net = agg_gross - fee_floor(agg_gross, bps).unwrap();
            let mut paid = 0u64;
            let mut left = total.min(supply);
            for u in users {
                let s = u.min(left);
                left -= s;
                let g = apply_ratio_floor(s, w).unwrap();
                paid += g - fee_ceil(g, bps).unwrap();
            }
            assert!(paid <= agg_net);
        }
    }

    #[test]
    fn donation_attack_is_unprofitable() {
                                                                           
                                                                         
                                                                      
        for (donation, victim) in [(1_000_000_000u64, 1_000_000u64), (10u64.pow(12), 5 * 10u64.pow(9))] {
            let a_shares = apply_ratio_floor(1, deposit_ratio(0, 0).unwrap()).unwrap();
            let nav = 1 + donation as i64;
            let v_shares = apply_ratio_floor(victim, deposit_ratio(a_shares, nav).unwrap()).unwrap();
            let s = a_shares + v_shares;
            let n = nav + victim as i64;
            let attacker_out = apply_ratio_floor(a_shares, withdraw_ratio(s, n).unwrap()).unwrap();
            assert!(attacker_out < donation, "attacker recovered {attacker_out} of {donation}");
        }
    }

    #[test]
    fn mgmt_fee_one_year() {
        let s = 1_000_000_000_000u64;
        let shares = mgmt_fee_shares(s, 200, SECONDS_PER_YEAR as i64).unwrap();
                                                 
        let frac = shares as f64 / (s + shares) as f64;
        assert!((frac - 0.02).abs() < 1e-9, "{frac}");
        assert_eq!(mgmt_fee_shares(s, 0, 1000).unwrap(), 0);
    }

    #[test]
    fn perf_fee_above_hwm_only() {
        let s = 1_000_000_000u64;
                                        
        let (sh, hwm) = perf_fee(1_000_000_000, s, PRICE_SCALE, 2000).unwrap();
        assert_eq!((sh, hwm), (0, PRICE_SCALE));
                                       
        let nav = 1_100_000_000i64;
        let (sh, hwm) = perf_fee(nav, s, PRICE_SCALE, 2000).unwrap();
        let value = apply_ratio_floor(sh, withdraw_ratio(s + sh, nav).unwrap()).unwrap();
        let expected = 20_000_000u64;                       
        assert!(value.abs_diff(expected) <= 2_000, "{value} vs {expected}");
        assert!(hwm > PRICE_SCALE && hwm <= price_e12(nav, s).unwrap());
                                                    
        let (sh2, hwm2) = perf_fee(1_050_000_000, s, hwm, 2000).unwrap();
        assert_eq!((sh2, hwm2), (0, hwm));
                                                                                 
                                                                   
        let (sh3, hwm3) = perf_fee(nav, s, PRICE_SCALE, 0).unwrap();
        assert_eq!((sh3, hwm3), (0, price_e12(nav, s).unwrap()));
        assert_eq!(perf_fee(nav, s, hwm3, 2000).unwrap(), (0, hwm3));
        assert_eq!(perf_fee(1_050_000_000, s, hwm3, 0).unwrap(), (0, hwm3), "fee off never lowers the mark");
    }

    #[test]
    fn insolvent_nav_rejected() {
        assert!(deposit_ratio(10, -(VIRTUAL_ASSETS as i64)).is_err());
        assert_eq!(price_e12(-(VIRTUAL_ASSETS as i64) - 5, 10).unwrap(), 0);
    }
}
