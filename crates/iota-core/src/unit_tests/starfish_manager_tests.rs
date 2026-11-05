// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{sync::Arc, time::Duration};

use arc_swap::ArcSwap;
use fastcrypto::traits::KeyPair;
use futures::FutureExt;
use iota_core::epoch_start_consensus_committee::get_consensus_committee;
use iota_metrics::{RegistryService, monitored_mpsc::unbounded_channel};
use iota_sdk_types::{CheckpointContents, CheckpointSummary};
use iota_swarm_config::network_config_builder::ConfigBuilder;
use iota_types::messages_checkpoint::CertifiedCheckpointSummary;
use prometheus_filtered::Registry;
use starfish_core::{
    CommitConsumerMonitor, CommitDigest as StarfishCommitDigest, CommitIndex,
    CommitRef as StarfishCommitRef, CommittedSubDag as StarfishCommittedSubDag, TestBlockHeader,
    VerifiedBlockHeader,
};
use tokio::{
    sync::{broadcast, mpsc},
    task::yield_now,
    time::sleep,
};

use crate::{
    authority::{
        AuthorityMetrics, AuthorityState, backpressure::BackpressureManager,
        test_authority_builder::TestAuthorityBuilder,
    },
    checkpoints::{CheckpointMetrics, CheckpointService, CheckpointServiceNoop},
    consensus_handler::{ConsensusHandler, ConsensusHandlerInitializer, StarfishConsensusHandler},
    consensus_manager::{
        ConsensusManagerMetrics, ConsensusManagerTrait, ReplayWaiter, replay_target,
        starfish_manager::StarfishManager,
    },
    consensus_validator::{IotaTxValidator, IotaTxValidatorMetrics},
    global_state_hasher::GlobalStateHasher,
    starfish_adapter::LazyStarfishClient,
};

pub fn checkpoint_service_for_testing(state: Arc<AuthorityState>) -> Arc<CheckpointService> {
    let (output, _result) = mpsc::channel::<(CheckpointContents, CheckpointSummary)>(10);
    let epoch_store = state.epoch_store_for_testing();
    let accumulator = Arc::new(GlobalStateHasher::new_for_tests(
        state.get_global_state_hash_store().clone(),
    ));
    let (certified_output, _certified_result) = mpsc::channel::<CertifiedCheckpointSummary>(10);

    let checkpoint_service = CheckpointService::build(
        state.clone(),
        state.get_checkpoint_store().clone(),
        epoch_store.clone(),
        state.get_transaction_cache_reader().clone(),
        Arc::downgrade(&accumulator),
        Box::new(output),
        Box::new(certified_output),
        CheckpointMetrics::new_for_tests(),
        3,
        100_000,
    );
    checkpoint_service.spawn(None).now_or_never().unwrap();
    checkpoint_service
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn test_starfish_manager() {
    // GIVEN
    let configs = ConfigBuilder::new_with_temp_dir()
        .committee_size(4.try_into().unwrap())
        .build();

    let config = &configs.validator_configs()[0];

    let consensus_config = config.consensus_config().unwrap();
    let registry_service = RegistryService::new(Registry::new());
    let secret = Arc::pin(config.authority_key_pair().copy());
    let genesis = config.genesis().unwrap();

    let metrics = Arc::new(ConsensusManagerMetrics::new(&Registry::new()));
    let client = Arc::new(LazyStarfishClient::default());

    let manager = StarfishManager::new(
        config.protocol_key_pair(),
        config.network_key_pair(),
        consensus_config.db_path().to_path_buf(),
        registry_service,
        metrics,
        client,
    );

    let boot_counter = *manager.boot_counter.lock().await;
    assert_eq!(boot_counter, 0);

    for i in 1..=3 {
        // A restart runs on a fresh epoch store, as on a node: the consensus
        // handler takes the store's assigned-commit receiver once.
        let state = TestAuthorityBuilder::new()
            .with_genesis_and_keypair(genesis, &secret)
            .build()
            .await;
        let epoch_store = state.epoch_store_for_testing();
        let consensus_handler_initializer = ConsensusHandlerInitializer::new_for_testing(
            state.clone(),
            checkpoint_service_for_testing(state.clone()),
        );

        // WHEN start starfish
        manager
            .start(
                config,
                epoch_store.clone(),
                consensus_handler_initializer,
                IotaTxValidator::new(
                    epoch_store.clone(),
                    Arc::new(CheckpointServiceNoop {}),
                    IotaTxValidatorMetrics::new(&Registry::new()),
                ),
            )
            .await;

        // THEN
        assert!(manager.is_running().await);
        let boot_counter = *manager.boot_counter.lock().await;
        if i == 1 || i == 2 {
            assert_eq!(boot_counter, 0);
        } else {
            assert_eq!(boot_counter, 1);
        }

        // Now try to shut it down
        sleep(Duration::from_secs(1)).await;

        // Simulate a commit by bumping the handled commit index so we can ensure that
        // boot counter increments only after the first run. Practically we want
        // to simulate a case where consensus engine restarts when no commits have
        // happened before for first run.
        if i > 1 {
            let monitor = manager
                .consumer_monitor
                .load_full()
                .expect("A consumer monitor should have been initialised");
            monitor.set_highest_handled_commit(100);
        }

        // WHEN
        manager.shutdown().await;

        // THEN
        assert!(!manager.is_running().await);

        // Yield to ensure that RocksDB releases the lock before the next iteration.
        yield_now().await;
    }
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn test_starfish_consensus_handler_handles_older_commits() {
    // GIVEN
    let configs = ConfigBuilder::new_with_temp_dir()
        .committee_size(4.try_into().unwrap())
        .build();

    let config = &configs.validator_configs()[0];
    let secret = Arc::pin(config.authority_key_pair().copy());
    let genesis = config.genesis().unwrap();

    let state = TestAuthorityBuilder::new()
        .with_genesis_and_keypair(genesis, &secret)
        .build()
        .await;

    let epoch_store = state.epoch_store_for_testing().clone();
    let new_epoch_start_state = epoch_store.epoch_start_state();
    let consensus_committee = get_consensus_committee(new_epoch_start_state);

    let metrics = Arc::new(AuthorityMetrics::new(&Registry::new()));
    let backpressure_manager = BackpressureManager::new_for_tests();

    let consensus_handler = ConsensusHandler::new(
        epoch_store.clone(),
        state.clone(),
        checkpoint_service_for_testing(state.clone()),
        state.execution_scheduler().clone(),
        state.get_object_cache_reader().clone(),
        Arc::new(ArcSwap::default()),
        consensus_committee.clone(),
        metrics,
        backpressure_manager.subscribe(),
    );

    // Create commits 1-10, where commits 1-7 are "older" (already processed at
    // startup) and commits 8-10 are "newer" (should be processed normally)
    let all_commits: Vec<_> = (1..=10)
        .map(|commit_idx| {
            let round = commit_idx as u32;
            let leader_authority = (commit_idx % consensus_committee.size() as u64) as u8;
            let leader_block_header = VerifiedBlockHeader::new_for_test(
                TestBlockHeader::new(round, leader_authority).build(),
            );
            StarfishCommittedSubDag::new(
                leader_block_header.reference(),
                vec![leader_block_header],
                vec![],
                vec![],
                1000 + commit_idx * 1000,
                StarfishCommitRef::new(commit_idx as u32, StarfishCommitDigest::MIN),
                vec![],
                vec![],
            )
        })
        .collect();

    // Set last_processed_commit_at_startup to 7
    let last_processed_commit_at_startup = 7;
    // Commits up to it were processed and flushed before the restart, so they
    // count as fully executed and later commits do not wait for them.
    epoch_store
        .handler_object_state_for_testing()
        .mark_fully_executed_for_testing(last_processed_commit_at_startup.into());

    let (commit_sender, commit_receiver) = unbounded_channel("consensus_output");
    let commit_consumer = starfish_core::CommitConsumer::new(commit_sender.clone(), 0);
    let commit_consumer_monitor = commit_consumer.monitor().clone();

    // WHEN we create the StarfishConsensusHandler
    let _handler = StarfishConsensusHandler::new(
        last_processed_commit_at_startup,
        consensus_handler,
        commit_receiver,
        commit_consumer_monitor.clone(),
    );

    // Send all commits in order
    for commit in all_commits {
        commit_sender.send(commit).unwrap();
    }

    // Give time for processing
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    // THEN verify that the highest handled commit is only updated for newer commits
    // (8-10) Commits 1-7 should have been processed as prior commits and
    // not update the monitor
    let highest_handled = commit_consumer_monitor.highest_handled_commit();
    assert_eq!(
        highest_handled, 10,
        "Expected highest handled commit to be 10, got {highest_handled}"
    );
}

#[test]
fn replay_target_is_the_last_validated_commit() {
    assert_eq!(replay_target(Some(15), 13, None), 15);
}

/// No commit above the resume point was validated, as in a fresh epoch, so
/// the builder does not wait for any commit consensus replays.
#[test]
fn replay_target_without_validated_commits_is_the_resume_point() {
    assert_eq!(replay_target(None, 13, None), 13);
}

/// A node that switched to executing synced checkpoints after commit 14 and
/// validated commit 15 when the switch abandoned it does not process commit
/// 15 again, so the replay waits only for commit 14.
#[test]
fn replay_target_after_a_switch_is_at_most_the_last_commit_before_it() {
    assert_eq!(replay_target(Some(15), 13, Some(14)), 14);
    assert_eq!(replay_target(Some(12), 10, Some(14)), 12);
    assert_eq!(replay_target(None, 13, Some(13)), 13);
}

/// Starts a [`ReplayWaiter`] on a consensus run that resumes after commit
/// `resume_point`, had commits up to `highest_observed` stored at startup and
/// waits for `target`. Returns the monitor the test advances and the wait.
async fn start_replay_wait(
    resume_point: CommitIndex,
    highest_observed: CommitIndex,
    target: Option<CommitIndex>,
) -> (Arc<CommitConsumerMonitor>, tokio::task::JoinHandle<()>) {
    let (commit_sender, _commit_receiver) = unbounded_channel("consensus_output");
    let monitor = starfish_core::CommitConsumer::new(commit_sender, resume_point).monitor();
    monitor.set_highest_observed_commit_at_startup(highest_observed);
    let (sender, receiver) = broadcast::channel(1);
    let wait = tokio::spawn(ReplayWaiter::new(receiver).wait_for_replay());
    assert!(
        sender.send((monitor.clone(), target)).is_ok(),
        "the waiter subscribed before the send"
    );
    yield_now().await;
    (monitor, wait)
}

/// Handles commits up to `index` one by one, and asserts after each that
/// `wait` has not finished.
async fn handle_commits_while_waiting(
    monitor: &CommitConsumerMonitor,
    wait: &tokio::task::JoinHandle<()>,
    through: CommitIndex,
) {
    for index in monitor.highest_handled_commit() + 1..=through {
        assert!(
            !wait.is_finished(),
            "the replay wait ended before commit {index}"
        );
        monitor.set_highest_handled_commit(index);
        yield_now().await;
    }
}

/// A validator restarts after validating commit 15 and parks on commit 16,
/// which it never validated: a first validation waits for execution, which
/// can need the checkpoint executor that node startup creates only once the
/// checkpoint builder has rebuilt what it built before the restart. The
/// builder's wait for the replay therefore ends at commit 15, while consensus
/// had stored commits up to 18.
#[tokio::test(flavor = "current_thread")]
async fn replay_wait_ends_at_the_last_validated_commit() {
    let (monitor, wait) = start_replay_wait(13, 18, Some(15)).await;
    handle_commits_while_waiting(&monitor, &wait, 14).await;

    monitor.set_highest_handled_commit(15);
    tokio::time::timeout(Duration::from_secs(10), wait)
        .await
        .expect("the replay wait must end once the last validated commit is handled")
        .unwrap();
}

/// Without deterministic validation the wait covers every commit consensus
/// had stored at startup.
#[tokio::test(flavor = "current_thread")]
async fn replay_wait_without_a_target_covers_every_stored_commit() {
    let (monitor, wait) = start_replay_wait(13, 18, None).await;
    handle_commits_while_waiting(&monitor, &wait, 17).await;

    monitor.set_highest_handled_commit(18);
    tokio::time::timeout(Duration::from_secs(10), wait)
        .await
        .expect("the replay wait must end once every stored commit is handled")
        .unwrap();
}

/// A target above what consensus had stored, which a lost consensus store
/// would leave, is capped there, so the wait still ends.
#[tokio::test(flavor = "current_thread")]
async fn replay_wait_is_capped_at_the_stored_commits() {
    let (monitor, wait) = start_replay_wait(13, 16, Some(20)).await;
    handle_commits_while_waiting(&monitor, &wait, 15).await;

    monitor.set_highest_handled_commit(16);
    tokio::time::timeout(Duration::from_secs(10), wait)
        .await
        .expect("the replay wait must end at the highest stored commit")
        .unwrap();
}
