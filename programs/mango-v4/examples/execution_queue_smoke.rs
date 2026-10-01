/// Smoke checks for the live execution queue implementations.
///
/// This is an executable example instead of a `cargo test` target because the
/// Rust test binaries in this workspace still crash at process startup on this
/// host before any test code runs. Run it with:
///
///   cargo run -p mango-v4 --example execution_queue_smoke --features 'enable-gpl no-entrypoint'
use anchor_lang::prelude::Pubkey;
use mango_v4::state::*;

const TEST_MARKET_A: u16 = 7;
const TEST_MARKET_B: u16 = 11;

fn alloc_zeroed_box<T>() -> Box<T> {
    let layout = std::alloc::Layout::new::<T>();
    let ptr = unsafe { std::alloc::alloc_zeroed(layout) as *mut T };
    assert!(!ptr.is_null());
    unsafe { Box::from_raw(ptr) }
}

fn with_large_stack<F: FnOnce() + Send + 'static>(f: F) {
    let builder = std::thread::Builder::new().stack_size(8 * 1024 * 1024);
    let handle = builder.spawn(f).expect("failed to spawn thread");
    handle.join().expect("smoke thread panicked");
}

fn pending_ctm_item(sequence: u64, accounts_hash_seed: u8) -> QueueItem {
    let mut item = QueueItem::default();
    item.sequence = sequence;
    item.kind = QueueItemKind::CtmWrapped as u8;
    item.status = QueueItemStatus::Pending as u8;
    item.accounts_hash = [accounts_hash_seed; 32];
    item
}

fn pending_liquidity_item(sequence: u64, kind: QueueItemKind) -> QueueItem {
    let mut item = QueueItem::default();
    item.sequence = sequence;
    item.kind = kind as u8;
    item.status = QueueItemStatus::Pending as u8;
    item
}

fn committed_v5_item(sequence: u64, hash_seed: u8) -> CommitItemV5 {
    let mut item = CommitItemV5::default();
    item.sequence = sequence;
    item.commit_hash = [hash_seed; 32];
    item.status = CommitStatusV5::Committed as u8;
    item
}

fn staged_direct_item(kind: u8, payload: &[u8], eligible_at_slot: u64) -> DirectItemV5 {
    let mut item = DirectItemV5::default();
    item.kind = kind;
    item.eligible_at_slot = eligible_at_slot;
    item.payload_len = payload.len() as u16;
    item.payload[..payload.len()].copy_from_slice(payload);
    item
}

fn compat_queue() -> Box<ExecutionQueue> {
    let mut queue = alloc_zeroed_box::<ExecutionQueue>();
    queue.init(
        Pubkey::new_unique(),
        Pubkey::new_unique(),
        Pubkey::new_unique(),
        1,
    );
    queue
}

fn compat_sub_queue_header(queue: &ExecutionQueue, market_index: u16) -> &SubQueueHeader {
    let slot = queue.find_sub_queue_slot(market_index).unwrap();
    &queue.sub_queue_headers[slot]
}

fn compat_sub_queue_header_mut(
    queue: &mut ExecutionQueue,
    market_index: u16,
) -> &mut SubQueueHeader {
    let slot = queue.find_sub_queue_slot(market_index).unwrap();
    &mut queue.sub_queue_headers[slot]
}

fn v2_queue() -> Box<ExecutionQueueV2> {
    let mut queue = alloc_zeroed_box::<ExecutionQueueV2>();
    queue.init(Pubkey::new_unique(), Pubkey::new_unique(), TEST_MARKET_A, 1);
    queue
}

fn v5_queue() -> Box<ExecutionQueueV5> {
    let mut queue = alloc_zeroed_box::<ExecutionQueueV5>();
    queue.init(Pubkey::new_unique(), Pubkey::new_unique(), 1);
    queue
}

fn direct_pool() -> Box<ExecutionQueueV5DirectPool> {
    let mut pool = alloc_zeroed_box::<ExecutionQueueV5DirectPool>();
    pool.init(
        Pubkey::new_unique(),
        Pubkey::new_unique(),
        Pubkey::new_unique(),
        TEST_MARKET_A,
        1,
    );
    pool
}

fn compat_queue_tracks_per_market_heads_and_liquidity() {
    with_large_stack(|| {
        let mut queue = compat_queue();

        queue
            .push_ctm(TEST_MARKET_A, pending_ctm_item(0, 1))
            .unwrap();
        queue
            .push_ctm(TEST_MARKET_A, pending_ctm_item(1, 2))
            .unwrap();
        queue
            .push_ctm(TEST_MARKET_B, pending_ctm_item(0, 3))
            .unwrap();
        queue
            .push_liquidity(pending_liquidity_item(50, QueueItemKind::LiquidityDeposit))
            .unwrap();

        assert_eq!(compat_sub_queue_header(&queue, TEST_MARKET_A).ctm_count, 2);
        assert_eq!(compat_sub_queue_header(&queue, TEST_MARKET_B).ctm_count, 1);
        assert_eq!(queue.global_header.liquidity_count, 1);
        assert_eq!(queue.global_header.total_count, 4);
        assert_eq!(queue.current_ctm_head(TEST_MARKET_A).unwrap().sequence, 0);
        assert_eq!(queue.current_ctm_head(TEST_MARKET_B).unwrap().sequence, 0);

        queue
            .clear_current_ctm_head_and_advance(TEST_MARKET_A)
            .unwrap();
        assert_eq!(
            compat_sub_queue_header(&queue, TEST_MARKET_A).next_sequence_to_execute,
            1
        );
        assert_eq!(
            compat_sub_queue_header(&queue, TEST_MARKET_B).next_sequence_to_execute,
            0
        );
        assert_eq!(queue.current_ctm_head(TEST_MARKET_A).unwrap().sequence, 1);
        assert_eq!(queue.current_ctm_head(TEST_MARKET_B).unwrap().sequence, 0);
        assert_eq!(queue.global_header.total_count, 3);

        queue.clear_ctm_item_at(TEST_MARKET_A, 1).unwrap();
        assert_eq!(compat_sub_queue_header(&queue, TEST_MARKET_A).ctm_count, 0);
        assert_eq!(
            compat_sub_queue_header(&queue, TEST_MARKET_A).next_sequence_to_execute,
            2
        );
        assert!(queue.current_ctm_head(TEST_MARKET_A).is_none());
        assert_eq!(queue.current_ctm_head(TEST_MARKET_B).unwrap().sequence, 0);
        assert_eq!(queue.global_header.total_count, 2);
    });
}

fn compat_queue_finds_hashes_and_normalizes_empty_market_heads() {
    with_large_stack(|| {
        let mut queue = compat_queue();
        queue
            .push_ctm(TEST_MARKET_A, pending_ctm_item(0, 1))
            .unwrap();
        queue
            .push_ctm(TEST_MARKET_A, pending_ctm_item(3, 9))
            .unwrap();

        assert!(queue
            .find_matching_ctm_item(TEST_MARKET_A, &[9; 32], 2)
            .is_none());

        let (sequence, item) = queue
            .find_matching_ctm_item(TEST_MARKET_A, &[9; 32], 4)
            .unwrap();
        assert_eq!(sequence, 3);
        assert_eq!(item.accounts_hash, [9; 32]);

        compat_sub_queue_header_mut(&mut queue, TEST_MARKET_A).gap_observed_slot = 77;
        queue.clear_ctm_item_at(TEST_MARKET_A, 0).unwrap();
        assert_eq!(
            compat_sub_queue_header(&queue, TEST_MARKET_A).next_sequence_to_execute,
            3
        );
        assert_eq!(
            compat_sub_queue_header(&queue, TEST_MARKET_A).gap_observed_slot,
            0
        );

        queue.clear_ctm_item_at(TEST_MARKET_A, 3).unwrap();
        assert_eq!(compat_sub_queue_header(&queue, TEST_MARKET_A).ctm_count, 0);
        assert_eq!(
            compat_sub_queue_header(&queue, TEST_MARKET_A).next_sequence_to_execute,
            4
        );
    });
}

fn execution_queue_v2_clears_heads_and_gaps_and_tracks_retries() {
    with_large_stack(|| {
        let mut queue = v2_queue();
        queue.push(pending_ctm_item(0, 1)).unwrap();
        queue.push(pending_ctm_item(2, 2)).unwrap();

        let retries = queue.increment_retry(0, 55).unwrap();
        assert_eq!(retries, 1);
        assert_eq!(queue.get_item(0).unwrap().first_failure_slot, 55);

        let (sequence, cleared) = queue.clear_head_or_gap().unwrap();
        assert_eq!(sequence, 0);
        assert!(cleared.is_some());
        assert_eq!(queue.header.live_count, 1);
        assert_eq!(queue.header.next_sequence_to_execute, 1);

        let (sequence, cleared) = queue.clear_head_or_gap().unwrap();
        assert_eq!(sequence, 1);
        assert!(cleared.is_none());
        assert_eq!(queue.header.live_count, 1);
        assert_eq!(queue.header.next_sequence_to_execute, 2);
        assert_eq!(queue.get_head_item().unwrap().sequence, 2);

        queue.clear_item_at(2).unwrap();
        assert_eq!(queue.header.live_count, 0);
        assert_eq!(queue.header.next_sequence_to_execute, 3);
    });
}

fn execution_queue_v2_reuses_ring_slots_after_wrap() {
    with_large_stack(|| {
        let mut queue = v2_queue();
        queue.push(pending_ctm_item(0, 1)).unwrap();
        let (sequence, cleared) = queue.clear_head_or_gap().unwrap();
        assert_eq!(sequence, 0);
        assert!(cleared.is_some());
        assert_eq!(queue.header.next_sequence_to_execute, 1);

        let wrap_sequence = EXECUTION_QUEUE_V2_PER_MARKET_CAPACITY as u64;
        queue.push(pending_ctm_item(wrap_sequence, 9)).unwrap();
        assert_eq!(ExecutionQueueV2::physical_index(wrap_sequence), 0);
        assert_eq!(
            queue.get_item(wrap_sequence).unwrap().accounts_hash,
            [9; 32]
        );
        assert_eq!(queue.next_enqueue_sequence(), wrap_sequence + 1);
    });
}

fn execution_queue_v5_commits_clear_heads_and_track_retries() {
    with_large_stack(|| {
        let mut queue = v5_queue();
        let sub_queue_idx = queue.allocate_sub_queue(TEST_MARKET_A, 0, 0, 4).unwrap();

        for seq in 0..3u64 {
            queue.sub_queue_headers[sub_queue_idx]
                .validate_commit_sequence(seq)
                .unwrap();
            queue
                .write_commit(sub_queue_idx, committed_v5_item(seq, seq as u8 + 1))
                .unwrap();
        }

        let retries = queue.increment_retry(sub_queue_idx, 0, 99).unwrap();
        assert_eq!(retries, 1);
        assert_eq!(
            queue.get_item(sub_queue_idx, 0).unwrap().first_failure_slot,
            99
        );
        assert_eq!(queue.header.total_count, 3);
        assert_eq!(queue.get_head_item(sub_queue_idx).unwrap().sequence, 0);

        assert_eq!(queue.clear_head(sub_queue_idx).unwrap(), 0);
        assert_eq!(queue.get_head_item(sub_queue_idx).unwrap().sequence, 1);
        assert_eq!(queue.header.total_count, 2);

        assert_eq!(queue.clear_head(sub_queue_idx).unwrap(), 1);
        assert_eq!(queue.clear_head(sub_queue_idx).unwrap(), 2);
        assert_eq!(queue.header.total_count, 0);
        assert_eq!(
            queue.sub_queue_headers[sub_queue_idx].next_sequence_to_execute,
            3
        );
        assert!(queue.get_head_item(sub_queue_idx).is_none());
    });
}

fn execution_queue_v5_reuses_ring_slots_after_full_capacity_cycle() {
    with_large_stack(|| {
        let mut queue = v5_queue();
        let sub_queue_idx = queue.allocate_sub_queue(TEST_MARKET_A, 0, 0, 4).unwrap();
        let capacity = EXECUTION_QUEUE_V5_PER_MARKET_CAPACITY as u64;

        for seq in 0..capacity {
            queue.sub_queue_headers[sub_queue_idx]
                .validate_commit_sequence(seq)
                .unwrap();
            queue
                .write_commit(sub_queue_idx, committed_v5_item(seq, 1))
                .unwrap();
        }

        for expected in 0..capacity {
            assert_eq!(queue.clear_head(sub_queue_idx).unwrap(), expected);
        }
        assert_eq!(queue.header.total_count, 0);

        queue.sub_queue_headers[sub_queue_idx]
            .validate_commit_sequence(capacity)
            .unwrap();
        queue
            .write_commit(sub_queue_idx, committed_v5_item(capacity, 9))
            .unwrap();

        assert_eq!(
            ExecutionQueueV5::physical_index(
                sub_queue_idx,
                SubQueueHeaderV5::ring_offset_for_sequence(capacity),
            ),
            ExecutionQueueV5::physical_index(sub_queue_idx, 0),
        );
        assert_eq!(
            queue.get_head_item(sub_queue_idx).unwrap().sequence,
            capacity
        );
        assert_eq!(
            queue.get_item(sub_queue_idx, capacity).unwrap().commit_hash,
            [9; 32]
        );
    });
}

fn execution_queue_v5_direct_retire_prefix_requires_in_order_completion() {
    with_large_stack(|| {
        let mut pool = direct_pool();
        pool.header.paused_ingress = 0;

        let first_id = pool
            .write_staged(staged_direct_item(1, &[1, 2, 3], 10))
            .unwrap();
        let second_id = pool
            .write_staged(staged_direct_item(2, &[4, 5], 11))
            .unwrap();
        assert_eq!(pool.staged_head().unwrap().item_id, first_id);

        pool.mark_promoted(first_id, 41).unwrap();
        pool.mark_promoted(second_id, 42).unwrap();

        pool.mark_terminal(second_id, DirectItemStatusV5::Executed)
            .unwrap();
        assert_eq!(pool.header.live_count, 2);
        assert_eq!(pool.header.next_item_to_retire, 0);
        assert!(pool.get_item(second_id).is_some());

        pool.mark_terminal(first_id, DirectItemStatusV5::Failed)
            .unwrap();
        assert_eq!(pool.header.live_count, 0);
        assert_eq!(pool.header.next_item_to_retire, 2);
        assert!(pool.get_item(first_id).is_none());
        assert!(pool.get_item(second_id).is_none());
    });
}

fn run_smoke(name: &str, f: fn()) {
    eprintln!("running {name}");
    f();
}

fn main() {
    run_smoke(
        "compat_queue_tracks_per_market_heads_and_liquidity",
        compat_queue_tracks_per_market_heads_and_liquidity,
    );
    run_smoke(
        "compat_queue_finds_hashes_and_normalizes_empty_market_heads",
        compat_queue_finds_hashes_and_normalizes_empty_market_heads,
    );
    run_smoke(
        "execution_queue_v2_clears_heads_and_gaps_and_tracks_retries",
        execution_queue_v2_clears_heads_and_gaps_and_tracks_retries,
    );
    run_smoke(
        "execution_queue_v2_reuses_ring_slots_after_wrap",
        execution_queue_v2_reuses_ring_slots_after_wrap,
    );
    run_smoke(
        "execution_queue_v5_commits_clear_heads_and_track_retries",
        execution_queue_v5_commits_clear_heads_and_track_retries,
    );
    run_smoke(
        "execution_queue_v5_reuses_ring_slots_after_full_capacity_cycle",
        execution_queue_v5_reuses_ring_slots_after_full_capacity_cycle,
    );
    run_smoke(
        "execution_queue_v5_direct_retire_prefix_requires_in_order_completion",
        execution_queue_v5_direct_retire_prefix_requires_in_order_completion,
    );
    eprintln!("all execution queue smoke checks passed");
}
