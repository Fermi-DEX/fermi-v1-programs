use anchor_lang::prelude::*;

use crate::error::*;
use crate::state::*;

fn perp_cancel_order_by_slot_inner<'info>(
    account: &AccountLoader<'info, FermiAccountFixed>,
    owner: Pubkey,
    perp_market: &AccountLoader<'info, PerpMarket>,
    bids: &AccountLoader<'info, BookSide>,
    asks: &AccountLoader<'info, BookSide>,
    slot: u8,
    expected_order_id: u128,
    skip_if_missing: bool,
) -> Result<bool> {
    let account_pk = account.key();
    let mut account = account.load_full_mut()?;
    require!(
        account.fixed.is_owner_or_delegate(owner),
        FermiError::SomeError
    );

    let perp_market = perp_market.load_mut()?;
    let mut book = Orderbook {
        bids: bids.load_mut()?,
        asks: asks.load_mut()?,
    };

    let slot = slot as usize;
                                                                       
                                                                             
    if skip_if_missing {
        let present = account
            .perp_order_by_raw_index(slot)
            .map(|oo| {
                oo.is_active_for_market(perp_market.perp_market_index) && oo.id == expected_order_id
            })
            .unwrap_or(false);
        if !present {
            msg!("perp order {expected_order_id} not at slot {slot}; cancel skipped");
            return Ok(false);
        }
    }
    let oo = account.perp_order_by_raw_index(slot).map_err(|_| {
        error_msg_typed!(
            FermiError::PerpOrderIdNotFound,
            "could not find perp order with id {expected_order_id} at slot {slot} in user account"
        )
    })?;
    require_msg_typed!(
        oo.is_active_for_market(perp_market.perp_market_index) && oo.id == expected_order_id,
        FermiError::PerpOrderIdNotFound,
        "could not find perp order with id {expected_order_id} at slot {slot} in user account"
    );

    book.cancel_order_by_slot(
        &mut account.borrow_mut(),
        &account_pk,
        slot,
        perp_market.perp_market_index,
    )?;

    Ok(true)
}

pub(crate) fn perp_cancel_order_by_slot_from_account_infos<'info>(
    dispatch_accounts: &[AccountInfo<'info>],
    slot: u8,
    expected_order_id: u128,
    skip_if_missing: bool,
) -> Result<bool> {
    require!(
        dispatch_accounts.len() >= 6,
        FermiError::ExecutionQueueDispatchAccountLayoutInvalid
    );

    let group = AccountLoader::try_from(&dispatch_accounts[0])
        .map_err(|_| error!(FermiError::ExecutionQueueDispatchAccountLayoutInvalid))?;
    let account = AccountLoader::try_from(&dispatch_accounts[1])
        .map_err(|_| error!(FermiError::ExecutionQueueDispatchAccountLayoutInvalid))?;
    let owner = *dispatch_accounts[2].key;
    let perp_market = AccountLoader::try_from(&dispatch_accounts[3])
        .map_err(|_| error!(FermiError::ExecutionQueueDispatchAccountLayoutInvalid))?;
    let bids = AccountLoader::try_from(&dispatch_accounts[4])
        .map_err(|_| error!(FermiError::ExecutionQueueDispatchAccountLayoutInvalid))?;
    let asks = AccountLoader::try_from(&dispatch_accounts[5])
        .map_err(|_| error!(FermiError::ExecutionQueueDispatchAccountLayoutInvalid))?;

    super::perp_cancel_order::validate_perp_cancel_order_queue_accounts(
        &group,
        &account,
        &perp_market,
        &bids,
        &asks,
        IxGate::PerpCancelOrderByClientOrderId,
        false,
    )?;

    perp_cancel_order_by_slot_inner(
        &account,
        owner,
        &perp_market,
        &bids,
        &asks,
        slot,
        expected_order_id,
        skip_if_missing,
    )
}
