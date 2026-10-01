use anchor_lang::prelude::*;
use anchor_lang::Discriminator;

use crate::accounts_ix::*;
use crate::error::*;
use crate::state::*;

pub fn admin_withdraw_excess_lamports<'info>(
    ctx: Context<'_, '_, '_, 'info, AdminWithdrawExcessLamports<'info>>,
) -> Result<()> {
    let group_key = ctx.accounts.group.key();
    let target = ctx.accounts.target.to_account_info();

    require_target_is_group_protocol_state(group_key, &target, ctx.remaining_accounts)?;

    let min_balance = Rent::get()?.minimum_balance(target.data_len());
    let destination = ctx.accounts.destination.to_account_info();
    let excess = move_excess_lamports(&target, &destination, min_balance)?;
    if excess == 0 {
        msg!("no excess lamports on {}", target.key);
        return Ok(());
    }

    msg!(
        "withdrew {} excess lamports from {} to {}, remaining {}",
        excess,
        target.key,
        destination.key,
        min_balance
    );
    Ok(())
}

/// Moves every lamport above `min_balance` from `target` to `destination` and
/// returns the amount moved.
fn move_excess_lamports(
    target: &AccountInfo,
    destination: &AccountInfo,
    min_balance: u64,
) -> Result<u64> {
    let excess = target.lamports().saturating_sub(min_balance);
    if excess > 0 {
        **target.try_borrow_mut_lamports()? -= excess;
        **destination.try_borrow_mut_lamports()? += excess;
    }
    Ok(excess)
}

/// Only accounts whose rent was paid by the protocol are eligible. User-owned
/// state such as FermiAccounts is deliberately not accepted.
fn require_target_is_group_protocol_state<'info>(
    group_key: Pubkey,
    target: &AccountInfo<'info>,
    remaining_accounts: &[AccountInfo<'info>],
) -> Result<()> {
    let discriminator: [u8; 8] = {
        let data = target.try_borrow_data()?;
        require!(
            data.len() >= 8,
            FermiError::AdminWithdrawExcessLamportsUnsupportedAccount
        );
        data[..8].try_into().unwrap()
    };

    let target_group = if discriminator == Group::DISCRIMINATOR {
        target.key()
    } else if discriminator == Bank::DISCRIMINATOR {
        AccountLoader::<Bank>::try_from(target)?.load()?.group
    } else if discriminator == MintInfo::DISCRIMINATOR {
        AccountLoader::<MintInfo>::try_from(target)?.load()?.group
    } else if discriminator == PerpMarket::DISCRIMINATOR {
        AccountLoader::<PerpMarket>::try_from(target)?.load()?.group
    } else if discriminator == ExecutionQueueV5::DISCRIMINATOR {
        AccountLoader::<ExecutionQueueV5>::try_from(target)?
            .load()?
            .header
            .group
    } else if discriminator == ExecutionQueueV5DirectPool::DISCRIMINATOR {
        AccountLoader::<ExecutionQueueV5DirectPool>::try_from(target)?
            .load()?
            .header
            .group
    } else if discriminator == ExecutionQueueAuthorityState::DISCRIMINATOR {
        Account::<ExecutionQueueAuthorityState>::try_from(target)?.group
    } else if discriminator == BookSide::DISCRIMINATOR || discriminator == EventQueue::DISCRIMINATOR
    {
        // These carry no group field; tie them to a PerpMarket of this group.
        require_eq!(
            remaining_accounts.len(),
            1,
            FermiError::AdminWithdrawExcessLamportsUnsupportedAccount
        );
        let perp_market_ai = &remaining_accounts[0];
        require_keys_neq!(perp_market_ai.key(), target.key());
        let perp_market_loader = AccountLoader::<PerpMarket>::try_from(perp_market_ai)?;
        let perp_market = perp_market_loader.load()?;
        let linked = if discriminator == BookSide::DISCRIMINATOR {
            perp_market.bids == target.key() || perp_market.asks == target.key()
        } else {
            perp_market.event_queue == target.key()
        };
        require!(
            linked,
            FermiError::AdminWithdrawExcessLamportsUnsupportedAccount
        );
        perp_market.group
    } else {
        return err!(FermiError::AdminWithdrawExcessLamportsUnsupportedAccount);
    };

    require_keys_eq!(target_group, group_key, FermiError::SomeError);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::mem::size_of;

    struct TestAccount {
        key: Pubkey,
        owner: Pubkey,
        lamports: u64,
        data: Vec<u8>,
    }

    impl TestAccount {
        fn new<T: Discriminator + bytemuck::Pod>(key: Pubkey, value: &T) -> Self {
            let mut data = T::DISCRIMINATOR.to_vec();
            data.extend_from_slice(bytemuck::bytes_of(value));
            Self::raw(key, data)
        }

        fn raw(key: Pubkey, data: Vec<u8>) -> Self {
            Self {
                key,
                owner: crate::ID,
                lamports: 1_000,
                data,
            }
        }

        fn info(&mut self) -> AccountInfo {
            AccountInfo::new(
                &self.key,
                false,
                true,
                &mut self.lamports,
                &mut self.data,
                &self.owner,
                false,
                0,
            )
        }
    }

    fn zeroed<T: bytemuck::Zeroable>() -> Box<T> {
        // Large zero-copy types do not fit on the test thread's stack.
        let layout = std::alloc::Layout::new::<T>();
        unsafe {
            let ptr = std::alloc::alloc_zeroed(layout) as *mut T;
            assert!(!ptr.is_null());
            Box::from_raw(ptr)
        }
    }

    fn check(
        group: Pubkey,
        target: &mut TestAccount,
        perp_market: Option<&mut TestAccount>,
    ) -> Result<()> {
        let target_info = target.info();
        let remaining: Vec<AccountInfo> = perp_market.into_iter().map(|a| a.info()).collect();
        require_target_is_group_protocol_state(group, &target_info, &remaining)
    }

    fn unsupported() -> Error {
        error!(FermiError::AdminWithdrawExcessLamportsUnsupportedAccount)
    }

    #[test]
    fn accepts_protocol_state_of_the_group() {
        let group = Pubkey::new_unique();

        let mut group_account = TestAccount::new(group, &*zeroed::<Group>());
        check(group, &mut group_account, None).unwrap();

        let mut bank = zeroed::<Bank>();
        bank.group = group;
        check(
            group,
            &mut TestAccount::new(Pubkey::new_unique(), &*bank),
            None,
        )
        .unwrap();

        let mut mint_info = zeroed::<MintInfo>();
        mint_info.group = group;
        check(
            group,
            &mut TestAccount::new(Pubkey::new_unique(), &*mint_info),
            None,
        )
        .unwrap();

        let mut queue = zeroed::<ExecutionQueueV5>();
        queue.header.group = group;
        check(
            group,
            &mut TestAccount::new(Pubkey::new_unique(), &*queue),
            None,
        )
        .unwrap();

        let mut pool = zeroed::<ExecutionQueueV5DirectPool>();
        pool.header.group = group;
        check(
            group,
            &mut TestAccount::new(Pubkey::new_unique(), &*pool),
            None,
        )
        .unwrap();

        let authority = ExecutionQueueAuthorityState {
            group,
            admin: Pubkey::new_unique(),
            ctm_signer: Pubkey::new_unique(),
            pending_ctm_signer: Pubkey::default(),
            pending_ctm_activation_slot: 0,
            bump: 255,
            _padding: [0; 7],
            reserved: [0; 16],
        };
        let mut data = Vec::new();
        authority.try_serialize(&mut data).unwrap();
        check(
            group,
            &mut TestAccount::raw(Pubkey::new_unique(), data),
            None,
        )
        .unwrap();
    }

    #[test]
    fn book_sides_and_event_queue_need_their_perp_market() {
        let group = Pubkey::new_unique();
        let bids_key = Pubkey::new_unique();
        let asks_key = Pubkey::new_unique();
        let event_queue_key = Pubkey::new_unique();

        let mut perp_market = zeroed::<PerpMarket>();
        perp_market.group = group;
        perp_market.bids = bids_key;
        perp_market.asks = asks_key;
        perp_market.event_queue = event_queue_key;
        let mut perp_market_account = TestAccount::new(Pubkey::new_unique(), &*perp_market);
        check(group, &mut perp_market_account, None).unwrap();

        let mut bids = TestAccount::new(bids_key, &*zeroed::<BookSide>());
        let mut asks = TestAccount::new(asks_key, &*zeroed::<BookSide>());
        let mut event_queue = TestAccount::new(event_queue_key, &*zeroed::<EventQueue>());
        check(group, &mut bids, Some(&mut perp_market_account)).unwrap();
        check(group, &mut asks, Some(&mut perp_market_account)).unwrap();
        check(group, &mut event_queue, Some(&mut perp_market_account)).unwrap();

        // missing perp market
        assert_eq!(check(group, &mut bids, None).unwrap_err(), unsupported());

        // a perp market that does not reference the book side
        let mut other = zeroed::<PerpMarket>();
        other.group = group;
        let mut other_account = TestAccount::new(Pubkey::new_unique(), &*other);
        assert_eq!(
            check(group, &mut bids, Some(&mut other_account)).unwrap_err(),
            unsupported()
        );

        // an event queue is not accepted in place of a book side and vice versa
        let mut swapped = TestAccount::new(event_queue_key, &*zeroed::<BookSide>());
        assert_eq!(
            check(group, &mut swapped, Some(&mut perp_market_account)).unwrap_err(),
            unsupported()
        );

        // the perp market belongs to another group
        assert!(check(
            Pubkey::new_unique(),
            &mut bids,
            Some(&mut perp_market_account)
        )
        .is_err());
    }

    #[test]
    fn rejects_other_groups_and_unsupported_accounts() {
        let group = Pubkey::new_unique();

        let mut bank = zeroed::<Bank>();
        bank.group = Pubkey::new_unique();
        assert!(check(
            group,
            &mut TestAccount::new(Pubkey::new_unique(), &*bank),
            None
        )
        .is_err());

        // another group's Group account
        let mut other_group = TestAccount::new(Pubkey::new_unique(), &*zeroed::<Group>());
        assert!(check(group, &mut other_group, None).is_err());

        // user FermiAccounts are never eligible
        let mut data = FermiAccount::DISCRIMINATOR.to_vec();
        data.resize(8 + size_of::<FermiAccountFixed>(), 0);
        assert_eq!(
            check(
                group,
                &mut TestAccount::raw(Pubkey::new_unique(), data),
                None
            )
            .unwrap_err(),
            unsupported()
        );

        // uninitialized or too-short data
        assert_eq!(
            check(
                group,
                &mut TestAccount::raw(Pubkey::new_unique(), vec![0; 4]),
                None
            )
            .unwrap_err(),
            unsupported()
        );
        assert_eq!(
            check(
                group,
                &mut TestAccount::raw(Pubkey::new_unique(), vec![0; 64]),
                None
            )
            .unwrap_err(),
            unsupported()
        );
    }

    #[test]
    fn moves_only_the_excess() {
        let mut target = TestAccount::raw(Pubkey::new_unique(), vec![]);
        let mut destination = TestAccount::raw(Pubkey::new_unique(), vec![]);
        target.lamports = 1_500;
        destination.lamports = 10;
        {
            let t = target.info();
            let d = destination.info();
            assert_eq!(move_excess_lamports(&t, &d, 1_200).unwrap(), 300);
            assert_eq!(move_excess_lamports(&t, &d, 1_200).unwrap(), 0);
            assert_eq!(move_excess_lamports(&t, &d, 5_000).unwrap(), 0);
        }
        assert_eq!(target.lamports, 1_200);
        assert_eq!(destination.lamports, 310);
    }
}
