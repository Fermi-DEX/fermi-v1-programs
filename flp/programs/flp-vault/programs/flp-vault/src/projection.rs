                                                                             
                                                                                    
use anchor_lang::prelude::*;
use mango_v4::state::{EventQueue, EventType, FermiAccountFixed, FermiAccountLoader,
    FermiAccountValue, FillEvent, Group, OutEvent, PerpMarket, QueueHeader, Side};
use crate::error::FlpError;

pub fn project<'info>(
    group_ai: &AccountInfo<'info>, account_ai: &AccountInfo<'info>,
    health: &[AccountInfo<'info>], queues: &[AccountInfo<'info>],
) -> Result<Option<FermiAccountValue>> {
    let group_loader = AccountLoader::<Group>::try_from(group_ai)?;
    let group = group_loader.load()?;
    let account_loader = AccountLoader::<FermiAccountFixed>::try_from(account_ai)?;
    let account = account_loader.load_full()?;
    require_keys_eq!(account.fixed.group, *group_ai.key, FlpError::FermiAccountMismatch);
    require!(account.active_perp_positions().count() == queues.len(), FlpError::WrongEventQueue);
    let mut projected = None;
    for (position, queue_ai) in account.active_perp_positions().zip(queues) {
        let market_ai = market_for_queue(group_ai.key, position.market_index, health, queue_ai, queues)?;
        replay_market(&mut projected, account_ai, market_ai, queue_ai, &group)?;
    }
    Ok(projected)
}

                                                                          
fn market_for_queue<'a, 'info>(group: &Pubkey, index: u16,
    health: &'a [AccountInfo<'info>], queue_ai: &AccountInfo, queues: &[AccountInfo],
) -> Result<&'a AccountInfo<'info>> {
    let mut found = None;
    for ai in health {
        if ai.owner != &mango_v4::ID || ai.try_borrow_data()?.get(..8)
            != Some(&<PerpMarket as anchor_lang::Discriminator>::DISCRIMINATOR[..]) { continue; }
        let loader = AccountLoader::<PerpMarket>::try_from(ai)?;
        let market = loader.load()?;
        require_keys_eq!(market.group, *group, FlpError::WrongEventQueue);
        if market.perp_market_index == index {
            require!(found.is_none(), FlpError::WrongEventQueue);
            require_keys_eq!(market.event_queue, *queue_ai.key, FlpError::WrongEventQueue);
            found = Some(ai);
        }
    }
    require!(queues.iter().filter(|ai| ai.key == queue_ai.key).count() == 1, FlpError::WrongEventQueue);
    found.ok_or_else(|| error!(FlpError::WrongEventQueue))
}

                                                                            
pub fn pending(group: &Pubkey, account_ai: &AccountInfo,
    health: &[AccountInfo], queues: &[AccountInfo],
) -> Result<u32> {
    let loader = AccountLoader::<FermiAccountFixed>::try_from(account_ai)?;
    let account = loader.load_full()?;
    require_keys_eq!(account.fixed.group, *group, FlpError::FermiAccountMismatch);
    require!(account.active_perp_positions().count() == queues.len(), FlpError::WrongEventQueue);
    let mut count = 0;
    for (position, queue_ai) in account.active_perp_positions().zip(queues) {
        market_for_queue(group, position.market_index, health, queue_ai, queues)?;
        let loader = AccountLoader::<EventQueue>::try_from(queue_ai)?;
        let queue = loader.load()?;
        require!(queue.header.head() < queue.buf.len() && queue.len() <= queue.buf.len(), FlpError::InvalidPendingEvent);
        for event in queue.iter() {
            let relevant = match EventType::try_from(event.event_type) {
                Ok(EventType::Fill) => {
                    let fill: &FillEvent = bytemuck::cast_ref(event);
                    fill.maker == *account_ai.key || fill.taker == *account_ai.key
                }
                Ok(EventType::Out) => {
                    let out = bytemuck::pod_read_unaligned::<OutEvent>(bytemuck::bytes_of(event));
                    out.owner == *account_ai.key
                }
                _ => false,
            };
            if relevant { count += 1; }
        }
    }
    Ok(count)
}

                                                                                  
                                                                                
                                                                                  
#[inline(never)]
fn replay_market<'info>(projected: &mut Option<FermiAccountValue>,
    account_ai: &AccountInfo<'info>, market_ai: &AccountInfo<'info>,
    queue_ai: &AccountInfo<'info>, group: &Group,
) -> Result<()> {
    let market_loader = AccountLoader::<PerpMarket>::try_from(market_ai)?;
    let mut market = *market_loader.load()?;
    let index = market.perp_market_index;
    let queue_loader = AccountLoader::<EventQueue>::try_from(queue_ai)?;
    let queue = queue_loader.load()?;
    require!(queue.header.head() < queue.buf.len() && queue.len() <= queue.buf.len(),
        FlpError::InvalidPendingEvent);
    for event in queue.iter() {
        match EventType::try_from(event.event_type).map_err(|_| error!(FlpError::InvalidPendingEvent))? {
            EventType::Fill => {
                let fill: &FillEvent = bytemuck::cast_ref(event);
                require!(Side::try_from(fill.taker_side).is_ok() && fill.maker_out <= 1
                    && fill.quantity > 0 && fill.price >= 0 && fill.price.checked_mul(fill.quantity).is_some()
                    && fill.maker_fee.is_finite() && fill.taker_fee.is_finite(), FlpError::InvalidPendingEvent);
                if fill.maker != *account_ai.key && fill.taker != *account_ai.key { continue; }
                let account = owned(projected, account_ai)?;
                if fill.maker == *account_ai.key {
                    account.perp_order_by_raw_index(fill.maker_slot as usize)?;
                    account.execute_perp_maker(index, &mut market, fill, group)?;
                }
                if fill.taker == *account_ai.key {
                    account.execute_perp_taker(index, &mut market, fill)?;
                }
            }
            EventType::Out => {
                let out = bytemuck::pod_read_unaligned::<OutEvent>(bytemuck::bytes_of(event));
                let side = Side::try_from(out.side).map_err(|_| error!(FlpError::InvalidPendingEvent))?;
                require!(out.quantity >= 0, FlpError::InvalidPendingEvent);
                if out.owner != *account_ai.key { continue; }
                let account = owned(projected, account_ai)?;
                account.perp_order_by_raw_index(out.owner_slot as usize)?;
                account.execute_perp_out_event(index, side, out.owner_slot as usize, out.quantity, out.order_id)?;
            }
            EventType::Liquidate => (),
        }
    }
    Ok(())
}

fn owned<'a>(copy: &'a mut Option<FermiAccountValue>, ai: &AccountInfo)
    -> Result<&'a mut FermiAccountValue> {
    if copy.is_none() {
        *copy = Some(FermiAccountValue::from_bytes(&ai.try_borrow_data()?[8..])?);
    }
    Ok(copy.as_mut().unwrap())
}
