// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};

use iota_common::register_debug_fatal_handler;
use iota_macros::{register_fail_point_if, sim_test};
use iota_test_transaction_builder::make_transfer_iota_transaction;
use iota_types::messages_checkpoint::CheckpointSummaryExt;
use test_cluster::TestClusterBuilder;
use tokio::time::sleep;
use tracing::info;

#[sim_test]
async fn basic_checkpoints_integration_test() {
    let test_cluster = TestClusterBuilder::new().build().await;
    let tx = make_transfer_iota_transaction(&test_cluster.wallet, None, None).await;
    let digest = *tx.digest();
    test_cluster.execute_transaction(tx).await;

    for _ in 0..600 {
        let all_included = test_cluster
            .swarm
            .validator_node_handles()
            .into_iter()
            .all(|handle| {
                handle.with(|node| {
                    node.state()
                        .epoch_store_for_testing()
                        .is_transaction_executed_in_checkpoint(&digest)
                        .unwrap()
                })
            });
        if all_included {
            // success
            return;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }

    panic!("Did not include transaction in checkpoint in 60 seconds");
}

#[sim_test]
async fn test_checkpoint_split_brain() {
    #[cfg(msim)]
    {
        // this test intentionally halts the network by causing a fork, so we cannot
        // panic on loss of liveness
        use iota_core::authority::{CheckpointTimeoutConfig, init_checkpoint_timeout_config};
        init_checkpoint_timeout_config(CheckpointTimeoutConfig {
            warning_timeout: Duration::from_secs(2),
            panic_timeout: None,
        });
    }

    let committee_size = 9;
    // count number of nodes that have reached split brain condition
    let count_split_brain_nodes: Arc<Mutex<AtomicUsize>> = Default::default();
    let count_clone = count_split_brain_nodes.clone();

    register_debug_fatal_handler!(
        "Split brain detected in checkpoint signature aggregation",
        move || {
            let counter = count_clone.lock().unwrap();
            counter.fetch_add(1, Ordering::Relaxed);
        }
    );

    register_fail_point_if("cp_execution_nondeterminism", || true);

    let test_cluster = TestClusterBuilder::new()
        .with_num_validators(committee_size)
        .build()
        .await;

    let tx = make_transfer_iota_transaction(&test_cluster.wallet, None, None).await;
    test_cluster
        .wallet
        .execute_transaction_may_fail(tx)
        .await
        .ok();

    // provide enough time for validators to detect split brain
    tokio::time::sleep(Duration::from_secs(20)).await;

    // all honest validators should eventually detect a split brain
    let final_count = count_split_brain_nodes.lock().unwrap();
    assert!(final_count.load(Ordering::Relaxed) >= 1);
}

#[sim_test]
async fn test_checkpoint_timestamps_non_decreasing() {
    let epoch_duration_ms = 10_000; // 10 seconds
    let num_epochs_to_run = 3;

    let test_cluster = TestClusterBuilder::new()
        .with_epoch_duration_ms(epoch_duration_ms)
        .disable_fullnode_pruning()
        .build()
        .await;

    sleep(Duration::from_millis(
        epoch_duration_ms * num_epochs_to_run + epoch_duration_ms / 2,
    ))
    .await;

    // Retrieve checkpoints and verify timestamps from the first full node.
    let full_node = test_cluster
        .swarm
        .fullnodes()
        .next()
        .expect("No full node is found");

    let checkpoint_store = full_node
        .get_node_handle()
        .unwrap()
        .state()
        .checkpoint_store
        .clone();

    let highest_executed_checkpoint = checkpoint_store
        .get_highest_executed_checkpoint()
        .expect("Failed to get highest executed checkpoint")
        .expect("No executed checkpoints found in store");

    assert!(
        highest_executed_checkpoint.epoch() > 0,
        "Test did not run long enough to cross epochs"
    );

    let mut current_seq = highest_executed_checkpoint.sequence_number();
    let mut prev_timestamp = highest_executed_checkpoint.timestamp();
    let mut checkpoints_checked = 0;

    // Iterate backwards from the highest checkpoint
    loop {
        if current_seq == 0 {
            info!("Reached checkpoint 0.");
            break;
        }
        current_seq -= 1;

        // Fetch the previous digest to continue iteration
        let current_checkpoint = checkpoint_store
            .get_checkpoint_by_sequence_number(current_seq)
            .expect("DB error getting current checkpoint")
            .unwrap_or_else(|| panic!("checkpoint missing for seq {current_seq}"));
        let current_timestamp = current_checkpoint.timestamp();
        assert!(
            current_timestamp <= prev_timestamp,
            "Timestamp decreased! current seq {current_seq}, {current_timestamp:?} vs {prev_timestamp:?}",
        );
        prev_timestamp = current_timestamp;
        checkpoints_checked += 1;
    }

    assert!(checkpoints_checked > 0, "Test created only 1 checkpoint",);
}

/// A build that exceeds `max_transactions_per_checkpoint` is split into
/// several checkpoints sharing one `checkpoint_height`. These tests drive real
/// validators through the two situations where the consensus quarantine's
/// handling of such a split matters: a crash between the execution of two
/// chunks, and a restart that has to rebuild chunks the network already
/// certified.
///
/// Kills and restarts nodes through the simulator, so it only builds under
/// `cargo simtest`.
#[cfg(msim)]
mod split_checkpoints {
    use std::{
        sync::{
            Arc, Weak,
            atomic::{AtomicBool, Ordering},
        },
        time::Duration,
    };

    use futures::future::join_all;
    use iota_core::authority::AuthorityState;
    use iota_macros::{register_fail_point, sim_test};
    use iota_protocol_config::ProtocolConfig;
    use iota_test_transaction_builder::batch_make_transfer_transactions;
    use iota_types::{base_types::AuthorityName, messages_checkpoint::CheckpointSequenceNumber};
    use test_cluster::{TestCluster, TestClusterBuilder};
    use tokio::time::{sleep, timeout};
    use tracing::info;

    const MAX_TRANSACTIONS_PER_CHECKPOINT: u64 = 3;

    /// Keeps every gas object of the wallet busy so commits regularly carry
    /// more than `MAX_TRANSACTIONS_PER_CHECKPOINT` transactions.
    fn spawn_transfer_traffic(
        test_cluster: Arc<TestCluster>,
        stop: Arc<AtomicBool>,
    ) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            while !stop.load(Ordering::SeqCst) {
                let txns = batch_make_transfer_transactions(&test_cluster.wallet, 25).await;
                join_all(
                    txns.into_iter()
                        .map(|tx| test_cluster.wallet.execute_transaction_may_fail(tx)),
                )
                .await;
                sleep(Duration::from_millis(100)).await;
            }
        })
    }

    fn highest_executed_checkpoint(
        test_cluster: &TestCluster,
        name: &AuthorityName,
    ) -> Option<CheckpointSequenceNumber> {
        test_cluster
            .swarm
            .node(name)
            .unwrap()
            .get_node_handle()
            .unwrap()
            .with(|node| {
                node.state()
                    .checkpoint_store
                    .get_highest_executed_checkpoint_seq_number()
                    .unwrap()
            })
    }

    fn last_built_checkpoint(
        test_cluster: &TestCluster,
        name: &AuthorityName,
    ) -> Option<CheckpointSequenceNumber> {
        test_cluster
            .swarm
            .node(name)
            .unwrap()
            .get_node_handle()
            .unwrap()
            .with(|node| {
                node.state()
                    .epoch_store_for_testing()
                    .last_built_checkpoint_builder_summary()
                    .unwrap()
                    .map(|summary| summary.summary.sequence_number)
            })
    }

    fn fullnode_highest_executed_checkpoint(
        test_cluster: &TestCluster,
    ) -> CheckpointSequenceNumber {
        test_cluster.fullnode_handle.iota_node.with(|node| {
            node.state()
                .checkpoint_store
                .get_highest_executed_checkpoint_seq_number()
                .unwrap()
                .unwrap_or_default()
        })
    }

    async fn wait_until(what: &str, mut condition: impl FnMut() -> bool) {
        timeout(Duration::from_secs(120), async {
            while !condition() {
                sleep(Duration::from_millis(200)).await;
            }
        })
        .await
        .unwrap_or_else(|_| panic!("timed out waiting until {what}"));
    }

    /// Waits until the validator has executed and built past the checkpoint
    /// the fullnode had executed when it was restarted, so a restarted builder
    /// that lost part of a split build fails the local fork check instead of
    /// silently stalling.
    async fn wait_for_validator_to_catch_up(test_cluster: &TestCluster, name: &AuthorityName) {
        let target = fullnode_highest_executed_checkpoint(test_cluster) + 5;
        wait_until("the restarted validator executes past the network", || {
            highest_executed_checkpoint(test_cluster, name).is_some_and(|seq| seq >= target)
        })
        .await;
        wait_until("the restarted validator builds past the network", || {
            last_built_checkpoint(test_cluster, name).is_some_and(|seq| seq >= target)
        })
        .await;
    }

    #[sim_test]
    async fn split_checkpoints_survive_a_crash_between_chunks_and_a_catch_up() {
        let _guard = ProtocolConfig::apply_overrides_for_testing(|_, mut config| {
            config.set_max_transactions_per_checkpoint_for_testing(MAX_TRANSACTIONS_PER_CHECKPOINT);
            config
        });
        let test_cluster = Arc::new(
            TestClusterBuilder::new()
                .with_num_validators(4)
                .build()
                .await,
        );
        let stop_traffic = Arc::new(AtomicBool::new(false));
        let traffic = spawn_transfer_traffic(test_cluster.clone(), stop_traffic.clone());
        let names = test_cluster.get_validator_pubkeys();

        // Kill a validator right after it executed the first checkpoint of a
        // split build: the quarantine has already flushed that checkpoint, the
        // remaining chunks of the same height are only in memory.
        let crashed = names[0];
        let crashed_handle = test_cluster
            .swarm
            .node(&crashed)
            .unwrap()
            .get_node_handle()
            .unwrap();
        let crashed_sim_id = crashed_handle.with(|_| iota_simulator::current_simnode_id());
        let crashed_state: Weak<AuthorityState> =
            crashed_handle.with(|node| Arc::downgrade(&node.state()));
        drop(crashed_handle);
        let killed = Arc::new(AtomicBool::new(false));
        {
            let killed = killed.clone();
            register_fail_point("highest-executed-checkpoint", move || {
                if iota_simulator::current_simnode_id() != crashed_sim_id
                    || killed.load(Ordering::SeqCst)
                {
                    return;
                }
                let Some(state) = crashed_state.upgrade() else {
                    return;
                };
                // The quarantine processes checkpoints ahead of this watermark
                // bump, so ask it directly: kill once the last executed
                // checkpoint has a successor of the same height that is not
                // executed yet.
                let executed_first_chunk = {
                    let epoch_store = state.epoch_store_for_testing();
                    let executed = epoch_store.highest_executed_checkpoint_for_testing();
                    let height_of = |seq| {
                        epoch_store
                            .get_built_checkpoint_builder_summary(seq)
                            .unwrap()
                            .and_then(|summary| summary.checkpoint_height)
                    };
                    (height_of(executed).is_some()
                        && height_of(executed) == height_of(executed + 1))
                    .then_some(executed)
                };
                drop(state);
                let Some(seq) = executed_first_chunk else {
                    return;
                };
                if !killed.swap(true, Ordering::SeqCst) {
                    info!(
                        "killing validator after executing checkpoint {seq}, the first of a split build"
                    );
                    iota_simulator::task::kill_current_node(None);
                }
            });
        }
        wait_until("a validator is killed between two chunks", || {
            killed.load(Ordering::SeqCst)
        })
        .await;
        // The simulator restarts a killed node on its own; stop and start it
        // explicitly to get a clean handle to the new instance.
        test_cluster.stop_node(&crashed);
        sleep(Duration::from_secs(5)).await;
        test_cluster.start_node(&crashed).await;
        wait_for_validator_to_catch_up(&test_cluster, &crashed).await;

        // Stop another validator while the network certifies split builds,
        // then let it catch up: state sync executes those checkpoints before
        // its builder rebuilds them.
        let lagging = names[1];
        test_cluster.stop_node(&lagging);
        sleep(Duration::from_secs(10)).await;
        test_cluster.start_node(&lagging).await;
        wait_for_validator_to_catch_up(&test_cluster, &lagging).await;

        stop_traffic.store(true, Ordering::SeqCst);
        traffic.await.unwrap();
    }
}
