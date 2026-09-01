// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Checks the applying path against traffic a real cluster produces.
//!
//! The unit tests in `iota-core` cover a single transfer. What only a cluster
//! gives is variety: shared object transactions, transactions that emit events,
//! and the end-of-epoch transaction. Those are the paths where applying a
//! checkpoint's results could diverge from executing its transactions.

use std::{fs, path::Path, time::Duration};

use iota_core::{authority::AuthorityState, transaction_outputs::TransactionOutputs};
use iota_macros::sim_test;
use iota_sdk_types::{ObjectId, TransactionDigest, Version};
use iota_types::{
    committee::EpochId,
    effects::TransactionEffectsAPI,
    full_checkpoint_content::{CheckpointData, CheckpointTransaction},
    transaction::TransactionAPI,
};
use test_cluster::TestClusterBuilder;

/// Reads every checkpoint the cluster wrote to its data ingestion directory,
/// ordered by sequence number.
fn read_checkpoints(dir: &Path) -> Vec<CheckpointData> {
    let mut checkpoints: Vec<CheckpointData> = fs::read_dir(dir)
        .expect("the cluster must have created the ingestion directory")
        .filter_map(|entry| {
            let path = entry.ok()?.path();
            (path.extension()? == "chk").then_some(path)
        })
        .map(|path| {
            let bytes = fs::read(&path).expect("checkpoint file must be readable");
            Blob::from_bytes::<CheckpointData>(&bytes).expect("checkpoint file must decode")
        })
        .collect();
    checkpoints.sort_by_key(|data| data.checkpoint_summary.sequence_number);
    checkpoints
}

/// A version the node no longer holds is only acceptable if a later
/// transaction superseded it: the version then leaves the live table when its
/// successor is committed, and its historic bucket expires with its epoch. A
/// version that was never superseded and is still absent is a write the node
/// failed to make.
fn assert_superseded_since(
    state: &AuthorityState,
    id: &ObjectId,
    version: Version,
    digest: &TransactionDigest,
) {
    let live = state.get_object(id);
    assert!(
        live.is_some_and(|live| live.version() > version),
        "transaction {digest} wrote version {version} of object {id}, which the node does not \
         have and has not superseded"
    );
}

/// Every object the applying path would write must match what the node stored
/// when it executed the transaction. This is the derivation checked against
/// ground truth rather than against itself.
fn assert_written_objects_match_the_store(
    state: &AuthorityState,
    tx: &CheckpointTransaction,
    applied: &TransactionOutputs,
) -> usize {
    let digest = tx.effects.transaction_digest();
    let mut compared = 0;
    for (id, object) in &applied.written {
        let Some(stored) = state
            .get_object_store()
            .get_object_by_key(id, object.version())
        else {
            assert_superseded_since(state, id, object.version(), digest);
            continue;
        };
        assert_eq!(
            stored.digest(),
            object.digest(),
            "transaction {digest}: applying would write a different {id} than execution did"
        );
        compared += 1;
    }
    compared
}

/// Every marker the applying path would write must match what the node stored.
/// This is where the owned-versus-shared deletion decision shows up, the one
/// place the two constructors read the input owner from different sources.
fn assert_markers_match_the_store(
    state: &AuthorityState,
    epoch: EpochId,
    tx: &CheckpointTransaction,
    applied: &TransactionOutputs,
) -> usize {
    let digest = tx.effects.transaction_digest();
    for (key, marker) in &applied.markers {
        let stored = state
            .get_object_cache_reader()
            .get_marker_value(&key.0, key.1, epoch)
            .unwrap_or_else(|| {
                panic!("transaction {digest} derived a marker for {key:?} that the node lacks")
            });
        assert_eq!(
            &stored, marker,
            "transaction {digest}: applying would mark {key:?} differently than execution did"
        );
    }
    applied.markers.len()
}

#[sim_test]
async fn applying_matches_execution_across_cluster_traffic() {
    let dir = tempfile::tempdir().unwrap();
    let cluster = TestClusterBuilder::new()
        .with_epoch_duration_ms(15_000)
        .with_data_ingestion_dir(dir.path().to_path_buf())
        .build()
        .await;

    // Ordinary user traffic alongside the system transactions the cluster
    // produces on its own.
    let addresses = cluster.get_addresses();
    let (sender, receiver) = (addresses[0], addresses[1]);
    for _ in 0..5 {
        cluster
            .transfer_iota_must_exceed(sender, receiver, 1_000_000)
            .await;
    }

    // Crossing a boundary brings in the end-of-epoch transaction, which the
    // applying path deliberately leaves to the executor.
    cluster.wait_for_epoch(Some(1)).await;
    tokio::time::sleep(Duration::from_secs(2)).await;

    let state = cluster.fullnode_handle.iota_node.state();
    let checkpoints = read_checkpoints(dir.path());
    assert!(
        checkpoints.len() > 5,
        "the cluster must have produced checkpoints to check, got {}",
        checkpoints.len()
    );

    let mut transactions = 0usize;
    let mut with_events = 0usize;
    let mut shared_object_transactions = 0usize;
    let mut end_of_epoch_transactions = 0usize;
    let mut markers_checked = 0usize;
    let mut objects_checked = 0usize;

    for checkpoint in &checkpoints {
        let epoch = checkpoint.checkpoint_summary.data().epoch;
        // The payloads must check out against the digests the effects record.
        // Run over real traffic this also covers the events branches, which a
        // plain transfer never reaches.
        checkpoint
            .verify_payload_digests()
            .expect("data the cluster produced must verify against its own effects");

        for tx in &checkpoint.transactions {
            transactions += 1;
            if tx.events.is_some() {
                with_events += 1;
            }
            if tx.transaction.transaction().is_end_of_epoch_tx() {
                end_of_epoch_transactions += 1;
                continue;
            }
            if !tx.effects.input_shared_objects().is_empty() {
                shared_object_transactions += 1;
            }

            let applied = TransactionOutputs::build_from_checkpoint_transaction(tx);
            assert_eq!(
                applied.effects, tx.effects,
                "the applied effects must be the certified ones"
            );
            objects_checked += assert_written_objects_match_the_store(&state, tx, &applied);
            markers_checked += assert_markers_match_the_store(&state, epoch, tx, &applied);
        }
    }

    // Guard the coverage this test exists for: without variety it would pass
    // while proving only what the unit tests already do.
    assert!(
        with_events > 0,
        "no transaction emitted events, so the events path went unchecked"
    );
    assert!(
        shared_object_transactions > 0,
        "no transaction touched a shared object, so that path went unchecked"
    );
    assert!(
        end_of_epoch_transactions > 0,
        "no end-of-epoch transaction was seen, so the skip went unchecked"
    );
    // Versions a later transaction superseded are excused above, so state how
    // many objects were actually compared against the store.
    assert!(
        objects_checked > 0,
        "every written object had been superseded, so nothing was compared"
    );
    println!(
        "checked {transactions} transactions: {with_events} with events, \
         {shared_object_transactions} touching shared objects, \
         {end_of_epoch_transactions} end-of-epoch, {markers_checked} markers"
    );
}

/// The end-of-epoch transaction must be left for the executor: it drives
/// reconfiguration and is never applied from checkpoint data.
#[sim_test]
async fn end_of_epoch_transaction_is_left_to_the_executor() {
    let dir = tempfile::tempdir().unwrap();
    let cluster = TestClusterBuilder::new()
        .with_epoch_duration_ms(15_000)
        .with_data_ingestion_dir(dir.path().to_path_buf())
        .build()
        .await;

    cluster.wait_for_epoch(Some(1)).await;
    tokio::time::sleep(Duration::from_secs(2)).await;

    let boundary = read_checkpoints(dir.path())
        .into_iter()
        .find(|data| data.checkpoint_summary.end_of_epoch_data.is_some())
        .expect("crossing an epoch must produce a boundary checkpoint");

    let change_epoch = boundary
        .end_of_epoch_transaction()
        .expect("a boundary checkpoint carries the change-epoch transaction");
    assert!(
        change_epoch.transaction.transaction().is_end_of_epoch_tx(),
        "the last transaction of a boundary checkpoint is the change-epoch one"
    );
}
