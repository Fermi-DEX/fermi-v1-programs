use crate::error::*;
use crate::state::*;
use anchor_lang::prelude::*;
use anchor_lang::solana_program::sysvar::instructions as tx_instructions;

#[derive(Accounts)]
pub struct ExecutionQueueV2InitAuthorityState<'info> {
    #[account(
        mut,
        constraint = group.load()?.admin == admin.key() @ FermiError::SomeError,
    )]
    pub group: AccountLoader<'info, Group>,
    #[account(
        init,
        payer = payer,
        space = EXECUTION_QUEUE_AUTHORITY_STATE_SPACE,
        seeds = [b"queue-authority".as_ref(), group.key().as_ref()],
        bump,
    )]
    pub authority_state: Account<'info, ExecutionQueueAuthorityState>,
    #[account(mut)]
    pub payer: Signer<'info>,
    pub admin: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct ExecutionQueueV2AuthorityAdmin<'info> {
    #[account(
        constraint = group.load()?.admin == admin.key() @ FermiError::SomeError,
    )]
    pub group: AccountLoader<'info, Group>,
    #[account(
        mut,
        has_one = group,
        seeds = [b"queue-authority".as_ref(), group.key().as_ref()],
        bump = authority_state.bump,
    )]
    pub authority_state: Account<'info, ExecutionQueueAuthorityState>,
    pub admin: Signer<'info>,
}

#[derive(Accounts)]
pub struct ExecutionQueueV2CloseAuthorityState<'info> {
    #[account(
        constraint = group.load()?.admin == admin.key() @ FermiError::SomeError,
    )]
    pub group: AccountLoader<'info, Group>,
    #[account(
        mut,
        has_one = group,
        seeds = [b"queue-authority".as_ref(), group.key().as_ref()],
        bump = authority_state.bump,
        close = receiver,
    )]
    pub authority_state: Account<'info, ExecutionQueueAuthorityState>,
    #[account(mut)]
    pub receiver: Signer<'info>,
    pub admin: Signer<'info>,
}

#[derive(Accounts)]
#[instruction(market_index: u16)]
pub struct ExecutionQueueV2Create<'info> {
    #[account(
        constraint = group.load()?.admin == admin.key() @ FermiError::SomeError,
    )]
    pub group: AccountLoader<'info, Group>,
    #[account(
        has_one = group,
        seeds = [b"queue-authority".as_ref(), group.key().as_ref()],
        bump = authority_state.bump,
    )]
    pub authority_state: Account<'info, ExecutionQueueAuthorityState>,
    #[account(
        init,
        payer = payer,
        space = 8,
        seeds = [
            b"execution-queue-v2".as_ref(),
            group.key().as_ref(),
            &market_index.to_le_bytes(),
        ],
        bump,
    )]
    /// CHECK: zero-copy init is deferred until resize/init completes.
    pub queue: UncheckedAccount<'info>,
    #[account(mut)]
    pub payer: Signer<'info>,
    pub admin: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(market_index: u16)]
pub struct ExecutionQueueV2Resize<'info> {
    pub group: AccountLoader<'info, Group>,
    #[account(
        has_one = group,
        seeds = [b"queue-authority".as_ref(), group.key().as_ref()],
        bump = authority_state.bump,
    )]
    pub authority_state: Account<'info, ExecutionQueueAuthorityState>,
    #[account(
        mut,
        seeds = [
            b"execution-queue-v2".as_ref(),
            group.key().as_ref(),
            &market_index.to_le_bytes(),
        ],
        bump,
    )]
    /// CHECK: chunked grow target.
    pub queue: UncheckedAccount<'info>,
    #[account(mut)]
    pub payer: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(market_index: u16)]
pub struct ExecutionQueueV2Init<'info> {
    #[account(
        constraint = group.load()?.admin == admin.key() @ FermiError::SomeError,
    )]
    pub group: AccountLoader<'info, Group>,
    #[account(
        has_one = group,
        seeds = [b"queue-authority".as_ref(), group.key().as_ref()],
        bump = authority_state.bump,
    )]
    pub authority_state: Account<'info, ExecutionQueueAuthorityState>,
    #[account(
        zero,
        seeds = [
            b"execution-queue-v2".as_ref(),
            group.key().as_ref(),
            &market_index.to_le_bytes(),
        ],
        bump,
    )]
    pub queue: AccountLoader<'info, ExecutionQueueV2>,
    pub admin: Signer<'info>,
}

#[derive(Accounts)]
pub struct ExecutionQueueV2Admin<'info> {
    #[account(
        constraint = group.load()?.admin == admin.key() @ FermiError::SomeError,
    )]
    pub group: AccountLoader<'info, Group>,
    #[account(
        has_one = group,
        seeds = [b"queue-authority".as_ref(), group.key().as_ref()],
        bump = authority_state.bump,
    )]
    pub authority_state: Account<'info, ExecutionQueueAuthorityState>,
    #[account(
        mut,
        constraint = queue.load()?.header.group == group.key() @ FermiError::SomeError,
        constraint = queue.load()?.header.authority_state == authority_state.key() @ FermiError::SomeError,
    )]
    pub queue: AccountLoader<'info, ExecutionQueueV2>,
    pub admin: Signer<'info>,
}

#[derive(Accounts)]
pub struct ExecutionQueueV2Close<'info> {
    #[account(
        constraint = group.load()?.admin == admin.key() @ FermiError::SomeError,
    )]
    pub group: AccountLoader<'info, Group>,
    #[account(
        has_one = group,
        seeds = [b"queue-authority".as_ref(), group.key().as_ref()],
        bump = authority_state.bump,
    )]
    pub authority_state: Account<'info, ExecutionQueueAuthorityState>,
    #[account(
        mut,
        constraint = queue.load()?.header.group == group.key() @ FermiError::SomeError,
        constraint = queue.load()?.header.authority_state == authority_state.key() @ FermiError::SomeError,
        close = receiver,
    )]
    pub queue: AccountLoader<'info, ExecutionQueueV2>,
    #[account(mut)]
    pub receiver: Signer<'info>,
    pub admin: Signer<'info>,
}

#[derive(Accounts)]
pub struct ExecutionQueueV2EnqueueCtm<'info> {
    pub group: AccountLoader<'info, Group>,
    #[account(
        mut,
        has_one = group,
        seeds = [b"queue-authority".as_ref(), group.key().as_ref()],
        bump = authority_state.bump,
    )]
    pub authority_state: Account<'info, ExecutionQueueAuthorityState>,
    #[account(
        mut,
        constraint = queue.load()?.header.group == group.key() @ FermiError::SomeError,
        constraint = queue.load()?.header.authority_state == authority_state.key() @ FermiError::SomeError,
    )]
    pub queue: AccountLoader<'info, ExecutionQueueV2>,
    #[account(address = tx_instructions::ID)]
    pub instructions: UncheckedAccount<'info>,
}

#[derive(Accounts)]
pub struct ExecutionQueueV2Execute<'info> {
    #[account(mut)]
    pub group: AccountLoader<'info, Group>,
    #[account(
        mut,
        has_one = group,
        seeds = [b"queue-authority".as_ref(), group.key().as_ref()],
        bump = authority_state.bump,
    )]
    pub authority_state: Account<'info, ExecutionQueueAuthorityState>,
    #[account(
        mut,
        constraint = queue.load()?.header.group == group.key() @ FermiError::SomeError,
        constraint = queue.load()?.header.authority_state == authority_state.key() @ FermiError::SomeError,
    )]
    pub queue: AccountLoader<'info, ExecutionQueueV2>,
}
