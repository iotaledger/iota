// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{sync::Arc, time::Duration};

use arc_swap::ArcSwap;
use fastcrypto::traits::KeyPair;
use futures::FutureExt;
use iota_metrics::{RegistryService, monitored_mpsc::unbounded_channel};
use iota_swarm_config::network_config_builder::ConfigBuilder;
use iota_types::{
    iota_system_state::epoch_start_iota_system_state::EpochStartSystemStateTrait,
    messages_checkpoint::{CertifiedCheckpointSummary, CheckpointContents, CheckpointSummary},
};
use prometheus::Registry;
use starfish_core::{
    CommitDigest as StarfishCommitDigest, CommitRef as StarfishCommitRef,
    CommittedSubDag as StarfishCommittedSubDag, TestBlockHeader, VerifiedBlockHeader,
};
use tokio::{sync::mpsc, task::yield_now, time::sleep};

use crate::{
    authority::{
        AuthorityMetrics, AuthorityState, backpressure::BackpressureManager,
        test_authority_builder::TestAuthorityBuilder,
    },
    checkpoints::{CheckpointMetrics, CheckpointService, CheckpointServiceNoop},
    consensus_handler::{ConsensusHandler, ConsensusHandlerInitializer, StarfishConsensusHandler},
    consensus_manager::{
        ConsensusManagerMetrics, ConsensusManagerTrait, starfish_manager::StarfishManager,
    },
    consensus_validator::{IotaTxValidator, IotaTxValidatorMetrics},
    starfish_adapter::LazyStarfishClient,
    state_accumulator::StateAccumulator,
};

pub fn checkpoint_service_for_testing(state: Arc<AuthorityState>) -> Arc<CheckpointService> {
    let (output, _result) = mpsc::channel::<(CheckpointContents, CheckpointSummary)>(10);
    let epoch_store = state.epoch_store_for_testing();
    let accumulator = Arc::new(StateAccumulator::new_for_tests(
        state.get_accumulator_store().clone(),
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
    checkpoint_service.spawn().now_or_never().unwrap();
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

    let state = TestAuthorityBuilder::new()
        .with_genesis_and_keypair(genesis, &secret)
        .build()
        .await;

    let metrics = Arc::new(ConsensusManagerMetrics::new(&Registry::new()));
    let epoch_store = state.epoch_store_for_testing();
    let client = Arc::new(LazyStarfishClient::default());

    let manager = StarfishManager::new(
        config.protocol_key_pair().copy(),
        config.network_key_pair().copy(),
        consensus_config.db_path().to_path_buf(),
        registry_service,
        metrics,
        client,
    );

    let boot_counter = *manager.boot_counter.lock().await;
    assert_eq!(boot_counter, 0);

    for i in 1..=3 {
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
                    state.transaction_manager().clone(),
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
    let consensus_committee = new_epoch_start_state.get_consensus_committee();

    let metrics = Arc::new(AuthorityMetrics::new(&Registry::new()));
    let backpressure_manager = BackpressureManager::new_for_tests();

    let consensus_handler = ConsensusHandler::new(
        epoch_store.clone(),
        checkpoint_service_for_testing(state.clone()),
        state.transaction_manager().clone(),
        state.get_object_cache_reader().clone(),
        state.get_transaction_cache_reader().clone(),
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
            )
        })
        .collect();

    // Set last_processed_commit_at_startup to 7
    let last_processed_commit_at_startup = 7;

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
        "Expected highest handled commit to be 10, got {}",
        highest_handled
    );
}
