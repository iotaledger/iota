// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::collections::HashMap;

use iota_data_ingestion_core::Worker;
use iota_kvstore::{KeyValueStoreReader, KvWorker, TransactionData, emulator::BigTableEmulator};
use iota_types::{
    base_types::VersionNumber,
    digests::{CheckpointDigest, TransactionDigest},
    effects::TransactionEffectsAPI,
    full_checkpoint_content::CheckpointData,
    object::Object,
    storage::ObjectKey,
    test_checkpoint_data_builder::TestCheckpointDataBuilder,
};

/// Build a small checkpoint with a mix of object creations and transfers so
/// every table receives more than one row.
fn build_test_checkpoint() -> CheckpointData {
    TestCheckpointDataBuilder::new(1)
        .start_transaction(0)
        .create_owned_object(0)
        .finish_transaction()
        .start_transaction(0)
        .transfer_object(0, 1)
        .finish_transaction()
        .start_transaction(2)
        .create_owned_object(1)
        .finish_transaction()
        .build_checkpoint()
}

/// Tests that a checkpoint is written to BigTable correctly, with objects,
/// transactions, and checkpoints data.
#[tokio::test]
#[cfg_attr(
    not(feature = "integration_tests"),
    ignore = "requires the BigTable emulator; run with --features integration_tests"
)]
async fn process_checkpoint_round_trips_objects_transactions_and_checkpoints() {
    let emulator = BigTableEmulator::start().await.unwrap();
    let mut client = emulator.client().unwrap();

    let checkpoint = build_test_checkpoint();
    let checkpoint_seq = checkpoint.checkpoint_summary.sequence_number;
    let checkpoint_digest = *checkpoint.checkpoint_summary.digest();

    // capture the expected state before handing the checkpoint to the KvWorker.
    let expected_objects = checkpoint
        .transactions
        .iter()
        .flat_map(|t| &t.output_objects)
        .map(|o| (ObjectKey(o.id(), o.version()), o.clone()))
        .collect::<HashMap<ObjectKey, Object>>();

    let expected_transactions = checkpoint
        .transactions
        .iter()
        .map(|t| {
            (
                *t.transaction.digest(),
                TransactionData::new(t, checkpoint_seq),
            )
        })
        .collect::<HashMap<TransactionDigest, TransactionData>>();
    let expected_contents = checkpoint.checkpoint_contents.clone();

    // write to BigTable
    let worker = KvWorker::new(client.clone());
    worker.process_checkpoint(checkpoint.into()).await.unwrap();

    // objects table: check that every output object comes back byte-identical.
    let object_keys: Vec<ObjectKey> = expected_objects.keys().copied().collect();
    let fetched = client.get_objects(&object_keys).await.unwrap();
    assert_eq!(fetched.len(), expected_objects.len());
    for object in &fetched {
        let key = ObjectKey(object.id(), object.version());
        assert_eq!(
            Some(object),
            expected_objects.get(&key),
            "object stored under {key:?} does not match expected",
        );
    }

    // transactions table: check that every transaction comes back byte-identical.
    let tx_digests: Vec<TransactionDigest> = expected_transactions.keys().copied().collect();
    let fetched = client.get_transactions(&tx_digests).await.unwrap();
    assert_eq!(fetched.len(), expected_transactions.len());
    for tx in &fetched {
        let digest = *tx.transaction.digest();
        let expected = &expected_transactions[&digest];
        assert_eq!(tx.effects.transaction_digest(), &digest);
        assert_eq!(tx.effects, expected.effects);
        assert_eq!(tx.events, expected.events);
        assert_eq!(tx.checkpoint_number, checkpoint_seq);
    }

    // checkpoints table: summary and contents round-trip by sequence number.
    let fetched = client.get_checkpoints(&[checkpoint_seq]).await.unwrap();
    assert_eq!(fetched.len(), 1);
    assert_eq!(*fetched[0].summary.digest(), checkpoint_digest);
    assert_eq!(fetched[0].summary.sequence_number, checkpoint_seq);
    assert_eq!(fetched[0].contents.digest(), expected_contents.digest());

    // checkpoints-by-digest index: digest resolves to the sequence number and,
    // through it, to the full checkpoint.
    let seqs = client
        .get_checkpoint_sequence_numbers([checkpoint_digest])
        .await
        .unwrap();
    assert_eq!(seqs, vec![checkpoint_seq]);
    let fetched = client
        .get_checkpoints_by_digest([checkpoint_digest])
        .await
        .unwrap();
    assert_eq!(fetched.len(), 1);
    assert_eq!(*fetched[0].summary.digest(), checkpoint_digest);
}

/// Tests that readers omit not-found keys when fetching objects and
/// transactions.
#[tokio::test]
#[cfg_attr(
    not(feature = "integration_tests"),
    ignore = "requires the BigTable emulator; run with --features integration_tests"
)]
async fn readers_omit_not_found_keys() {
    let emulator = BigTableEmulator::start().await.unwrap();
    let mut client = emulator.client().unwrap();

    let checkpoint = build_test_checkpoint();
    let checkpoint_seq = checkpoint.checkpoint_summary.sequence_number;
    let stored_object_id = checkpoint.transactions[0].output_objects[0].id();

    let worker = KvWorker::new(client.clone());
    worker.process_checkpoint(checkpoint.into()).await.unwrap();

    let missing_key = ObjectKey(stored_object_id, VersionNumber::from_u64(999));
    let fetched = client.get_objects(&[missing_key]).await.unwrap();
    assert!(fetched.is_empty());

    let fetched = client
        .get_transactions(&[TransactionDigest::random()])
        .await
        .unwrap();
    assert!(fetched.is_empty());

    let fetched = client.get_checkpoints(&[checkpoint_seq + 1]).await.unwrap();
    assert!(fetched.is_empty());

    let fetched = client
        .get_checkpoint_sequence_numbers([CheckpointDigest::random()])
        .await
        .unwrap();
    assert!(fetched.is_empty());
}
