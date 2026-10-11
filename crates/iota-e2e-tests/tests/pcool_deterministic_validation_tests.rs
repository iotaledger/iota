// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Cluster tests for the P-COOL deterministic-validation bookkeeping.
//!
//! Gated to the simulator for the reason given in
//! `pcool_transaction_flow_tests.rs`: the protocol-config override that turns
//! the feature on is thread-local, and only the simulator runs every node on
//! the test's thread.
//!
//! Run with: `cargo simtest -p iota-e2e-tests pcool_deterministic_validation`.

#![cfg(msim)]

use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    time::Duration,
};

use iota_macros::{register_fail_point, register_fail_point_async, sim_test};
use iota_protocol_config::ProtocolConfig;
use iota_sdk_types::TransactionEffects;
use iota_test_transaction_builder::{emit_new_random_u128, publish_basics_package};
use iota_types::{
    base_types::dbg_addr,
    effects::{TransactionEffectsAPI, TransactionEffectsExt},
    storage::ObjectKey,
};
use test_cluster::{TestCluster, TestClusterBuilder};

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
    // The victim completes no more randomness rounds: a round its peers
    // complete from now on stays in flight on the victim.
    let round_in_flight = Arc::new(AtomicBool::new(false));
    {
        let (holds, round_in_flight) = (holds.clone(), round_in_flight.clone());
        register_fail_point_async("randomness-delay", move || {
            let hold = holds();
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
    // The victim's builder stops before its next write until a round is in
    // flight, so the kill lands on that write.
    {
        let (holds, hold_builder) = (holds.clone(), hold_builder.clone());
        register_fail_point_async("checkpoint-builder-write-summaries", move || {
            let (holds, hold_builder) = (holds.clone(), hold_builder.clone());
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
async fn test_crash_after_post_consensus_verdicts_recovers() {
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
        register_fail_point("crash-after-post-consensus-verdicts", move || {
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

    // No saved decision of a transfer outlives the flush of its commit.
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
                    .has_post_consensus_verdict_for_testing(index, digest)
                    .unwrap()
            });
            assert!(
                !saved,
                "commit {index} flushed, so its decision for {digest:?} must be gone"
            );
        }
    }
}
