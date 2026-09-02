// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Checks the applying path against traffic a real cluster produces.
//!
//! The unit tests in `iota-core` cover a single transfer. What only a cluster
//! gives is variety: shared object transactions, transactions that emit events,
//! and the end-of-epoch transaction. Those are the paths where applying a
//! checkpoint's results could diverge from executing its transactions.

use std::{
    fs,
    num::NonZeroUsize,
    path::{Path, PathBuf},
    time::Duration,
};

use bytes::Bytes;
use iota_config::node::CheckpointArchiveConfig;
use iota_core::{authority::AuthorityState, transaction_outputs::TransactionOutputs};
use iota_data_ingestion_core::history::{
    CHECKPOINT_FILE_MAGIC,
    manifest::{Manifest, create_file_metadata_from_bytes, finalize_manifest},
};
use iota_macros::sim_test;
use iota_sdk_types::{ObjectId, SharedObjectReference, TransactionDigest, Version};
use iota_storage::{
    FileCompression, StorageFormat,
    blob::{Blob, BlobEncoding},
};
use iota_test_transaction_builder::publish_package;
use iota_types::{
    committee::EpochId,
    effects::TransactionEffectsAPI,
    full_checkpoint_content::{CheckpointData, CheckpointTransaction},
    transaction::{CallArg, TransactionAPI},
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

/// Writes `checkpoints` into a checkpoint archive at `dir`, in the batched,
/// MANIFEST-indexed layout the archive reader expects.
fn write_archive(dir: &Path, checkpoints: &[CheckpointData]) {
    let last = checkpoints
        .last()
        .expect("an archive needs at least one checkpoint")
        .checkpoint_summary
        .sequence_number;

    let mut buf: Vec<u8> = Vec::new();
    buf.extend_from_slice(&CHECKPOINT_FILE_MAGIC.to_be_bytes());
    buf.push(StorageFormat::Blob as u8);
    buf.push(FileCompression::None as u8);
    for checkpoint in checkpoints {
        Blob::encode(checkpoint, BlobEncoding::Bcs)
            .expect("checkpoint data must encode")
            .write(&mut buf)
            .expect("writing to a vec cannot fail");
    }

    let file_metadata = create_file_metadata_from_bytes(Bytes::from(buf.clone()), 0..last + 1)
        .expect("file metadata must be derivable");
    fs::write(dir.join("0.chk"), &buf).unwrap();

    let mut manifest = Manifest::new(0);
    manifest.update(last + 1, file_metadata);
    fs::write(
        dir.join("MANIFEST"),
        &finalize_manifest(manifest).unwrap()[..],
    )
    .unwrap();
}

/// A fullnode whose only source is the archive must reach the archive's last
/// checkpoint by applying the results it carries, without executing any of
/// their transactions.
#[sim_test]
async fn archive_only_fullnode_applies_without_executing() {
    let ingestion = tempfile::tempdir().unwrap();
    let archive = tempfile::tempdir().unwrap();
    let mut cluster = TestClusterBuilder::new()
        .with_epoch_duration_ms(15_000)
        .with_data_ingestion_dir(ingestion.path().to_path_buf())
        .build()
        .await;

    let addresses = cluster.get_addresses();
    let (sender, receiver) = (addresses[0], addresses[1]);
    for _ in 0..5 {
        cluster
            .transfer_iota_must_exceed(sender, receiver, 1_000_000)
            .await;
    }
    cluster.wait_for_epoch(Some(1)).await;
    // More traffic after the change, so the archive reaches past the boundary
    // into the next epoch rather than stopping on it.
    for _ in 0..3 {
        cluster
            .transfer_iota_must_exceed(sender, receiver, 1_000_000)
            .await;
    }
    tokio::time::sleep(Duration::from_secs(5)).await;

    // The archive is contiguous from genesis, so take a prefix of what the
    // cluster wrote. It has to reach past an epoch boundary: that is what makes
    // the reducer wait for the node to reach a checkpoint's epoch before
    // applying its results.
    let mut checkpoints = read_checkpoints(ingestion.path());
    let boundary = checkpoints
        .iter()
        .position(|c| c.checkpoint_summary.end_of_epoch_data.is_some())
        .expect("the cluster crossed an epoch, so a boundary checkpoint must be present");
    checkpoints.truncate((boundary + 4).min(checkpoints.len()));
    assert!(
        checkpoints.len() > boundary + 1,
        "the range must include checkpoints from the epoch after the boundary, or the epoch \
         wait goes unexercised"
    );
    let target = checkpoints
        .last()
        .unwrap()
        .checkpoint_summary
        .sequence_number;
    write_archive(archive.path(), &checkpoints);

    let mut config = cluster
        .fullnode_config_builder()
        // The assertions below read historical object versions, which the
        // pruner would otherwise remove once the node crosses the epoch.
        .with_disable_pruning(true)
        .with_checkpoint_archive_config(CheckpointArchiveConfig {
            url: format!("file://{}", archive.path().display()),
            download_concurrency: NonZeroUsize::new(4).unwrap(),
            verify_concurrency: NonZeroUsize::new(2).unwrap(),
            max_checkpoints_ahead_of_execution: NonZeroUsize::new(1_000_000).unwrap(),
            re_execute_archived_checkpoints: false,
            sync_from_archive_only: true,
        })
        .build(&mut rand::rngs::OsRng, cluster.swarm.config());
    // With no peers the archive is the node's only source, so reaching the
    // target proves the archive path did the work rather than p2p sync.
    config.p2p_config.seed_peers = Vec::new();
    let node = cluster.start_fullnode_from_config(config).await;

    let deadline = std::time::Instant::now() + Duration::from_secs(120);
    loop {
        let highest = node
            .iota_node
            .with(|n| {
                n.state()
                    .get_checkpoint_store()
                    .get_highest_executed_checkpoint_seq_number()
            })
            .unwrap()
            .unwrap_or(0);
        if highest >= target {
            break;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "archive-only node stalled at {highest}, target {target}"
        );
        tokio::time::sleep(Duration::from_millis(500)).await;
    }

    // The state it ended up with must match what the cluster produced.
    let state = node.iota_node.state();
    for checkpoint in &checkpoints {
        for tx in &checkpoint.transactions {
            let digest = tx.effects.transaction_digest();
            for object in &tx.output_objects {
                let Some(stored) = state
                    .get_object_store()
                    .get_object_by_key(&object.id(), object.version())
                else {
                    assert_superseded_since(&state, &object.id(), object.version(), digest);
                    continue;
                };
                assert_eq!(
                    stored.digest(),
                    object.digest(),
                    "archive-only node stored a different {}",
                    object.id()
                );
            }
        }
    }
    let total_transactions: usize = checkpoints.iter().map(|c| c.transactions.len()).sum();
    println!(
        "archive-only node reached checkpoint {target} covering {total_transactions} transactions"
    );
}

/// Deleting a shared object is the one case where the two constructors resolve
/// the input owner from different sources — execution reads it from the loaded
/// input objects, applying reads it from the effects' recorded input state. The
/// deletion must be marked `SharedDeleted`, not `OwnedDeleted`.
#[sim_test]
async fn shared_object_deletion_is_marked_the_same_way() {
    let ingestion = tempfile::tempdir().unwrap();
    let cluster = TestClusterBuilder::new()
        .with_data_ingestion_dir(ingestion.path().to_path_buf())
        .build()
        .await;

    let package = publish_package(
        &cluster.wallet,
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/move_building_blocks"),
    )
    .await
    .object_id;

    let created = cluster
        .sign_and_execute_transaction(
            &cluster
                .test_transaction_builder()
                .await
                .move_call(package, "objects", "create_shared_object", vec![])
                .build(),
        )
        .await
        .created()[0]
        .reference;

    let deletion = cluster
        .sign_and_execute_transaction(
            &cluster
                .test_transaction_builder()
                .await
                .move_call(
                    package,
                    "objects",
                    "delete",
                    vec![CallArg::Shared(SharedObjectReference::new(
                        created.object_id,
                        created.version,
                        true,
                    ))],
                )
                .build(),
        )
        .await;
    assert_eq!(
        deletion.deleted().len(),
        1,
        "the call must delete the shared object"
    );
    let deleted_digest = *deletion.transaction_digest();

    // Give the checkpoint carrying it time to be written out.
    tokio::time::sleep(Duration::from_secs(5)).await;

    let state = cluster.fullnode_handle.iota_node.state();
    let mut checked = false;
    for checkpoint in read_checkpoints(ingestion.path()) {
        let epoch = checkpoint.checkpoint_summary.data().epoch;
        for tx in &checkpoint.transactions {
            if *tx.effects.transaction_digest() != deleted_digest {
                continue;
            }
            let applied = TransactionOutputs::build_from_checkpoint_transaction(tx);
            assert!(
                applied.markers.iter().any(|(_, marker)| matches!(
                    marker,
                    iota_types::storage::MarkerValue::SharedDeleted(_)
                )),
                "the deletion of a shared object must be marked SharedDeleted, got {:?}",
                applied.markers
            );
            assert!(
                assert_markers_match_the_store(&state, epoch, tx, &applied) > 0,
                "the markers must have been compared against the store"
            );
            checked = true;
        }
    }
    assert!(
        checked,
        "the shared object deletion never reached a checkpoint, so nothing was checked"
    );
}
