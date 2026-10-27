use anchor_lang::prelude::*;
use solana_address_lookup_table_program as solana_alt;

use crate::accounts_ix::*;
use crate::error::*;

                                               
pub fn alt_set(ctx: Context<AltSet>, index: u8) -> Result<()> {
    let alt_bytes = ctx.accounts.address_lookup_table.try_borrow_data()?;
    solana_alt::state::AddressLookupTable::deserialize(&alt_bytes)
        .map_err(|e| error_msg!("could not deserialize alt: {}", e))?;

                                      
                                                               
                                                                        
                                                                          
                                                      
      
                 
                                          
                                    
      
                                                                                 
      

    let mut group = ctx.accounts.group.load_mut()?;
    group.address_lookup_tables[index as usize] = ctx.accounts.address_lookup_table.key();
    Ok(())
}
