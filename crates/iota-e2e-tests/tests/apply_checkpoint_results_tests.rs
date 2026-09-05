// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Checks the applying path against traffic a real cluster produces.
//!
//! The unit tests in `iota-core` cover a single transfer. What only a cluster
//! gives is variety: shared object transactions, transactions that emit events,
//! and the end-of-epoch transaction. Those are the paths where applying a
//! checkpoint's results could diverge from executing its transactions.

use std::{
    collections::{HashMap, HashSet},
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
use iota_sdk_types::{ObjectId, SharedObjectReference, Version};
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

/// What the checkpoints in the range go on to do with each object: the newest
/// version any of them writes, and whether any of them removes it.
///
/// A version the node no longer holds is only acceptable if this says a later
/// transaction replaced or removed it — the version then leaves the live table
/// when its successor is committed, and its historic bucket expires with its
/// epoch. A version nothing touched again and that is still absent is a write
/// the node failed to make.
struct LaterHistory {
    newest_written: HashMap<ObjectId, Version>,
    removed: HashSet<ObjectId>,
}

impl LaterHistory {
    fn of(checkpoints: &[CheckpointData]) -> Self {
        let mut newest_written: HashMap<ObjectId, Version> = HashMap::new();
        let mut removed = HashSet::new();
        for checkpoint in checkpoints {
            for tx in &checkpoint.transactions {
                for object in &tx.output_objects {
                    newest_written
                        .entry(object.id())
                        .and_modify(|v| *v = (*v).max(object.version()))
                        .or_insert(object.version());
                }
                for reference in tx.removed_object_refs_post_version() {
                    removed.insert(reference.object_id);
                }
            }
        }
        Self {
            newest_written,
            removed,
        }
    }

    fn explains_absence(&self, id: &ObjectId, version: Version) -> bool {
        self.removed.contains(id)
            || self
                .newest_written
                .get(id)
                .is_some_and(|newest| *newest > version)
    }
}

/// Every object the applying path would write must match what the node stored
/// when it executed the transaction. This is the derivation checked against
/// ground truth rather than against itself.
fn assert_written_objects_match_the_store(
    state: &AuthorityState,
    tx: &CheckpointTransaction,
    applied: &TransactionOutputs,
    later: &LaterHistory,
) -> usize {
    let digest = tx.effects.transaction_digest();
    let mut compared = 0;
    for (id, object) in &applied.written {
        let Some(stored) = state
            .get_object_store()
            .get_object_by_key(id, object.version())
        else {
            assert!(
                later.explains_absence(id, object.version()),
                "transaction {digest} wrote version {} of object {id}, which the node does not \
                 have and no later checkpoint in the range replaced or removed",
                object.version()
            );
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
    let later = LaterHistory::of(&checkpoints);

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
            objects_checked += assert_written_objects_match_the_store(&state, tx, &applied, &later);
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

/// How many transactions the node's execution driver has run, read from the
/// metric it publishes rather than the private counter behind it.
fn executed_transactions(node: &test_cluster::FullNodeHandle) -> u64 {
    node.iota_node.with(|n| {
        n.registry_service()
            .gather_all()
            .iter()
            .find(|family| family.name() == "execution_driver_executed_transactions")
            .and_then(|family| family.get_metric().first())
            .map(|metric| metric.get_counter().value() as u64)
            .expect("the execution driver publishes its executed-transaction count")
    })
}

/// Bytes the node is holding in downloaded-but-not-committed checkpoint
/// results, or `None` on a node configured to re-execute, which keeps none.
fn retained_results_bytes(node: &test_cluster::FullNodeHandle) -> Option<i64> {
    node.iota_node.with(|n| {
        n.registry_service()
            .gather_all()
            .iter()
            .find(|family| family.name() == "checkpoint_results_cache_retained_bytes")
            .and_then(|family| family.get_metric().first())
            .map(|metric| metric.get_gauge().value() as i64)
    })
}

/// Syncs a fullnode whose only source is a checkpoint archive, and reports how
/// many of the archive's transactions its execution driver ran.
///
/// With `re_execute` unset the results are applied, so the driver should run
/// almost nothing; with it set every transaction goes through execution.
async fn sync_archive_only_fullnode(
    re_execute: bool,
    results_cache_size_bytes: Option<usize>,
    tamper: Option<fn(&mut [CheckpointData])>,
) -> (u64, usize, usize) {
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
    // Only what the archive serves is corrupted; the assertions below still
    // compare the node's state against the results the cluster really produced.
    if let Some(tamper) = tamper {
        let mut corrupted = checkpoints.clone();
        tamper(&mut corrupted);
        write_archive(archive.path(), &corrupted);
    } else {
        write_archive(archive.path(), &checkpoints);
    }

    let mut config = cluster
        .fullnode_config_builder()
        // The assertions below read historical object versions, which the
        // pruner would otherwise remove once the node crosses the epoch.
        .with_disable_pruning(true)
        .with_checkpoint_archive_config(CheckpointArchiveConfig {
            url: format!("file://{}", archive.path().display()),
            download_concurrency: NonZeroUsize::new(4).unwrap(),
            verify_concurrency: NonZeroUsize::new(2).unwrap(),
            re_execute_archived_checkpoints: re_execute,
            results_cache_size_bytes: results_cache_size_bytes.unwrap_or(256 * 1024 * 1024),
        })
        .build(&mut rand::rngs::OsRng, cluster.swarm.config());
    // The archive is the node's only source, so reaching the target proves the
    // archive path did the work rather than p2p sync. Emptying the seed peers
    // as well means nothing is dialled even to be declined.
    let mut state_sync = config.p2p_config.state_sync.clone().unwrap_or_default();
    state_sync.sync_from_archive_only = Some(true);
    config.p2p_config.state_sync = Some(state_sync);
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
    let later = LaterHistory::of(&checkpoints);
    for checkpoint in &checkpoints {
        for tx in &checkpoint.transactions {
            let digest = tx.effects.transaction_digest();
            for object in &tx.output_objects {
                let Some(stored) = state
                    .get_object_store()
                    .get_object_by_key(&object.id(), object.version())
                else {
                    assert!(
                        later.explains_absence(&object.id(), object.version()),
                        "transaction {digest} wrote version {} of object {}, which the \
                         archive-only node does not have and no later checkpoint in the range \
                         replaced or removed",
                        object.version(),
                        object.id()
                    );
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
    let boundaries = checkpoints
        .iter()
        .filter(|c| c.checkpoint_summary.end_of_epoch_data.is_some())
        .count();
    // Every result is either committed or dropped as the executor passes it,
    // so a node that has caught up holds none.
    if let Some(retained) = retained_results_bytes(&node) {
        assert_eq!(
            retained, 0,
            "checkpoint results were still held after reaching the archive's last checkpoint"
        );
    }
    let executed = executed_transactions(&node);
    println!(
        "archive-only node (re_execute={re_execute}) reached checkpoint {target}: executed \
         {executed} of {total_transactions} transactions, {boundaries} epoch boundaries"
    );
    (executed, total_transactions, boundaries)
}

/// Applying the archive's results must leave the execution driver with almost
/// nothing to do: only the end-of-epoch transactions stay on its path.
#[sim_test]
async fn archive_only_fullnode_applies_without_executing() {
    let (executed, total, boundaries) = sync_archive_only_fullnode(false, None, None).await;
    // Only the end-of-epoch transactions are left to the executor, one per
    // boundary in the range.
    assert!(
        executed <= boundaries as u64,
        "the node executed {executed} of {total} transactions with only {boundaries} epoch \
         boundaries in range, so more than reconfiguration went through execution"
    );
}

/// The same range with re-execution configured must put every transaction
/// through the execution driver. Together with the test above this is what
/// shows the flag has an effect rather than being inert.
#[sim_test]
async fn archive_only_fullnode_re_executes_when_configured() {
    let (executed, total, _) = sync_archive_only_fullnode(true, None, None).await;
    // Genesis is already executed when the node starts, so it never reaches the
    // driver; everything else in the range must.
    assert!(
        executed + 2 >= total as u64,
        "the node executed only {executed} of {total} transactions, so it did not re-execute \
         the archive's contents"
    );
}

/// An archive whose payloads do not match the effects must not have its
/// results committed. The check happens where the results are cached, so this
/// is what shows a tampered payload still reaches the executor as an ordinary
/// checkpoint rather than being written unchecked.
#[sim_test]
async fn tampered_archive_payloads_are_executed_instead_of_committed() {
    let (executed, total, boundaries) = sync_archive_only_fullnode(
        false,
        None,
        Some(|checkpoints: &mut [CheckpointData]| {
            // Rewrite one output object so its contents no longer hash to the
            // digest its effects record.
            let checkpoint = checkpoints
                .iter_mut()
                .find(|c| {
                    c.checkpoint_summary.sequence_number > 0
                        && c.transactions.iter().any(|t| !t.output_objects.is_empty())
                })
                .expect("the archive carries a transaction that wrote an object");
            let transaction = checkpoint
                .transactions
                .iter_mut()
                .find(|t| !t.output_objects.is_empty())
                .unwrap();
            let mut object = transaction.output_objects[0].as_inner().clone();
            object.storage_rebate += 1;
            transaction.output_objects[0] = object.into();
        }),
    )
    .await;

    // The tampered checkpoint's transactions go through execution, so more
    // runs than the end-of-epoch transactions alone.
    assert!(
        executed > boundaries as u64,
        "the node executed {executed} of {total} transactions with {boundaries} epoch \
         boundaries in range, so the tampered checkpoint was committed rather than executed"
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

/// A budget too small to hold every checkpoint's results forces some to be
/// committed from the archive and the rest executed, interleaved.
///
/// That mix is what a node upgrading into this feature sees, and it is the
/// case that used to crash: results written when state sync downloaded them
/// raced the executor's writes for older checkpoints, and object versions
/// reached the writeback cache out of order. Committing them in the executor's
/// ordered stage is what makes one writer of them again.
#[sim_test]
async fn interleaved_committed_and_executed_checkpoints_stay_ordered() {
    // Enough for a couple of checkpoints, so most results are dropped.
    let (executed, total, boundaries) =
        sync_archive_only_fullnode(false, Some(64 * 1024), None).await;

    assert!(
        executed > boundaries as u64,
        "the budget must have been too small to commit everything, or this proves nothing: \
         executed {executed} of {total} with {boundaries} boundaries"
    );
    assert!(
        executed < total as u64,
        "some results must still have been committed: executed {executed} of {total}"
    );
}
