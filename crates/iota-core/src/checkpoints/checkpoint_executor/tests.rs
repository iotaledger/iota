// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{sync::Arc, time::Duration};

use iota_config::node::ExpensiveSafetyCheckConfig;
use iota_metrics::spawn_monitored_task;
use iota_sdk_types::{CheckpointCommitment, EndOfEpochData, GasCostSummary};
use iota_swarm_config::test_utils::{CommitteeFixture, empty_contents};
use iota_types::{
    committee::ProtocolVersion,
    iota_system_state::epoch_start_iota_system_state::EpochStartSystemState,
    messages_checkpoint::{
        ECMHLiveObjectSetDigest, VerifiedCheckpoint, VerifiedCheckpointContents,
    },
    supported_protocol_versions::SupportedProtocolVersions,
};
use tokio::time::timeout;
use typed_store::Map;

use super::*;
use crate::{
    authority::{
        AuthorityState,
        epoch_start_configuration::{EpochFlag, EpochStartConfiguration},
        test_authority_builder::TestAuthorityBuilder,
    },
    checkpoints::{
        CheckpointStore, EMPTY_CHECKPOINT_CONTENTS_DIGEST, FullCheckpointContentsCache,
        FullCheckpointContentsCacheMetrics, test_checkpoint_with_contents,
    },
    global_state_hasher::GlobalStateHasher,
};

/// The fallback (per-item) load path runs when contents were not synced via
/// state sync — the validator case. It must populate the contents cache so
/// state-sync peers can be served without reconstruction.
#[tokio::test]
pub async fn test_fallback_load_populates_contents_cache() {
    let tmp_dir = iota_common::tempdir();
    let checkpoint_store = CheckpointStore::new(tmp_dir.path());
    let (_state, executor, _hasher, committee) = init_executor_test(checkpoint_store.clone()).await;

    // sync_new_checkpoints persists only the digest-form contents, like a
    // validator's checkpoint builder, so the executor takes the fallback path.
    let checkpoint = sync_new_checkpoints(&checkpoint_store, 1, None, &committee)
        .pop()
        .unwrap();
    let seq = checkpoint.sequence_number();
    assert!(
        checkpoint_store
            .get_full_checkpoint_contents_by_sequence_number(seq)
            .is_none()
    );

    executor.load_checkpoint_transactions(checkpoint.clone());

    let cached = checkpoint_store
        .get_full_checkpoint_contents_by_sequence_number(seq)
        .expect("fallback load should populate the contents cache");
    assert_eq!(
        cached.checkpoint_contents().digest(),
        checkpoint.contents_digest
    );
    // The peer-serving lookup by contents digest must hit too.
    assert!(
        checkpoint_store
            .get_full_checkpoint_contents_by_digest(&checkpoint.contents_digest)
            .is_some()
    );
}

/// With the cache disabled (budget 0), the fallback load must not populate it.
#[tokio::test]
pub async fn test_fallback_load_skips_contents_cache_when_disabled() {
    let tmp_dir = iota_common::tempdir();
    let checkpoint_store = CheckpointStore::new_with_contents_cache(
        tmp_dir.path(),
        FullCheckpointContentsCache::new(0, FullCheckpointContentsCacheMetrics::new_for_tests()),
    );
    let (_state, executor, _hasher, committee) = init_executor_test(checkpoint_store.clone()).await;

    let checkpoint = sync_new_checkpoints(&checkpoint_store, 1, None, &committee)
        .pop()
        .unwrap();
    let seq = checkpoint.sequence_number();

    executor.load_checkpoint_transactions(checkpoint);

    assert!(
        checkpoint_store
            .get_full_checkpoint_contents_by_sequence_number(seq)
            .is_none()
    );
}

/// During deep catch-up the cache window rides the state-sync frontier far
/// ahead of the executor; the fallback load must not displace it with entries
/// that lowest-seq eviction would remove immediately.
#[tokio::test]
pub async fn test_fallback_load_skips_contents_cache_below_window() {
    let tmp_dir = iota_common::tempdir();
    let checkpoint_store = CheckpointStore::new_with_contents_cache(
        tmp_dir.path(),
        // A 1-byte budget any real entry exceeds, so the cache is at budget
        // as soon as the frontier entry below lands.
        FullCheckpointContentsCache::new(1, FullCheckpointContentsCacheMetrics::new_for_tests()),
    );
    let (_state, executor, _hasher, committee) = init_executor_test(checkpoint_store.clone()).await;

    // Simulate the state-sync frontier far ahead of the executor.
    let frontier_seq = 10_000;
    let frontier_contents = FullCheckpointContents::random_for_testing();
    let frontier_checkpoint = test_checkpoint_with_contents(frontier_seq, &frontier_contents);
    checkpoint_store.cache_full_checkpoint_contents(
        frontier_checkpoint.sequence_number(),
        frontier_checkpoint.contents_digest,
        frontier_contents,
    );

    let checkpoint = sync_new_checkpoints(&checkpoint_store, 1, None, &committee)
        .pop()
        .unwrap();
    let seq = checkpoint.sequence_number();

    executor.load_checkpoint_transactions(checkpoint);

    assert!(
        checkpoint_store
            .get_full_checkpoint_contents_by_sequence_number(seq)
            .is_none()
    );
    assert!(
        checkpoint_store
            .get_full_checkpoint_contents_by_sequence_number(frontier_seq)
            .is_some()
    );
}

/// Test checkpoint executor happy path, test that checkpoint executor correctly
/// picks up where it left off in the event of a mid-epoch node crash.
#[tokio::test]
pub async fn test_checkpoint_executor_crash_recovery() {
    telemetry_subscribers::init_for_testing();

    let buffer_size = num_cpus::get() * 2;
    let tmp_dir = iota_common::tempdir();
    let checkpoint_store = CheckpointStore::new(tmp_dir.path());

    let (state, executor, accumulator, committee): (
        Arc<AuthorityState>,
        CheckpointExecutor,
        Arc<GlobalStateHasher>,
        CommitteeFixture,
    ) = init_executor_test(checkpoint_store.clone()).await;

    assert!(
        checkpoint_store
            .get_highest_executed_checkpoint_seq_number()
            .unwrap()
            .is_none()
    );
    let checkpoints = sync_new_checkpoints(&checkpoint_store, buffer_size, None, &committee);

    let epoch_store = state.epoch_store_for_testing().clone();
    let executor_handle = spawn_monitored_task!(async move { executor.run_epoch(None).await });

    // Use a timer to ensure all checkpoints are executed
    let timeout_duration = Duration::from_secs(60);
    tokio::time::timeout(timeout_duration, async {
        loop {
            let highest_executed = checkpoint_store
                .get_highest_executed_checkpoint_seq_number()
                .unwrap()
                .unwrap_or_default();

            if highest_executed == (buffer_size as u64) - 1 {
                break;
            }

            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    })
    .await
    .expect("Timeout waiting for checkpoints to be executed");

    // Simulate node restart
    executor_handle.abort();

    // Sync more checkpoints in the meantime
    let _ = sync_new_checkpoints(
        &checkpoint_store,
        buffer_size,
        Some(checkpoints.last().cloned().unwrap()),
        &committee,
    );

    // Restart checkpoint executor and ensure that it picks up where it left off
    let executor = CheckpointExecutor::new_for_tests(
        epoch_store.clone(),
        checkpoint_store.clone(),
        state.clone(),
        accumulator.clone(),
    );

    let executor_handle = spawn_monitored_task!(async move { executor.run_epoch(None).await });

    // Use a timer to ensure all checkpoints are executed
    tokio::time::timeout(timeout_duration, async {
        loop {
            let highest_executed = checkpoint_store
                .get_highest_executed_checkpoint_seq_number()
                .unwrap()
                .unwrap_or_default();

            if highest_executed == 2 * (buffer_size as u64) - 1 {
                break;
            }

            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    })
    .await
    .expect("Timeout waiting for checkpoints to be executed after restart");

    executor_handle.abort();
}

/// Test that checkpoint execution correctly signals end of epoch after
/// receiving last checkpoint of epoch, then resumes executing checkpoints
/// from the next epoch if called after reconfig
///
/// TODO(william) disabling reconfig unit tests here for now until we can work
/// on correctly inserting transactions, especially the change_epoch tx. As it
/// stands, this is better tested in existing reconfig simtests
#[tokio::test]
#[ignore]
pub async fn test_checkpoint_executor_cross_epoch() {
    let buffer_size = 10;
    let num_to_sync_per_epoch = buffer_size * 2;
    let tmp_dir = iota_common::tempdir();
    let checkpoint_store = CheckpointStore::new(tmp_dir.path());

    let (authority_state, executor, accumulator, first_committee): (
        Arc<AuthorityState>,
        CheckpointExecutor,
        Arc<GlobalStateHasher>,
        CommitteeFixture,
    ) = init_executor_test(checkpoint_store.clone()).await;

    let epoch_store = authority_state.epoch_store_for_testing();
    let epoch = epoch_store.epoch();
    assert_eq!(epoch, 0);

    assert!(
        checkpoint_store
            .get_highest_executed_checkpoint_seq_number()
            .unwrap()
            .is_none()
    );

    // sync 20 checkpoints
    let cold_start_checkpoints = sync_new_checkpoints(
        &checkpoint_store,
        num_to_sync_per_epoch,
        None,
        &first_committee,
    );

    // sync end of epoch checkpoint
    let last_executed_checkpoint = cold_start_checkpoints.last().cloned().unwrap();
    let (end_of_epoch_0_checkpoint, second_committee) = sync_end_of_epoch_checkpoint(
        authority_state.clone(),
        &checkpoint_store,
        last_executed_checkpoint.clone(),
        &first_committee,
    )
    .await;

    // sync 20 more checkpoints
    let next_epoch_checkpoints = sync_new_checkpoints(
        &checkpoint_store,
        num_to_sync_per_epoch,
        Some(end_of_epoch_0_checkpoint.clone()),
        &second_committee,
    );

    authority_state
        .get_checkpoint_store()
        .tables
        .epoch_last_checkpoint_map
        .insert(
            &end_of_epoch_0_checkpoint.epoch,
            &end_of_epoch_0_checkpoint.sequence_number(),
        )
        .unwrap();
    authority_state
        .get_checkpoint_store()
        .tables
        .certified_checkpoints
        .insert(
            &end_of_epoch_0_checkpoint.sequence_number(),
            end_of_epoch_0_checkpoint.serializable_ref(),
        )
        .unwrap();
    // sync end of epoch checkpoint
    let last_executed_checkpoint = next_epoch_checkpoints.last().cloned().unwrap();
    let (_end_of_epoch_1_checkpoint, _third_committee) = sync_end_of_epoch_checkpoint(
        authority_state.clone(),
        &checkpoint_store,
        last_executed_checkpoint.clone(),
        &second_committee,
    )
    .await;

    // Ensure root state hash for epoch does not exist before we close epoch
    assert!(
        authority_state
            .get_global_state_hash_store()
            .get_root_state_hash_for_epoch(0)
            .unwrap()
            .is_none()
    );

    // Ensure executor reaches end of epoch in a timely manner
    timeout(Duration::from_secs(5), async {
        executor.run_epoch(None).await;
    })
    .await
    .unwrap();

    // We should have synced up to epoch boundary
    assert_eq!(
        checkpoint_store
            .get_highest_executed_checkpoint_seq_number()
            .unwrap()
            .unwrap(),
        num_to_sync_per_epoch as u64,
    );

    let first_epoch = 0;

    // Ensure root state hash for epoch exists at end of epoch
    authority_state
        .get_global_state_hash_store()
        .get_root_state_hash_for_epoch(first_epoch)
        .unwrap()
        .expect("root state hash for epoch should exist");

    let system_state = EpochStartSystemState::new_for_testing_with_epoch(1);

    let new_epoch_store = authority_state
        .reconfigure(
            &authority_state.epoch_store_for_testing(),
            SupportedProtocolVersions::SYSTEM_DEFAULT,
            second_committee.committee().clone(),
            EpochStartConfiguration::new(
                system_state,
                Default::default(),
                authority_state.get_object_store(),
                EpochFlag::default_flags_for_new_epoch(&authority_state.config),
            )
            .unwrap(),
            accumulator.clone(),
            &ExpensiveSafetyCheckConfig::default(),
            // Since the expensive checks are disabled, per the line above, the value we pass here
            // won't be used.
            0,
            checkpoint_store
                .get_highest_executed_checkpoint_seq_number()
                .unwrap()
                .unwrap(),
        )
        .await
        .unwrap();

    // Create a new executor for the next epoch since run_epoch consumes self
    let executor = CheckpointExecutor::new_for_tests(
        new_epoch_store.clone(),
        checkpoint_store.clone(),
        authority_state.clone(),
        accumulator.clone(),
    );

    // checkpoint execution should resume starting at checkpoints
    // of next epoch
    timeout(Duration::from_secs(5), async {
        executor.run_epoch(None).await;
    })
    .await
    .unwrap();

    assert_eq!(
        checkpoint_store
            .get_highest_executed_checkpoint_seq_number()
            .unwrap()
            .unwrap(),
        2 * num_to_sync_per_epoch as u64 + 1,
    );

    let second_epoch = 1;
    assert!(second_epoch == new_epoch_store.epoch());

    authority_state
        .get_global_state_hash_store()
        .get_root_state_hash_for_epoch(second_epoch)
        .unwrap()
        .expect("root state hash for epoch should exist");
}

/// Test that if we crash at end of epoch / during reconfig, we recover on
/// startup by starting at the old epoch and immediately retrying reconfig
///
/// TODO(william) disabling reconfig unit tests here for now until we can work
/// on correctly inserting transactions, especially the change_epoch tx. As it
/// stands, this is better tested in existing reconfig simtests
#[tokio::test]
#[ignore]
pub async fn test_reconfig_crash_recovery() {
    let tmp_dir = iota_common::tempdir();
    let checkpoint_store = CheckpointStore::new(tmp_dir.path());

    // new Node (syncing from checkpoint 0)
    let (authority_state, executor, accumulator, first_committee): (
        Arc<AuthorityState>,
        CheckpointExecutor,
        Arc<GlobalStateHasher>,
        CommitteeFixture,
    ) = init_executor_test(checkpoint_store.clone()).await;

    assert!(
        checkpoint_store
            .get_highest_executed_checkpoint_seq_number()
            .unwrap()
            .is_none()
    );

    // sync 1 checkpoint
    let checkpoint = sync_new_checkpoints(&checkpoint_store, 1, None, &first_committee)
        .pop()
        .unwrap();

    // sync end of epoch checkpoint
    let (end_of_epoch_checkpoint, second_committee) = sync_end_of_epoch_checkpoint(
        authority_state.clone(),
        &checkpoint_store,
        checkpoint,
        &first_committee,
    )
    .await;
    // sync 1 more checkpoint
    let _next_epoch_checkpoints = sync_new_checkpoints(
        &checkpoint_store,
        1,
        Some(end_of_epoch_checkpoint.clone()),
        &second_committee,
    );

    timeout(Duration::from_secs(1), async {
        executor.run_epoch(None).await;
    })
    .await
    .unwrap();

    // Check that we stopped execution at epoch boundary
    assert_eq!(
        checkpoint_store
            .get_highest_executed_checkpoint_seq_number()
            .unwrap()
            .unwrap(),
        end_of_epoch_checkpoint.sequence_number(),
    );

    // Drop and re-instantiate checkpoint executor without performing reconfig. This
    // is logically equivalent to reconfig crashing and the node restarting, in
    // which case executor should be able to infer that, rather than beginning
    // execution of the next epoch, we should immediately exit so that reconfig
    // can be reattempted.
    let executor = CheckpointExecutor::new_for_tests(
        authority_state.epoch_store_for_testing().clone(),
        checkpoint_store.clone(),
        authority_state.clone(),
        accumulator.clone(),
    );

    timeout(Duration::from_millis(200), async {
        executor.run_epoch(None).await;
    })
    .await
    .unwrap();

    // Check that we have still not gone beyond epoch boundary
    assert_eq!(
        checkpoint_store
            .get_highest_executed_checkpoint_seq_number()
            .unwrap()
            .unwrap(),
        end_of_epoch_checkpoint.sequence_number(),
    );
}

async fn init_executor_test(
    store: Arc<CheckpointStore>,
) -> (
    Arc<AuthorityState>,
    CheckpointExecutor,
    Arc<GlobalStateHasher>,
    CommitteeFixture,
) {
    let network_config =
        iota_swarm_config::network_config_builder::ConfigBuilder::new_with_temp_dir().build();
    let state = TestAuthorityBuilder::new()
        .with_network_config(&network_config, 0)
        .build()
        .await;

    let accumulator = GlobalStateHasher::new_for_tests(state.get_global_state_hash_store().clone());
    let accumulator = Arc::new(accumulator);

    let executor = CheckpointExecutor::new_for_tests(
        state.epoch_store_for_testing().clone(),
        store.clone(),
        state.clone(),
        accumulator.clone(),
    );
    (
        state,
        executor,
        accumulator,
        CommitteeFixture::from_network_config(&network_config),
    )
}

/// Creates and simulates syncing of a new checkpoint by StateSync, i.e. new
/// checkpoint is persisted, along with its contents, highest synced checkpoint
/// watermark is updated. Returns created checkpoints
fn sync_new_checkpoints(
    checkpoint_store: &CheckpointStore,
    number_of_checkpoints: usize,
    previous_checkpoint: Option<VerifiedCheckpoint>,
    committee: &CommitteeFixture,
) -> Vec<VerifiedCheckpoint> {
    let (ordered_checkpoints, contents, _sequence_number_to_digest, _checkpoints) =
        committee.make_empty_checkpoints(number_of_checkpoints, previous_checkpoint);

    for (checkpoint, content) in ordered_checkpoints.iter().zip(contents.iter()) {
        sync_checkpoint(checkpoint_store, checkpoint, content);
    }

    ordered_checkpoints
}

async fn sync_end_of_epoch_checkpoint(
    authority_state: Arc<AuthorityState>,
    checkpoint_store: &CheckpointStore,
    previous_checkpoint: VerifiedCheckpoint,
    committee: &CommitteeFixture,
) -> (VerifiedCheckpoint, CommitteeFixture) {
    let new_committee = CommitteeFixture::generate(
        rand::rand_core::UnwrapErr(rand::rngs::SysRng),
        committee.committee().epoch + 1,
        4,
    );
    let (_sequence_number, _digest, checkpoint) = committee.make_end_of_epoch_checkpoint(
        previous_checkpoint,
        Some(EndOfEpochData {
            next_epoch_committee: new_committee.committee().committee_members(),
            next_epoch_protocol_version: ProtocolVersion::MIN.as_u64(),
            epoch_commitments: vec![CheckpointCommitment::EcmhLiveObjectSet {
                digest: ECMHLiveObjectSetDigest::default().digest,
            }],
            // Do not simulate supply changes in tests.
            // We would need to build this checkpoint after the below execution of advance_epoch to
            // obtain this number from the SystemEpochInfoEvent.
            epoch_supply_change: 0,
        }),
    );
    authority_state
        .create_and_execute_advance_epoch_tx(
            &authority_state.epoch_store_for_testing().clone(),
            &GasCostSummary::new(0, 0, 0, 0, 0),
            checkpoint.sequence_number(),
            0,      // epoch_start_timestamp_ms
            vec![], // scores
        )
        .await
        .expect("Failed to create and execute advance epoch tx");
    sync_checkpoint(checkpoint_store, &checkpoint, &empty_contents());
    (checkpoint, new_committee)
}

fn sync_checkpoint(
    checkpoint_store: &CheckpointStore,
    checkpoint: &VerifiedCheckpoint,
    contents: &VerifiedCheckpointContents,
) {
    checkpoint_store
        .insert_verified_checkpoint(checkpoint)
        .unwrap();
    checkpoint_store
        .insert_checkpoint_contents(contents.clone().into_checkpoint_contents())
        .unwrap();
    checkpoint_store
        .update_highest_synced_checkpoint(checkpoint)
        .unwrap();
}

/// The synced watermark promises that the contents of every checkpoint up to
/// it are stored: the checkpoint executor loads them as soon as its wait
/// returns, and panics when they are missing. A write that would carry the
/// watermark past a checkpoint whose contents are still being downloaded
/// moves nothing, and the executor waiting for that checkpoint is released
/// once the checkpoint is marked synced itself.
#[tokio::test]
async fn the_synced_watermark_does_not_pass_a_checkpoint_the_write_skips() {
    let tmp_dir = iota_common::tempdir();
    let checkpoint_store = CheckpointStore::new(tmp_dir.path());
    let committee =
        CommitteeFixture::generate(rand::rand_core::UnwrapErr(rand::rngs::SysRng), 0, 4);
    // Random contents, so that no checkpoint finds its contents under the
    // digest of another.
    let (checkpoints, contents, _, _) = committee.make_random_checkpoints(3, None);
    for checkpoint in &checkpoints {
        checkpoint_store
            .insert_verified_checkpoint(checkpoint)
            .unwrap();
    }
    for i in [0, 2] {
        checkpoint_store
            .insert_checkpoint_contents(contents[i].clone().into_checkpoint_contents())
            .unwrap();
    }
    assert!(
        checkpoint_store
            .update_highest_synced_checkpoint(&checkpoints[0])
            .unwrap()
    );

    let downloading = checkpoints[1].sequence_number();
    let mut waiter = tokio::spawn({
        let checkpoint_store = checkpoint_store.clone();
        async move {
            checkpoint_store
                .notify_read_synced_checkpoint(downloading)
                .await
        }
    });

    // Checkpoint 2 is marked synced while the contents of checkpoint 1 are
    // still being downloaded.
    assert!(
        !checkpoint_store
            .update_highest_synced_checkpoint(&checkpoints[2])
            .unwrap()
    );
    assert_eq!(
        checkpoint_store
            .get_highest_synced_checkpoint_seq_number()
            .unwrap(),
        Some(0)
    );
    assert!(
        timeout(Duration::from_millis(200), &mut waiter)
            .await
            .is_err(),
        "the waiter for checkpoint {downloading} was released before its contents were stored"
    );

    // The contents arrive, and the checkpoints are marked synced in order.
    checkpoint_store
        .insert_checkpoint_contents(contents[1].clone().into_checkpoint_contents())
        .unwrap();
    assert!(
        checkpoint_store
            .update_highest_synced_checkpoint(&checkpoints[1])
            .unwrap()
    );
    let released = timeout(Duration::from_secs(5), waiter)
        .await
        .expect("the waiter should be released once its checkpoint is synced")
        .unwrap();
    assert_eq!(released.sequence_number(), downloading);
    assert!(
        checkpoint_store
            .update_highest_synced_checkpoint(&checkpoints[2])
            .unwrap()
    );
}

/// The longest time a test waits for the checkpoint executor when it does not
/// check how long the executor takes.
const EXECUTOR_WAIT_TIMEOUT: Duration = Duration::from_secs(30);

async fn wait_for_executed_checkpoint(
    checkpoint_store: &CheckpointStore,
    seq: CheckpointSequenceNumber,
    wait_timeout: Duration,
) {
    timeout(
        wait_timeout,
        checkpoint_store.notify_read_executed_checkpoint(seq),
    )
    .await
    .unwrap_or_else(|_| panic!("checkpoint {seq} was not executed within {wait_timeout:?}"));
}

/// Checkpoint 1 is verified, but its contents are not stored yet. Any node is
/// in this state while state sync downloads the contents. Then a write tries
/// to mark checkpoint 2 synced. Only a validator makes such a write, for a
/// checkpoint from consensus, and on a validator the contents of checkpoint 1
/// are stored by then. The checkpoint executor loads the contents of each
/// checkpoint that the checkpoint store gives it, and it panics if they are
/// not in the store. The test checks that the executor does not get
/// checkpoint 1 while its contents are not in the store, also after the
/// warning timeout of the executor, when the executor reads the synced
/// watermark again. When the contents of checkpoint 1 are stored and
/// checkpoints 1 and 2 are marked synced, the executor executes both. The test
/// guards the checkpoint store and the executor together, from the side that
/// reads the synced watermark.
#[tokio::test(start_paused = true)]
async fn checkpoint_store_gives_executor_checkpoint_only_after_its_contents_stored() {
    telemetry_subscribers::init_for_testing();
    let tmp_dir = iota_common::tempdir();
    let checkpoint_store = CheckpointStore::new(tmp_dir.path());
    let (_state, executor, _hasher, committee) = init_executor_test(checkpoint_store.clone()).await;
    let (checkpoints, contents, _, _) = committee.make_empty_checkpoints(3, None);
    sync_checkpoint(&checkpoint_store, &checkpoints[0], &contents[0]);
    for checkpoint in &checkpoints[1..] {
        checkpoint_store
            .insert_verified_checkpoint(checkpoint)
            .unwrap();
    }
    // Checkpoints 1 and 2 are empty, and all empty checkpoints share one
    // contents row, which the checkpoint store writes when it opens. Without
    // this row, checkpoints 1 and 2 have no contents, and the executor can
    // still execute them after the test stores the row again.
    checkpoint_store
        .tables
        .checkpoint_content
        .remove(&EMPTY_CHECKPOINT_CONTENTS_DIGEST)
        .unwrap();
    assert!(
        checkpoint_store
            .get_checkpoint_contents(&checkpoints[1].contents_digest)
            .unwrap()
            .is_none()
    );

    let executor_handle = spawn_monitored_task!(async move { executor.run_epoch(None).await });
    wait_for_executed_checkpoint(&checkpoint_store, 0, EXECUTOR_WAIT_TIMEOUT).await;

    // The test checks what the executor gets, not what the write returns.
    let _ = checkpoint_store
        .update_highest_synced_checkpoint(&checkpoints[2])
        .unwrap();
    // Longer than the warning timeout of the executor (5 s by default, set by
    // `NEW_CHECKPOINT_WARNING_TIMEOUT_MS`), after which the executor reads the
    // synced watermark again. The clock of the test starts paused, so this
    // wait takes no real time.
    let past_warning_timeout = Duration::from_secs(6);
    tokio::time::sleep(past_warning_timeout).await;
    assert!(
        !executor_handle.is_finished(),
        "the executor stopped: it got checkpoint 1, whose contents are not in the store"
    );
    assert_eq!(
        checkpoint_store
            .get_highest_executed_checkpoint_seq_number()
            .unwrap(),
        Some(0)
    );

    checkpoint_store
        .insert_checkpoint_contents(contents[1].clone().into_checkpoint_contents())
        .unwrap();
    for checkpoint in &checkpoints[1..] {
        checkpoint_store
            .update_highest_synced_checkpoint(checkpoint)
            .unwrap();
    }
    wait_for_executed_checkpoint(&checkpoint_store, 2, EXECUTOR_WAIT_TIMEOUT).await;
    executor_handle.abort();
}

/// The checkpoint executor waits for checkpoint 1, and a write marks
/// checkpoint 1 synced. The test checks that the write wakes the executor: the
/// executor executes checkpoint 1 before its warning timeout, after which it
/// reads the synced watermark again without a wake-up.
#[tokio::test(start_paused = true)]
async fn executor_executes_synced_checkpoint_before_warning_timeout() {
    telemetry_subscribers::init_for_testing();
    let tmp_dir = iota_common::tempdir();
    let checkpoint_store = CheckpointStore::new(tmp_dir.path());
    let (_state, executor, _hasher, committee) = init_executor_test(checkpoint_store.clone()).await;
    let (checkpoints, contents, _, _) = committee.make_empty_checkpoints(2, None);
    sync_checkpoint(&checkpoint_store, &checkpoints[0], &contents[0]);
    checkpoint_store
        .insert_verified_checkpoint(&checkpoints[1])
        .unwrap();

    let executor_handle = spawn_monitored_task!(async move { executor.run_epoch(None).await });
    wait_for_executed_checkpoint(&checkpoint_store, 0, EXECUTOR_WAIT_TIMEOUT).await;
    // A waiter that registers after the write returns at once, with or without
    // a wake-up. On the single-threaded test runtime, a registered waiter has
    // also read the watermark.
    timeout(EXECUTOR_WAIT_TIMEOUT, async {
        while checkpoint_store.num_synced_checkpoint_waiters_for_testing() == 0 {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("the executor should wait for checkpoint 1");

    checkpoint_store
        .update_highest_synced_checkpoint(&checkpoints[1])
        .unwrap();
    // Shorter than the warning timeout of the executor (5 s by default, set by
    // `NEW_CHECKPOINT_WARNING_TIMEOUT_MS`).
    let before_warning_timeout = Duration::from_secs(1);
    wait_for_executed_checkpoint(&checkpoint_store, 1, before_warning_timeout).await;
    executor_handle.abort();
}
