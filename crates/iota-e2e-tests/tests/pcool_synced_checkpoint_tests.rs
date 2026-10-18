// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Cluster tests for P-COOL deterministic validation on committee validators
//! that execute only what their own consensus handler assigned, which
//! `pcool_deterministic_validation` makes them do. Such a validator executes synced checkpoints
//! only once it switches to doing so for the rest of an epoch, after the network has certified the
//! epoch's last checkpoint.
//!
//! Gated to the simulator for the reason given in
//! `pcool_transaction_flow_tests.rs`: the protocol-config override is
//! thread-local, and only the simulator runs every node on the test's thread.
//!
//! Run with: `cargo simtest -p iota-e2e-tests --test pcool_synced_checkpoint_tests`.

#![cfg(msim)]

use std::{
    sync::{
        Arc, Mutex, Weak,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    time::Duration,
};

use futures::future::join_all;
use iota_common::register_debug_fatal_handler;
use iota_core::{authority::AuthorityState, consensus_manager::ConsensusReplayTargetReport};
use iota_macros::{
    clear_fail_point, register_fail_point, register_fail_point_arg, register_fail_point_async,
    sim_test,
};
use iota_protocol_config::ProtocolConfig;
use iota_sdk_types::{Address, TransactionKind};
use iota_test_transaction_builder::TestTransactionBuilder;
use iota_types::{
    base_types::{AuthorityName, ConciseableName, EpochId, dbg_addr},
    messages_checkpoint::CheckpointSequenceNumber,
    transaction::TransactionAPI,
};
use rand::distr::{Distribution, Uniform};
use test_cluster::{TestCluster, TestClusterBuilder};
use tokio::{
    task::JoinHandle,
    time::{Instant, sleep, timeout},
};
use tracing::info;

/// Enables deterministic validation, which also makes committee validators
/// skip synced checkpoint execution. The guard must be held for the whole
/// test.
fn enable_deterministic_validation_for_testing() -> iota_protocol_config::OverrideGuard {
    ProtocolConfig::apply_overrides_for_testing(|_, mut config| {
        config.enable_pcool_deterministic_validation_for_testing();
        config
    })
}

/// Builds a four-validator cluster and checks that the flag reached the
/// validators.
async fn build_cluster(epoch_duration_ms: u64) -> Arc<TestCluster> {
    let test_cluster = TestClusterBuilder::new()
        .with_num_validators(4)
        .with_epoch_duration_ms(epoch_duration_ms)
        .build()
        .await;
    for handle in test_cluster.all_validator_handles() {
        handle.with(|node| {
            let epoch_store = node.state().epoch_store_for_testing();
            let config = epoch_store.protocol_config();
            assert!(
                config.pcool_deterministic_validation(),
                "the protocol-config override must reach the validators"
            );
        });
    }
    Arc::new(test_cluster)
}

/// Sends transfers from every gas coin of `senders` until `stop` is set.
/// Every second coin is spent by two transfers at once, so commits carry
/// conflicting pairs that post-consensus validation must resolve the same way
/// on every validator. Returns how many transfers succeeded.
fn spawn_transfer_traffic_with_double_spends(
    test_cluster: Arc<TestCluster>,
    senders: Vec<Address>,
    stop: Arc<AtomicBool>,
) -> JoinHandle<usize> {
    tokio::spawn(async move {
        let gas_price = test_cluster.get_reference_gas_price().await;
        let mut succeeded = 0;
        while !stop.load(Ordering::SeqCst) {
            let Ok(accounts) = test_cluster.wallet.get_all_accounts_and_gas_objects().await else {
                sleep(Duration::from_millis(500)).await;
                continue;
            };
            let mut transactions = Vec::new();
            for (sender, coins) in accounts {
                if !senders.contains(&sender) {
                    continue;
                }
                for (index, coin) in coins.into_iter().enumerate() {
                    let spends = if index % 2 == 0 { 2 } else { 1 };
                    for recipient in 0..spends {
                        let tx = TestTransactionBuilder::new(sender, coin, gas_price)
                            .transfer_iota(Some(1), dbg_addr(recipient + 2))
                            .build();
                        transactions.push(test_cluster.wallet.sign_transaction(&tx));
                    }
                }
            }
            let responses = join_all(transactions.into_iter().map(|tx| {
                timeout(
                    Duration::from_secs(30),
                    test_cluster.wallet.execute_transaction_may_fail(tx),
                )
            }))
            .await;
            succeeded += responses
                .iter()
                .filter(|response| {
                    matches!(response, Ok(Ok(response)) if response.status_ok() == Some(true))
                })
                .count();
            sleep(Duration::from_millis(100)).await;
        }
        succeeded
    })
}

/// Stops the traffic and asserts that some of it succeeded, so a test cannot
/// pass on a cluster that executed nothing.
async fn stop_traffic(traffic: JoinHandle<usize>, stop: &AtomicBool) {
    stop.store(true, Ordering::SeqCst);
    let succeeded = traffic.await.unwrap();
    assert!(succeeded > 0, "no transfer of the traffic succeeded");
}

/// What a running validator reports about its current epoch.
#[derive(Clone, Copy, Debug)]
struct ValidatorProgress {
    epoch: EpochId,
    executing_synced_checkpoints: bool,
    highest_executed: CheckpointSequenceNumber,
    highest_synced: CheckpointSequenceNumber,
    last_built: CheckpointSequenceNumber,
}

/// `None` while the validator is stopped.
fn progress(test_cluster: &TestCluster, name: &AuthorityName) -> Option<ValidatorProgress> {
    test_cluster
        .swarm
        .node(name)
        .unwrap()
        .get_node_handle()
        .map(|handle| {
            handle.with(|node| {
                let state = node.state();
                let epoch_store = state.epoch_store_for_testing();
                ValidatorProgress {
                    epoch: epoch_store.epoch(),
                    executing_synced_checkpoints: epoch_store.is_executing_synced_checkpoints(),
                    highest_executed: state
                        .get_checkpoint_store()
                        .get_highest_executed_checkpoint_seq_number()
                        .unwrap()
                        .unwrap_or_default(),
                    highest_synced: state
                        .get_checkpoint_store()
                        .get_highest_synced_checkpoint_seq_number()
                        .unwrap()
                        .unwrap_or_default(),
                    last_built: epoch_store
                        .last_built_checkpoint_builder_summary()
                        .unwrap()
                        .map(|summary| summary.summary.sequence_number)
                        .unwrap_or_default(),
                }
            })
        })
}

fn fullnode_highest_executed_checkpoint(test_cluster: &TestCluster) -> CheckpointSequenceNumber {
    test_cluster.fullnode_handle.iota_node.with(|node| {
        node.state()
            .get_checkpoint_store()
            .get_highest_executed_checkpoint_seq_number()
            .unwrap()
            .unwrap_or_default()
    })
}

/// Waits until every validator has executed and built a checkpoint a few past
/// the one the fullnode has executed, then compares each checkpoint up to that
/// one for which the validator holds both its own summary and the certified
/// one. Earlier epochs are covered by the in-node fork check, which panics on
/// a built checkpoint that differs from the certified one.
async fn assert_validators_reach_the_same_checkpoints(test_cluster: &TestCluster) {
    let target = fullnode_highest_executed_checkpoint(test_cluster) + 5;
    assert_validators_reach_the_same_checkpoints_up_to(test_cluster, target).await;
}

/// Waits until every validator has executed and built checkpoint `target`, and
/// checks that each checkpoint it built up to there is the certified one.
async fn assert_validators_reach_the_same_checkpoints_up_to(
    test_cluster: &TestCluster,
    target: CheckpointSequenceNumber,
) {
    for name in test_cluster.get_validator_pubkeys() {
        timeout(Duration::from_secs(180), async {
            while progress(test_cluster, &name)
                .is_none_or(|p| p.highest_executed < target || p.last_built < target)
            {
                sleep(Duration::from_millis(500)).await;
            }
        })
        .await
        .unwrap_or_else(|_| {
            panic!(
                "validator {} must execute and build checkpoint {target}: {:?}",
                name.concise(),
                progress(test_cluster, &name),
            )
        });
        let handle = test_cluster
            .swarm
            .node(&name)
            .unwrap()
            .get_node_handle()
            .unwrap();
        handle.with(|node| {
            let store = node.state().get_checkpoint_store().clone();
            for sequence_number in 1..=target {
                let local = store
                    .get_locally_computed_checkpoint(sequence_number)
                    .unwrap();
                let certified = store
                    .get_checkpoint_by_sequence_number(sequence_number)
                    .unwrap();
                if let (Some(local), Some(certified)) = (local, certified) {
                    assert_eq!(
                        local.digest(),
                        *certified.digest(),
                        "validator {}'s checkpoint {sequence_number} must match the certified one",
                        name.concise(),
                    );
                }
            }
        });
    }
}

/// Waits until each of `names` runs epoch `epoch` or a later one.
async fn wait_for_validators_to_reach_epoch(
    test_cluster: &TestCluster,
    names: &[AuthorityName],
    epoch: EpochId,
    within: Duration,
) {
    timeout(within, async {
        while names
            .iter()
            .any(|name| progress(test_cluster, name).is_none_or(|p| p.epoch < epoch))
        {
            sleep(Duration::from_millis(200)).await;
        }
    })
    .await
    .unwrap_or_else(|_| {
        panic!(
            "every validator must reach epoch {epoch}: {:?}",
            names
                .iter()
                .map(|name| progress(test_cluster, name))
                .collect::<Vec<_>>()
        )
    });
}

/// The simulator node id of a validator, read without keeping its handle: a
/// held handle pins a killed node's store, and the restarted node could not
/// open it.
fn sim_node_id(test_cluster: &TestCluster, name: &AuthorityName) -> iota_simulator::task::NodeId {
    test_cluster
        .swarm
        .node(name)
        .unwrap()
        .get_node_handle()
        .unwrap()
        .with(|_| iota_simulator::current_simnode_id())
}

/// Delays every transaction execution on the node `sim_id` by up to
/// `max_delay_ms` while `enabled` is set.
fn delay_execution_on(
    sim_id: iota_simulator::task::NodeId,
    max_delay_ms: u64,
    enabled: Arc<AtomicBool>,
) {
    delay_execution_on_nodes(move |id| id == sim_id, max_delay_ms, enabled);
}

/// Delays every transaction execution on the nodes `selects` picks by up to
/// `max_delay_ms` while `enabled` is set. A node started again gets a new
/// simulator node id, so `selects` can pick it before it starts.
fn delay_execution_on_nodes(
    selects: impl Fn(iota_simulator::task::NodeId) -> bool + Send + Sync + 'static,
    max_delay_ms: u64,
    enabled: Arc<AtomicBool>,
) {
    register_fail_point_async("transaction_execution_delay", move || {
        let delay = selects(iota_simulator::current_simnode_id()) && enabled.load(Ordering::SeqCst);
        async move {
            if delay {
                let delay = Uniform::new(0, max_delay_ms)
                    .unwrap()
                    .sample(&mut rand::rng());
                sleep(Duration::from_millis(delay)).await;
            }
        }
    });
}

/// Stops `victim` early in the epoch it runs and starts it again once every
/// other validator has moved to a later epoch, so the victim has missed its
/// epoch's last commits and must switch to executing synced checkpoints.
/// Returns the epoch it switched in.
///
/// The switch is seen from the checkpoint executor: the epoch's last
/// checkpoint is executed after the switch, and the next epoch can start a few
/// milliseconds later, too soon for polling the node to see it.
async fn stop_until_the_epoch_ends_then_wait_for_the_switch(
    test_cluster: &TestCluster,
    victim: &AuthorityName,
) -> EpochId {
    let names = test_cluster.get_validator_pubkeys();
    let others: Vec<_> = names.iter().copied().filter(|n| n != victim).collect();
    let epoch = progress(test_cluster, victim).unwrap().epoch;
    test_cluster.stop_node(victim);
    wait_for_validators_to_reach_epoch(test_cluster, &others, epoch + 1, Duration::from_secs(120))
        .await;

    // The restarted victim, as its simulator node id and a weak reference
    // that does not pin its store.
    let restarted: Arc<Mutex<Option<(iota_simulator::task::NodeId, Weak<AuthorityState>)>>> =
        Default::default();
    let switched_in_epochs: Arc<Mutex<Vec<EpochId>>> = Default::default();
    {
        let restarted = restarted.clone();
        let switched_in_epochs = switched_in_epochs.clone();
        register_fail_point("highest-executed-checkpoint", move || {
            let Some((sim_id, state)) = restarted.lock().unwrap().clone() else {
                return;
            };
            if iota_simulator::current_simnode_id() != sim_id {
                return;
            }
            let Some(state) = state.upgrade() else {
                return;
            };
            let epoch_store = state.epoch_store_for_testing();
            if epoch_store.is_executing_synced_checkpoints() {
                let mut switched_in_epochs = switched_in_epochs.lock().unwrap();
                if !switched_in_epochs.contains(&epoch_store.epoch()) {
                    switched_in_epochs.push(epoch_store.epoch());
                }
            }
        });
    }
    test_cluster.start_node(victim).await;
    *restarted.lock().unwrap() = test_cluster
        .swarm
        .node(victim)
        .unwrap()
        .get_node_handle()
        .map(|handle| {
            handle.with(|node| {
                (
                    iota_simulator::current_simnode_id(),
                    Arc::downgrade(&node.state()),
                )
            })
        });

    timeout(Duration::from_secs(120), async {
        while progress(test_cluster, victim).is_none_or(|p| p.epoch == epoch) {
            sleep(Duration::from_millis(100)).await;
        }
    })
    .await
    .unwrap_or_else(|_| {
        panic!(
            "the restarted validator must leave epoch {epoch}: {:?}",
            progress(test_cluster, victim)
        )
    });
    clear_fail_point("highest-executed-checkpoint");
    assert_eq!(
        *switched_in_epochs.lock().unwrap(),
        vec![epoch],
        "the restarted validator must switch to executing synced checkpoints in epoch {epoch} \
         only"
    );
    epoch
}

/// A validator stopped mid-epoch under double-spend traffic replays the
/// commits it missed when it starts again and catches up within the epoch. Its
/// validation must decide every transaction as its peers did, so it builds the
/// certified checkpoints, and the whole cluster reaches later epochs.
#[sim_test]
async fn test_mid_epoch_restart_catches_up_without_fork() {
    telemetry_subscribers::init_for_testing();
    let _guard = enable_deterministic_validation_for_testing();
    let test_cluster = build_cluster(60_000).await;
    let names = test_cluster.get_validator_pubkeys();
    let stop = Arc::new(AtomicBool::new(false));
    let traffic = spawn_transfer_traffic_with_double_spends(
        test_cluster.clone(),
        test_cluster.get_addresses(),
        stop.clone(),
    );

    let lagging = names[1];
    sleep(Duration::from_secs(5)).await;
    test_cluster.stop_node(&lagging);
    sleep(Duration::from_secs(10)).await;
    test_cluster.start_node(&lagging).await;
    assert_validators_reach_the_same_checkpoints(&test_cluster).await;
    let lagging_progress = progress(&test_cluster, &lagging).unwrap();
    assert!(
        !lagging_progress.executing_synced_checkpoints,
        "a validator restarted within its epoch must catch up through its own consensus: \
         {lagging_progress:?}"
    );

    wait_for_validators_to_reach_epoch(&test_cluster, &names, 2, Duration::from_secs(180)).await;
    stop_traffic(traffic, &stop).await;
    assert_validators_reach_the_same_checkpoints(&test_cluster).await;
}

/// A validator that misses the last commits of its epoch, because it is
/// stopped before the epoch ends and started only once its peers have moved
/// on, switches to executing synced checkpoints for the rest of that epoch and
/// joins the next epoch through its own consensus.
#[sim_test]
async fn test_validator_missing_the_epochs_last_commits_switches_and_reaches_the_next_epoch() {
    telemetry_subscribers::init_for_testing();
    let _guard = enable_deterministic_validation_for_testing();
    let test_cluster = build_cluster(20_000).await;
    let names = test_cluster.get_validator_pubkeys();
    let victim = names[0];
    let stop = Arc::new(AtomicBool::new(false));
    let traffic = spawn_transfer_traffic_with_double_spends(
        test_cluster.clone(),
        test_cluster.get_addresses(),
        stop.clone(),
    );
    sleep(Duration::from_secs(5)).await;

    let epoch = stop_until_the_epoch_ends_then_wait_for_the_switch(&test_cluster, &victim).await;
    wait_for_validators_to_reach_epoch(&test_cluster, &names, epoch + 2, Duration::from_secs(180))
        .await;
    let victim_progress = progress(&test_cluster, &victim).unwrap();
    assert!(
        !victim_progress.executing_synced_checkpoints,
        "the switch must last only for the epoch it happened in: {victim_progress:?}"
    );

    stop_traffic(traffic, &stop).await;
    assert_validators_reach_the_same_checkpoints(&test_cluster).await;
}

/// One validator executes every transaction up to two seconds late, so its
/// consensus handler, which waits for that execution before it validates,
/// falls behind its own consensus. When the network ends an epoch, the
/// validator has not built the epoch's last checkpoints and switches to
/// executing synced checkpoints, with commits stored that its handler has not
/// processed. It is killed right after it executes a synced checkpoint, twice
/// in each of up to three epochs, and restarts on its own each time.
///
/// On restart its consensus handler replays the commits it validated, then
/// meets stored commits it never processed, while its store already holds the
/// outputs of the synced checkpoints executed after the switch. The second
/// restart in an epoch replays any such commit the first restart validated
/// before the node switched again. It must not panic or fork, and every
/// validator must reach later epochs. The test requires at least one restart
/// with such stored commits, so it cannot pass without reaching that state.
#[sim_test]
async fn test_restart_after_switching_to_synced_checkpoints_does_not_fork() {
    const KILLS: usize = 6;
    const KILLS_PER_EPOCH: usize = 2;
    telemetry_subscribers::init_for_testing();
    let _guard = enable_deterministic_validation_for_testing();
    let test_cluster = build_cluster(15_000).await;
    let names = test_cluster.get_validator_pubkeys();
    let victim = names[0];
    let victim_sim_id = sim_node_id(&test_cluster, &victim);
    let delaying = Arc::new(AtomicBool::new(true));
    delay_execution_on(victim_sim_id, 2_000, delaying.clone());

    // A weak reference, so it does not pin a killed node's store. It is
    // refreshed below whenever the victim runs again.
    let victim_state = Arc::new(Mutex::new(Weak::<AuthorityState>::new()));
    let refresh_victim_state = || {
        if let Some(handle) = test_cluster.swarm.node(&victim).unwrap().get_node_handle() {
            *victim_state.lock().unwrap() = handle.with(|node| Arc::downgrade(&node.state()));
        }
    };
    refresh_victim_state();

    // The epoch of every kill of the victim, each right after a synced
    // checkpoint.
    let killed_in_epochs: Arc<Mutex<Vec<EpochId>>> = Default::default();
    {
        let victim_state = victim_state.clone();
        let killed_in_epochs = killed_in_epochs.clone();
        register_fail_point("highest-executed-checkpoint", move || {
            if iota_simulator::current_simnode_id() != victim_sim_id {
                return;
            }
            let Some(state) = victim_state.lock().unwrap().upgrade() else {
                return;
            };
            let epoch_store = state.epoch_store_for_testing();
            let epoch = epoch_store.epoch();
            let switched = epoch_store.is_executing_synced_checkpoints();
            drop(epoch_store);
            drop(state);
            let mut killed_in_epochs = killed_in_epochs.lock().unwrap();
            let kills_in_epoch = killed_in_epochs.iter().filter(|e| **e == epoch).count();
            if switched && killed_in_epochs.len() < KILLS && kills_in_epoch < KILLS_PER_EPOCH {
                killed_in_epochs.push(epoch);
                drop(killed_in_epochs);
                iota_simulator::task::kill_current_node(None);
            }
        });
    }
    // For every start of the victim's consensus, the commit its replay waits
    // for and the highest commit consensus had stored.
    let replay_targets: Arc<Mutex<Vec<(u32, u32)>>> = Default::default();
    {
        let replay_targets = replay_targets.clone();
        register_fail_point_arg("consensus-replay-target", move || {
            if iota_simulator::current_simnode_id() != victim_sim_id {
                return None;
            }
            let replay_targets = replay_targets.clone();
            let report: ConsensusReplayTargetReport = Arc::new(move |target, highest_observed| {
                replay_targets
                    .lock()
                    .unwrap()
                    .push((target, highest_observed));
            });
            Some(report)
        });
    }

    let stop = Arc::new(AtomicBool::new(false));
    let traffic = spawn_transfer_traffic_with_double_spends(
        test_cluster.clone(),
        test_cluster.get_addresses(),
        stop.clone(),
    );

    let reached = || {
        killed_in_epochs.lock().unwrap().len() >= KILLS
            || (killed_in_epochs.lock().unwrap().len() >= KILLS_PER_EPOCH
                && replay_targets
                    .lock()
                    .unwrap()
                    .iter()
                    .any(|(target, highest_observed)| highest_observed > target))
    };
    timeout(Duration::from_secs(300), async {
        while !reached() {
            refresh_victim_state();
            sleep(Duration::from_millis(100)).await;
        }
    })
    .await
    .unwrap_or_else(|_| {
        panic!(
            "the victim must be killed after switching, was killed in epochs {:?}",
            killed_in_epochs.lock().unwrap()
        )
    });
    // Let the last restart's replay and the commits after it run.
    sleep(Duration::from_secs(10)).await;
    let last_killed_epoch = *killed_in_epochs.lock().unwrap().last().unwrap();
    assert!(
        replay_targets
            .lock()
            .unwrap()
            .iter()
            .any(|(target, highest_observed)| highest_observed > target),
        "a restart after the switch must find stored commits above its replay target; \
         killed in epochs {:?}, replay targets and highest stored commits {:?}",
        killed_in_epochs.lock().unwrap(),
        replay_targets.lock().unwrap(),
    );

    delaying.store(false, Ordering::SeqCst);
    wait_for_validators_to_reach_epoch(
        &test_cluster,
        &names,
        last_killed_epoch + 2,
        Duration::from_secs(180),
    )
    .await;
    stop_traffic(traffic, &stop).await;
    assert_validators_reach_the_same_checkpoints(&test_cluster).await;
    clear_fail_point("transaction_execution_delay");
    clear_fail_point("highest-executed-checkpoint");
    clear_fail_point("consensus-replay-target");
}

/// The checkpoints `name` built itself and holds as synced but has not
/// executed, among them those whose transactions are not all in its
/// perpetual store. Those transactions are only in memory, so a crash loses
/// them.
fn unexecuted_locally_built_checkpoints_without_stored_transactions(
    test_cluster: &TestCluster,
    name: &AuthorityName,
) -> Vec<CheckpointSequenceNumber> {
    let handle = test_cluster
        .swarm
        .node(name)
        .unwrap()
        .get_node_handle()
        .unwrap();
    handle.with(|node| {
        let state = node.state();
        let store = state.get_checkpoint_store();
        let database = state.database_for_testing();
        let executed = store
            .get_highest_executed_checkpoint_seq_number()
            .unwrap()
            .unwrap_or_default();
        let synced = store
            .get_highest_synced_checkpoint_seq_number()
            .unwrap()
            .unwrap_or_default();
        (executed + 1..=synced)
            .filter(|&sequence_number| {
                if store
                    .get_locally_computed_checkpoint(sequence_number)
                    .unwrap()
                    .is_none()
                {
                    return false;
                }
                let checkpoint = store
                    .get_checkpoint_by_sequence_number(sequence_number)
                    .unwrap()
                    .expect("a synced checkpoint is stored");
                let digests: Vec<_> = store
                    .get_checkpoint_contents(&checkpoint.contents_digest)
                    .unwrap()
                    .expect("the contents of a synced checkpoint are stored")
                    .transactions()
                    .iter()
                    .map(|digests| digests.transaction)
                    .collect();
                database
                    .multi_get_transaction_blocks(&digests)
                    .unwrap()
                    .iter()
                    .any(Option::is_none)
            })
            .collect()
    })
}

/// How many commits have their consensus commit prologue in one of
/// `checkpoints` of `name`, read from its transaction cache, which also holds
/// transactions executed only in memory. Each commit has one prologue.
fn commits_in_checkpoints(
    test_cluster: &TestCluster,
    name: &AuthorityName,
    checkpoints: &[CheckpointSequenceNumber],
) -> usize {
    let handle = test_cluster
        .swarm
        .node(name)
        .unwrap()
        .get_node_handle()
        .unwrap();
    handle.with(|node| {
        let state = node.state();
        let store = state.get_checkpoint_store();
        let digests: Vec<_> = checkpoints
            .iter()
            .flat_map(|&sequence_number| {
                let checkpoint = store
                    .get_checkpoint_by_sequence_number(sequence_number)
                    .unwrap()
                    .expect("a synced checkpoint is stored");
                store
                    .get_checkpoint_contents(&checkpoint.contents_digest)
                    .unwrap()
                    .expect("the contents of a synced checkpoint are stored")
                    .transactions()
                    .iter()
                    .map(|digests| digests.transaction)
                    .collect::<Vec<_>>()
            })
            .collect();
        state
            .get_transaction_cache_reader()
            .multi_get_transaction_blocks(&digests)
            .into_iter()
            .flatten()
            .filter(|transaction| {
                matches!(
                    transaction.data().transaction().kind(),
                    TransactionKind::ConsensusCommitPrologueV1(_)
                )
            })
            .count()
    })
}

/// The simulator node ids of every node of the cluster but `name`. They stay
/// the same while those nodes run; `name` gets a new one whenever it starts
/// again.
fn sim_node_ids_of_all_nodes_but(
    test_cluster: &TestCluster,
    name: &AuthorityName,
) -> Vec<iota_simulator::task::NodeId> {
    let mut ids: Vec<_> = test_cluster
        .get_validator_pubkeys()
        .iter()
        .filter(|other| *other != name)
        .map(|other| sim_node_id(test_cluster, other))
        .collect();
    ids.push(
        test_cluster
            .fullnode_handle
            .iota_node
            .with(|_| iota_simulator::current_simnode_id()),
    );
    ids
}

/// Holds the checkpoint executors of `names` at the next checkpoint each built
/// itself, once they have executed their first checkpoints, until each has at
/// least `at_risk` checkpoints it built certified and synced without their
/// transactions on disk. The traffic must already run. Returns the flag that
/// keeps holding the executors while set.
async fn hold_executors_until_locally_built_checkpoints_are_unexecuted(
    test_cluster: &TestCluster,
    names: &[AuthorityName],
    at_risk: usize,
) -> Arc<AtomicBool> {
    let sim_ids: Vec<_> = names
        .iter()
        .map(|name| sim_node_id(test_cluster, name))
        .collect();
    let holding = Arc::new(AtomicBool::new(false));
    {
        let holding = holding.clone();
        register_fail_point_async("execute-locally-built-checkpoint", move || {
            let hold = sim_ids.contains(&iota_simulator::current_simnode_id());
            let holding = holding.clone();
            async move {
                while hold && holding.load(Ordering::SeqCst) {
                    sleep(Duration::from_millis(100)).await;
                }
            }
        });
    }
    timeout(Duration::from_secs(60), async {
        while names
            .iter()
            .any(|name| progress(test_cluster, name).unwrap().highest_executed < 3)
        {
            sleep(Duration::from_millis(200)).await;
        }
    })
    .await
    .expect("the validators must execute their first checkpoints");

    holding.store(true, Ordering::SeqCst);
    timeout(Duration::from_secs(60), async {
        while names.iter().any(|name| {
            unexecuted_locally_built_checkpoints_without_stored_transactions(test_cluster, name)
                .len()
                < at_risk
        }) {
            sleep(Duration::from_millis(200)).await;
        }
    })
    .await
    .unwrap_or_else(|_| {
        panic!(
            "the held validators must certify checkpoints they built: {:?}",
            names
                .iter()
                .map(|name| progress(test_cluster, name))
                .collect::<Vec<_>>()
        )
    });
    holding
}

/// Holds every transaction execution on the nodes `selects` picks while
/// `holding` is set.
fn hold_execution_on_nodes(
    selects: impl Fn(iota_simulator::task::NodeId) -> bool + Send + Sync + 'static,
    holding: Arc<AtomicBool>,
) {
    register_fail_point_async("transaction_execution_delay", move || {
        let hold = selects(iota_simulator::current_simnode_id());
        let holding = holding.clone();
        async move {
            while hold && holding.load(Ordering::SeqCst) {
                sleep(Duration::from_millis(100)).await;
            }
        }
    });
}

/// A validator whose checkpoint executor falls behind its own checkpoint
/// builder can switch to executing synced checkpoints while certified
/// checkpoints it built itself have not executed yet. Their transactions are
/// then only in memory: state sync downloads no contents for a checkpoint this
/// node certified itself, or for one whose contents it still held in memory
/// from building it. After a crash at that point, the restarted validator
/// replays its commits up to the one it stopped at and then executes synced
/// checkpoints again. It must execute those checkpoints even while the replay
/// executes their transactions slowly, and the cluster must reach later epochs
/// on the same checkpoints.
///
/// The executor is held at the first checkpoint it built, so it never reaches
/// a checkpoint it did not build, where it would switch once the epoch's last
/// checkpoint is certified. The test switches the validator itself instead,
/// mid-epoch, which saves the same state.
#[sim_test]
async fn test_restart_after_switching_with_unexecuted_locally_built_checkpoints() {
    const AT_RISK_BEFORE_THE_SWITCH: usize = 3;
    telemetry_subscribers::init_for_testing();
    let _guard = enable_deterministic_validation_for_testing();
    let test_cluster = build_cluster(60_000).await;
    let names = test_cluster.get_validator_pubkeys();
    let victim = names[0];
    let others = sim_node_ids_of_all_nodes_but(&test_cluster, &victim);
    let delaying = Arc::new(AtomicBool::new(false));
    delay_execution_on_nodes(move |id| !others.contains(&id), 3_000, delaying.clone());

    let stop = Arc::new(AtomicBool::new(false));
    let traffic = spawn_transfer_traffic_with_double_spends(
        test_cluster.clone(),
        test_cluster.get_addresses(),
        stop.clone(),
    );
    let holding = hold_executors_until_locally_built_checkpoints_are_unexecuted(
        &test_cluster,
        &[victim],
        AT_RISK_BEFORE_THE_SWITCH,
    )
    .await;

    let victim_handle = test_cluster
        .swarm
        .node(&victim)
        .unwrap()
        .get_node_handle()
        .unwrap();
    let epoch = victim_handle
        .with_async(|node| async move {
            let epoch_store = node.state().epoch_store_for_testing().clone();
            epoch_store.start_executing_synced_checkpoints().await;
            epoch_store.epoch()
        })
        .await;
    let last_commit_before_switch = victim_handle.with(|node| {
        node.state()
            .epoch_store_for_testing()
            .last_commit_before_synced_checkpoint_execution()
            .unwrap()
    });
    assert!(
        last_commit_before_switch.is_some(),
        "the switch must save the last commit the handler processed"
    );
    drop(victim_handle);
    // State sync marks the checkpoints the victim built last as synced once
    // its peers certify them, without downloading their contents.
    timeout(Duration::from_secs(30), async {
        while progress(&test_cluster, &victim).is_none_or(|p| p.highest_synced < p.last_built) {
            sleep(Duration::from_millis(200)).await;
        }
    })
    .await
    .unwrap_or_else(|_| {
        panic!(
            "every checkpoint the victim built must become synced: {:?}",
            progress(&test_cluster, &victim)
        )
    });
    let at_risk =
        unexecuted_locally_built_checkpoints_without_stored_transactions(&test_cluster, &victim);
    assert!(
        at_risk.len() >= AT_RISK_BEFORE_THE_SWITCH,
        "the victim must hold synced checkpoints it built whose transactions are only in \
         memory: {at_risk:?}, {:?}",
        progress(&test_cluster, &victim)
    );
    info!(
        ?at_risk,
        ?last_commit_before_switch,
        "stopping the victim after the switch"
    );

    test_cluster.stop_node(&victim);
    holding.store(false, Ordering::SeqCst);
    delaying.store(true, Ordering::SeqCst);
    // Lets the stopped node's tasks end, so they release its store.
    sleep(Duration::from_secs(5)).await;
    test_cluster.start_node(&victim).await;
    sleep(Duration::from_secs(20)).await;
    delaying.store(false, Ordering::SeqCst);

    wait_for_validators_to_reach_epoch(&test_cluster, &names, epoch + 2, Duration::from_secs(240))
        .await;
    stop_traffic(traffic, &stop).await;
    assert_validators_reach_the_same_checkpoints(&test_cluster).await;
    clear_fail_point("execute-locally-built-checkpoint");
    clear_fail_point("transaction_execution_delay");
}

/// A validator restarted while its checkpoint executor is behind its own
/// checkpoint builder replays its commits to execute again the transactions of
/// the certified checkpoints it built, which were only in memory. When that
/// replay stalls past the startup wait for the rebuild, the executor starts
/// anyway and waits at the first such checkpoint. Once the epoch's last
/// checkpoint is certified, the validator switches to executing synced
/// checkpoints, which stops the replay below the commits of the later
/// checkpoints it built. It must still execute those checkpoints, and the
/// cluster must reach later epochs on the same checkpoints.
///
/// The executor is held before the restart as in
/// `test_restart_after_switching_with_unexecuted_locally_built_checkpoints`.
/// After the restart, every transaction execution on the validator is held so
/// the replay cannot get past the commits it waits for. Test builds panic when
/// the startup wait for the rebuild times out, which production only logs, so
/// the test logs it instead.
#[sim_test]
async fn test_restart_with_a_stalled_replay_and_unexecuted_locally_built_checkpoints() {
    const AT_RISK_BEFORE_THE_RESTART: usize = 4;
    telemetry_subscribers::init_for_testing();
    let _guard = enable_deterministic_validation_for_testing();
    register_debug_fatal_handler!("Timed out waiting for checkpoints to be rebuilt", || {});
    let test_cluster = build_cluster(60_000).await;
    let names = test_cluster.get_validator_pubkeys();
    let victim = names[0];
    let others = sim_node_ids_of_all_nodes_but(&test_cluster, &victim);
    let holding_execution = Arc::new(AtomicBool::new(false));
    hold_execution_on_nodes(move |id| !others.contains(&id), holding_execution.clone());

    let stop = Arc::new(AtomicBool::new(false));
    let traffic = spawn_transfer_traffic_with_double_spends(
        test_cluster.clone(),
        test_cluster.get_addresses(),
        stop.clone(),
    );
    let holding_executor = hold_executors_until_locally_built_checkpoints_are_unexecuted(
        &test_cluster,
        &[victim],
        AT_RISK_BEFORE_THE_RESTART,
    )
    .await;
    // The commits of these checkpoints are above every commit whose
    // transactions are on disk. After the restart nothing executes, so the
    // replay processes at most K commits above those and the switch saves a
    // commit no higher. With more than K commits in these checkpoints, some
    // lie above it, and the replay never executes their transactions.
    let horizon_distance = test_cluster
        .swarm
        .node(&victim)
        .unwrap()
        .get_node_handle()
        .unwrap()
        .with(|node| {
            node.state()
                .epoch_store_for_testing()
                .protocol_config()
                .pcool_deterministic_validation_horizon_distance_or_default()
        });
    let (at_risk, commits_at_risk) = timeout(Duration::from_secs(60), async {
        loop {
            let at_risk = unexecuted_locally_built_checkpoints_without_stored_transactions(
                &test_cluster,
                &victim,
            );
            let commits = commits_in_checkpoints(&test_cluster, &victim, &at_risk);
            if commits as u64 > horizon_distance {
                return (at_risk, commits);
            }
            sleep(Duration::from_millis(200)).await;
        }
    })
    .await
    .unwrap_or_else(|_| {
        panic!(
            "the checkpoints the held victim built must cover more than {horizon_distance} \
             commits: {:?}",
            progress(&test_cluster, &victim)
        )
    });
    let epoch = progress(&test_cluster, &victim).unwrap().epoch;
    info!(
        ?at_risk,
        commits_at_risk, "stopping the victim before any switch"
    );

    test_cluster.stop_node(&victim);
    holding_executor.store(false, Ordering::SeqCst);
    holding_execution.store(true, Ordering::SeqCst);
    // Lets the stopped node's tasks end, so they release its store.
    sleep(Duration::from_secs(5)).await;
    test_cluster.start_node(&victim).await;

    let last_commit_before_switch = timeout(Duration::from_secs(300), async {
        loop {
            let saved = test_cluster
                .swarm
                .node(&victim)
                .unwrap()
                .get_node_handle()
                .and_then(|handle| {
                    handle.with(|node| {
                        let epoch_store = node.state().epoch_store_for_testing();
                        (epoch_store.epoch() == epoch)
                            .then(|| {
                                epoch_store
                                    .last_commit_before_synced_checkpoint_execution()
                                    .unwrap()
                            })
                            .flatten()
                    })
                });
            if let Some(saved) = saved {
                return saved;
            }
            sleep(Duration::from_millis(200)).await;
        }
    })
    .await
    .unwrap_or_else(|_| {
        panic!(
            "the restarted victim must switch to executing synced checkpoints in epoch {epoch}: \
             {:?}",
            progress(&test_cluster, &victim)
        )
    });
    info!(last_commit_before_switch, "the restarted victim switched");
    holding_execution.store(false, Ordering::SeqCst);

    let target_epoch = names[1..]
        .iter()
        .filter_map(|name| progress(&test_cluster, name))
        .map(|p| p.epoch)
        .max()
        .unwrap()
        + 1;
    wait_for_validators_to_reach_epoch(
        &test_cluster,
        &names,
        target_epoch,
        Duration::from_secs(300),
    )
    .await;
    stop_traffic(traffic, &stop).await;
    assert_validators_reach_the_same_checkpoints(&test_cluster).await;
    clear_fail_point("execute-locally-built-checkpoint");
    clear_fail_point("transaction_execution_delay");
}

/// Every validator restarts at once while each holds certified checkpoints it
/// built itself whose transactions were only in memory. No validator then has
/// those checkpoints' contents on disk, so each one must execute them again
/// from its own replay before it can load them. Every validator must execute
/// past them, and the network must reach a later epoch on the same
/// checkpoints.
///
/// The executors are held at the first checkpoint each validator built, as in
/// `test_restart_after_switching_with_unexecuted_locally_built_checkpoints`.
/// The fullnode stays stopped, because it stores the contents it downloaded
/// and would serve them.
#[sim_test]
async fn test_restart_of_every_validator_with_unexecuted_locally_built_checkpoints() {
    const AT_RISK_BEFORE_THE_RESTART: usize = 2;
    telemetry_subscribers::init_for_testing();
    let _guard = enable_deterministic_validation_for_testing();
    let test_cluster = build_cluster(60_000).await;
    let names = test_cluster.get_validator_pubkeys();

    let stop = Arc::new(AtomicBool::new(false));
    let traffic = spawn_transfer_traffic_with_double_spends(
        test_cluster.clone(),
        test_cluster.get_addresses(),
        stop.clone(),
    );
    let holding = hold_executors_until_locally_built_checkpoints_are_unexecuted(
        &test_cluster,
        &names,
        AT_RISK_BEFORE_THE_RESTART,
    )
    .await;
    stop_traffic(traffic, &stop).await;
    let epoch = progress(&test_cluster, &names[0]).unwrap().epoch;
    let at_risk: Vec<_> = names
        .iter()
        .map(|name| {
            unexecuted_locally_built_checkpoints_without_stored_transactions(&test_cluster, name)
        })
        .collect();
    assert!(
        at_risk
            .iter()
            .all(|checkpoints| checkpoints.len() >= AT_RISK_BEFORE_THE_RESTART),
        "every validator must hold synced checkpoints it built whose transactions are only in \
         memory: {at_risk:?}"
    );
    let highest_at_risk = at_risk.iter().flatten().copied().max().unwrap();
    info!(?at_risk, "stopping every node");

    for fullnode in test_cluster.swarm.fullnodes() {
        fullnode.stop();
    }
    test_cluster.stop_all_validators().await;
    holding.store(false, Ordering::SeqCst);
    // Lets the stopped nodes' tasks end, so they release their stores.
    sleep(Duration::from_secs(5)).await;
    test_cluster.start_all_validators().await;

    timeout(Duration::from_secs(120), async {
        while names.iter().any(|name| {
            progress(&test_cluster, name).is_none_or(|p| p.highest_executed <= highest_at_risk)
        }) {
            sleep(Duration::from_millis(500)).await;
        }
    })
    .await
    .unwrap_or_else(|_| {
        panic!(
            "every validator must execute past checkpoint {highest_at_risk}: {:?}",
            names
                .iter()
                .map(|name| progress(&test_cluster, name))
                .collect::<Vec<_>>()
        )
    });

    wait_for_validators_to_reach_epoch(&test_cluster, &names, epoch + 1, Duration::from_secs(240))
        .await;
    let highest_executed = names
        .iter()
        .filter_map(|name| progress(&test_cluster, name))
        .map(|p| p.highest_executed)
        .max()
        .unwrap();
    assert_validators_reach_the_same_checkpoints_up_to(&test_cluster, highest_executed + 5).await;
    clear_fail_point("execute-locally-built-checkpoint");
}

/// Every validator crashes while executing the last checkpoint of an epoch,
/// before it records it as executed, and all of them are down at once. No
/// checkpoint is certified after the epoch's last one, and no node has that
/// checkpoint synced over stored contents to advertise it, so after the
/// restart each validator must mark it synced from its own store and replay.
/// Every validator must reach the next epoch.
///
/// The fullnode is stopped from the start, because it would download the
/// last checkpoint's contents and serve them.
#[sim_test]
async fn test_crash_of_every_validator_while_executing_the_last_checkpoint_of_an_epoch() {
    const RESTART_AFTER: Duration = Duration::from_secs(10);
    telemetry_subscribers::init_for_testing();
    let _guard = enable_deterministic_validation_for_testing();
    let test_cluster = build_cluster(20_000).await;
    let names = test_cluster.get_validator_pubkeys();
    for fullnode in test_cluster.swarm.fullnodes() {
        fullnode.stop();
    }
    let epoch = progress(&test_cluster, &names[0]).unwrap().epoch;

    // Each validator before its first kill, as its simulator node id and a
    // weak reference that does not pin its store. A restarted validator has
    // a new state, so it is not killed again.
    let validators: Vec<(iota_simulator::task::NodeId, Weak<AuthorityState>)> = names
        .iter()
        .map(|name| {
            test_cluster
                .swarm
                .node(name)
                .unwrap()
                .get_node_handle()
                .unwrap()
                .with(|node| {
                    (
                        iota_simulator::current_simnode_id(),
                        Arc::downgrade(&node.state()),
                    )
                })
        })
        .collect();
    let killed: Arc<Mutex<Vec<iota_simulator::task::NodeId>>> = Default::default();
    {
        let killed = killed.clone();
        register_fail_point("highest-executed-checkpoint", move || {
            let sim_id = iota_simulator::current_simnode_id();
            let Some(state) = validators
                .iter()
                .find(|(id, _)| *id == sim_id)
                .and_then(|(_, state)| state.upgrade())
            else {
                return;
            };
            let store = state.get_checkpoint_store();
            let next = store
                .get_highest_executed_checkpoint_seq_number()
                .unwrap()
                .map_or(0, |executed| executed + 1);
            if store.get_epoch_last_checkpoint_seq_number(epoch).unwrap() != Some(next) {
                return;
            }
            drop(state);
            killed.lock().unwrap().push(sim_id);
            iota_simulator::task::kill_current_node(Some(RESTART_AFTER));
        });
    }

    timeout(Duration::from_secs(120), async {
        while killed.lock().unwrap().len() < names.len() {
            sleep(Duration::from_millis(100)).await;
        }
    })
    .await
    .unwrap_or_else(|_| {
        panic!(
            "every validator must be killed while executing the last checkpoint of epoch \
             {epoch}, killed: {:?}",
            killed.lock().unwrap()
        )
    });

    wait_for_validators_to_reach_epoch(&test_cluster, &names, epoch + 1, Duration::from_secs(120))
        .await;
    clear_fail_point("highest-executed-checkpoint");
}

/// Every validator crashes after its checkpoint aggregator stored the
/// certified last checkpoint of an epoch and before it handed it to state
/// sync, so no node verifies that checkpoint. After the restart the
/// aggregator does not send it again and no checkpoint of the epoch comes
/// after it, so state sync must verify it from the store and mark it synced.
/// Every validator must reach the next epoch.
///
/// The fullnode is stopped from the start, because it would download the
/// last checkpoint from a validator and serve it.
#[sim_test]
async fn test_crash_of_every_validator_between_certifying_and_verifying_the_last_checkpoint_of_an_epoch()
 {
    const RESTART_AFTER: Duration = Duration::from_secs(10);
    telemetry_subscribers::init_for_testing();
    let _guard = enable_deterministic_validation_for_testing();
    let test_cluster = build_cluster(20_000).await;
    let names = test_cluster.get_validator_pubkeys();
    for fullnode in test_cluster.swarm.fullnodes() {
        fullnode.stop();
    }
    let epoch = progress(&test_cluster, &names[0]).unwrap().epoch;

    let validators: Vec<_> = names
        .iter()
        .map(|name| sim_node_id(&test_cluster, name))
        .collect();
    let killed: Arc<Mutex<Vec<iota_simulator::task::NodeId>>> = Default::default();
    {
        let killed = killed.clone();
        register_fail_point(
            "certified-end-of-epoch-checkpoint-before-state-sync",
            move || {
                let sim_id = iota_simulator::current_simnode_id();
                if !validators.contains(&sim_id) {
                    return;
                }
                {
                    let mut killed = killed.lock().unwrap();
                    if killed.contains(&sim_id) {
                        return;
                    }
                    killed.push(sim_id);
                }
                iota_simulator::task::kill_current_node(Some(RESTART_AFTER));
            },
        );
    }

    timeout(Duration::from_secs(120), async {
        while killed.lock().unwrap().len() < names.len() {
            sleep(Duration::from_millis(100)).await;
        }
    })
    .await
    .unwrap_or_else(|_| {
        panic!(
            "every validator must be killed after certifying the last checkpoint of epoch \
             {epoch}, killed: {:?}",
            killed.lock().unwrap()
        )
    });

    wait_for_validators_to_reach_epoch(&test_cluster, &names, epoch + 1, Duration::from_secs(120))
        .await;
    clear_fail_point("certified-end-of-epoch-checkpoint-before-state-sync");
}

/// Runs double-spend traffic while one validator is killed `kills` times at
/// `fail_point`, at the first hit after a pause, and restarts on its own each
/// time. The cluster must then reach the same checkpoints on every validator
/// and a later epoch.
async fn run_crashes_at_fail_point_under_traffic(fail_point: &'static str, kills: usize) {
    telemetry_subscribers::init_for_testing();
    let _guard = enable_deterministic_validation_for_testing();
    let test_cluster = build_cluster(60_000).await;
    let names = test_cluster.get_validator_pubkeys();
    let victim = names[0];
    let victim_sim_id = sim_node_id(&test_cluster, &victim);

    let stop = Arc::new(AtomicBool::new(false));
    let traffic = spawn_transfer_traffic_with_double_spends(
        test_cluster.clone(),
        test_cluster.get_addresses(),
        stop.clone(),
    );

    let killed = Arc::new(AtomicUsize::new(0));
    let next_kill = Arc::new(Mutex::new(Instant::now() + Duration::from_secs(5)));
    {
        let killed = killed.clone();
        register_fail_point(fail_point, move || {
            if iota_simulator::current_simnode_id() != victim_sim_id
                || killed.load(Ordering::SeqCst) >= kills
            {
                return;
            }
            {
                let mut next_kill = next_kill.lock().unwrap();
                if Instant::now() < *next_kill {
                    return;
                }
                *next_kill = Instant::now() + Duration::from_secs(10);
            }
            killed.fetch_add(1, Ordering::SeqCst);
            iota_simulator::task::kill_current_node(None);
        });
    }

    timeout(Duration::from_secs(30 * kills as u64), async {
        while killed.load(Ordering::SeqCst) < kills {
            sleep(Duration::from_millis(200)).await;
        }
    })
    .await
    .unwrap_or_else(|_| {
        panic!(
            "the victim must be killed {kills} times at {fail_point}, was killed {}",
            killed.load(Ordering::SeqCst)
        )
    });
    sleep(Duration::from_secs(5)).await;
    assert_validators_reach_the_same_checkpoints(&test_cluster).await;
    wait_for_validators_to_reach_epoch(&test_cluster, &names, 2, Duration::from_secs(180)).await;
    stop_traffic(traffic, &stop).await;
    assert_validators_reach_the_same_checkpoints(&test_cluster).await;
    clear_fail_point(fail_point);
}

/// A validator killed repeatedly right after a checkpoint's bookkeeping batch
/// is durable, before the checkpoint's outputs are.
#[sim_test]
async fn test_crashes_after_checkpoint_bookkeeping_write_do_not_fork() {
    run_crashes_at_fail_point_under_traffic("crash-after-checkpoint-bookkeeping-write", 3).await;
}

/// A validator killed repeatedly right after it saved a commit's validation
/// decisions, before it scheduled the commit's transactions.
#[sim_test]
async fn test_crashes_after_post_consensus_verdicts_do_not_fork() {
    run_crashes_at_fail_point_under_traffic("crash-after-post-consensus-verdicts", 3).await;
}

/// One validator executes every transaction up to 300 ms late under
/// double-spend traffic. Its consensus handler waits for that execution before
/// it validates, and its checkpoint executor waits for the checkpoints that
/// handler's commits produce. Neither may wait on the other for good: the
/// validator keeps building and executing its own checkpoints, does not
/// switch to synced checkpoints within the epoch, catches up once the delay
/// stops and reaches later epochs.
#[sim_test]
async fn test_executor_waiting_for_a_slow_handler_does_not_deadlock_or_fork() {
    telemetry_subscribers::init_for_testing();
    let _guard = enable_deterministic_validation_for_testing();
    let test_cluster = build_cluster(60_000).await;
    let names = test_cluster.get_validator_pubkeys();
    let slow = names[2];
    let delaying = Arc::new(AtomicBool::new(true));
    delay_execution_on(sim_node_id(&test_cluster, &slow), 300, delaying.clone());

    let stop = Arc::new(AtomicBool::new(false));
    let traffic = spawn_transfer_traffic_with_double_spends(
        test_cluster.clone(),
        test_cluster.get_addresses(),
        stop.clone(),
    );
    // The slow validator must keep making progress on its own checkpoints
    // while the delay holds.
    let mut built = progress(&test_cluster, &slow).unwrap().last_built;
    for _ in 0..4 {
        sleep(Duration::from_secs(5)).await;
        let p = progress(&test_cluster, &slow).unwrap();
        assert!(
            p.last_built > built && p.highest_executed > 0,
            "the slow validator must keep building checkpoints: built {built} before, now {p:?}"
        );
        assert!(
            !p.executing_synced_checkpoints,
            "the slow validator must not switch to synced checkpoints mid-epoch: {p:?}"
        );
        built = p.last_built;
    }
    assert_validators_reach_the_same_checkpoints(&test_cluster).await;

    delaying.store(false, Ordering::SeqCst);
    wait_for_validators_to_reach_epoch(&test_cluster, &names, 2, Duration::from_secs(180)).await;
    stop_traffic(traffic, &stop).await;
    assert_validators_reach_the_same_checkpoints(&test_cluster).await;
    clear_fail_point("transaction_execution_delay");
}
