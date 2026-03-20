// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::time::Duration;

use iota_types::{base_types::TransactionDigest, error::IotaError};
use tokio::time::timeout;
use typed_store::Map;

use crate::authority::test_authority_builder::TestAuthorityBuilder;

#[tokio::test]
async fn test_notify_read_executed_transactions_to_checkpoint() {
    let authority_state = TestAuthorityBuilder::new().build().await;
    let store = authority_state.epoch_store_for_testing();
    let checkpoint_sequence_1 = 10;
    let checkpoint_sequence_2 = 12;

    let txes_to_be_notified = vec![
        TransactionDigest::random(),
        TransactionDigest::random(),
        TransactionDigest::random(),
    ];

    // Insert only the first transaction already
    store
        .insert_finalized_transactions(
            vec![txes_to_be_notified[0]].as_slice(),
            checkpoint_sequence_1,
        )
        .expect("Should not fail");

    // Now register to get notified for the addition of some of the above
    // transactions
    let txes_to_be_notified_cloned = txes_to_be_notified.clone();
    let handle = tokio::spawn(async move {
        let notify = store.transactions_executed_in_checkpoint_notify(txes_to_be_notified_cloned);
        notify.await
    });

    // Now insert the rest of the transactions
    let store = authority_state.epoch_store_for_testing();
    store
        .insert_finalized_transactions(&txes_to_be_notified[1..], checkpoint_sequence_2)
        .expect("Should not fail");

    // We should get notified about all the transactions having been executed via
    // checkpoints
    let _ = timeout(Duration::from_secs(5), handle)
        .await
        .expect("Should not timeout")
        .expect("Should not fail");

    // And the transactions should be found into the table
    let result = store
        .multi_get_transaction_checkpoint(txes_to_be_notified.as_slice())
        .expect("Should not fail");
    assert_eq!(result.len(), txes_to_be_notified.len());

    assert_eq!(result[0].unwrap(), checkpoint_sequence_1);
    assert_eq!(result[1].unwrap(), checkpoint_sequence_2);
    assert_eq!(result[2].unwrap(), checkpoint_sequence_2);
}

/// Tests the race condition fix: when a transaction was already dropped and
/// persisted to the `dropped_transactions` table *before* the client calls
/// `notify_read_dropped_digests`, the method should return immediately
/// instead of hanging until timeout.
#[tokio::test]
async fn test_notify_read_dropped_digests_returns_persisted_error() {
    let authority_state = TestAuthorityBuilder::new().build().await;
    let store = authority_state.epoch_store_for_testing();

    let digest = TransactionDigest::random();
    let expected_error = IotaError::TransactionExpired;

    // Simulate the consensus handler having already persisted the drop.
    store
        .tables()
        .expect("tables should be available")
        .dropped_transactions
        .insert(&digest, &expected_error)
        .expect("insert should not fail");

    // notify_read_dropped_digests should return immediately from the DB check,
    // not hang waiting for a notification that already fired.
    let result = timeout(
        Duration::from_secs(5),
        store.notify_read_dropped_digests(digest),
    )
    .await
    .expect("should not timeout — the DB lookup should return immediately")
    .expect("should not return a storage error");

    assert_eq!(result.to_string(), expected_error.to_string());
}

/// Tests the normal (non-race) path: the client registers before the drop
/// is persisted, and the notification resolves the future.
#[tokio::test]
async fn test_notify_read_dropped_digests_waits_for_notification() {
    let authority_state = TestAuthorityBuilder::new().build().await;
    let store = authority_state.epoch_store_for_testing();

    let digest = TransactionDigest::random();
    let expected_error = IotaError::TransactionExpired;

    // Spawn a task that waits for the drop notification.
    let store_clone = store.clone();
    let expected_error_clone = expected_error.clone();
    let handle = tokio::spawn(async move {
        store_clone
            .notify_read_dropped_digests(digest)
            .await
            .expect("should not return a storage error")
    });

    // Simulate the consensus handler: write to table, then notify.
    let tables = store.tables().expect("tables should be available");
    tables
        .dropped_transactions
        .insert(&digest, &expected_error_clone)
        .expect("insert should not fail");
    store
        .dropped_tx_notify_read
        .notify(&digest, &expected_error_clone);

    let result = timeout(Duration::from_secs(5), handle)
        .await
        .expect("should not timeout")
        .expect("task should not panic");

    assert_eq!(result.to_string(), expected_error.to_string());
}
