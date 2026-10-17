// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Cluster tests for P-COOL deterministic validation.
//!
//! Gated to the simulator for the reason given in
//! `pcool_transaction_flow_tests.rs`: the protocol-config override that turns
//! the feature on is thread-local, and only the simulator runs every node on
//! the test's thread.
//!
//! Run with: `cargo simtest -p iota-e2e-tests pcool_deterministic_validation`.

#![cfg(msim)]

use std::{
    collections::{BTreeSet, HashMap},
    path::PathBuf,
    sync::{
        Arc, Mutex, Weak,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    time::Duration,
};

use futures::future::join_all;
use iota_core::authority::AuthorityState;
use iota_json_rpc_types::IotaTransactionBlockEffectsAPI;
use iota_macros::{
    clear_fail_point, register_fail_point, register_fail_point_async, register_fail_point_if,
    sim_test,
};
use iota_protocol_config::ProtocolConfig;
use iota_sdk_types::{
    Address, ExecutionError, ExecutionStatus, ObjectId, ObjectReference, Owner,
    SharedObjectReference, Transaction, TransactionEffects, TypeTag, Version,
};
use iota_test_transaction_builder::{
    TestTransactionBuilder, emit_new_random_u128, publish_basics_package,
};
use iota_types::{
    base_types::{AuthorityName, ConciseableName, EpochId, dbg_addr},
    effects::{TransactionEffectsAPI, TransactionEffectsExt},
    iota_system_state::epoch_start_iota_system_state::EpochStartSystemStateTrait,
    messages_checkpoint::CheckpointSequenceNumber,
    storage::ObjectKey,
    transaction::CallArg,
};
use prometheus_filtered::core::Collector;
use rand::distr::{Distribution, Uniform};
use test_cluster::{TestCluster, TestClusterBuilder};
use tokio::{
    task::JoinHandle,
    time::{Instant, sleep, timeout},
};

/// Enables the P-COOL flow and the deterministic-validation bookkeeping. The
/// guard must be held for the whole test.
fn enable_deterministic_validation_for_testing() -> iota_protocol_config::OverrideGuard {
    ProtocolConfig::apply_overrides_for_testing(|_, mut config| {
        config.enable_pcool_deterministic_validation_for_testing();
        config
    })
}

async fn transfer(test_cluster: &TestCluster) -> TransactionEffects {
    let tx = test_cluster
        .test_transaction_builder_with_sender(test_cluster.get_address_0())
        .await
        .transfer_iota(Some(1_000_000), dbg_addr(2))
        .build();
    test_cluster.sign_and_execute_transaction(&tx).await
}

/// The keys of the handler rows a transaction's execution produces: every
/// object it wrote, at the version it wrote.
fn written_keys(effects: &TransactionEffects) -> Vec<ObjectKey> {
    effects
        .all_changed_objects()
        .into_iter()
        .map(|(owned_ref, _)| {
            ObjectKey(
                owned_ref.reference().object_id,
                owned_ref.reference().version,
            )
        })
        .collect()
}

/// Sends transfers from every gas coin of `senders` until `stop` is set.
/// Every second coin is spent by two transfers at once, so commits carry
/// conflicting pairs that post-consensus validation must resolve the same way
/// on every validator. Coin versions are read from the fullnode, so a
/// transfer may also name a version that is already consumed. Returns how many
/// transfers succeeded.
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

fn highest_executed_checkpoint(
    test_cluster: &TestCluster,
    name: &AuthorityName,
) -> CheckpointSequenceNumber {
    test_cluster
        .swarm
        .node(name)
        .unwrap()
        .get_node_handle()
        .map(|handle| {
            handle.with(|node| {
                node.state()
                    .get_checkpoint_store()
                    .get_highest_executed_checkpoint_seq_number()
                    .unwrap()
                    .unwrap_or_default()
            })
        })
        .unwrap_or_default()
}

/// The highest checkpoint the validator built in its current epoch.
fn last_built_checkpoint(
    test_cluster: &TestCluster,
    name: &AuthorityName,
) -> CheckpointSequenceNumber {
    test_cluster
        .swarm
        .node(name)
        .unwrap()
        .get_node_handle()
        .map(|handle| {
            handle.with(|node| {
                node.state()
                    .epoch_store_for_testing()
                    .last_built_checkpoint_builder_summary()
                    .unwrap()
                    .map(|summary| summary.summary.sequence_number)
                    .unwrap_or_default()
            })
        })
        .unwrap_or_default()
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
/// one. A validator prunes its own summaries of earlier epochs when it executes
/// an epoch's last checkpoint, so in practice only checkpoints of its current
/// epoch are compared here. Earlier epochs are covered by the in-node fork
/// check: a validator whose validation decided differently builds a
/// checkpoint that differs from the certified one, which that check turns
/// into a panic, and stops executing.
async fn assert_validators_reach_the_same_checkpoints(test_cluster: &TestCluster) {
    let target = fullnode_highest_executed_checkpoint(test_cluster) + 5;
    for name in test_cluster.get_validator_pubkeys() {
        timeout(Duration::from_secs(180), async {
            while highest_executed_checkpoint(test_cluster, &name) < target
                || last_built_checkpoint(test_cluster, &name) < target
            {
                sleep(Duration::from_millis(500)).await;
            }
        })
        .await
        .unwrap_or_else(|_| {
            panic!(
                "validator {} must execute and build checkpoint {target}: executed {}, built {}",
                name.concise(),
                highest_executed_checkpoint(test_cluster, &name),
                last_built_checkpoint(test_cluster, &name),
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
) {
    let current_epoch = |name| {
        test_cluster
            .swarm
            .node(name)
            .unwrap()
            .get_node_handle()
            .map(|handle| handle.with(|node| node.state().epoch_store_for_testing().epoch()))
            .unwrap_or_default()
    };
    timeout(Duration::from_secs(180), async {
        while names.iter().any(|name| current_epoch(name) < epoch) {
            sleep(Duration::from_millis(500)).await;
        }
    })
    .await
    .unwrap_or_else(|_| panic!("every validator must reach epoch {epoch}"));
}

/// Builds a four-validator cluster with deterministic validation on and checks
/// that the override reached the validators.
async fn build_cluster(epoch_duration_ms: u64) -> Arc<TestCluster> {
    let test_cluster = TestClusterBuilder::new()
        .with_num_validators(4)
        .with_epoch_duration_ms(epoch_duration_ms)
        .build()
        .await;
    for handle in test_cluster.all_validator_handles() {
        handle.with(|node| {
            assert!(
                node.state()
                    .epoch_store_for_testing()
                    .protocol_config()
                    .pcool_deterministic_validation(),
                "the protocol-config override must reach the validators"
            );
        });
    }
    Arc::new(test_cluster)
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

/// Runs double-spend traffic while one validator is killed `kills` times at
/// `fail_point`, at the first hit after a pause, and restarts on its own each
/// time. The cluster must then reach the same checkpoints on every validator.
async fn run_crashes_at_fail_point_under_traffic(fail_point: &'static str, kills: usize) {
    telemetry_subscribers::init_for_testing();
    let _guard = enable_deterministic_validation_for_testing();
    let test_cluster = build_cluster(600_000).await;
    let victim = test_cluster.get_validator_pubkeys()[0];
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
                *next_kill = Instant::now() + Duration::from_secs(15);
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
    sleep(Duration::from_secs(10)).await;
    stop_traffic(traffic, &stop).await;
    assert_validators_reach_the_same_checkpoints(&test_cluster).await;
    clear_fail_point(fail_point);
}

/// A validator killed after a checkpoint's bookkeeping batch is durable, but
/// before the checkpoint's outputs are, re-executes the checkpoint on restart,
/// keeps executing checkpoints, and ends up with the same handler rows as its
/// peers. The fullnode, outside the committee, keeps no bookkeeping.
#[sim_test]
async fn test_crash_after_checkpoint_bookkeeping_write_recovers() {
    telemetry_subscribers::init_for_testing();
    let _guard = enable_deterministic_validation_for_testing();
    let test_cluster = TestClusterBuilder::new()
        .with_epoch_duration_ms(600_000)
        .build()
        .await;

    let names = test_cluster.get_validator_pubkeys();
    let (victim, peer) = (names[0], names[1]);
    let node_handle = |name| {
        test_cluster
            .swarm
            .node(&name)
            .unwrap()
            .get_node_handle()
            .unwrap()
    };
    // Read without keeping the handle: a held handle pins the killed node's
    // store, and the restarted node could not open it.
    let victim_sim_id = node_handle(victim).with(|node| {
        assert!(
            node.state()
                .epoch_store_for_testing()
                .protocol_config()
                .pcool_deterministic_validation(),
            "the protocol-config override must reach the validators"
        );
        iota_simulator::current_simnode_id()
    });

    let fired = Arc::new(AtomicBool::new(false));
    {
        let fired = fired.clone();
        register_fail_point("crash-after-checkpoint-bookkeeping-write", move || {
            if iota_simulator::current_simnode_id() == victim_sim_id
                && !fired.swap(true, Ordering::SeqCst)
            {
                iota_simulator::task::kill_current_node(None);
            }
        });
    }

    // The victim dies in the checkpoint that carries this transaction's rows,
    // and restarts on its own.
    let mut transfers = vec![transfer(&test_cluster).await];
    tokio::time::timeout(Duration::from_secs(60), async {
        while !fired.load(Ordering::SeqCst) {
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    })
    .await
    .expect("the victim must write a checkpoint's bookkeeping and crash");
    for _ in 0..3 {
        transfers.push(transfer(&test_cluster).await);
    }

    // The victim executes past every checkpoint the fullnode has.
    let target = test_cluster
        .fullnode_handle
        .iota_node
        .with(|node| {
            node.state()
                .get_checkpoint_store()
                .get_highest_executed_checkpoint_seq_number()
        })
        .unwrap()
        .expect("the fullnode has executed the transfers' checkpoints");

    // The fullnode runs no consensus handler and keeps no bookkeeping: none of
    // its executions leaves a handler row.
    for key in transfers.iter().flat_map(written_keys) {
        let row = test_cluster.fullnode_handle.iota_node.with(|node| {
            node.state()
                .epoch_store_for_testing()
                .handler_processed_object(&key)
                .unwrap()
        });
        assert_eq!(row, None, "the fullnode keeps a handler row for {key:?}");
    }
    let highest_executed = |name| {
        node_handle(name).with(|node| {
            node.state()
                .get_checkpoint_store()
                .get_highest_executed_checkpoint_seq_number()
                .unwrap()
                .unwrap_or_default()
        })
    };
    tokio::time::timeout(Duration::from_secs(120), async {
        while highest_executed(victim) < target {
            tokio::time::sleep(Duration::from_millis(500)).await;
        }
    })
    .await
    .expect("the restarted victim must keep executing checkpoints");

    // Its handler rows match the peer's for every transfer, including the one
    // whose checkpoint it re-executed. The rows may still be on their way
    // while the victim's handler replays, so wait for them.
    let rows = |name, key: ObjectKey| {
        node_handle(name).with(|node| {
            node.state()
                .epoch_store_for_testing()
                .handler_processed_object(&key)
                .unwrap()
        })
    };
    for key in transfers.iter().flat_map(written_keys) {
        let expected = rows(peer, key).expect("the peer has a handler row for every transfer");
        tokio::time::timeout(Duration::from_secs(60), async {
            while rows(victim, key) != Some(expected) {
                tokio::time::sleep(Duration::from_millis(500)).await;
            }
        })
        .await
        .unwrap_or_else(|_| {
            panic!(
                "the victim's handler row at {key:?} must match the peer's: {:?} vs {expected:?}",
                rows(victim, key)
            )
        });
    }
    clear_fail_point("crash-after-checkpoint-bookkeeping-write");
}

/// A validator killed between the builder writing a checkpoint's summary and
/// the hasher saving the checkpoint's state hash, while a randomness round is
/// in flight, must catch up after the restart, and after a second one.
///
/// Peers no longer resend their partial signatures for a round they
/// completed, so after the restart that round can only come from the
/// executor, which forwards it from a synced checkpoint when it gets there.
/// The replayed commit that reserved the round waits for it, so the consensus
/// replay never finishes; the builder waits for the replay before it rebuilds
/// anything; and the executor, started by the start-up timeout, waits for the
/// builder's hash of the checkpoint. The executor must hash that checkpoint
/// itself: once the handler has executed its transactions, it holds every
/// effect it needs.
#[sim_test]
async fn test_restart_after_crash_before_the_state_hash_is_saved_catches_up() {
    telemetry_subscribers::init_for_testing();
    let _guard = enable_deterministic_validation_for_testing();
    let test_cluster = TestClusterBuilder::new()
        .with_epoch_duration_ms(1_200_000)
        .build()
        .await;
    let package = publish_basics_package(&test_cluster.wallet).await;
    // Randomness works before the test interferes with it.
    emit_new_random_u128(&test_cluster.wallet, package.object_id).await;

    let victim = test_cluster.get_validator_pubkeys()[0];
    let node_handle = |name| {
        test_cluster
            .swarm
            .node(&name)
            .unwrap()
            .get_node_handle()
            .unwrap()
    };
    // Read without keeping the handle: a held handle pins the killed node's
    // store, and the restarted node could not open it.
    let victim_sim_id = node_handle(victim).with(|_| iota_simulator::current_simnode_id());
    let highest_executed = |name| {
        node_handle(name).with(|node| {
            node.state()
                .get_checkpoint_store()
                .get_highest_executed_checkpoint_seq_number()
                .unwrap()
                .unwrap_or_default()
        })
    };

    // Every fail point acts on the victim's first life only, between arming
    // and the kill.
    let armed = Arc::new(AtomicBool::new(false));
    let hold_builder = Arc::new(AtomicBool::new(false));
    let write_paused = Arc::new(AtomicBool::new(false));
    let hold_randomness = Arc::new(AtomicBool::new(false));
    let kill_armed = Arc::new(AtomicBool::new(false));
    let killed = Arc::new(AtomicBool::new(false));
    let on_victim = move || iota_simulator::current_simnode_id() == victim_sim_id;
    let holds = {
        let (armed, killed) = (armed.clone(), killed.clone());
        move || on_victim() && armed.load(Ordering::SeqCst) && !killed.load(Ordering::SeqCst)
    };
    // The victim's hasher saves no more state hashes.
    {
        let holds = holds.clone();
        register_fail_point_async("checkpoint-state-hasher-accumulate", move || {
            let hold = holds();
            async move {
                if hold {
                    std::future::pending::<()>().await;
                }
            }
        });
    }
    // Once a write is paused, the victim completes no more randomness rounds:
    // a round its peers complete from then on stays in flight on the victim.
    let round_in_flight = Arc::new(AtomicBool::new(false));
    {
        let (holds, hold_randomness, round_in_flight) = (
            holds.clone(),
            hold_randomness.clone(),
            round_in_flight.clone(),
        );
        register_fail_point_async("randomness-delay", move || {
            let hold = holds() && hold_randomness.load(Ordering::SeqCst);
            if hold {
                round_in_flight.store(true, Ordering::SeqCst);
            }
            async move {
                if hold {
                    std::future::pending::<()>().await;
                }
            }
        });
    }
    // The victim's builder stops before its next write until the kill is
    // armed, so the kill lands on that write.
    {
        let (holds, hold_builder, write_paused) =
            (holds.clone(), hold_builder.clone(), write_paused.clone());
        register_fail_point_async("checkpoint-builder-write-summaries", move || {
            let (holds, hold_builder) = (holds.clone(), hold_builder.clone());
            if holds() && hold_builder.load(Ordering::SeqCst) {
                write_paused.store(true, Ordering::SeqCst);
            }
            async move {
                while holds() && hold_builder.load(Ordering::SeqCst) {
                    tokio::time::sleep(Duration::from_millis(100)).await;
                }
            }
        });
    }
    // The kill, right after the next summary is durable and before its hash
    // could be saved; the node restarts on its own, with every hold off.
    {
        let (holds, kill_armed, killed) = (holds.clone(), kill_armed.clone(), killed.clone());
        register_fail_point("checkpoint-summaries-written", move || {
            if holds() && kill_armed.load(Ordering::SeqCst) && !killed.swap(true, Ordering::SeqCst)
            {
                iota_simulator::task::kill_current_node(None);
            }
        });
    }
    // A restarted validator gives up waiting for its builder after 120 s and
    // starts the executor anyway; release builds only log that.
    let start_up_timeouts = Arc::new(AtomicUsize::new(0));
    {
        let start_up_timeouts = start_up_timeouts.clone();
        iota_common::logging::intercept_debug_fatal::register_callback(
            "Timed out waiting for checkpoints to be rebuilt",
            move || {
                start_up_timeouts.fetch_add(1, Ordering::SeqCst);
            },
        );
    }

    hold_builder.store(true, Ordering::SeqCst);
    armed.store(true, Ordering::SeqCst);
    // A paused write first. The builder builds in height order, so once it has
    // built a checkpoint, the update of every round reserved before it has
    // executed, and a round held from now on is reserved at its last commit or
    // later, which the restarted validator replays.
    tokio::time::timeout(Duration::from_secs(60), async {
        while !write_paused.load(Ordering::SeqCst) {
            transfer(&test_cluster).await;
        }
    })
    .await
    .expect("the victim's builder must pause before writing a checkpoint");
    hold_randomness.store(true, Ordering::SeqCst);
    // Rounds are reserved on commits at least 500 ms apart, used or not.
    tokio::time::timeout(Duration::from_secs(60), async {
        while !round_in_flight.load(Ordering::SeqCst) {
            transfer(&test_cluster).await;
        }
    })
    .await
    .expect("the victim's peers must complete a round the victim holds");
    kill_armed.store(true, Ordering::SeqCst);
    hold_builder.store(false, Ordering::SeqCst);
    tokio::time::timeout(Duration::from_secs(60), async {
        while !killed.load(Ordering::SeqCst) {
            transfer(&test_cluster).await;
        }
    })
    .await
    .expect("the victim must write a checkpoint's summary and die");
    for _ in 0..3 {
        transfer(&test_cluster).await;
    }
    // The restarted victim is reachable once its start-up is done, and its
    // start-up waits up to 120 s for the builder.
    tokio::time::timeout(Duration::from_secs(240), async {
        while test_cluster
            .swarm
            .node(&victim)
            .unwrap()
            .get_node_handle()
            .is_none()
        {
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
    })
    .await
    .expect("the killed victim must come back");
    let target = test_cluster
        .fullnode_handle
        .iota_node
        .with(|node| {
            node.state()
                .get_checkpoint_store()
                .get_highest_executed_checkpoint_seq_number()
                .unwrap()
        })
        .expect("the fullnode has executed checkpoints");

    // The restarted victim; if it does not catch up, restart it once more.
    let mut after_restarts = Vec::new();
    for restart in 1..=2 {
        if restart > 1 {
            test_cluster.stop_node(&victim);
            // The stopped node's tasks must let go of its store first.
            tokio::time::sleep(Duration::from_secs(5)).await;
            test_cluster.start_node(&victim).await;
        }
        let caught_up = tokio::time::timeout(Duration::from_secs(300), async {
            while highest_executed(victim) < target {
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
        })
        .await
        .is_ok();
        after_restarts.push((
            restart,
            highest_executed(victim),
            start_up_timeouts.load(Ordering::SeqCst),
        ));
        if caught_up {
            break;
        }
    }
    assert!(
        highest_executed(victim) >= target,
        "the victim must execute past checkpoint {target} after a restart; \
         (restart, highest executed, start-up timeouts so far): {after_restarts:?}"
    );
}

/// A validator killed right after it saved a commit's validation decisions,
/// before the commit's output flushed, replays the commit on restart from the
/// saved decisions. It keeps building the same checkpoints as its peers, and
/// the saved decisions of every flushed commit are gone.
#[sim_test]
async fn test_crash_after_post_consensus_decisions_recovers() {
    telemetry_subscribers::init_for_testing();
    let _guard = enable_deterministic_validation_for_testing();
    let test_cluster = TestClusterBuilder::new()
        .with_epoch_duration_ms(600_000)
        .build()
        .await;

    let names = test_cluster.get_validator_pubkeys();
    let (victim, peer) = (names[0], names[1]);
    let node_handle = |name| {
        test_cluster
            .swarm
            .node(&name)
            .unwrap()
            .get_node_handle()
            .unwrap()
    };
    // Read without keeping the handle: a held handle pins the killed node's
    // store, and the restarted node could not open it.
    let victim_sim_id = node_handle(victim).with(|_| iota_simulator::current_simnode_id());

    let fired = Arc::new(AtomicBool::new(false));
    {
        let fired = fired.clone();
        register_fail_point("crash-after-post-consensus-decisions", move || {
            if iota_simulator::current_simnode_id() == victim_sim_id
                && !fired.swap(true, Ordering::SeqCst)
            {
                iota_simulator::task::kill_current_node(None);
            }
        });
    }

    // The victim dies in the first commit with a user transaction, and
    // restarts on its own.
    let mut transfers = vec![transfer(&test_cluster).await];
    tokio::time::timeout(Duration::from_secs(60), async {
        while !fired.load(Ordering::SeqCst) {
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    })
    .await
    .expect("the victim must save a commit's validation decisions and crash");
    for _ in 0..3 {
        transfers.push(transfer(&test_cluster).await);
    }

    let target = test_cluster
        .fullnode_handle
        .iota_node
        .with(|node| {
            node.state()
                .get_checkpoint_store()
                .get_highest_executed_checkpoint_seq_number()
        })
        .unwrap()
        .expect("the fullnode has executed the transfers' checkpoints");
    let highest_executed = |name| {
        node_handle(name).with(|node| {
            node.state()
                .get_checkpoint_store()
                .get_highest_executed_checkpoint_seq_number()
                .unwrap()
                .unwrap_or_default()
        })
    };
    tokio::time::timeout(Duration::from_secs(120), async {
        while highest_executed(victim) < target {
            tokio::time::sleep(Duration::from_millis(500)).await;
        }
    })
    .await
    .expect("the restarted victim must keep executing checkpoints");

    // Every checkpoint the victim built itself matches the certified one.
    for sequence_number in 1..=target {
        let certified = node_handle(peer).with(|node| {
            node.state()
                .get_checkpoint_store()
                .get_checkpoint_by_sequence_number(sequence_number)
                .unwrap()
                .expect("the peer has every certified checkpoint up to the target")
        });
        let local = node_handle(victim).with(|node| {
            node.state()
                .get_checkpoint_store()
                .get_locally_computed_checkpoint(sequence_number)
                .unwrap()
        });
        if let Some(local) = local {
            assert_eq!(
                local.digest(),
                *certified.digest(),
                "the victim's checkpoint {sequence_number} must match the certified one"
            );
        }
    }

    // No saved decision of a transfer outlives the flush of its commit. The
    // last flushed commit is read from the tables, as the validator keeps
    // running.
    let resume_point = node_handle(victim).with(|node| {
        node.state()
            .epoch_store_for_testing()
            .tables()
            .unwrap()
            .get_last_consensus_index()
            .unwrap()
            .map(|index| index.sub_dag_index)
            .unwrap_or_default()
    });
    for effects in &transfers {
        let digest = *effects.transaction_digest();
        for index in 0..=resume_point {
            let saved = node_handle(victim).with(|node| {
                node.state()
                    .epoch_store_for_testing()
                    .has_post_consensus_decision_for_testing(index, digest)
                    .unwrap()
            });
            assert!(
                !saved,
                "commit {index} flushed, so its decision for {digest:?} must be gone"
            );
        }
    }
    clear_fail_point("crash-after-post-consensus-decisions");
}

/// A faulty validator can sequence a transaction admission would refuse. One
/// whose receiving argument names an object nobody created must not be kept:
/// the scheduler would wait for that object forever, its commit would never
/// complete, and every handler would park at the horizon wait. Validation
/// reports the object missing instead, nobody executes the transaction, and
/// checkpoints keep coming on every validator.
#[sim_test]
async fn test_receiving_an_object_nobody_created_does_not_halt_the_cluster() {
    use std::{net::SocketAddr, path::PathBuf};

    use iota_core::authority_client::validator_v2::ValidatorV2API;
    use iota_macros::{clear_fail_point, register_fail_point_if};
    use iota_sdk_types::{ObjectDigest, ObjectId, ObjectReference, Owner, Version};
    use iota_test_transaction_builder::publish_package;
    use iota_types::{messages_grpc::TxStatusUpdate, transaction::CallArg};

    let _guard = enable_deterministic_validation_for_testing();
    telemetry_subscribers::init_for_testing();
    let client_ip = SocketAddr::new([127, 0, 0, 1].into(), 0);
    let test_cluster = TestClusterBuilder::new()
        .with_epoch_duration_ms(600_000)
        .build()
        .await;
    let node_handle = |name| {
        test_cluster
            .swarm
            .node(&name)
            .unwrap()
            .get_node_handle()
            .unwrap()
    };

    let package = {
        let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        path.push("tests/move_test_code");
        publish_package(&test_cluster.wallet, path).await.object_id
    };
    let start = test_cluster
        .test_transaction_builder()
        .await
        .move_call(package, "tto", "start", vec![])
        .build();
    let start_effects = test_cluster.sign_and_execute_transaction(&start).await;
    // `start` creates a parent and a child owned by the parent's id.
    let created = start_effects.created();
    let parent = created
        .iter()
        .find(|parent| {
            created.iter().any(|child| {
                matches!(child.owner(), Owner::Address(owner)
                    if ObjectId::from(*owner) == parent.reference().object_id)
            })
        })
        .map(|parent| *parent.reference())
        .expect("start creates a child owned by the parent's id");
    let unseen = ObjectReference::new(ObjectId::random(), Version::from(1), ObjectDigest::random());
    let receive = test_cluster
        .test_transaction_builder()
        .await
        .move_call(
            package,
            "tto",
            "receiver",
            vec![
                CallArg::ImmutableOrOwned(parent),
                CallArg::Receiving(unseen),
            ],
        )
        .build();
    let receive = test_cluster.wallet.sign_transaction(&receive);
    let digest = *receive.digest();

    // One validator skips its input checks and submits it, as a faulty one can.
    let faulty = test_cluster.get_validator_pubkeys()[0];
    let faulty_sim_id = node_handle(faulty).with(|_| iota_simulator::current_simnode_id());
    register_fail_point_if("admit-without-input-checks", move || {
        iota_simulator::current_simnode_id() == faulty_sim_id
    });
    let authority_clients = test_cluster
        .authority_aggregator()
        .authority_clients
        .clone();
    let submitted = tokio::time::timeout(
        Duration::from_secs(60),
        authority_clients[&faulty]
            .authority_client()
            .submit_tx(vec![receive], Some(client_ip)),
    )
    .await;
    clear_fail_point("admit-without-input-checks");
    let submitted = submitted
        .expect("the submission must be answered within 60s")
        .unwrap();
    assert!(
        matches!(
            submitted.as_slice(),
            [(submitted_digest, TxStatusUpdate::Submitted)] if *submitted_digest == digest
        ),
        "the validator skipping its input checks must submit the transaction, got {submitted:?}"
    );

    // Every validator keeps executing checkpoints well past the horizon
    // distance after the submission.
    let highest_executed = |name| {
        node_handle(name).with(|node| {
            node.state()
                .get_checkpoint_store()
                .get_highest_executed_checkpoint_seq_number()
                .unwrap()
                .unwrap_or_default()
        })
    };
    for validator in test_cluster.get_validator_pubkeys() {
        let target = highest_executed(validator) + 20;
        tokio::time::timeout(Duration::from_secs(180), async {
            while highest_executed(validator) < target {
                tokio::time::sleep(Duration::from_millis(500)).await;
            }
        })
        .await
        .unwrap_or_else(|_| {
            panic!("validator {validator:?} must keep executing checkpoints, stuck below {target}")
        });
    }

    // Nobody executed the transaction.
    for validator in test_cluster.get_validator_pubkeys() {
        let executed = node_handle(validator).with(|node| {
            node.state()
                .get_transaction_cache_reader()
                .try_get_executed_effects(&digest)
                .unwrap()
        });
        assert!(
            executed.is_none(),
            "validator {validator:?} executed a receive of an object nobody created"
        );
    }
}

/// A validator stopped while the network keeps processing transfers replays
/// the commits it missed on restart, faster than it re-executes them. Its
/// validation must still decide every transaction as its peers did, so it
/// builds the certified checkpoints. Without deterministic validation, such a
/// validator drops transfers whose gas version it has not executed yet.
#[sim_test]
async fn test_stopped_validator_catches_up_without_fork() {
    telemetry_subscribers::init_for_testing();
    let _guard = enable_deterministic_validation_for_testing();
    let test_cluster = build_cluster(600_000).await;
    let stop = Arc::new(AtomicBool::new(false));
    let traffic = spawn_transfer_traffic_with_double_spends(
        test_cluster.clone(),
        test_cluster.get_addresses(),
        stop.clone(),
    );

    let lagging = test_cluster.get_validator_pubkeys()[1];
    sleep(Duration::from_secs(5)).await;
    test_cluster.stop_node(&lagging);
    sleep(Duration::from_secs(10)).await;
    test_cluster.start_node(&lagging).await;
    assert_validators_reach_the_same_checkpoints(&test_cluster).await;

    stop_traffic(traffic, &stop).await;
    assert_validators_reach_the_same_checkpoints(&test_cluster).await;
}

/// One validator executes every transaction with a delay of up to one second
/// while the traffic carries double spends, so its consensus handler runs
/// ahead of its execution and validation waits for execution to catch
/// up. The same validator is then restarted mid-epoch with the delay still in
/// place. Every validator must reach the same checkpoints.
#[sim_test]
async fn test_execution_stall_and_restart_on_one_validator_does_not_fork() {
    telemetry_subscribers::init_for_testing();
    let _guard = enable_deterministic_validation_for_testing();
    let test_cluster = build_cluster(600_000).await;
    let stalled = test_cluster.get_validator_pubkeys()[2];
    let stalled_sim_id = sim_node_id(&test_cluster, &stalled);
    let stall = Arc::new(AtomicBool::new(true));
    {
        let stall = stall.clone();
        register_fail_point_async("transaction_execution_delay", move || {
            let stall = iota_simulator::current_simnode_id() == stalled_sim_id
                && stall.load(Ordering::SeqCst);
            async move {
                if stall {
                    let delay = Uniform::new(0, 1_000).unwrap().sample(&mut rand::rng());
                    sleep(Duration::from_millis(delay)).await;
                }
            }
        });
    }

    let stop = Arc::new(AtomicBool::new(false));
    let traffic = spawn_transfer_traffic_with_double_spends(
        test_cluster.clone(),
        test_cluster.get_addresses(),
        stop.clone(),
    );
    sleep(Duration::from_secs(10)).await;
    assert_validators_reach_the_same_checkpoints(&test_cluster).await;

    test_cluster.stop_node(&stalled);
    sleep(Duration::from_secs(5)).await;
    test_cluster.start_node(&stalled).await;
    sleep(Duration::from_secs(5)).await;
    assert_validators_reach_the_same_checkpoints(&test_cluster).await;

    stall.store(false, Ordering::SeqCst);
    stop_traffic(traffic, &stop).await;
    assert_validators_reach_the_same_checkpoints(&test_cluster).await;
    clear_fail_point("transaction_execution_delay");
}

/// A validator killed right after it saved a commit's validation decisions,
/// several times while double spends are in flight, replays each commit from
/// the saved decisions and builds the certified checkpoints.
#[sim_test]
async fn test_crashes_after_post_consensus_decisions_under_double_spends() {
    run_crashes_at_fail_point_under_traffic("crash-after-post-consensus-decisions", 3).await;
}

/// A validator killed right after a checkpoint's bookkeeping batch is durable
/// and before the checkpoint's outputs are, several times while double spends
/// are in flight, re-executes the checkpoint and builds the certified
/// checkpoints.
#[sim_test]
async fn test_crashes_after_checkpoint_bookkeeping_write_under_double_spends() {
    run_crashes_at_fail_point_under_traffic("crash-after-checkpoint-bookkeeping-write", 3).await;
}

/// A validator killed after a checkpoint's bookkeeping and outputs are durable
/// but before the executed-checkpoint watermark moves re-executes a checkpoint
/// whose outputs it already has, so the checkpoint writes no new bookkeeping.
/// It must still build the certified checkpoints.
#[sim_test]
async fn test_crashes_before_executed_checkpoint_watermark_under_double_spends() {
    run_crashes_at_fail_point_under_traffic("highest-executed-checkpoint", 2).await;
}

/// A validator killed at the shared `crash` fail points: before a
/// transaction's outputs are written to the cache, while a checkpoint's output
/// batch is built (before its bookkeeping batch), after a checkpoint's outputs
/// before the watermark, and after a commit's validation before its
/// transactions are scheduled. Every validator must reach the same
/// checkpoints.
#[sim_test]
async fn test_crashes_at_shared_crash_points_under_double_spends() {
    run_crashes_at_fail_point_under_traffic("crash", 3).await;
}

/// A validator stopped while the network runs transactions that use
/// randomness misses those randomness rounds. After restart it catches up from
/// the certified checkpoints, which carry the randomness state updates of
/// those rounds, and one more such transaction runs. It must build the
/// certified checkpoints.
#[sim_test]
async fn test_validator_catches_up_across_randomness_rounds_without_fork() {
    telemetry_subscribers::init_for_testing();
    let _guard = enable_deterministic_validation_for_testing();
    let test_cluster = build_cluster(600_000).await;
    let package_ref = publish_basics_package(&test_cluster.wallet).await;
    let lagging = test_cluster.get_validator_pubkeys()[0];

    test_cluster.stop_node(&lagging);
    for _ in 0..3 {
        emit_new_random_u128(&test_cluster.wallet, package_ref.object_id).await;
    }
    sleep(Duration::from_secs(10)).await;
    test_cluster.start_node(&lagging).await;
    emit_new_random_u128(&test_cluster.wallet, package_ref.object_id).await;
    assert_validators_reach_the_same_checkpoints(&test_cluster).await;
}

/// Validators prune every consumed object version as soon as its checkpoint is
/// executed, which is the test-cluster default. Under double-spend traffic
/// across several epoch changes, and with one validator stopped across an
/// epoch change, every validator must reach the same checkpoints.
#[sim_test]
async fn test_aggressive_pruning_across_epoch_changes_does_not_fork() {
    telemetry_subscribers::init_for_testing();
    let _guard = enable_deterministic_validation_for_testing();
    let test_cluster = build_cluster(15_000).await;
    let names = test_cluster.get_validator_pubkeys();
    for name in &names {
        assert_eq!(
            test_cluster
                .swarm
                .node(name)
                .unwrap()
                .config()
                .authority_store_pruning_config
                .num_epochs_to_retain,
            0,
            "validators must prune consumed object versions right away"
        );
    }
    let stop = Arc::new(AtomicBool::new(false));
    let traffic = spawn_transfer_traffic_with_double_spends(
        test_cluster.clone(),
        test_cluster.get_addresses(),
        stop.clone(),
    );

    wait_for_validators_to_reach_epoch(&test_cluster, &names, 1).await;
    let lagging = names[3];
    test_cluster.stop_node(&lagging);
    wait_for_validators_to_reach_epoch(&test_cluster, &names[..3], 2).await;
    test_cluster.start_node(&lagging).await;
    wait_for_validators_to_reach_epoch(&test_cluster, &names, 4).await;

    stop_traffic(traffic, &stop).await;
    assert_validators_reach_the_same_checkpoints(&test_cluster).await;
}

/// With one validator stopped and another executing every transaction up to
/// 0.7 s late, the three running validators must keep certifying and executing
/// checkpoints.
///
/// The late validator's handler waits, before validating commit C, until commit
/// C − 2 is fully executed, so it falls behind the other two. Their handlers
/// then wait for randomness rounds that no validator starts while the maximum
/// number of rounds is in flight, until a checkpoint holding a later round is
/// executed. With three of four validators running, certifying that checkpoint
/// needs each running validator's own signature, so each must give its own
/// signature to its aggregator without waiting for its handler. Randomness
/// rounds are made every 50 ms, so the running validators reach this point
/// within about 30 s and well below checkpoint 100; a validator that only got
/// its own signature through its handler would stop there for good.
#[sim_test]
async fn test_stopped_and_late_validator_do_not_halt_the_cluster() {
    telemetry_subscribers::init_for_testing();
    let _guard = ProtocolConfig::apply_overrides_for_testing(|_, mut config| {
        config.enable_pcool_deterministic_validation_for_testing();
        config.set_random_beacon_min_round_interval_ms_for_testing(50);
        config
    });
    let test_cluster = build_cluster(600_000).await;
    let names = test_cluster.get_validator_pubkeys();
    let (late, stopped) = (names[1], names[3]);
    let late_sim_id = sim_node_id(&test_cluster, &late);
    register_fail_point_async("transaction_execution_delay", move || {
        let late = iota_simulator::current_simnode_id() == late_sim_id;
        async move {
            if late {
                let delay = Uniform::new(0, 700).unwrap().sample(&mut rand::rng());
                sleep(Duration::from_millis(delay)).await;
            }
        }
    });

    test_cluster.stop_node(&stopped);
    let traffic = spawn_transfer_traffic_with_double_spends(
        test_cluster.clone(),
        test_cluster.get_addresses(),
        Arc::new(AtomicBool::new(false)),
    );
    let target = 100;
    let running = &names[..3];
    timeout(Duration::from_secs(90), async {
        while running
            .iter()
            .any(|name| highest_executed_checkpoint(&test_cluster, name) < target)
        {
            sleep(Duration::from_millis(500)).await;
        }
    })
    .await
    .unwrap_or_else(|_| {
        panic!(
            "every running validator must execute checkpoint {target}: executed {:?}",
            running
                .iter()
                .map(|name| highest_executed_checkpoint(&test_cluster, name))
                .collect::<Vec<_>>(),
        )
    });
    // Awaiting the traffic would also wait out the timeouts of double spends
    // still in flight.
    traffic.abort();
    clear_fail_point("transaction_execution_delay");
}

/// Creates a shared counter from `sender` with `gas`, and returns its
/// reference, or `None` if the transaction did not go through.
async fn create_counter(
    test_cluster: &TestCluster,
    sender: Address,
    gas: ObjectReference,
    gas_price: u64,
    package_id: ObjectId,
) -> Option<ObjectReference> {
    let wallet = &test_cluster.wallet;
    let create = TestTransactionBuilder::new(sender, gas, gas_price)
        .call_counter_create(package_id)
        .build();
    let response = timeout(
        Duration::from_secs(30),
        wallet.execute_transaction_may_fail(wallet.sign_transaction(&create)),
    )
    .await
    .ok()?
    .ok()?;
    response.effects.as_ref().and_then(|effects| {
        effects
            .created()
            .iter()
            .find(|created| matches!(created.owner, Owner::Shared(_)))
            .map(|created| created.reference)
    })
}

/// In every epoch until `stop` is set, each of `senders` creates two shared
/// counters, then submits for each of them a transaction that deletes it and,
/// a quarter second later and without waiting for the deletion, one that
/// increments it. The deletions of all senders are spread 240 ms apart from
/// 600 ms before the epoch is due to close, which `observer` reports, to
/// 600 ms after it. Some deletion is then sequenced shortly before the epoch's
/// final round, and its increment after it. Returns how many deletions
/// succeeded.
fn spawn_shared_object_deletions_around_epoch_close(
    test_cluster: Arc<TestCluster>,
    observer: AuthorityName,
    epoch_duration_ms: u64,
    senders: Vec<Address>,
    package_id: ObjectId,
    stop: Arc<AtomicBool>,
) -> JoinHandle<usize> {
    const PAIRS_PER_SENDER: usize = 2;
    tokio::spawn(async move {
        let loops = senders.into_iter().enumerate().map(|(index, sender)| {
            let test_cluster = test_cluster.clone();
            let stop = stop.clone();
            async move {
                let gas_price = test_cluster.get_reference_gas_price().await;
                let wallet = &test_cluster.wallet;
                let now_ms = || {
                    std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap()
                        .as_millis() as u64
                };
                let mut deleted = 0;
                let mut last_epoch = None;
                while !stop.load(Ordering::SeqCst) {
                    let Some((epoch, close_ms)) = test_cluster
                        .swarm
                        .node(&observer)
                        .unwrap()
                        .get_node_handle()
                        .map(|handle| {
                            handle.with(|node| {
                                let epoch_store = node.state().epoch_store_for_testing();
                                (
                                    epoch_store.epoch(),
                                    epoch_store.epoch_start_state().epoch_start_timestamp_ms()
                                        + epoch_duration_ms,
                                )
                            })
                        })
                    else {
                        sleep(Duration::from_millis(200)).await;
                        continue;
                    };
                    if last_epoch == Some(epoch) {
                        sleep(Duration::from_millis(200)).await;
                        continue;
                    }
                    last_epoch = Some(epoch);

                    let mut counters = Vec::with_capacity(PAIRS_PER_SENDER);
                    for _ in 0..PAIRS_PER_SENDER {
                        let Ok(Some(gas)) =
                            wallet.get_one_gas_object_owned_by_address(sender).await
                        else {
                            break;
                        };
                        let Some(counter) =
                            create_counter(&test_cluster, sender, gas, gas_price, package_id).await
                        else {
                            break;
                        };
                        counters.push(counter);
                    }
                    let coins = wallet
                        .get_gas_objects_owned_by_address(sender, None)
                        .await
                        .unwrap_or_default();
                    if counters.len() < PAIRS_PER_SENDER || coins.len() < 2 * PAIRS_PER_SENDER {
                        continue;
                    }
                    let pairs = counters.iter().zip(coins.chunks(2)).enumerate().map(
                        |(pair, (counter, gas))| {
                            let slot = (index * PAIRS_PER_SENDER + pair) as u64;
                            let delete_at_ms = (close_ms + slot * 240).saturating_sub(600);
                            let delete = wallet.sign_transaction(
                                &TestTransactionBuilder::new(sender, gas[0], gas_price)
                                    .call_counter_delete(
                                        package_id,
                                        counter.object_id,
                                        counter.version,
                                    )
                                    .build(),
                            );
                            let increment = wallet.sign_transaction(
                                &TestTransactionBuilder::new(sender, gas[1], gas_price)
                                    .call_counter_increment(
                                        package_id,
                                        counter.object_id,
                                        counter.version,
                                    )
                                    .build(),
                            );
                            async move {
                                sleep(Duration::from_millis(delete_at_ms.saturating_sub(now_ms())))
                                    .await;
                                let (deletion, _) = tokio::join!(
                                    timeout(
                                        Duration::from_secs(30),
                                        wallet.execute_transaction_may_fail(delete),
                                    ),
                                    async {
                                        sleep(Duration::from_millis(250)).await;
                                        timeout(
                                            Duration::from_secs(30),
                                            wallet.execute_transaction_may_fail(increment),
                                        )
                                        .await
                                    },
                                );
                                matches!(
                                    deletion,
                                    Ok(Ok(ref response)) if response.status_ok() == Some(true)
                                )
                            }
                        },
                    );
                    deleted += join_all(pairs)
                        .await
                        .into_iter()
                        .filter(|deleted| *deleted)
                        .count();
                }
                deleted
            }
        });
        join_all(loops).await.into_iter().sum()
    })
}

/// The number of user transactions the validator's consensus handler has seen
/// sequenced in the current epoch, summed over the authorities that proposed
/// them.
fn sequenced_user_transactions(state: &AuthorityState) -> i64 {
    state
        .metrics
        .consensus_committed_user_transactions
        .collect()
        .iter()
        .flat_map(|family| family.get_metric())
        .map(|metric| metric.get_gauge().value() as i64)
        .sum()
}

/// One validator keeps admitting user transactions and submitting them to
/// consensus after its epoch starts closing, so commits after the epoch's
/// final round carry user transactions. The traffic carries double spends and
/// transactions on a shared object whose deletion was sequenced shortly
/// before. Once that validator stops accepting user transactions for the
/// epoch, it executes every transaction up to 200 ms late, so its consensus
/// handler processes the commits after the final round while the transactions
/// of the last commits before it are still being executed. A longer lag, or
/// one that starts earlier, makes it switch to executing synced checkpoints
/// and skip the commits after the final round.
///
/// In each of two epochs whose commits after the final round carry a user
/// transaction, that validator is killed once it has executed the epoch's last
/// checkpoint, before it reconfigures. On restart it replays those commits
/// against a store that has executed every transaction of the epoch. It must
/// not panic, every validator must reach the next epochs, and all of them
/// must reach the same checkpoints. The test does not check how validation
/// treats user transactions after the final round; the unit tests of
/// post-consensus validation cover that.
#[sim_test]
async fn test_restart_replays_user_transactions_sequenced_after_the_final_round() {
    const KILLS: usize = 2;
    const EPOCH_DURATION_MS: u64 = 15_000;
    telemetry_subscribers::init_for_testing();
    let _guard = enable_deterministic_validation_for_testing();
    let test_cluster = build_cluster(EPOCH_DURATION_MS).await;
    let package_id = publish_basics_package(&test_cluster.wallet).await.object_id;
    let names = test_cluster.get_validator_pubkeys();
    let victim = names[0];
    let victim_sim_id = sim_node_id(&test_cluster, &victim);
    // A weak reference, so it does not pin a killed node's store. It is
    // refreshed below once the victim restarts with a new state.
    let victim_state = Arc::new(Mutex::new(Weak::<AuthorityState>::new()));
    let refresh_victim_state = || {
        if let Some(handle) = test_cluster.swarm.node(&victim).unwrap().get_node_handle() {
            *victim_state.lock().unwrap() = handle.with(|node| Arc::downgrade(&node.state()));
        }
    };
    refresh_victim_state();

    register_fail_point_if("admit-user-transactions-after-epoch-close", move || {
        iota_simulator::current_simnode_id() == victim_sim_id
    });
    let lagging = Arc::new(AtomicBool::new(true));
    {
        let lagging = lagging.clone();
        let victim_state = victim_state.clone();
        register_fail_point_async("transaction_execution_delay", move || {
            let lag = iota_simulator::current_simnode_id() == victim_sim_id
                && lagging.load(Ordering::SeqCst)
                && victim_state.lock().unwrap().upgrade().is_some_and(|state| {
                    !state
                        .epoch_store_for_testing()
                        .get_reconfig_state_read_lock_guard()
                        .should_accept_tx()
                });
            async move {
                if lag {
                    let delay = Uniform::new(0, 200).unwrap().sample(&mut rand::rng());
                    sleep(Duration::from_millis(delay)).await;
                }
            }
        });
    }

    // For each epoch, the victim's count of sequenced user transactions at
    // its first commit after the final round, and whether a later commit of
    // that epoch raised it. That first commit holds the final round, so a
    // raise means a user transaction was sequenced after the final round.
    let after_final_round: Arc<Mutex<HashMap<EpochId, (i64, bool)>>> = Default::default();
    {
        let victim_state = victim_state.clone();
        let after_final_round = after_final_round.clone();
        register_fail_point_if(
            "correlated-crash-after-consensus-commit-boundary",
            move || {
                if iota_simulator::current_simnode_id() != victim_sim_id {
                    return false;
                }
                let Some(state) = victim_state.lock().unwrap().upgrade() else {
                    return false;
                };
                let epoch_store = state.epoch_store_for_testing();
                if epoch_store
                    .get_reconfig_state_read_lock_guard()
                    .should_accept_tx()
                {
                    return false;
                }
                let sequenced = sequenced_user_transactions(&state);
                let mut after_final_round = after_final_round.lock().unwrap();
                let (at_final_round, raised) = after_final_round
                    .entry(epoch_store.epoch())
                    .or_insert((sequenced, false));
                *raised |= sequenced > *at_final_round;
                false
            },
        );
    }

    // The victim is killed just before it reconfigures, after it executed the
    // epoch's last checkpoint. After the restart it waits before reconfiguring
    // so its consensus handler replays the commits after the final round.
    let killed_in_epochs = Arc::new(Mutex::new(Vec::new()));
    let replaying = Arc::new(AtomicBool::new(false));
    {
        let victim_state = victim_state.clone();
        let after_final_round = after_final_round.clone();
        let killed_in_epochs = killed_in_epochs.clone();
        register_fail_point_async("reconfig_delay", move || {
            let mut wait_for_replay = false;
            if iota_simulator::current_simnode_id() == victim_sim_id {
                let state = victim_state.lock().unwrap().upgrade();
                if replaying.swap(false, Ordering::SeqCst) {
                    wait_for_replay = true;
                } else if let Some(state) = state {
                    let epoch = state.epoch_store_for_testing().epoch();
                    drop(state);
                    let raised = after_final_round
                        .lock()
                        .unwrap()
                        .get(&epoch)
                        .is_some_and(|(_, raised)| *raised);
                    let mut killed_in_epochs = killed_in_epochs.lock().unwrap();
                    if raised && killed_in_epochs.len() < KILLS {
                        killed_in_epochs.push(epoch);
                        drop(killed_in_epochs);
                        replaying.store(true, Ordering::SeqCst);
                        iota_simulator::task::kill_current_node(None);
                    }
                }
            }
            async move {
                if wait_for_replay {
                    sleep(Duration::from_secs(5)).await;
                }
            }
        });
    }

    let addresses = test_cluster.get_addresses();
    let stop = Arc::new(AtomicBool::new(false));
    let deletions = spawn_shared_object_deletions_around_epoch_close(
        test_cluster.clone(),
        names[1],
        EPOCH_DURATION_MS,
        addresses[..3].to_vec(),
        package_id,
        stop.clone(),
    );
    let traffic = spawn_transfer_traffic_with_double_spends(
        test_cluster.clone(),
        addresses[3..].to_vec(),
        stop.clone(),
    );

    // Epochs in which the victim switched to executing synced checkpoints, so
    // its consensus handler skipped the rest of the epoch's commits.
    let mut switched_in_epochs = BTreeSet::new();
    timeout(Duration::from_secs(600), async {
        while killed_in_epochs.lock().unwrap().len() < KILLS {
            refresh_victim_state();
            if let Some(state) = victim_state.lock().unwrap().upgrade() {
                let epoch_store = state.epoch_store_for_testing();
                if epoch_store.is_executing_synced_checkpoints() {
                    switched_in_epochs.insert(epoch_store.epoch());
                }
            }
            sleep(Duration::from_millis(200)).await;
        }
    })
    .await
    .unwrap_or_else(|_| {
        panic!(
            "the victim must be killed in {KILLS} epochs whose commits after the final round \
             carry a user transaction, was killed in epochs {:?}; it executed synced \
             checkpoints in epochs {switched_in_epochs:?}",
            killed_in_epochs.lock().unwrap()
        )
    });
    lagging.store(false, Ordering::SeqCst);
    let last_killed_epoch = *killed_in_epochs.lock().unwrap().last().unwrap();
    wait_for_validators_to_reach_epoch(&test_cluster, &names, last_killed_epoch + 2).await;

    stop_traffic(traffic, &stop).await;
    deletions.await.unwrap();
    assert_validators_reach_the_same_checkpoints(&test_cluster).await;
    clear_fail_point("admit-user-transactions-after-epoch-close");
    clear_fail_point("transaction_execution_delay");
    clear_fail_point("correlated-crash-after-consensus-commit-boundary");
    clear_fail_point("reconfig_delay");
}

/// The regulated coin of the `move_test_code` package. Its coin and deny cap
/// belong to `owner`, who published the package.
struct RegulatedCoin {
    package: ObjectId,
    coin: ObjectId,
    coin_type: TypeTag,
    deny_cap: ObjectId,
    owner: Address,
    deny_list_initial_version: Version,
}

impl RegulatedCoin {
    /// Publishes the `move_test_code` package from the cluster's default sender,
    /// which becomes the `owner` of the regulated coin and its deny cap.
    async fn publish(test_cluster: &TestCluster) -> Self {
        let deny_list_initial_version = test_cluster
            .get_object_from_fullnode_store(&ObjectId::DENY_LIST)
            .await
            .unwrap()
            .owner
            .into_opt_shared()
            .expect("the deny list must be shared");
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/move_test_code");
        let tx = test_cluster
            .test_transaction_builder()
            .await
            .publish(path)
            .build();
        let effects = test_cluster.sign_and_execute_transaction(&tx).await;

        let mut package = None;
        let mut coin = None;
        let mut deny_cap = None;
        for created in effects.created() {
            let object_id = created.reference().object_id;
            let object = test_cluster
                .get_object_from_fullnode_store(&object_id)
                .await
                .unwrap();
            if object.is_package() {
                package = Some(object_id);
            } else if object.is_coin() {
                coin = Some((
                    object_id,
                    object.opt_coin_type().cloned().unwrap(),
                    *created.owner().as_address(),
                ));
            } else if object.data.opt_object_type().unwrap().is_deny_cap_v1() {
                deny_cap = Some(object_id);
            }
        }
        let (coin, coin_type, owner) = coin.expect("the package must create its regulated coin");

        Self {
            package: package.expect("publishing must create the package"),
            coin,
            coin_type,
            deny_cap: deny_cap.expect("the package must create its deny cap"),
            owner,
            deny_list_initial_version,
        }
    }

    /// Creates an object owned by `owner` that coins can be sent to.
    async fn create_parent(&self, test_cluster: &TestCluster) -> ObjectId {
        let tx = test_cluster
            .test_transaction_builder_with_sender(self.owner)
            .await
            .move_call(self.package, "coin_receiver", "create", vec![])
            .build();
        let effects = test_cluster.sign_and_execute_transaction(&tx).await;

        effects.created()[0].reference().object_id
    }

    /// A transaction by `owner` that splits a new coin off its regulated coin
    /// and sends it to `recipient`.
    async fn send_tx(&self, test_cluster: &TestCluster, recipient: Address) -> Transaction {
        let coin = test_cluster.get_latest_object_ref(&self.coin).await;
        test_cluster
            .test_transaction_builder_with_sender(self.owner)
            .await
            .move_call(
                ObjectId::FRAMEWORK,
                "pay",
                "split_and_transfer",
                vec![
                    CallArg::ImmutableOrOwned(coin),
                    CallArg::pure(&1_000u64),
                    CallArg::pure(&recipient),
                ],
            )
            .with_type_args(vec![self.coin_type.clone()])
            .build()
    }

    /// A transaction by `owner` that sends its whole regulated coin to
    /// `recipient`.
    async fn transfer_tx(&self, test_cluster: &TestCluster, recipient: Address) -> Transaction {
        let coin = test_cluster.get_latest_object_ref(&self.coin).await;
        test_cluster
            .test_transaction_builder_with_sender(self.owner)
            .await
            .transfer(coin, recipient)
            .build()
    }

    /// A transaction by `owner` that receives `coin`, sent to `parent`, and
    /// sends it on to `recipient`.
    async fn receive_tx(
        &self,
        test_cluster: &TestCluster,
        parent: ObjectId,
        coin: ObjectId,
        recipient: Address,
    ) -> Transaction {
        let parent = test_cluster.get_latest_object_ref(&parent).await;
        let coin = test_cluster.get_latest_object_ref(&coin).await;

        test_cluster
            .test_transaction_builder_with_sender(self.owner)
            .await
            .move_call(
                self.package,
                "coin_receiver",
                "receive_and_send",
                vec![
                    CallArg::ImmutableOrOwned(parent),
                    CallArg::Receiving(coin),
                    CallArg::pure(&recipient),
                ],
            )
            .with_type_args(vec![self.coin_type.clone()])
            .build()
    }

    /// Adds `owner` to the deny list of the coin type.
    async fn deny(&self, test_cluster: &TestCluster) -> TransactionEffects {
        self.change_deny_list(test_cluster, true).await
    }

    /// Removes `owner` from the deny list of the coin type.
    async fn undeny(&self, test_cluster: &TestCluster) -> TransactionEffects {
        self.change_deny_list(test_cluster, false).await
    }

    async fn change_deny_list(
        &self,
        test_cluster: &TestCluster,
        denied: bool,
    ) -> TransactionEffects {
        let function = if denied {
            "deny_list_v1_add"
        } else {
            "deny_list_v1_remove"
        };
        let deny_cap = test_cluster.get_latest_object_ref(&self.deny_cap).await;
        let tx = test_cluster
            .test_transaction_builder_with_sender(self.owner)
            .await
            .move_call(
                ObjectId::FRAMEWORK,
                "coin",
                function,
                vec![
                    CallArg::Shared(SharedObjectReference::new(
                        ObjectId::DENY_LIST,
                        self.deny_list_initial_version,
                        true,
                    )),
                    CallArg::ImmutableOrOwned(deny_cap),
                    CallArg::pure(&self.owner),
                ],
            )
            .with_type_args(vec![self.coin_type.clone()])
            .build();

        test_cluster.sign_and_execute_transaction(&tx).await
    }

    /// Executes `tx`, which admission accepts and post-consensus validation
    /// keeps, and checks that execution fails it with the denial of `owner` on
    /// every validator and the fullnode, charging gas.
    async fn assert_denied_at_execution(&self, test_cluster: &TestCluster, tx: &Transaction) {
        let (effects, _) = test_cluster
            .execute_transaction_return_raw_effects(test_cluster.sign_transaction(tx))
            .await
            .expect("the transaction must be admitted and kept");
        let denied = ExecutionStatus::Failure {
            error: ExecutionError::AddressDeniedForCoin {
                address: self.owner,
                coin_type: self.coin_type.to_canonical_string(false),
            },
            command: None,
        };
        assert_eq!(
            effects.status(),
            &denied,
            "execution must fail the kept transaction for the denial settled before this epoch"
        );
        assert!(
            effects.gas_cost_summary().gas_used() > 0,
            "the failed transaction must be charged gas"
        );
        assert!(
            effects.created().is_empty(),
            "the failed transaction must create nothing"
        );

        let on_validators = effects_on_validators(test_cluster, &effects).await;
        for on_validator in &on_validators {
            assert_eq!(
                on_validator.status(),
                &denied,
                "every validator must fail the transaction with the denial"
            );
            assert_eq!(
                on_validator, &on_validators[0],
                "validators must hold the same effects"
            );
        }
        assert_eq!(
            effects_on_fullnode(test_cluster, &effects).await,
            on_validators[0],
            "the fullnode must hold the validators' effects"
        );
    }
}

/// The effects of the transaction that `effects` describes, as the fullnode
/// holds them once it has executed the transaction from a checkpoint.
async fn effects_on_fullnode(
    test_cluster: &TestCluster,
    effects: &TransactionEffects,
) -> TransactionEffects {
    let digest = *effects.transaction_digest();
    tokio::time::timeout(
        Duration::from_secs(60),
        test_cluster
            .fullnode_handle
            .iota_node
            .with_async(|node| async move {
                node.state()
                    .get_transaction_cache_reader()
                    .try_notify_read_executed_effects("", &[digest])
                    .await
            }),
    )
    .await
    .expect("the fullnode must execute the transaction within 60s")
    .expect("the executed effects must be readable")
    .remove(0)
}

/// The effects of the transaction that `effects` describes, as every validator
/// holds them once it has executed the transaction.
async fn effects_on_validators(
    test_cluster: &TestCluster,
    effects: &TransactionEffects,
) -> Vec<TransactionEffects> {
    let digest = *effects.transaction_digest();
    let mut effects_on_validators = Vec::new();
    for handle in test_cluster.all_validator_handles() {
        let on_validator = tokio::time::timeout(
            Duration::from_secs(60),
            handle.with_async(|node| async move {
                node.state()
                    .get_transaction_cache_reader()
                    .try_notify_read_executed_effects("", &[digest])
                    .await
            }),
        )
        .await
        .expect("every validator must execute the transaction within 60s")
        .expect("the executed effects must be readable")
        .remove(0);
        effects_on_validators.push(on_validator);
    }

    effects_on_validators
}

/// The owner of `object_id` in the latest version the fullnode has.
async fn owner_of(test_cluster: &TestCluster, object_id: &ObjectId) -> Owner {
    test_cluster
        .get_object_from_fullnode_store(object_id)
        .await
        .unwrap()
        .owner
}

/// A sender denied for a coin type, with the denial settled at an epoch
/// boundary and removed in the epoch after it. Post-consensus validation
/// reads the deny list for the inputs as of the commit's horizon, so once the
/// removal is the horizon distance of commits old, every transaction below is
/// kept. Execution still checks the owners of the coins it writes and the
/// coins Move code receives against the deny list settled before this epoch:
/// - a split that leaves a coin with the sender fails with the denial;
/// - a transfer of the sender's whole coin succeeds;
/// - a transaction receiving such a coin fails with the denial and moves nothing.
///
/// Each one has the same effects on every validator and the fullnode, and a
/// failure is charged gas.
#[sim_test]
async fn test_removed_denial_lets_a_whole_coin_transfer_through_after_the_horizon() {
    telemetry_subscribers::init_for_testing();
    let _guard = enable_deterministic_validation_for_testing();
    let test_cluster = TestClusterBuilder::new()
        .with_epoch_duration_ms(600_000)
        .build()
        .await;
    let horizon_distance = test_cluster.all_validator_handles()[0].with(|node| {
        let epoch_store = node.state().epoch_store_for_testing();
        let protocol_config = epoch_store.protocol_config();
        assert!(
            protocol_config.pcool_deterministic_validation(),
            "the protocol-config override must reach the validators"
        );
        protocol_config.pcool_deterministic_validation_horizon_distance_or_default()
    });

    let regulated = RegulatedCoin::publish(&test_cluster).await;

    // The regulated coin is sent to an object the sender owns, to be received
    // from it later.
    let parent = regulated.create_parent(&test_cluster).await;
    let send_tx = regulated.send_tx(&test_cluster, parent.into()).await;
    let effects = test_cluster.sign_and_execute_transaction(&send_tx).await;
    let received = effects.created()[0].reference().object_id;

    // The denial settles at the epoch boundary. Its removal in the next epoch
    // counts for the inputs once the horizon reaches it, and for received
    // coins only at the next epoch boundary.
    regulated.deny(&test_cluster).await;
    test_cluster.force_new_epoch().await;
    let removal = regulated.undeny(&test_cluster).await;

    // Admission reads the latest deny list on the validator that takes the
    // submission, so every validator must have executed the removal first.
    effects_on_validators(&test_cluster, &removal).await;
    // Each transaction is submitted once the one before it has executed, so
    // it is sequenced in a later commit: after these, the spend's commit is
    // more than the horizon distance above the removal's.
    for _ in 0..horizon_distance {
        transfer(&test_cluster).await;
    }

    let split_tx = regulated.send_tx(&test_cluster, dbg_addr(2)).await;
    regulated
        .assert_denied_at_execution(&test_cluster, &split_tx)
        .await;

    let transfer_tx = regulated.transfer_tx(&test_cluster, dbg_addr(2)).await;
    let (effects, _) = test_cluster
        .execute_transaction_return_raw_effects(test_cluster.sign_transaction(&transfer_tx))
        .await
        .expect("the transfer must be admitted and kept");
    assert_eq!(effects.status(), &ExecutionStatus::Success);
    for on_validator in effects_on_validators(&test_cluster, &effects).await {
        assert_eq!(
            on_validator, effects,
            "validators must hold the same effects"
        );
    }
    assert_eq!(
        effects_on_fullnode(&test_cluster, &effects).await,
        effects,
        "the fullnode must hold the validators' effects"
    );
    assert_eq!(
        owner_of(&test_cluster, &regulated.coin).await,
        Owner::Address(dbg_addr(2)),
        "the transfer must move the coin"
    );

    let receive_tx = regulated
        .receive_tx(&test_cluster, parent, received, dbg_addr(2))
        .await;
    regulated
        .assert_denied_at_execution(&test_cluster, &receive_tx)
        .await;
    assert_eq!(
        owner_of(&test_cluster, &received).await,
        Owner::Address(parent.into()),
        "the failed transaction must leave the received coin with the parent"
    );
}

/// A transaction that names a shared object deleted at or below the horizon
/// of its commit is dropped there, and every validator records the drop as
/// its final status. Submitted through the fullnode, it fails at once with
/// the error for an input not available at the commit, instead of being
/// retried until the fullnode's wait for finality times out.
#[sim_test]
async fn test_drop_at_the_commit_fails_fast_through_the_fullnode() {
    telemetry_subscribers::init_for_testing();
    let _guard = enable_deterministic_validation_for_testing();
    let test_cluster = build_cluster(600_000).await;
    let wallet = &test_cluster.wallet;
    let package_id = publish_basics_package(wallet).await.object_id;
    let (sender, gas) = wallet.get_one_gas_object().await.unwrap().unwrap();
    let gas_price = test_cluster.get_reference_gas_price().await;
    let counter = create_counter(&test_cluster, sender, gas, gas_price, package_id)
        .await
        .expect("the counter is created");

    let gas = wallet.get_object_ref(gas.object_id).await.unwrap();
    let delete = TestTransactionBuilder::new(sender, gas, gas_price)
        .call_counter_delete(package_id, counter.object_id, counter.version)
        .build();
    test_cluster.sign_and_execute_transaction(&delete).await;
    // Lets enough commits pass for the deletion's commit to be at or below
    // the horizon of the commit that sequences the increment.
    sleep(Duration::from_secs(5)).await;

    let gas = wallet.get_object_ref(gas.object_id).await.unwrap();
    let increment = wallet.sign_transaction(
        &TestTransactionBuilder::new(sender, gas, gas_price)
            .call_counter_increment(package_id, counter.object_id, counter.version)
            .build(),
    );
    let started = Instant::now();
    let error = timeout(
        Duration::from_secs(20),
        wallet.execute_transaction_may_fail(increment),
    )
    .await
    .expect("the fullnode must answer before its wait for finality times out")
    .expect_err("an increment of a deleted counter must not execute");
    let message = format!("{error:#}");
    assert!(
        message.contains("can't be used: at the consensus commit"),
        "unexpected error: {message}"
    );
    assert!(
        started.elapsed() < Duration::from_secs(10),
        "the drop took {:?} to reach the client",
        started.elapsed()
    );
}
