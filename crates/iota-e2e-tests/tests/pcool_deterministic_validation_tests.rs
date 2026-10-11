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
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

use iota_macros::{register_fail_point, sim_test};
use iota_protocol_config::ProtocolConfig;
use iota_sdk_types::{
    Address, ExecutionError, ExecutionStatus, ObjectId, Owner, SharedObjectReference, Transaction,
    TransactionEffects, TypeTag, Version,
};
use iota_types::{
    base_types::dbg_addr,
    effects::{TransactionEffectsAPI, TransactionEffectsExt},
    storage::ObjectKey,
    transaction::CallArg,
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
    // its executions leaves a sync-ahead record.
    for key in transfers.iter().flat_map(written_keys) {
        let record = test_cluster.fullnode_handle.iota_node.with(|node| {
            node.state()
                .epoch_store_for_testing()
                .sync_ahead_record(&key.0)
                .unwrap()
        });
        assert_eq!(
            record, None,
            "the fullnode keeps a sync-ahead record for {key:?}"
        );
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
    /// every validator, charging gas.
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
    }
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
/// boundary and removed in the epoch after it, is admitted and kept by
/// post-consensus validation. On every validator, execution then fails a
/// transaction that spends a coin of that type and one that receives such a
/// coin: both end with the denial in their effects, are charged gas, and move
/// nothing.
#[sim_test]
async fn test_denied_sender_spending_and_receiving_a_coin_fails_with_effects() {
    telemetry_subscribers::init_for_testing();
    let _guard = enable_deterministic_validation_for_testing();
    let test_cluster = TestClusterBuilder::new()
        .with_epoch_duration_ms(600_000)
        .build()
        .await;
    test_cluster.all_validator_handles()[0].with(|node| {
        assert!(
            node.state()
                .epoch_store_for_testing()
                .protocol_config()
                .pcool_deterministic_validation(),
            "the protocol-config override must reach the validators"
        );
    });

    let regulated = RegulatedCoin::publish(&test_cluster).await;

    // The regulated coin is sent to an object the sender owns, to be received
    // from it later.
    let parent = regulated.create_parent(&test_cluster).await;
    let send_tx = regulated.send_tx(&test_cluster, parent.into()).await;
    let effects = test_cluster.sign_and_execute_transaction(&send_tx).await;
    let received = effects.created()[0].reference().object_id;

    // The denial settles at the epoch boundary, and its removal takes effect
    // only at the next one. After the removal, the latest deny list allows the
    // sender while the list settled before this epoch still denies it:
    // admission reads the first, and execution the second.
    regulated.deny(&test_cluster).await;
    test_cluster.force_new_epoch().await;
    let removal = regulated.undeny(&test_cluster).await;

    // Admission reads the latest deny list on the validator that takes the
    // submission, so every validator must have executed the removal first.
    effects_on_validators(&test_cluster, &removal).await;

    let spend_tx = regulated.send_tx(&test_cluster, dbg_addr(2)).await;
    regulated
        .assert_denied_at_execution(&test_cluster, &spend_tx)
        .await;

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
