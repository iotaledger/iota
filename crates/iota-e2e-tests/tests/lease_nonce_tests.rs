// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Transactions that carry a lease in their valid window, on a network that
//! runs the P-COOL flow: the first transaction of a sender with a given lease
//! holds it until its last valid epoch, and no other transaction of that
//! sender with the same lease executes until then.

use std::time::Duration;

use iota_json_rpc_types::IotaTransactionBlockResponse;
use iota_macros::sim_test;
use iota_sdk_types::{
    Identifier, ObjectId, ObjectReference, Transaction, TransactionDigest, TransactionExpiration,
    TypeTag, ValidWindow,
};
use iota_types::{
    base_types::dbg_addr,
    programmable_transaction_builder::ProgrammableTransactionBuilder,
    transaction::{TEST_ONLY_GAS_UNIT_FOR_TRANSFER, TransactionAPI, TransactionEnvelope},
};
use test_cluster::{TestCluster, TestClusterBuilder};

const LEASE: [u8; 32] = [7; 32];

fn current_epoch(test_cluster: &TestCluster) -> u64 {
    test_cluster
        .fullnode_handle
        .iota_node
        .with(|node| node.state().epoch_store_for_testing().epoch())
}

/// A window from the current epoch to `epochs` epochs later, with `lease`.
fn window(test_cluster: &TestCluster, epochs: u64, lease: Option<[u8; 32]>) -> ValidWindow {
    let epoch = current_epoch(test_cluster);
    let genesis_hash = test_cluster
        .fullnode_handle
        .iota_node
        .with(|node| node.state().get_chain_identifier().digest());
    ValidWindow {
        first_valid: epoch,
        last_valid: epoch + epochs,
        genesis_hash,
        note: 0,
        lease,
    }
}

async fn coins(test_cluster: &TestCluster, count: usize) -> Vec<ObjectReference> {
    let coins = test_cluster
        .wallet
        .get_gas_objects_owned_by_address(test_cluster.get_address_0(), count)
        .await
        .unwrap();
    assert_eq!(coins.len(), count);
    coins
}

/// A transfer of 1 nano to `recipient`, paid with `coin`, that expires with
/// `window`.
async fn transfer(
    test_cluster: &TestCluster,
    coin: ObjectReference,
    recipient: u8,
    window: ValidWindow,
) -> TransactionEnvelope {
    let rgp = test_cluster.get_reference_gas_price().await;
    let mut data = Transaction::new_transfer_iota(
        dbg_addr(recipient),
        test_cluster.get_address_0(),
        Some(1),
        coin,
        rgp * TEST_ONLY_GAS_UNIT_FOR_TRANSFER,
        rgp,
    );
    *data.expiration_mut_for_testing() = TransactionExpiration::ValidWindow(Box::new(window));
    test_cluster.wallet.sign_transaction(&data)
}

/// A call of `verified_nonce::verify_lease_nonce` that leaves its result
/// unused. The result cannot be dropped, so the transaction fails.
async fn unused_lease_nonce(
    test_cluster: &TestCluster,
    coin: ObjectReference,
    window: ValidWindow,
) -> TransactionEnvelope {
    let rgp = test_cluster.get_reference_gas_price().await;
    let mut builder = ProgrammableTransactionBuilder::new();
    let nonce_key = builder.pure(0u64).unwrap();
    builder.programmable_move_call(
        ObjectId::FRAMEWORK,
        Identifier::new("verified_nonce").unwrap(),
        Identifier::new("verify_lease_nonce").unwrap(),
        vec![TypeTag::U64],
        vec![nonce_key],
    );
    let mut data = Transaction::new_programmable(
        test_cluster.get_address_0(),
        vec![coin],
        builder.finish(),
        rgp * TEST_ONLY_GAS_UNIT_FOR_TRANSFER * 10,
        rgp,
    );
    *data.expiration_mut_for_testing() = TransactionExpiration::ValidWindow(Box::new(window));
    test_cluster.wallet.sign_transaction(&data)
}

async fn execute(
    test_cluster: &TestCluster,
    transaction: TransactionEnvelope,
) -> anyhow::Result<IotaTransactionBlockResponse> {
    tokio::time::timeout(
        Duration::from_secs(120),
        test_cluster
            .wallet
            .execute_transaction_may_fail(transaction),
    )
    .await
    .expect("the transaction neither finalized nor failed within 120s")
}

/// The validators that have executed `digest`.
fn executed_on(test_cluster: &TestCluster, digest: &TransactionDigest) -> usize {
    test_cluster
        .all_validator_handles()
        .iter()
        .filter(|handle| {
            handle.with(|node| {
                node.state()
                    .get_transaction_cache_reader()
                    .try_get_executed_effects(digest)
                    .unwrap()
                    .is_some()
            })
        })
        .count()
}

#[sim_test]
async fn only_one_of_two_transactions_with_the_same_lease_executes() {
    let test_cluster = TestClusterBuilder::new().build().await;
    let coins = coins(&test_cluster, 2).await;
    let window = window(&test_cluster, 1, Some(LEASE));
    let first = transfer(&test_cluster, coins[0], 2, window).await;
    let second = transfer(&test_cluster, coins[1], 3, window).await;
    let digests = [*first.digest(), *second.digest()];

    let (first, second) = tokio::join!(
        execute(&test_cluster, first),
        execute(&test_cluster, second)
    );

    let succeeded = |result: &anyhow::Result<IotaTransactionBlockResponse>| {
        result
            .as_ref()
            .is_ok_and(|response| response.status_ok().unwrap_or(false))
    };
    assert!(
        succeeded(&first) ^ succeeded(&second),
        "exactly one must execute; first={first:?}, second={second:?}"
    );
    let (winner, loser) = if succeeded(&first) {
        (digests[0], digests[1])
    } else {
        (digests[1], digests[0])
    };
    let validators = test_cluster.all_validator_handles().len();
    tokio::time::timeout(Duration::from_secs(60), async {
        while executed_on(&test_cluster, &winner) < validators {
            tokio::time::sleep(Duration::from_millis(500)).await;
        }
    })
    .await
    .expect("every validator executes the transaction that took the lease");
    assert_eq!(executed_on(&test_cluster, &loser), 0);
}

#[sim_test]
async fn a_transaction_without_a_lease_is_not_affected() {
    let test_cluster = TestClusterBuilder::new().build().await;
    let coins = coins(&test_cluster, 3).await;
    let leased = window(&test_cluster, 1, Some(LEASE));
    let not_leased = window(&test_cluster, 1, None);

    let transactions = [
        transfer(&test_cluster, coins[0], 2, leased).await,
        transfer(&test_cluster, coins[1], 3, not_leased).await,
        transfer(&test_cluster, coins[2], 4, not_leased).await,
    ];

    for transaction in transactions {
        let response = execute(&test_cluster, transaction).await.unwrap();
        assert_eq!(response.status_ok(), Some(true));
    }
}

#[sim_test]
async fn a_failed_transaction_still_holds_its_lease() {
    let test_cluster = TestClusterBuilder::new().build().await;
    let coins = coins(&test_cluster, 2).await;
    let window = window(&test_cluster, 1, Some(LEASE));

    let failing = unused_lease_nonce(&test_cluster, coins[0], window).await;
    let response = execute(&test_cluster, failing).await.unwrap();
    assert_eq!(
        response.status_ok(),
        Some(false),
        "the result of verify_lease_nonce cannot be left unused"
    );

    let with_the_same_lease = transfer(&test_cluster, coins[1], 2, window).await;
    let digest = *with_the_same_lease.digest();
    let result = execute(&test_cluster, with_the_same_lease).await;

    let error = format!("{:?}", result.expect_err("the lease is held"));
    assert!(error.contains("Lease"), "{error}");
    assert_eq!(executed_on(&test_cluster, &digest), 0);
}

#[sim_test]
async fn a_lease_is_held_across_an_epoch_change_until_its_last_valid_epoch() {
    let test_cluster = TestClusterBuilder::new().build().await;
    let coins = coins(&test_cluster, 3).await;

    // Runs in the first epoch and may run until the next one.
    let holder = transfer(
        &test_cluster,
        coins[0],
        2,
        window(&test_cluster, 1, Some(LEASE)),
    )
    .await;
    let response = execute(&test_cluster, holder).await.unwrap();
    assert_eq!(response.status_ok(), Some(true));

    // In the holder's last valid epoch the lease is still held.
    test_cluster.force_new_epoch().await;
    let in_the_last_valid_epoch = transfer(
        &test_cluster,
        coins[1],
        3,
        window(&test_cluster, 0, Some(LEASE)),
    )
    .await;
    let digest = *in_the_last_valid_epoch.digest();
    let result = execute(&test_cluster, in_the_last_valid_epoch).await;
    let error = format!("{:?}", result.expect_err("the lease is held"));
    assert!(error.contains("Lease"), "{error}");
    assert_eq!(executed_on(&test_cluster, &digest), 0);

    // After it, the lease is free again.
    test_cluster.force_new_epoch().await;
    let after_the_last_valid_epoch = transfer(
        &test_cluster,
        coins[2],
        4,
        window(&test_cluster, 0, Some(LEASE)),
    )
    .await;
    let response = execute(&test_cluster, after_the_last_valid_epoch)
        .await
        .unwrap();
    assert_eq!(response.status_ok(), Some(true));
}

#[sim_test]
async fn a_lease_is_rejected_while_enable_lease_nonce_is_off() {
    // `ProtocolConfig::apply_overrides_for_testing` is thread-local and does
    // not reach the tasks a cluster spawns, so the flag goes through the
    // protocol config environment overrides.
    // SAFETY: set before the cluster is built, on the test thread; each test
    // runs in its own process.
    unsafe {
        std::env::set_var("IOTA_PROTOCOL_CONFIG_OVERRIDE_ENABLE", "1");
        std::env::set_var(
            "IOTA_PROTOCOL_CONFIG_FEATURE_FLAGS_OVERRIDE_ENABLE_LEASE_NONCE",
            "false",
        );
    }
    let test_cluster = TestClusterBuilder::new().build().await;
    let coins = coins(&test_cluster, 2).await;

    let leased = transfer(
        &test_cluster,
        coins[0],
        2,
        window(&test_cluster, 1, Some(LEASE)),
    )
    .await;
    let digest = *leased.digest();
    let result = execute(&test_cluster, leased).await;
    assert!(result.is_err(), "{result:?}");
    assert_eq!(executed_on(&test_cluster, &digest), 0);

    // The window itself is still accepted.
    let not_leased = transfer(&test_cluster, coins[1], 3, window(&test_cluster, 1, None)).await;
    let response = execute(&test_cluster, not_leased).await.unwrap();
    assert_eq!(response.status_ok(), Some(true));
}
