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
use iota_core::{authority::AuthorityState, consensus_manager::ConsensusReplayTargetReport};
use iota_macros::{
    clear_fail_point, register_fail_point, register_fail_point_arg, register_fail_point_async,
    sim_test,
};
use iota_protocol_config::ProtocolConfig;
use iota_sdk_types::Address;
use iota_test_transaction_builder::TestTransactionBuilder;
use iota_types::{
    base_types::{AuthorityName, ConciseableName, EpochId, dbg_addr},
    messages_checkpoint::CheckpointSequenceNumber,
};
use rand::distr::{Distribution, Uniform};
use test_cluster::{TestCluster, TestClusterBuilder};
use tokio::{
    task::JoinHandle,
    time::{Instant, sleep, timeout},
};

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
    register_fail_point_async("transaction_execution_delay", move || {
        let delay =
            iota_simulator::current_simnode_id() == sim_id && enabled.load(Ordering::SeqCst);
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
