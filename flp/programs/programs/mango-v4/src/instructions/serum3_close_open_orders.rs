use anchor_lang::prelude::*;

use crate::accounts_ix::*;
use crate::error::FermiError;
use crate::state::*;

pub fn serum3_close_open_orders(ctx: Context<Serum3CloseOpenOrders>) -> Result<()> {
      
                 
      
    let mut account = ctx.accounts.account.load_full_mut()?;
                            
    require!(
        account.fixed.is_owner_or_delegate(ctx.accounts.owner.key()),
        FermiError::SomeError
    );

    let serum_market = ctx.accounts.serum_market.load()?;

                              
    require!(
        account
            .serum3_orders(serum_market.market_index)?
            .open_orders
            == ctx.accounts.open_orders.key(),
        FermiError::SomeError
    );

      
               
      
    cpi_close_open_orders(ctx.accounts)?;

                                                                                              
                                                                                 
    let (base_position, _) = account.token_position_mut(serum_market.base_token_index)?;
    base_position.decrement_in_use();
    let (quote_position, _) = account.token_position_mut(serum_market.quote_token_index)?;
    quote_position.decrement_in_use();

                                                      
    account.deactivate_serum3_orders(serum_market.market_index)?;

    Ok(())
}

fn cpi_close_open_orders(ctx: &Serum3CloseOpenOrders) -> Result<()> {
    use crate::serum3_cpi;
    let group = ctx.group.load()?;
    serum3_cpi::CloseOpenOrders {
        program: ctx.serum_program.to_account_info(),
        market: ctx.serum_market_external.to_account_info(),
        open_orders: ctx.open_orders.to_account_info(),
        open_orders_authority: ctx.group.to_account_info(),
        sol_destination: ctx.sol_destination.to_account_info(),
    }
    .call(&group)
}
