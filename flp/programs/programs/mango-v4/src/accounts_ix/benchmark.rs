use anchor_lang::prelude::*;

#[derive(Accounts)]
pub struct Benchmark<'info> {
                                                                                            
    pub dummy: UncheckedAccount<'info>,
}
