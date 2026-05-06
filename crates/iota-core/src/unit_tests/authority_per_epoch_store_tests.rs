// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{collections::HashMap, time::Duration};

use iota_types::base_types::{AuthorityName, TransactionDigest};
use tokio::time::timeout;
use typed_store::rocks::DBBatch;

use crate::authority::{
    authority_per_epoch_store::{
        AuthorityPerEpochStore, consensus_quarantine::ConsensusCommitOutput,
    },
    test_authority_builder::TestAuthorityBuilder,
};

/// Records an overload notification through the same path
/// `process_consensus_transaction` uses: buffer it in `ConsensusCommitOutput`
/// and flush via `write_to_batch`. This is the only sanctioned way to populate
/// `authority_overload_notifications` outside the consensus loop.
fn flush_overload_notification(
    store: &AuthorityPerEpochStore,
    authority: AuthorityName,
    percentage: u8,
) {
    let mut output = ConsensusCommitOutput::new(0);
    output.record_overload_notification(authority, percentage);
    output.set_default_commit_stats_for_testing();
    let mut batch: DBBatch = store.db_batch_for_test();
    output.write_to_batch(store, &mut batch).unwrap();
    batch.write().unwrap();
}

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
            0,
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
        .insert_finalized_transactions(&txes_to_be_notified[1..], checkpoint_sequence_2, 0)
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

/// Persisted overload notifications must round-trip through
/// `ConsensusCommitOutput::record_overload_notification` (flushed via
/// `write_to_batch`) -> `load_overload_notifications`. Re-recording in a
/// subsequent commit overwrites the previous percentage.
#[tokio::test]
async fn test_load_overload_notifications_round_trip() {
    let authority_state = TestAuthorityBuilder::new().build().await;
    let store = authority_state.epoch_store_for_testing();
    let me = store.name;

    assert!(
        store.load_overload_notifications().unwrap().is_empty(),
        "no notifications recorded yet",
    );

    flush_overload_notification(&store, me, 25);
    assert_eq!(
        store
            .load_overload_notifications()
            .unwrap()
            .get(&me)
            .copied(),
        Some(25),
    );

    // A subsequent commit's recorder call from the same authority overwrites
    // the prior value once flushed.
    flush_overload_notification(&store, me, 75);
    assert_eq!(
        store
            .load_overload_notifications()
            .unwrap()
            .get(&me)
            .copied(),
        Some(75),
    );
}

/// `compute_quorum_load_shedding_percentage` must read the percentile from
/// the supplied map without consulting the DB. With a single-authority test
/// committee the 2f+1 quorum value is just that authority's reported value
/// (or 0 when absent). This is the same code path used to apply in-batch
/// `OverloadNotificationV1` overrides on top of the persisted map before
/// dropping user transactions.
#[tokio::test]
async fn test_compute_quorum_load_shedding_percentage_uses_overlay() {
    let authority_state = TestAuthorityBuilder::new().build().await;
    let store = authority_state.epoch_store_for_testing();
    let me = store.name;

    // Empty overlay -> 0%.
    let empty: HashMap<_, _> = HashMap::new();
    assert_eq!(store.compute_quorum_load_shedding_percentage(&empty), 0);

    // Overlay-only value is reflected without touching the DB.
    let mut overlay = HashMap::new();
    overlay.insert(me, 60);
    assert_eq!(store.compute_quorum_load_shedding_percentage(&overlay), 60);

    // Persist a different value and confirm the overlay wins, mirroring the
    // last-writer-wins behavior of the pre-pass merge step.
    flush_overload_notification(&store, me, 10);
    let mut merged = store.load_overload_notifications().unwrap();
    merged.insert(me, 90);
    assert_eq!(store.compute_quorum_load_shedding_percentage(&merged), 90);

    // Without the overlay, only the persisted value is visible.
    assert_eq!(store.get_quorum_load_shedding_percentage().unwrap(), 10);
}
