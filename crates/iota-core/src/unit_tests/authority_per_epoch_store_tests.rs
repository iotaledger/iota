// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{collections::HashMap, time::Duration};

use iota_types::{
    base_types::{AuthorityName, TransactionDigest},
    messages_consensus::ConsensusTransaction,
};
use tokio::time::timeout;
use typed_store::rocks::DBBatch;

use crate::{
    authority::{
        authority_per_epoch_store::{
            AuthorityPerEpochStore, consensus_quarantine::ConsensusCommitOutput,
        },
        test_authority_builder::TestAuthorityBuilder,
    },
    consensus_handler::SequencedConsensusTransactionKey,
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
/// `load_overload_notifications` must return the same notification map
/// whether a given authority's latest reported percentage lives in the
/// persisted `authority_overload_notifications` DBMap or in a still-queued
/// `ConsensusCommitOutput` inside `ConsensusOutputQuarantine`. The "logical
/// state" of overload notifications is the union of (a) everything flushed to
/// disk so far and (b) everything processed but not yet flushed; the read
/// path must surface that union, not just (a).
///
/// This invariance under the disk/queue split is what makes the
/// post-consensus drop decision deterministic: the same set of notifications
/// fed into `compute_quorum_load_shedding_percentage` regardless of where the
/// flush boundary happens to fall when the read is taken.
///
/// The test puts the same authority's value first in (a), then in (b), then
/// back in (a) (mirroring what a fresh start sees after a queued commit has
/// been flushed). All three reads must agree, and the derived
/// `get_quorum_load_shedding_percentage` must match.
#[tokio::test]
async fn test_load_overload_notifications_invariant_under_disk_queue_split() {
    let authority_state = TestAuthorityBuilder::new().build().await;
    let store = authority_state.epoch_store_for_testing();
    let authority_name = store.name;

    // Disk holds an earlier value for the authority.
    flush_overload_notification(&store, authority_name, 30);
    assert_eq!(
        store.load_overload_notifications().unwrap().get(&authority_name).copied(),
        Some(30),
    );

    // A later commit has been processed and its output is sitting in the
    // quarantine queue, *not* yet flushed. This is the common steady-state
    // condition while consensus runs ahead of the checkpoint executor —
    // potentially many commits queue up before any of them flush.
    let mut later = ConsensusCommitOutput::default();
    later.record_overload_notification(authority_name, 80);
    later.set_default_commit_stats_for_testing();
    store.push_consensus_output_for_tests(later);

    // The read that drives the post-consensus drop decision must see the
    // queued value, not the stale on-disk one.
    let from_queue = store
        .load_overload_notifications()
        .unwrap()
        .get(&authority_name)
        .copied();
    assert_eq!(
        from_queue,
        Some(80),
        "queued notifications must be visible via load_overload_notifications",
    );

    // Now the same logical state sits fully on disk (as it would after the
    // queued commit drains to RocksDB). The read must produce the same
    // notification map as before.
    flush_overload_notification(&store, authority_name, 80);
    let from_disk = store
        .load_overload_notifications()
        .unwrap()
        .get(&authority_name)
        .copied();
    assert_eq!(
        from_queue, from_disk,
        "load_overload_notifications must be invariant under the disk/queue split",
    );

    // The derived quorum percentage must agree with the union view.
    assert_eq!(store.get_quorum_load_shedding_percentage().unwrap(), 80);
}

/// `consensus_message_processed` dedups by `ConsensusTransactionKey` for the
/// remainder of an epoch — once a key has been observed, any later submission
/// with the same key is silently dropped by `verify_consensus_transaction`.
/// `OverloadNotificationV1` therefore needs a per-submission disambiguator in
/// its key; without one, re-sending the same percentage value within an epoch
/// (e.g. when a validator's local percentage oscillates 0 → high → 0) would
/// produce a colliding key and the second submission would never reach the
/// recorded notifications map, freezing peers' view at the stale high value.
///
/// This test asserts:
///   1. Two `new_overload_notification_v1` calls with the same authority and
///      percentage produce distinct consensus transaction keys.
///   2. After marking the first key as processed in the quarantine, the
///      second key is *not* considered processed.
///   3. Driving the recorded overload-notification map through the
///      0 → high → 0 oscillation produces the expected sequence of
///      `load_overload_notifications` values, and the quorum percentage
///      returns to 0 at the end.
#[tokio::test]
async fn test_overload_notification_resend_same_percentage_updates_quorum() {
    let authority_state = TestAuthorityBuilder::new().build().await;
    let store = authority_state.epoch_store_for_testing();
    let me = store.name;

    // (1) Two notifications with identical authority+percentage must produce
    //     distinct consensus transaction keys. The disambiguator is the
    //     wall-clock generation embedded by `new_overload_notification_v1`;
    //     sleep 2ms between calls to guarantee a tick.
    let first = ConsensusTransaction::new_overload_notification_v1(me, 0);
    tokio::time::sleep(Duration::from_millis(2)).await;
    let second = ConsensusTransaction::new_overload_notification_v1(me, 0);
    assert_ne!(
        first.key(),
        second.key(),
        "two submissions of the same percentage must produce distinct keys; \
         otherwise consensus_message_processed will silently drop the second \
         one and the local oscillation will never propagate to peers",
    );

    // (2) Marking the first key as processed in the quarantine must not
    //     cause the second key to look processed. This is the invariant
    //     `verify_consensus_transaction` relies on to admit the second
    //     submission for processing.
    let first_seq_key = SequencedConsensusTransactionKey::External(first.key());
    let second_seq_key = SequencedConsensusTransactionKey::External(second.key());
    let mut commit_for_first = ConsensusCommitOutput::default();
    commit_for_first.record_consensus_message_processed(first_seq_key.clone());
    commit_for_first.set_default_commit_stats_for_testing();
    store.push_consensus_output_for_tests(commit_for_first);

    assert!(
        store.is_consensus_message_processed(&first_seq_key).unwrap(),
        "first submission's key should be marked processed after recording",
    );
    assert!(
        !store
            .is_consensus_message_processed(&second_seq_key)
            .unwrap(),
        "second submission's key must NOT be considered processed — it is a \
         distinct key and must be admitted by verify_consensus_transaction",
    );

    // (3) Walk through the 0 → high → 0 oscillation. At each step
    //     `load_overload_notifications` and the derived quorum percentage
    //     must reflect the latest reported value.
    flush_overload_notification(&store, me, 0);
    assert_eq!(
        store
            .load_overload_notifications()
            .unwrap()
            .get(&me)
            .copied(),
        Some(0),
        "after first 0% notification, loaded value is 0",
    );
    assert_eq!(
        store.get_quorum_load_shedding_percentage().unwrap(),
        0,
        "quorum reflects 0% baseline",
    );

    flush_overload_notification(&store, me, 80);
    assert_eq!(
        store
            .load_overload_notifications()
            .unwrap()
            .get(&me)
            .copied(),
        Some(80),
        "after climbing to 80%, loaded value updates",
    );
    assert_eq!(
        store.get_quorum_load_shedding_percentage().unwrap(),
        80,
        "quorum climbs to 80%",
    );

    // The critical step: re-send 0% after having already sent 0% earlier.
    // Before the fix, the second 0% transaction's key matched the first one's
    // and consensus silently deduped it, so this recorder call would never
    // run in production. With distinct keys it runs and overwrites the 80%.
    flush_overload_notification(&store, me, 0);
    assert_eq!(
        store
            .load_overload_notifications()
            .unwrap()
            .get(&me)
            .copied(),
        Some(0),
        "after returning to 0%, loaded value updates back to 0 — this is the \
         recovery path that was broken before the generation field was added",
    );
    assert_eq!(
        store.get_quorum_load_shedding_percentage().unwrap(),
        0,
        "quorum returns to 0% — peers no longer see a stale high value",
    );
}
