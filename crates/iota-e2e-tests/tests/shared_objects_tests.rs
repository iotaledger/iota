// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    collections::{HashMap, hash_map::Entry},
    net::SocketAddr,
    ops::Deref,
    time::{Duration, SystemTime},
};

use futures::{future::join_all, join};
use iota_config::node::AuthorityOverloadConfig;
use iota_core::{
    authority_client::validator_v2::ValidatorV2API, consensus_adapter::position_submit_certificate,
};
use iota_json_rpc_types::IotaTransactionBlockEffectsAPI;
use iota_macros::{register_fail_point_async, sim_test};
use iota_node::IotaNodeHandle;
use iota_sdk_types::{
    Event, ExecutionError, ExecutionStatus, ObjectId, SharedObjectReference, TransactionDigest,
    TransactionEffects, Version,
};
use iota_swarm_config::genesis_config::{AccountConfig, DEFAULT_GAS_AMOUNT};
use iota_test_transaction_builder::{
    TestTransactionBuilder, publish_basics_package, publish_basics_package_and_make_counter,
};
use iota_types::{
    base_types::AuthorityName,
    effects::TransactionEffectsAPI,
    error::{IotaError, UserInputError},
    messages_grpc::{
        GetTxStatusRequest, LayoutGenerationOption, ObjectInfoRequest, TxStatusQuery,
        TxStatusUpdate,
    },
    storage::ObjectKey,
    transaction::{CallArg, TransactionEnvelope},
};
use rand::distr::Distribution;
use test_cluster::{TestCluster, TestClusterBuilder, override_pcool_flow};
use tokio::time::{sleep, timeout};

/// Send a simple shared object transaction to IOTA and ensures the client gets
/// back a response.
#[sim_test]
async fn shared_object_transaction() {
    let test_cluster = TestClusterBuilder::new().build().await;
    let (sender, mut objects) = test_cluster.wallet.get_one_account().await.unwrap();
    let rgp = test_cluster.get_reference_gas_price().await;
    let transaction = TestTransactionBuilder::new(sender, objects.pop().unwrap(), rgp)
        .call_staking(
            objects.pop().unwrap(),
            test_cluster
                .swarm
                .active_validators()
                .next()
                .unwrap()
                .config()
                .iota_address(),
        )
        .build();

    test_cluster
        .sign_and_execute_transaction(&transaction)
        .await;
}

/// Delete a shared object as the object owner
#[sim_test]
async fn shared_object_deletion() {
    let test_cluster = TestClusterBuilder::new().build().await;

    let (package, counter) = publish_basics_package_and_make_counter(&test_cluster.wallet).await;
    let package_id = package.object_id;
    let counter_id = counter.object_id;
    let counter_initial_shared_version = counter.version;

    // Make a transaction to delete the counter.
    let transaction = test_cluster
        .test_transaction_builder()
        .await
        .call_counter_delete(package_id, counter_id, counter_initial_shared_version)
        .build();
    let effects = test_cluster
        .sign_and_execute_transaction(&transaction)
        .await;

    assert_eq!(effects.deleted().len(), 1);
    assert_eq!(effects.input_shared_objects().len(), 1);

    // assert the shared object was deleted
    let deleted_obj_id = effects.deleted()[0].object_id;
    assert_eq!(deleted_obj_id, counter_id);
}

#[sim_test]
async fn shared_object_deletion_multiple_times() {
    let _pcool_guard = override_pcool_flow(false);
    let num_deletions = 300;
    let mut test_cluster = TestClusterBuilder::new()
        .with_accounts(vec![AccountConfig {
            address: None,
            gas_amounts: vec![DEFAULT_GAS_AMOUNT; num_deletions],
        }])
        .build()
        .await;

    let (package, counter) = publish_basics_package_and_make_counter(&test_cluster.wallet).await;
    let package_id = package.object_id;
    let counter_id = counter.object_id;
    let counter_initial_shared_version = counter.version;

    let accounts_and_gas = test_cluster
        .wallet
        .get_all_accounts_and_gas_objects()
        .await
        .unwrap();
    let sender = accounts_and_gas[0].0;
    let gas_coins = accounts_and_gas[0].1.clone();

    // Make a bunch transactions that all want to delete the counter object.
    let mut txs = vec![];
    for coin_ref in gas_coins.into_iter() {
        let transaction = test_cluster
            .test_transaction_builder_with_gas_object(sender, coin_ref)
            .await
            .call_counter_delete(package_id, counter_id, counter_initial_shared_version)
            .build();
        let signed = test_cluster.sign_transaction(&transaction);
        let client_ip = SocketAddr::new([127, 0, 0, 1].into(), 0);
        test_cluster
            .create_certificate(signed.clone(), Some(client_ip))
            .await
            .unwrap();
        txs.push(signed);
    }

    // Submit all the deletion transactions to the validators.
    let validators = test_cluster.get_validator_pubkeys();
    let submissions = txs.iter().map(|tx| async {
        test_cluster
            .submit_transaction_to_validators(tx.clone(), &validators)
            .await
            .unwrap();
        *tx.digest()
    });
    let digests = join_all(submissions).await;

    // Start a new fullnode and let it sync from genesis and wait for us to see all
    // the deletion transactions.
    let fullnode = test_cluster.spawn_new_fullnode().await.iota_node;
    fullnode
        .state()
        .get_transaction_cache_reader()
        .notify_read_executed_effects_for_testing("", &digests)
        .await;
}

/// The P-COOL counterpart of `shared_object_deletion_multiple_times`: every
/// deletion is sent with `submit_tx` to every validator at once. One deletion
/// succeeds, and each other one is kept and fails or is dropped at the commit,
/// depending on whether the winner's commit is above its commit's horizon.
#[sim_test]
async fn shared_object_deletion_multiple_times_under_pcool() {
    let num_deletions = 300;
    let mut test_cluster = TestClusterBuilder::new()
        .with_epoch_duration_ms(600_000)
        .with_accounts(vec![AccountConfig {
            address: None,
            gas_amounts: vec![DEFAULT_GAS_AMOUNT; num_deletions],
        }])
        .build()
        .await;
    let (package, counter) = publish_basics_package_and_make_counter(&test_cluster.wallet).await;
    let deletions = sign_counter_deletions(
        &test_cluster,
        package.object_id,
        counter.object_id,
        counter.version,
    )
    .await;
    let digests: Vec<_> = deletions.iter().map(|tx| *tx.digest()).collect();

    submit_tx_to_validators(
        &test_cluster,
        &deletions,
        &test_cluster.get_validator_pubkeys(),
    )
    .await;
    let outcomes = outcomes_on_every_validator(&test_cluster, &digests).await;

    assert_deletion_outcomes(&mut test_cluster, counter.object_id, &outcomes).await;
}

#[sim_test]
async fn shared_object_deletion_multiple_times_cert_racing() {
    let _pcool_guard = override_pcool_flow(false);
    let num_deletions = 10;
    let mut test_cluster = TestClusterBuilder::new()
        .with_accounts(vec![AccountConfig {
            address: None,
            gas_amounts: vec![DEFAULT_GAS_AMOUNT; num_deletions],
        }])
        .build()
        .await;

    let (package, counter) = publish_basics_package_and_make_counter(&test_cluster.wallet).await;
    let package_id = package.object_id;
    let counter_id = counter.object_id;
    let counter_initial_shared_version = counter.version;

    let accounts_and_gas = test_cluster
        .wallet
        .get_all_accounts_and_gas_objects()
        .await
        .unwrap();
    let sender = accounts_and_gas[0].0;
    let gas_coins = accounts_and_gas[0].1.clone();

    // Make a bunch transactions that all want to delete the counter object.
    let validators = test_cluster.get_validator_pubkeys();
    let mut digests = vec![];
    for coin_ref in gas_coins.into_iter() {
        let transaction = test_cluster
            .test_transaction_builder_with_gas_object(sender, coin_ref)
            .await
            .call_counter_delete(package_id, counter_id, counter_initial_shared_version)
            .build();
        let signed = test_cluster.sign_transaction(&transaction);
        let client_ip = SocketAddr::new([127, 0, 0, 1].into(), 0);
        test_cluster
            .create_certificate(signed.clone(), Some(client_ip))
            .await
            .unwrap();
        test_cluster
            .submit_transaction_to_validators(signed.clone(), &validators)
            .await
            .unwrap();
        digests.push(*signed.digest());
    }

    // Start a new fullnode and let it sync from genesis and wait for us to see all
    // the deletion transactions.
    let fullnode = test_cluster.spawn_new_fullnode().await.iota_node;
    fullnode
        .state()
        .get_transaction_cache_reader()
        .notify_read_executed_effects_for_testing("", &digests)
        .await;
}

/// The P-COOL counterpart of `shared_object_deletion_multiple_times_cert_racing`:
/// each deletion is sent with `submit_tx` to every validator once the previous
/// one is decided.
#[sim_test]
async fn shared_object_deletion_multiple_times_cert_racing_under_pcool() {
    let num_deletions = 10;
    let mut test_cluster = TestClusterBuilder::new()
        .with_epoch_duration_ms(600_000)
        .with_accounts(vec![AccountConfig {
            address: None,
            gas_amounts: vec![DEFAULT_GAS_AMOUNT; num_deletions],
        }])
        .build()
        .await;
    let (package, counter) = publish_basics_package_and_make_counter(&test_cluster.wallet).await;
    let deletions = sign_counter_deletions(
        &test_cluster,
        package.object_id,
        counter.object_id,
        counter.version,
    )
    .await;

    let validators = test_cluster.get_validator_pubkeys();
    let mut outcomes = HashMap::new();
    for deletion in &deletions {
        submit_tx_to_validators(&test_cluster, std::slice::from_ref(deletion), &validators).await;
        outcomes.extend(outcomes_on_every_validator(&test_cluster, &[*deletion.digest()]).await);
    }

    assert_deletion_outcomes(&mut test_cluster, counter.object_id, &outcomes).await;
}

/// Test for execution of shared object certs that are sequenced after a shared
/// object is deleted. The test strategy is:
/// 0. Inject a random delay just before execution of a transaction.
/// 1. Create a shared object
/// 2. Create a delete cert and two increment certs, but do not execute any of them yet.
/// 3. Execute the delete cert.
/// 4. Execute the two increment certs.
///
/// The two execution certs should be immediately executable (because they have
/// a missing input). Therefore validators may execute them in either order. The
/// injected delay ensures that we will explore all possible orders, and
/// `submit_transaction_to_validators` verifies that we get the same effects
/// regardless of the order. (checkpoint fork detection will also test this).
#[sim_test]
async fn shared_object_deletion_multi_certs() {
    let _pcool_guard = override_pcool_flow(false);
    // cause random delay just before tx is executed
    register_fail_point_async("transaction_execution_delay", move || async move {
        let delay = {
            let dist = rand::distr::Uniform::new(0, 1000).unwrap();
            let mut rng = rand::rng();
            dist.sample(&mut rng)
        };
        sleep(Duration::from_millis(delay)).await;
    });

    let mut test_cluster = TestClusterBuilder::new().build().await;

    let (package, counter) = publish_basics_package_and_make_counter(&test_cluster.wallet).await;
    let package_id = package.object_id;
    let counter_id = counter.object_id;
    let counter_initial_shared_version = counter.version;

    let accounts_and_gas = test_cluster
        .wallet
        .get_all_accounts_and_gas_objects()
        .await
        .unwrap();

    let sender = accounts_and_gas[0].0;
    let gas1 = accounts_and_gas[0].1[0];
    let gas2 = accounts_and_gas[0].1[1];
    let gas3 = accounts_and_gas[0].1[2];

    // Make a transaction to delete the counter.
    let delete_tx = test_cluster
        .test_transaction_builder_with_gas_object(sender, gas1)
        .await
        .call_counter_delete(package_id, counter_id, counter_initial_shared_version)
        .build();
    let delete_tx = test_cluster.sign_transaction(&delete_tx);

    let inc_tx_a = test_cluster
        .test_transaction_builder_with_gas_object(sender, gas2)
        .await
        .call_counter_increment(package_id, counter_id, counter_initial_shared_version)
        .build();
    let inc_tx_a = test_cluster.sign_transaction(&inc_tx_a);
    let inc_tx_a_digest = *inc_tx_a.digest();

    let inc_tx_b = test_cluster
        .test_transaction_builder_with_gas_object(sender, gas3)
        .await
        .call_counter_increment(package_id, counter_id, counter_initial_shared_version)
        .build();
    let inc_tx_b = test_cluster.sign_transaction(&inc_tx_b);
    let inc_tx_b_digest = *inc_tx_b.digest();
    let client_ip = SocketAddr::new([127, 0, 0, 1].into(), 0);

    let _ = test_cluster
        .create_certificate(delete_tx.clone(), Some(client_ip))
        .await
        .unwrap();
    let _ = test_cluster
        .create_certificate(inc_tx_a.clone(), Some(client_ip))
        .await
        .unwrap();
    let _ = test_cluster
        .create_certificate(inc_tx_b.clone(), Some(client_ip))
        .await
        .unwrap();

    let validators = test_cluster.get_validator_pubkeys();

    // delete obj on all validators, await effects
    test_cluster
        .submit_transaction_to_validators(delete_tx, &validators)
        .await
        .unwrap();

    // now submit remaining txns simultaneously
    join!(
        async {
            test_cluster
                .submit_transaction_to_validators(inc_tx_a, &validators)
                .await
                .unwrap()
        },
        async {
            test_cluster
                .submit_transaction_to_validators(inc_tx_b, &validators)
                .await
                .unwrap()
        }
    );

    // Start a new fullnode that is not on the write path
    let fullnode = test_cluster.spawn_new_fullnode().await.iota_node;
    fullnode
        .state()
        .get_transaction_cache_reader()
        .notify_read_executed_effects_for_testing("", &[inc_tx_a_digest, inc_tx_b_digest])
        .await;
}

/// The P-COOL counterpart of `shared_object_deletion_multi_certs`: a deletion,
/// then two increments sent at once, all with `submit_tx` to every validator,
/// while every node delays each execution by a random amount. Each increment
/// is kept and fails or is dropped at the commit, the same way on every
/// validator.
#[sim_test]
async fn shared_object_deletion_multi_certs_under_pcool() {
    register_fail_point_async("transaction_execution_delay", move || async move {
        let delay = {
            let dist = rand::distr::Uniform::new(0, 1000).unwrap();
            let mut rng = rand::rng();
            dist.sample(&mut rng)
        };
        sleep(Duration::from_millis(delay)).await;
    });

    let mut test_cluster = TestClusterBuilder::new()
        .with_epoch_duration_ms(600_000)
        .build()
        .await;
    let (package, counter) = publish_basics_package_and_make_counter(&test_cluster.wallet).await;
    let package_id = package.object_id;
    let counter_id = counter.object_id;
    let counter_initial_shared_version = counter.version;
    let accounts_and_gas = test_cluster
        .wallet
        .get_all_accounts_and_gas_objects()
        .await
        .unwrap();
    let sender = accounts_and_gas[0].0;
    let gas = &accounts_and_gas[0].1;

    let delete_tx = test_cluster.sign_transaction(
        &test_cluster
            .test_transaction_builder_with_gas_object(sender, gas[0])
            .await
            .call_counter_delete(package_id, counter_id, counter_initial_shared_version)
            .build(),
    );
    let mut increments = Vec::new();
    for gas in &gas[1..3] {
        increments.push(
            test_cluster.sign_transaction(
                &test_cluster
                    .test_transaction_builder_with_gas_object(sender, *gas)
                    .await
                    .call_counter_increment(package_id, counter_id, counter_initial_shared_version)
                    .build(),
            ),
        );
    }

    let validators = test_cluster.get_validator_pubkeys();
    submit_tx_to_validators(&test_cluster, std::slice::from_ref(&delete_tx), &validators).await;
    let mut outcomes = outcomes_on_every_validator(&test_cluster, &[*delete_tx.digest()]).await;
    submit_tx_to_validators(&test_cluster, &increments, &validators).await;
    let increment_digests: Vec<_> = increments.iter().map(|tx| *tx.digest()).collect();
    outcomes.extend(outcomes_on_every_validator(&test_cluster, &increment_digests).await);

    assert_deletion_outcomes(&mut test_cluster, counter_id, &outcomes).await;
}

/// End-to-end shared transaction test for an IOTA validator. It does not test
/// the client or wallet, but tests the end-to-end flow from IOTA to consensus.
#[sim_test]
async fn call_shared_object_contract() {
    let test_cluster = TestClusterBuilder::new().build().await;
    let (package, counter) = publish_basics_package_and_make_counter(&test_cluster.wallet).await;
    let package_id = package.object_id;
    let counter_id = counter.object_id;
    let counter_initial_shared_version = counter.version;
    let counter_object_arg = CallArg::Shared(SharedObjectReference::new(
        counter_id,
        counter_initial_shared_version,
        true,
    ));
    let counter_object_arg_imm = CallArg::Shared(SharedObjectReference::new(
        counter_id,
        counter_initial_shared_version,
        false,
    ));
    let counter_creation_transaction = test_cluster
        .get_object_from_fullnode_store(&counter_id)
        .await
        .unwrap()
        .previous_transaction;

    // Send two read only transactions
    let (sender, objects) = test_cluster.wallet.get_one_account().await.unwrap();
    let rgp = test_cluster.get_reference_gas_price().await;
    let mut prev_assert_value_txs = Vec::new();
    for gas in objects {
        // Ensure the value of the counter is `0`.
        let transaction = TestTransactionBuilder::new(sender, gas, rgp)
            .move_call(
                package_id,
                "counter",
                "assert_value",
                vec![
                    counter_object_arg_imm.clone(),
                    CallArg::Pure(0u64.to_le_bytes().to_vec()),
                ],
            )
            .build();
        let effects = test_cluster
            .sign_and_execute_transaction(&transaction)
            .await;
        // Check that all reads must depend on the creation of the counter, but not to
        // any previous reads.
        assert!(
            effects
                .dependencies()
                .contains(&counter_creation_transaction)
        );
        assert!(
            prev_assert_value_txs
                .iter()
                .all(|tx| { !effects.dependencies().contains(tx) })
        );
        prev_assert_value_txs.push(*effects.transaction_digest());
    }

    // Make a transaction to increment the counter.
    let transaction = test_cluster
        .test_transaction_builder()
        .await
        .call_counter_increment(package_id, counter_id, counter_initial_shared_version)
        .build();
    let effects = test_cluster
        .sign_and_execute_transaction(&transaction)
        .await;
    let increment_transaction = *effects.transaction_digest();
    assert!(
        effects
            .dependencies()
            .contains(&counter_creation_transaction)
    );
    // Previously executed assert_value transaction(s) are not a dependency because
    // they took immutable reference to shared object
    assert!(
        prev_assert_value_txs
            .iter()
            .all(|tx| { !effects.dependencies().contains(tx) })
    );

    // assert_value can take both mutable and immutable references
    // it is allowed to pass mutable shared object arg to move call taking immutable
    // reference
    let mut assert_value_mut_transaction = None;
    for imm in [true, false] {
        // Ensure the value of the counter is `1`.
        let transaction = test_cluster
            .test_transaction_builder()
            .await
            .move_call(
                package_id,
                "counter",
                "assert_value",
                vec![
                    if imm {
                        counter_object_arg_imm.clone()
                    } else {
                        counter_object_arg.clone()
                    },
                    CallArg::Pure(1u64.to_le_bytes().to_vec()),
                ],
            )
            .build();
        let effects = test_cluster
            .sign_and_execute_transaction(&transaction)
            .await;
        assert!(effects.dependencies().contains(&increment_transaction));
        if let Some(prev) = assert_value_mut_transaction {
            assert!(effects.dependencies().contains(&prev));
        }
        assert_value_mut_transaction = Some(*effects.transaction_digest());
    }

    let assert_value_mut_transaction = assert_value_mut_transaction.unwrap();

    // And last check - attempt to send increment transaction with immutable
    // reference
    let transaction = test_cluster
        .test_transaction_builder()
        .await
        .move_call(
            package_id,
            "counter",
            "increment",
            vec![counter_object_arg_imm],
        )
        .build();
    let effects = test_cluster
        .wallet
        .execute_transaction_may_fail(test_cluster.wallet.sign_transaction(&transaction))
        .await
        .unwrap()
        .effects
        .unwrap();
    // Transaction fails
    assert!(effects.status().is_err(),);
    assert!(
        effects
            .status()
            .to_string()
            .contains("Immutable objects cannot be passed by mutable reference")
    );
    assert!(
        effects
            .dependencies()
            .contains(&assert_value_mut_transaction)
    );
}

#[ignore("Issue - https://github.com/iotaledger/iota/issues/4010")]
#[sim_test]
async fn access_clock_object_test() {
    let test_cluster = TestClusterBuilder::new().build().await;
    let package_id = publish_basics_package(&test_cluster.wallet).await.object_id;

    let transaction = test_cluster.wallet.sign_transaction(
        &test_cluster
            .test_transaction_builder()
            .await
            .move_call(
                package_id,
                "clock",
                "access",
                vec![CallArg::CLOCK_IMMUTABLE],
            )
            .build(),
    );
    let digest = *transaction.digest();
    let start = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap();
    let (effects, events) = test_cluster
        .execute_transaction_return_raw_effects(transaction)
        .await
        .unwrap();
    assert!(effects.status().is_success());

    let finish = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap();
    assert!(matches!(effects.status(), ExecutionStatus::Success));

    assert_eq!(1, events.len());
    let event = events.first().unwrap();
    let Event { contents, .. } = event;

    use serde::{Deserialize, Serialize};
    #[derive(Serialize, Deserialize)]
    struct TimeEvent {
        timestamp_ms: u64,
    }
    let event = bcs::from_bytes::<TimeEvent>(contents).unwrap();

    // Some sanity checks on the timestamp that we got
    assert!(event.timestamp_ms >= start.as_millis() as u64);
    assert!(event.timestamp_ms <= finish.as_millis() as u64);

    let mut attempt = 0;
    loop {
        let checkpoint = test_cluster.fullnode_handle.iota_node.with(|node| {
            node.state()
                .get_transaction_checkpoint_for_tests(
                    &digest,
                    &node.state().epoch_store_for_testing(),
                )
                .unwrap()
        });
        let Some(checkpoint) = checkpoint else {
            attempt += 1;
            if attempt > 30 {
                panic!("Could not get transaction checkpoint");
            }
            tokio::time::sleep(Duration::from_secs(1)).await;
            continue;
        };

        // Timestamp that we have read in a smart contract
        // should match timestamp of the checkpoint where transaction is included
        assert_eq!(checkpoint.timestamp_ms, event.timestamp_ms);
        break;
    }
}

#[sim_test]
async fn shared_object_sync() {
    let _pcool_guard = override_pcool_flow(false);
    let test_cluster = TestClusterBuilder::new()
        // Set the threshold high enough so it won't be triggered.
        .with_authority_overload_config(AuthorityOverloadConfig {
            max_txn_age_in_queue: Duration::from_secs(60),
            ..Default::default()
        })
        .build()
        .await;
    let package_id = publish_basics_package(&test_cluster.wallet).await.object_id;

    // Since we use submit_transaction_to_validators in this test, which does not go
    // through fullnode, we need to manage gas objects ourselves.
    let (sender, mut objects) = test_cluster.wallet.get_one_account().await.unwrap();
    let rgp = test_cluster.get_reference_gas_price().await;
    // Send a transaction to create a counter, to all but one authority.
    let create_counter_transaction = test_cluster.wallet.sign_transaction(
        &TestTransactionBuilder::new(sender, objects.pop().unwrap(), rgp)
            .call_counter_create(package_id)
            .build(),
    );
    let committee = test_cluster.committee().deref().clone();
    let validators = test_cluster.get_validator_pubkeys();
    let (slow_validators, fast_validators): (Vec<_>, Vec<_>) =
        validators.iter().partition(|name| {
            position_submit_certificate(&committee, name, create_counter_transaction.digest()) > 0
        });

    let (effects, _) = test_cluster
        .submit_transaction_to_validators(create_counter_transaction.clone(), &slow_validators)
        .await
        .unwrap();
    assert!(effects.status().is_success());
    let counter_ref = *effects.created()[0].reference();

    // Check that the counter object exists in at least one of the validators the
    // transaction was sent to.
    for validator in test_cluster.swarm.validator_node_handles() {
        if slow_validators.contains(&validator.state().name) {
            assert!(
                validator
                    .state()
                    .handle_object_info_request(ObjectInfoRequest::latest_object_info_request(
                        counter_ref.object_id,
                        LayoutGenerationOption::None,
                    ))
                    .await
                    .is_ok()
            );
        }
    }

    // Check that the validator that wasn't sent the transaction is unaware of the
    // counter object
    for validator in test_cluster.swarm.validator_node_handles() {
        if fast_validators.contains(&validator.state().name) {
            assert!(
                validator
                    .state()
                    .handle_object_info_request(ObjectInfoRequest::latest_object_info_request(
                        counter_ref.object_id,
                        LayoutGenerationOption::None,
                    ))
                    .await
                    .is_err()
            );
        }
    }

    // Make a transaction to increment the counter.
    let increment_counter_transaction = test_cluster.wallet.sign_transaction(
        &TestTransactionBuilder::new(sender, objects.pop().unwrap(), rgp)
            .call_counter_increment(package_id, counter_ref.object_id, counter_ref.version)
            .build(),
    );

    // Let's submit the transaction to the original set of validators, except the
    // first.
    let (effects, _) = test_cluster
        .submit_transaction_to_validators(increment_counter_transaction.clone(), &validators[1..])
        .await
        .unwrap();
    assert!(effects.status().is_success());

    // Submit transactions to the out-of-date authority.
    // It will succeed because we share owned object certificates through consensus
    let (effects, _) = test_cluster
        .submit_transaction_to_validators(increment_counter_transaction, &validators[0..1])
        .await
        .unwrap();
    assert!(effects.status().is_success());
}

/// The P-COOL counterpart of `shared_object_sync`. One validator lags: its
/// execution is paused before the counter's creation. It rejects an increment
/// sent to it with `submit_tx`, as the counter is missing from its store,
/// while an up-to-date validator submits it. Once the lagging validator
/// catches up, it executes the increment with the same effects.
///
/// Simulator only, because the pause is a fail point.
#[cfg(msim)]
#[sim_test]
async fn shared_object_sync_under_pcool() {
    use std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    };

    use iota_macros::clear_fail_point;

    let test_cluster = TestClusterBuilder::new()
        .with_epoch_duration_ms(600_000)
        .build()
        .await;
    let package_id = publish_basics_package(&test_cluster.wallet).await.object_id;
    let (sender, mut objects) = test_cluster.wallet.get_one_account().await.unwrap();
    let rgp = test_cluster.get_reference_gas_price().await;
    let validators = test_cluster.get_validator_pubkeys();
    let (lagging, up_to_date) = (validators[0], validators[1]);

    let lagging_sim_id = test_cluster
        .swarm
        .node(&lagging)
        .unwrap()
        .get_node_handle()
        .unwrap()
        .with(|_| iota_simulator::current_simnode_id());
    let paused = Arc::new(AtomicBool::new(true));
    {
        let paused = paused.clone();
        register_fail_point_async("transaction_execution_delay", move || {
            let paused = paused.clone();
            async move {
                if iota_simulator::current_simnode_id() == lagging_sim_id {
                    while paused.load(Ordering::SeqCst) {
                        sleep(Duration::from_millis(50)).await;
                    }
                }
            }
        });
    }

    let create_counter_transaction = test_cluster.wallet.sign_transaction(
        &TestTransactionBuilder::new(sender, objects.pop().unwrap(), rgp)
            .call_counter_create(package_id)
            .build(),
    );
    let counter_ref = *test_cluster
        .execute_transaction(create_counter_transaction)
        .await
        .created()[0]
        .reference();
    for validator in test_cluster.swarm.validator_node_handles() {
        let name = validator.state().name;
        let found = validator
            .state()
            .handle_object_info_request(ObjectInfoRequest::latest_object_info_request(
                counter_ref.object_id,
                LayoutGenerationOption::None,
            ))
            .await
            .is_ok();
        assert_eq!(
            found,
            name != lagging,
            "only the lagging validator may miss the counter"
        );
    }

    let increment_counter_transaction = test_cluster.wallet.sign_transaction(
        &TestTransactionBuilder::new(sender, objects.pop().unwrap(), rgp)
            .call_counter_increment(package_id, counter_ref.object_id, counter_ref.version)
            .build(),
    );
    let increment_digest = *increment_counter_transaction.digest();
    let clients = test_cluster
        .authority_aggregator()
        .authority_clients
        .clone();
    let statuses = clients[&lagging]
        .authority_client()
        .submit_tx(vec![increment_counter_transaction.clone()], None)
        .await
        .unwrap();
    assert!(
        matches!(
            statuses.as_slice(),
            [(
                digest,
                TxStatusUpdate::Rejected {
                    error: IotaError::UserInput {
                        error: UserInputError::ObjectNotFound {
                            object_id,
                            version: None,
                        },
                    },
                },
            )] if *digest == increment_digest && *object_id == counter_ref.object_id
        ),
        "the lagging validator must reject the increment as the counter is missing, got \
         {statuses:?}"
    );

    submit_tx_to_validators(
        &test_cluster,
        std::slice::from_ref(&increment_counter_transaction),
        &[up_to_date],
    )
    .await;
    timeout(
        Duration::from_secs(60),
        test_cluster
            .fullnode_handle
            .iota_node
            .state()
            .get_transaction_cache_reader()
            .notify_read_executed_effects_for_testing("", &[increment_digest]),
    )
    .await
    .expect("the up-to-date validators must execute the increment");

    paused.store(false, Ordering::SeqCst);
    let outcomes = outcomes_on_every_validator(&test_cluster, &[increment_digest]).await;
    clear_fail_point("transaction_execution_delay");
    let effects = outcomes[&increment_digest]
        .as_ref()
        .expect("every validator must execute the increment");
    assert!(effects.status().is_success(), "{:?}", effects.status());

    // Caught up, the lagging validator answers a resubmission with the
    // effects it executed.
    let statuses = clients[&lagging]
        .authority_client()
        .submit_tx(vec![increment_counter_transaction], None)
        .await
        .unwrap();
    assert!(
        matches!(
            statuses.as_slice(),
            [(digest, TxStatusUpdate::Executed { effects_digest, .. })]
                if *digest == increment_digest && *effects_digest == effects.digest()
        ),
        "the caught-up validator must report the increment executed, got {statuses:?}"
    );

    assert_every_node_has_the_same_checkpoints(
        &test_cluster,
        &test_cluster.fullnode_handle.iota_node,
    )
    .await;
}

/// Send a simple shared object transaction to IOTA and ensures the client gets
/// back a response.
#[sim_test]
async fn replay_shared_object_transaction() {
    let test_cluster = TestClusterBuilder::new().build().await;
    let package_id = publish_basics_package(&test_cluster.wallet).await.object_id;

    // Send a transaction to create a counter (only to one authority) -- twice.
    let create_counter_transaction = test_cluster.wallet.sign_transaction(
        &test_cluster
            .test_transaction_builder()
            .await
            .call_counter_create(package_id)
            .build(),
    );

    let mut version = None;
    for _ in 0..2 {
        let effects = test_cluster
            .execute_transaction(create_counter_transaction.clone())
            .await;

        // Ensure the sequence number of the shared object did not change.
        let curr = effects.created()[0].reference().version;
        if let Some(prev) = version {
            assert_eq!(prev, curr, "Version of shared object did not change.");
        }

        version = Some(curr);
    }
}

/// Signs one deletion of the counter `counter_id` for each gas coin of the
/// first account.
async fn sign_counter_deletions(
    test_cluster: &TestCluster,
    package_id: ObjectId,
    counter_id: ObjectId,
    counter_initial_shared_version: Version,
) -> Vec<TransactionEnvelope> {
    let accounts_and_gas = test_cluster
        .wallet
        .get_all_accounts_and_gas_objects()
        .await
        .unwrap();
    let (sender, gas_coins) = &accounts_and_gas[0];
    let mut deletions = Vec::with_capacity(gas_coins.len());
    for coin_ref in gas_coins {
        let transaction = test_cluster
            .test_transaction_builder_with_gas_object(*sender, *coin_ref)
            .await
            .call_counter_delete(package_id, counter_id, counter_initial_shared_version)
            .build();
        deletions.push(test_cluster.sign_transaction(&transaction));
    }
    deletions
}

/// The most transactions or queries one `submit_tx` or `get_tx_status`
/// request may carry.
const MAX_ITEMS_PER_VALIDATOR_V2_REQUEST: usize = 32;

/// Sends `transactions` with `submit_tx` to each of `validators` at once, and
/// checks that every validator submitted each of them to consensus or had
/// already executed it.
async fn submit_tx_to_validators(
    test_cluster: &TestCluster,
    transactions: &[TransactionEnvelope],
    validators: &[AuthorityName],
) {
    let clients = test_cluster
        .authority_aggregator()
        .authority_clients
        .clone();
    let submissions = validators.iter().flat_map(|name| {
        let client = clients[name].clone();
        transactions
            .chunks(MAX_ITEMS_PER_VALIDATOR_V2_REQUEST)
            .map(move |chunk| {
                let client = client.clone();
                async move {
                    let statuses = client
                        .authority_client()
                        .submit_tx(chunk.to_vec(), None)
                        .await;
                    (name, statuses)
                }
            })
    });
    for (name, statuses) in join_all(submissions).await {
        for (digest, status) in statuses.unwrap() {
            assert!(
                matches!(
                    status,
                    TxStatusUpdate::Submitted | TxStatusUpdate::Executed { .. }
                ),
                "validator {name:?} must submit {digest:?}, got {status:?}"
            );
        }
    }
}

/// What happened to a transaction: its effects once executed, or the error of
/// its drop at a commit.
type Outcome = Result<TransactionEffects, IotaError>;

/// Waits until every validator has executed or dropped each of `digests`,
/// checks that they all agree, and returns the outcome of each.
async fn outcomes_on_every_validator(
    test_cluster: &TestCluster,
    digests: &[TransactionDigest],
) -> HashMap<TransactionDigest, Outcome> {
    let clients = test_cluster
        .authority_aggregator()
        .authority_clients
        .clone();
    let mut agreed: HashMap<TransactionDigest, Outcome> = HashMap::new();
    for (name, client) in clients.iter() {
        for chunk in digests.chunks(MAX_ITEMS_PER_VALIDATOR_V2_REQUEST) {
            let mut pending = chunk.to_vec();
            let decided = timeout(Duration::from_secs(120), async {
                while !pending.is_empty() {
                    let request = GetTxStatusRequest {
                        queries: pending
                            .iter()
                            .map(|digest| TxStatusQuery {
                                transaction_digest: *digest,
                                include_details: true,
                            })
                            .collect(),
                    };
                    let statuses = client
                        .authority_client()
                        .get_tx_status(request, None)
                        .await
                        .unwrap();
                    for (digest, status) in statuses {
                        let outcome = match status {
                            TxStatusUpdate::Executed {
                                details: Some(details),
                                ..
                            } => Ok(details.effects),
                            TxStatusUpdate::Rejected { error } => Err(error),
                            // The validator's wait timed out; the query is sent again.
                            TxStatusUpdate::Expired { .. } => continue,
                            other => {
                                panic!("validator {name:?} reported {other:?} for {digest:?}")
                            }
                        };
                        pending.retain(|pending| *pending != digest);
                        match agreed.entry(digest) {
                            Entry::Vacant(entry) => {
                                entry.insert(outcome);
                            }
                            Entry::Occupied(entry) => assert_eq!(
                                entry.get().as_ref().map(|effects| effects.digest()),
                                outcome.as_ref().map(|effects| effects.digest()),
                                "validator {name:?} disagrees with another one on {digest:?}"
                            ),
                        }
                    }
                }
            })
            .await;
            assert!(
                decided.is_ok(),
                "validator {name:?} must decide {pending:?} within 120s"
            );
        }
    }
    agreed
}

/// Checks `outcomes`, those of transactions that each delete or use the shared
/// object `object_id`: exactly one deleted it, and each other one was either
/// kept and failed with `InputObjectDeleted`, in a commit whose horizon is
/// below the deletion's commit, or dropped at a commit because the deletion
/// was at or below its horizon. Then checks that a new fullnode executes every
/// kept transaction and that every node has the same checkpoints.
async fn assert_deletion_outcomes(
    test_cluster: &mut TestCluster,
    object_id: ObjectId,
    outcomes: &HashMap<TransactionDigest, Outcome>,
) {
    let horizon_distance = test_cluster
        .protocol_config()
        .pcool_deterministic_validation_horizon_distance();
    let successes: Vec<_> = outcomes
        .values()
        .filter_map(|outcome| outcome.as_ref().ok())
        .filter(|effects| effects.status().is_success())
        .collect();
    let [deletion] = successes.as_slice() else {
        panic!(
            "exactly one transaction must succeed, got {}",
            successes.len()
        );
    };
    assert_eq!(
        deletion
            .deleted()
            .iter()
            .map(|deleted| deleted.object_id)
            .collect::<Vec<_>>(),
        vec![object_id]
    );

    let validator = test_cluster.swarm.validator_node_handles()[0].clone();
    let (_, tombstone) = validator
        .with(|node| {
            node.state()
                .epoch_store_for_testing()
                .highest_handler_processed_row(&object_id)
                .unwrap()
        })
        .expect("the deletion must leave a handler row");
    assert_eq!(tombstone.deleted_by, Some(*deletion.transaction_digest()));
    let deletion_commit = tombstone.produced_at;

    let mut dropped = 0;
    for (digest, outcome) in outcomes {
        match outcome {
            Ok(effects) if effects.transaction_digest() == deletion.transaction_digest() => {}
            Ok(effects) => {
                assert!(
                    matches!(
                        effects.status(),
                        ExecutionStatus::Failure {
                            error: ExecutionError::InputObjectDeleted,
                            ..
                        }
                    ),
                    "{digest:?} must fail on the deleted object, got {:?}",
                    effects.status()
                );
                let gas_row = ObjectKey(
                    effects.gas_object().reference().object_id,
                    effects.lamport_version(),
                );
                let commit = validator
                    .with(|node| {
                        node.state()
                            .epoch_store_for_testing()
                            .handler_processed_object(&gas_row)
                            .unwrap()
                    })
                    .expect("a kept transaction must leave a handler row for its gas coin")
                    .produced_at;
                assert!(
                    (deletion_commit..deletion_commit + horizon_distance).contains(&commit),
                    "{digest:?} was kept at commit {commit}, while the deletion at commit \
                     {deletion_commit} is at or below its horizon"
                );
            }
            Err(error) => {
                assert_eq!(
                    error,
                    &IotaError::from(UserInputError::InputObjectNotAvailableAtCommit {
                        object_id,
                        version: None,
                    }),
                    "{digest:?} must be dropped for the deleted object"
                );
                dropped += 1;
            }
        }
    }
    for node in test_cluster.swarm.validator_node_handles() {
        let metrics = node.with(|node| node.state().metrics.clone());
        let deleted_at_or_below_horizon = metrics
            .consensus_handler_validation_reader_verdicts
            .with_label_values(&["drop", "SharedDeletedAtOrBelowHorizon"])
            .get();
        assert_eq!(
            deleted_at_or_below_horizon,
            metrics
                .consensus_handler_validation_dropped_transactions
                .get(),
            "every drop at a commit must be of a shared object deleted at or below the horizon"
        );
        assert!(deleted_at_or_below_horizon >= dropped);
    }

    let executed: Vec<_> = outcomes
        .iter()
        .filter(|(_, outcome)| outcome.is_ok())
        .map(|(digest, _)| *digest)
        .collect();
    let fullnode = test_cluster.spawn_new_fullnode().await.iota_node;
    timeout(
        Duration::from_secs(120),
        fullnode
            .state()
            .get_transaction_cache_reader()
            .notify_read_executed_effects_for_testing("", &executed),
    )
    .await
    .expect("a new fullnode must execute every kept transaction");
    assert_every_node_has_the_same_checkpoints(test_cluster, &fullnode).await;
}

/// Waits until every node has executed the highest checkpoint `fullnode` has
/// executed, then checks that they all hold the same certified checkpoints up
/// to it, and that each checkpoint a validator built is the certified one.
async fn assert_every_node_has_the_same_checkpoints(
    test_cluster: &TestCluster,
    fullnode: &IotaNodeHandle,
) {
    let highest_executed = |node: &IotaNodeHandle| {
        node.with(|node| {
            node.state()
                .get_checkpoint_store()
                .get_highest_executed_checkpoint_seq_number()
                .unwrap()
                .unwrap_or_default()
        })
    };
    let target = highest_executed(fullnode);
    let certified = |node: &IotaNodeHandle| {
        node.with(|node| {
            let store = node.state().get_checkpoint_store().clone();
            (0..=target)
                .map(|sequence_number| {
                    *store
                        .get_checkpoint_by_sequence_number(sequence_number)
                        .unwrap()
                        .unwrap_or_else(|| panic!("checkpoint {sequence_number} must be stored"))
                        .digest()
                })
                .collect::<Vec<_>>()
        })
    };
    let expected = certified(fullnode);
    let validators = test_cluster.swarm.validator_node_handles();
    for node in validators
        .iter()
        .chain([&test_cluster.fullnode_handle.iota_node])
    {
        timeout(Duration::from_secs(120), async {
            while highest_executed(node) < target {
                sleep(Duration::from_millis(500)).await;
            }
        })
        .await
        .unwrap_or_else(|_| panic!("every node must execute checkpoint {target}"));
        assert_eq!(certified(node), expected);
    }
    for node in &validators {
        node.with(|node| {
            let store = node.state().get_checkpoint_store().clone();
            for sequence_number in 0..=target {
                let local = store
                    .get_locally_computed_checkpoint(sequence_number)
                    .unwrap();
                if let Some(local) = local {
                    assert_eq!(
                        local.digest(),
                        expected[sequence_number as usize],
                        "validator {:?} built checkpoint {sequence_number} differently",
                        node.state().name
                    );
                }
            }
        });
    }
}
