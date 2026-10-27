                                                                                 
use anchor_lang::prelude::*;
use anchor_lang::solana_program::program_pack::Pack;
use fixed::types::I80F48;
use mango_v4::accounts_zerocopy::AccountInfoRef;
use mango_v4::health::{Prices, ScanningAccountRetriever};
use mango_v4::state::{
    Bank, FermiAccountFixed, FermiAccountLoader, OracleAccountInfos,
    TokenIndex,
};

#[cfg(feature = "v5-mm-tests")]
use mango_v4::state::PerpMarketIndex;

use crate::error::FlpError;
use crate::state::NavSnapshot;

pub struct NavComputation {
    pub snapshot: NavSnapshot,
                                                                                 
    pub valuation: crate::valuation::Valuation,
                                                                            
    quote_token_index: TokenIndex,
}

impl NavComputation {
                                                                                           
                                                                               
                                                             
                                                                              
                                                                                  
                                                                                
    pub fn refresh_quote_after_flow<'info>(
        &mut self,
        fermi_account_ai: &AccountInfo<'info>,
        quote_bank_ai: &AccountInfo<'info>,
        upnl_haircut_bps: u16,
    ) -> Result<()> {
        let loader = AccountLoader::<FermiAccountFixed>::try_from(fermi_account_ai)?;
        let account = (&loader).load_full()?;
        let bank_loader = AccountLoader::<Bank>::try_from(quote_bank_ai)?;
        let bank = bank_loader.load()?;
        let native = account.active_token_positions().find(|p| p.token_index == bank.token_index)
            .map(|p| p.native(&bank)).unwrap_or(I80F48::ZERO);
        require!(bank.token_index == self.quote_token_index, FlpError::UnsupportedPortfolio);
        require!(account.active_token_positions().all(|p| p.token_index == self.quote_token_index),
            FlpError::UnsupportedPortfolio);
        self.valuation.quote_cash = native;
        let (mid, low, high, quote) = fold_nav(&self.valuation, upnl_haircut_bps)?;
        self.snapshot = NavSnapshot {
            slot: Clock::get()?.slot,
            nav_mid: i80_to_i64(mid)?,
            nav_low: i80_to_i64(low)?,
            nav_high: i80_to_i64(high.ceil())?,
            init_ratio_bps: crate::state::HEALTH_UNAVAILABLE,
            spot_quote: i80_to_i64(quote)?,
        };
        Ok(())
    }

                                                                                 
                                                          
    pub fn unhaircut_low(&self, haircut_bps: u16) -> Result<i64> {
        let (_, with_haircut, _, _) =
            fold_nav(&self.valuation, haircut_bps)?;
        let (_, without, _, _) =
            fold_nav(&self.valuation, 0)?;
        self.snapshot
            .nav_low
            .checked_add(i80_to_i64(without)? - i80_to_i64(with_haircut)?)
            .ok_or_else(|| error!(FlpError::MathOverflow))
    }

                                                                         
                                                              
    pub fn positive_upnl(&self) -> Result<u64> {
        let mut positive = I80F48::ZERO;
        for pi in &self.valuation.perp_infos {
            let pnl = pi.quote + I80F48::from(pi.base_lots)
                * I80F48::from(pi.base_lot_size) * pi.base_prices.oracle;
            if pnl > 0 {
                positive = positive.checked_add(pnl).ok_or(FlpError::MathOverflow)?;
            }
        }
                                                                                
        positive.checked_to_num::<u64>().ok_or_else(|| error!(FlpError::MathOverflow))
    }

                                                                                  
                                                                                
    pub fn replace_idle_quote(&mut self, before: u64, after: u64) -> Result<()> {
        let delta = i128::from(after) - i128::from(before);
        let adjust = |value: i64| i64::try_from(i128::from(value) + delta)
            .map_err(|_| error!(FlpError::MathOverflow));
        let mid = adjust(self.snapshot.nav_mid)?;
        let low = adjust(self.snapshot.nav_low)?;
        let high = adjust(self.snapshot.nav_high)?;
        self.snapshot.nav_mid = mid;
        self.snapshot.nav_low = low;
        self.snapshot.nav_high = high;
        Ok(())
    }

                                                                                 
                                                                       
                                                                                 
    pub fn include_idle_quote(&mut self, amount: u64) -> Result<()> {
        let amount = i64::try_from(amount).map_err(|_| FlpError::MathOverflow)?;
        for value in [&mut self.snapshot.nav_mid, &mut self.snapshot.nav_low,
            &mut self.snapshot.nav_high] {
            *value = value.checked_add(amount).ok_or(FlpError::MathOverflow)?;
        }
        Ok(())
    }
}

fn i80_to_i64(v: I80F48) -> Result<i64> {
    v.checked_to_num::<i64>()
        .ok_or_else(|| error!(FlpError::MathOverflow))
}

fn lo(a: I80F48, b: I80F48) -> I80F48 {
    a.min(b)
}
fn hi(a: I80F48, b: I80F48) -> I80F48 {
    a.max(b)
}

                                                                          
                                                      
#[cfg(any(test, feature = "v5-mm-tests"))]
fn convert(amount: I80F48, numerator: I80F48, denominator: I80F48) -> Result<I80F48> {
    require!(numerator > 0 && denominator > 0, FlpError::InvalidParams);
    amount.checked_mul(numerator).and_then(|v| v.checked_div(denominator))
        .ok_or_else(|| error!(FlpError::MathOverflow))
}

                                                                             
                                                                             
                                                         
#[cfg(any(test, feature = "v5-mm-tests"))]
fn quote_values(amount: I80F48, same_token: bool, prices: &Prices, quote: &Prices)
    -> Result<(I80F48, I80F48, I80F48)>
{
    if same_token { return Ok((amount, amount, amount)); }
    let mid = convert(amount, prices.oracle, quote.oracle)?;
    let (low_num, low_den, high_num, high_den) = if amount >= 0 {
        (lo(prices.oracle, prices.stable), hi(quote.oracle, quote.stable),
         hi(prices.oracle, prices.stable), lo(quote.oracle, quote.stable))
    } else {
        (hi(prices.oracle, prices.stable), lo(quote.oracle, quote.stable),
         lo(prices.oracle, prices.stable), hi(quote.oracle, quote.stable))
    };
    Ok((mid, convert(amount, low_num, low_den)?, convert(amount, high_num, high_den)?))
}

                                                                     
                                                                           
pub fn fold_nav(cache: &crate::valuation::Valuation, upnl_haircut_bps: u16)
    -> Result<(I80F48, I80F48, I80F48, I80F48)> {
    let mut mid = cache.quote_cash;
    let mut low = cache.quote_cash;
    let mut high = cache.quote_cash;
    let keep = I80F48::from_num(10_000 - upnl_haircut_bps.min(10_000) as i64)
        / I80F48::from_num(10_000);
    for pi in &cache.perp_infos {
        let base = I80F48::from(pi.base_lots) * I80F48::from(pi.base_lot_size);
        let (bo, bs) = (pi.base_prices.oracle, pi.base_prices.stable);
        mid += pi.quote + base * bo;
        let base_low = if base >= 0 { base * lo(bo, bs) } else { base * hi(bo, bs) };
        let mut pnl_low = pi.quote + base_low;
        if pnl_low > 0 { pnl_low *= keep; }
        low += pnl_low;
        let base_high = if base >= 0 { base * hi(bo, bs) } else { base * lo(bo, bs) };
        high += pi.quote + base_high;
    }
    Ok((mid, low, high, cache.quote_cash))
}

                                                                               
                                                                           
pub fn compute_nav<'info>(
    group: &Pubkey,
    fermi_account_ai: &AccountInfo<'info>,
    health_ais: &[AccountInfo<'info>],
    quote_bank_ai: &AccountInfo<'info>,
    quote_oracle_ai: &AccountInfo<'info>,
    upnl_haircut_bps: u16,
) -> Result<NavComputation> {
    compute_nav_inner(group, fermi_account_ai, health_ais, quote_bank_ai,
        quote_oracle_ai, upnl_haircut_bps, None)
}

pub fn compute_projected_nav<'info>(
    group_ai: &AccountInfo<'info>, fermi_account_ai: &AccountInfo<'info>,
    health_ais: &[AccountInfo<'info>], queues: &[AccountInfo<'info>],
    quote_bank_ai: &AccountInfo<'info>, quote_oracle_ai: &AccountInfo<'info>,
    upnl_haircut_bps: u16,
) -> Result<NavComputation> {
    compute_nav_inner(group_ai.key, fermi_account_ai, health_ais, quote_bank_ai,
        quote_oracle_ai, upnl_haircut_bps, Some((group_ai, queues)))
}

fn compute_nav_inner<'info>(
    group: &Pubkey, fermi_account_ai: &AccountInfo<'info>,
    health_ais: &[AccountInfo<'info>], quote_bank_ai: &AccountInfo<'info>,
    quote_oracle_ai: &AccountInfo<'info>, upnl_haircut_bps: u16,
    projection: Option<(&AccountInfo<'info>, &[AccountInfo<'info>])>,
) -> Result<NavComputation> {
    let loader = AccountLoader::<FermiAccountFixed>::try_from(fermi_account_ai)?;
    let account = (&loader).load_full()?;
    require_keys_eq!(account.fixed.group, *group, FlpError::FermiAccountMismatch);
    require!(!account.fixed.being_liquidated(), FlpError::BeingLiquidated);

    let projected = match projection {
        Some((group_ai, queues)) => crate::projection::project(group_ai, fermi_account_ai, health_ais, queues)?,
        None => None,
    };
    let quote_token_index = {
        let bank_loader = AccountLoader::<Bank>::try_from(quote_bank_ai)?;
        let bank = bank_loader.load()?;
        require_keys_eq!(bank.group, *group, FlpError::FermiAccountMismatch);
        bank.token_index
    };
    let retriever = ScanningAccountRetriever::new(health_ais, group)?;
    let cache = match &projected {
        Some(value) => crate::valuation::Valuation::load(&value.borrow(), &retriever, quote_token_index)?,
        None => crate::valuation::Valuation::load(&account.borrow(), &retriever, quote_token_index)?,
    };
    drop(retriever);
    let bank_loader = AccountLoader::<Bank>::try_from(quote_bank_ai)?;
    let bank = bank_loader.load()?;
    let quote_prices = if let Some(prices) = &cache.quote_prices {
        prices.clone()
    } else {
        let oracle_ref = AccountInfoRef::borrow(quote_oracle_ai)?;
        let oracle = OracleAccountInfos::from_reader(&oracle_ref);
        Prices { oracle: bank.oracle_price(&oracle, Some(Clock::get()?.slot))?,
            stable: bank.stable_price() }
    };
    require!(quote_prices.oracle > 0 && quote_prices.stable > 0, FlpError::InvalidParams);
    #[cfg(feature = "v5-mm-tests")]
    let perp_indexes: Vec<PerpMarketIndex> = account
        .active_perp_positions()
        .map(|pp| pp.market_index)
        .collect();

    let (mid, low, high, spot_quote) =
        fold_nav(&cache, upnl_haircut_bps)?;
    drop(bank);
                                                                             
                                                                    
    #[cfg(feature = "v5-mm-tests")]
    {
        let retriever = ScanningAccountRetriever::new(health_ais, group)?;
        let reference = match &projected {
            Some(value) => mango_v4::health::new_health_cache(&value.borrow(), &retriever, Clock::get()?.unix_timestamp as u64)?,
            None => mango_v4::health::new_health_cache(&account.borrow(), &retriever, Clock::get()?.unix_timestamp as u64)?,
        };
        assert_eq!((mid, low, high, spot_quote), reference_fold_nav(&reference, &perp_indexes,
            quote_token_index, &quote_prices, upnl_haircut_bps)?);
    }
    let snapshot = NavSnapshot {
        slot: Clock::get()?.slot,
        nav_mid: i80_to_i64(mid)?,
        nav_low: i80_to_i64(low)?,
        nav_high: i80_to_i64(high.ceil())?,
        init_ratio_bps: crate::state::HEALTH_UNAVAILABLE,
        spot_quote: i80_to_i64(spot_quote)?,
    };
    Ok(NavComputation { snapshot, valuation: cache, quote_token_index })
}

pub fn token_account_amount(ai: &AccountInfo) -> Result<u64> {
    if ai.data_is_empty() {
        return Ok(0);
    }
    require_keys_eq!(*ai.owner, anchor_spl::token::ID, FlpError::WrongTokenAccount);
    let data = ai.try_borrow_data()?;
    let account = anchor_spl::token::spl_token::state::Account::unpack(&data)?;
    Ok(account.amount)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_quote_identity_and_cross_token_bounds() {
        for q in [0.5, 0.9, 1.0, 1.1, 2.0] {
            let quote = Prices { oracle: I80F48::from_num(q), stable: I80F48::ONE };
            let asset = Prices { oracle: I80F48::from_num(80), stable: I80F48::from_num(100) };
            for amount in [-1_000_000i64, -1, 0, 1, 1_000_000] {
                let n = I80F48::from(amount);
                assert_eq!(quote_values(n, true, &quote, &quote).unwrap(), (n, n, n));
                let (mid, low, high) = quote_values(n, false, &asset, &quote).unwrap();
                assert!(low <= mid && mid <= high);
                                                                               
                for a in [asset.oracle, asset.stable] {
                    for b in [quote.oracle, quote.stable] {
                        let value = convert(n, a, b).unwrap();
                        assert!(low <= value && value <= high);
                    }
                }
            }
        }
        let quote = Prices { oracle: I80F48::ZERO, stable: I80F48::ONE };
        assert!(quote_values(I80F48::ONE, false, &Prices::new_single_price(I80F48::ONE), &quote).is_err());
    }
}


#[cfg(feature = "v5-mm-tests")]
fn reference_fold_nav(
    cache: &mango_v4::health::HealthCache,
    perp_market_indexes: &[PerpMarketIndex],
    quote_token_index: TokenIndex,
    quote_prices: &Prices,
    upnl_haircut_bps: u16,
) -> Result<(I80F48, I80F48, I80F48, I80F48)> {
    let mut mid = I80F48::ZERO;
    let mut low = I80F48::ZERO;
    let mut high = I80F48::ZERO;
    let mut spot_quote = I80F48::ZERO;
    for ti in cache.token_infos.iter() {
        let bal = ti.balance_spot;
        let (m, l, h) = quote_values(bal, ti.token_index == quote_token_index,
            &ti.prices, quote_prices)?;
        mid += m;
        low += l;
        high += h;
        if ti.token_index == quote_token_index {
            spot_quote = bal;
        }
    }
    let keep = I80F48::from_num(10_000 - upnl_haircut_bps.min(10_000) as i64)
        / I80F48::from_num(10_000);
    for idx in perp_market_indexes {
        let pi = cache.perp_info(*idx)?;
        let settle = &cache.token_info(pi.settle_token_index)?.prices;
        let base = I80F48::from(pi.base_lots) * I80F48::from(pi.base_lot_size);
        let (bo, bs) = (pi.base_prices.oracle, pi.base_prices.stable);

        let pnl_mid = pi.quote + base * bo;
        let same_token = pi.settle_token_index == quote_token_index;
        mid += quote_values(pnl_mid, same_token, settle, quote_prices)?.0;

        let base_low = if base >= 0 { base * lo(bo, bs) } else { base * hi(bo, bs) };
        let mut pnl_low = pi.quote + base_low;
        if pnl_low > 0 {
            pnl_low *= keep;
        }
        low += quote_values(pnl_low, same_token, settle, quote_prices)?.1;

        let base_high = if base >= 0 { base * hi(bo, bs) } else { base * lo(bo, bs) };
        high += quote_values(pi.quote + base_high, same_token, settle, quote_prices)?.2;
    }
    Ok((mid, low, high, spot_quote))
}
