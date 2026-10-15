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
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

use iota_macros::{register_fail_point, sim_test};
use iota_protocol_config::ProtocolConfig;
use iota_sdk_types::TransactionEffects;
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
