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
        Arc, Mutex,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    time::Duration,
};

use futures::future::join_all;
use iota_macros::{register_fail_point, register_fail_point_async, sim_test};
use iota_protocol_config::ProtocolConfig;
use iota_sdk_types::{Address, TransactionEffects};
use iota_test_transaction_builder::{
    TestTransactionBuilder, emit_new_random_u128, publish_basics_package,
};
use iota_types::{
    base_types::{AuthorityName, ConciseableName, EpochId, dbg_addr},
    effects::{TransactionEffectsAPI, TransactionEffectsExt},
    messages_checkpoint::CheckpointSequenceNumber,
    storage::ObjectKey,
};
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
/// the one the fullnode has executed, then asserts that every checkpoint a
/// validator built itself matches the certified one. A validator whose
/// validation decided differently builds a checkpoint that differs from the
/// certified one, which the in-node fork check turns into a panic, and stops
/// executing.
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
}

/// A validator killed right after it saved a commit's validation decisions,
/// several times while double spends are in flight, replays each commit from
/// the saved decisions and builds the certified checkpoints.
#[sim_test]
async fn test_crashes_after_post_consensus_verdicts_under_double_spends() {
    run_crashes_at_fail_point_under_traffic("crash-after-post-consensus-verdicts", 3).await;
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

/// A validator stopped while the network completes randomness rounds catches
/// up after restart from the randomness in certified checkpoints, with its
/// validation waiting on executions that include randomness transactions. It
/// must build the certified checkpoints.
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
/// one second late, the three running validators must keep committing and
/// change epochs.
///
/// Currently fails: the running validators stop for good. The handler of the
/// late validator waits, before validating commit C, until commit C − 2 is
/// fully executed, and that commit holds a randomness round. No validator
/// signs a new randomness round while the maximum number of rounds is already
/// in flight, and a slot frees only when a checkpoint holding a later round is
/// executed. That checkpoint needs the late validator's signature, which
/// reaches its own aggregator only through its waiting handler, so neither
/// side moves.
#[sim_test]
async fn test_stopped_and_late_validator_do_not_halt_the_cluster() {
    telemetry_subscribers::init_for_testing();
    let _guard = enable_deterministic_validation_for_testing();
    let test_cluster = build_cluster(15_000).await;
    let names = test_cluster.get_validator_pubkeys();
    let (late, stopped) = (names[1], names[3]);
    let late_sim_id = sim_node_id(&test_cluster, &late);
    register_fail_point_async("transaction_execution_delay", move || {
        let late = iota_simulator::current_simnode_id() == late_sim_id;
        async move {
            if late {
                let delay = Uniform::new(0, 1_000).unwrap().sample(&mut rand::rng());
                sleep(Duration::from_millis(delay)).await;
            }
        }
    });

    test_cluster.stop_node(&stopped);
    let stop = Arc::new(AtomicBool::new(false));
    let traffic = spawn_transfer_traffic_with_double_spends(
        test_cluster.clone(),
        test_cluster.get_addresses(),
        stop.clone(),
    );
    wait_for_validators_to_reach_epoch(&test_cluster, &names[..3], 4).await;
    stop_traffic(traffic, &stop).await;
}
