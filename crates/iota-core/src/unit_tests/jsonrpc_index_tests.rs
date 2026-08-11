// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_sdk_types::{
    Address, ObjectDigest, ObjectId, Owner, StructTag, TransactionDigest, Version,
};
use iota_types::{
    base_types::{ObjectInfo, ObjectType},
    effects::TransactionEffectsAPI,
    gas_coin::GAS,
    messages_checkpoint::CheckpointContentsExt,
    test_checkpoint_data_builder::TestCheckpointDataBuilder,
};
use prometheus_filtered::Registry;
use typed_store::Map;

use super::{IndexStore, history_cf_name};
use crate::{checkpoints::CheckpointStore, test_utils::executed_checkpoint};

/// Opens an `IndexStore` at `path` without running the rebuild path.
fn open_index_store(path: std::path::PathBuf) -> IndexStore {
    IndexStore::new_without_init(path, &Registry::default(), Some(128))
}

/// Closes the store's database, waiting until every handle is released
/// so the same path can be reopened. Accepts the store owned or in an
/// `Arc`, as long as the passed handle is the last one.
async fn close_index_store(index_store: impl std::borrow::Borrow<IndexStore>) {
    let weak_db = std::sync::Arc::downgrade(&index_store.borrow().tables.meta.db);
    drop(index_store);
    assert!(super::wait_for_database_close(weak_db).await);
}

/// Closes the store and reopens the same path, as a restart does.
async fn reopen_index_store(index_store: IndexStore, path: std::path::PathBuf) -> IndexStore {
    close_index_store(index_store).await;
    open_index_store(path)
}

/// An empty authority store under `dir`, for driving the rebuild and
/// backfill paths.
fn open_authority_store(dir: &std::path::Path) -> std::sync::Arc<super::AuthorityStore> {
    crate::authority::AuthorityStore::open_no_genesis(
        std::sync::Arc::new(
            crate::authority::authority_store_tables::AuthorityPerpetualTables::open(dir, None),
        ),
        false,
        &Registry::default(),
    )
    .unwrap()
}

/// An authority state whose genesis checkpoint is executed, plus the
/// genesis transaction's digest.
async fn genesis_authority_state() -> (
    std::sync::Arc<crate::authority::AuthorityState>,
    TransactionDigest,
) {
    let authority_state = crate::authority::test_authority_builder::TestAuthorityBuilder::new()
        .insert_genesis_checkpoint()
        .build()
        .await;
    let checkpoint_store = &authority_state.checkpoint_store;
    let genesis_checkpoint = checkpoint_store
        .get_checkpoint_by_sequence_number(0)
        .unwrap()
        .unwrap();
    checkpoint_store
        .update_highest_executed_checkpoint(&genesis_checkpoint)
        .unwrap();
    let genesis_contents = checkpoint_store
        .get_checkpoint_contents(&genesis_checkpoint.contents_digest)
        .unwrap()
        .unwrap();
    let genesis_tx_digest = genesis_contents.iter().next().unwrap().transaction;
    (authority_state, genesis_tx_digest)
}

fn mark_checkpoint_executed(checkpoint_store: &CheckpointStore, sequence_number: u64) {
    let checkpoint = executed_checkpoint(0, sequence_number);
    checkpoint_store
        .insert_verified_checkpoint(&checkpoint)
        .unwrap();
    checkpoint_store
        .update_highest_executed_checkpoint(&checkpoint)
        .unwrap();
}

/// Seeds `epochs` history buckets with one transaction each.
fn seed_history_buckets(index_store: &IndexStore, epochs: u64) {
    for epoch in 0..epochs {
        let bucket = index_store.ensure_history_bucket(epoch).unwrap();
        let mut batch = index_store.tables.meta.batch();
        batch
            .insert_batch_tagged(&bucket.tx_order, [(epoch, TransactionDigest::random())])
            .unwrap();
        batch.write().unwrap();
    }
}

/// A query that snapshotted the history buckets before a `prune` must
/// report an error for the dropped epoch's rows, as [`IndexStore::prune`]
/// documents, rather than panicking.
#[tokio::test]
async fn test_prune_racing_a_reader_reports_an_error() {
    let tmp_dir = iota_common::tempdir();
    let index_store = open_index_store(tmp_dir.path().to_path_buf());
    seed_history_buckets(&index_store, 2);

    // Every digest probe and range scan reads through such a snapshot.
    let snapshot = index_store.history_buckets(false);
    assert_eq!(snapshot.len(), 2);

    assert_eq!(index_store.prune(1).unwrap(), Some(1));

    assert!(
        snapshot[0]
            .tx_order
            .safe_range_iter(..)
            .next()
            .expect("the scan must yield an error item")
            .is_err()
    );
    assert!(
        snapshot[0]
            .tx_order
            .safe_range_iter_reversed(..)
            .next()
            .expect("the reverse scan must yield an error item")
            .is_err()
    );
    assert!(snapshot[0].txs_seq.get(&Default::default()).is_err());

    // The retained bucket keeps serving, and a retry no longer sees the
    // dropped one.
    assert!(
        snapshot[1]
            .tx_order
            .safe_range_iter(..)
            .next()
            .expect("the retained bucket must still yield a row")
            .is_ok()
    );
    assert_eq!(index_store.history_buckets(false).len(), 1);
    assert_eq!(
        index_store
            .get_transactions(None, None, None, false)
            .unwrap()
            .len(),
        1
    );
}

/// RocksDB unregisters a column family before it attempts the drop, so a
/// bucket whose drop failed can neither be read nor dropped again:
/// `prune` must let it go instead of leaving it for a retry that would
/// fail every query walking it.
#[tokio::test]
async fn test_a_failed_drop_still_removes_the_bucket() {
    let tmp_dir = iota_common::tempdir();
    let index_store = open_index_store(tmp_dir.path().to_path_buf());
    seed_history_buckets(&index_store, 2);

    // Makes the pruner's own drop fail: the column family is already gone.
    index_store
        .database_for_testing()
        .drop_cf(&history_cf_name(0))
        .unwrap();

    assert_eq!(index_store.prune(1).unwrap(), Some(1));
    assert_eq!(index_store.history_buckets(false).len(), 1);
    assert_eq!(
        index_store
            .get_transactions(None, None, None, false)
            .unwrap()
            .len(),
        1
    );
    assert!(index_store.ensure_history_bucket(0).is_err());
}

/// A failed drop leaves the column family on disk while its bucket is
/// already unreadable, so the next open must drop it instead of serving
/// the pruned epoch again.
#[tokio::test]
async fn test_a_bucket_below_the_floor_is_dropped_at_open() {
    let tmp_dir = iota_common::tempdir();
    let index_store = open_index_store(tmp_dir.path().to_path_buf());
    seed_history_buckets(&index_store, 2);
    assert_eq!(index_store.prune(1).unwrap(), Some(1));

    // Stands in for a drop that failed: the column family is on disk
    // below the persisted floor.
    index_store
        .database_for_testing()
        .create_cf(
            &history_cf_name(0),
            &typed_store::rocksdb::Options::default(),
        )
        .unwrap();

    let index_store = reopen_index_store(index_store, tmp_dir.path().to_path_buf()).await;
    assert_eq!(index_store.history_buckets(false).len(), 1);
    assert!(index_store.ensure_history_bucket(0).is_err());
    assert_eq!(
        index_store
            .get_transactions(None, None, None, false)
            .unwrap()
            .len(),
        1
    );

    // The drop must reach the disk, not just the bucket map.
    close_index_store(index_store).await;
    assert!(
        !typed_store::rocks::list_tables(tmp_dir.path().to_path_buf())
            .unwrap()
            .contains(&history_cf_name(0))
    );
}

/// A retention floor that cannot be read fails the open, which a restart
/// retries: the database itself is intact, so it must not reach the
/// wipe-and-rebuild path `IndexStore::new` takes for an unopenable one.
#[tokio::test]
async fn test_a_failed_floor_read_fails_the_open() {
    let tmp_dir = iota_common::tempdir();
    let opened = IndexStore::open_index_db(&tmp_dir.path().join("indexes")).unwrap();

    // Makes the floor read fail: RocksDB unregisters the column family.
    opened.db.drop_cf("earliest_retained_epoch").unwrap();

    assert!(
        IndexStore::finish_open(
            opened,
            &Registry::default(),
            Some(128),
            0,
            Default::default(),
            None,
        )
        .is_err()
    );
}

/// Queries running concurrently with repeated pruning must never panic:
/// readers hold bucket handles across the pruner's column-family drops.
#[tokio::test]
async fn test_concurrent_prune_and_queries_never_panic() {
    use std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    };

    const EPOCHS: u64 = 64;

    let tmp_dir = iota_common::tempdir();
    let index_store = Arc::new(open_index_store(tmp_dir.path().to_path_buf()));
    seed_history_buckets(&index_store, EPOCHS);

    let stop = Arc::new(AtomicBool::new(false));
    let mut workers: Vec<_> = (0..3)
        .map(|_| {
            let index_store = index_store.clone();
            let stop = stop.clone();
            // Blocking threads, so the reads race the drops instead of
            // interleaving at await points.
            std::thread::spawn(move || {
                while !stop.load(Ordering::Relaxed) {
                    let _ = index_store.get_transactions(None, None, Some(1000), false);
                    let _ = index_store.get_transaction_seq(&Default::default());
                }
            })
        })
        .collect();
    workers.push({
        let index_store = index_store.clone();
        let stop = stop.clone();
        // Recreates low epochs like a backfill would, racing the drops.
        // Opening a bucket spawns metrics sampling tasks, so the thread
        // needs the runtime context the real backfill gets from
        // `spawn_blocking`.
        let runtime = tokio::runtime::Handle::current();
        std::thread::spawn(move || {
            let _guard = runtime.enter();
            let mut round = 0;
            while !stop.load(Ordering::Relaxed) {
                let _ = index_store.ensure_history_bucket(round % 8);
                round += 1;
            }
        })
    });

    for retained in (1..EPOCHS).rev() {
        index_store.prune(retained).unwrap();
    }
    stop.store(true, Ordering::Relaxed);
    for worker in workers {
        worker.join().expect("a worker thread panicked");
    }

    // No bucket left in the map may point at a dropped column family.
    for bucket in index_store.history_buckets(false) {
        bucket
            .txs_seq
            .get(&Default::default())
            .expect("every bucket in the map must be readable");
    }

    assert_eq!(
        index_store
            .get_transactions(None, None, None, false)
            .unwrap()
            .len(),
        1
    );
}

/// A pruned epoch's bucket must never be recreated:
/// `ensure_history_bucket` refuses epochs below the earliest retained
/// one, in this process and, because it is persisted, after a reopen.
#[tokio::test]
async fn test_pruned_epochs_are_not_recreated() {
    let tmp_dir = iota_common::tempdir();
    let index_store = open_index_store(tmp_dir.path().to_path_buf());
    seed_history_buckets(&index_store, 2);
    assert_eq!(index_store.prune(1).unwrap(), Some(1));
    assert!(index_store.ensure_history_bucket(0).is_err());
    assert!(index_store.ensure_history_bucket(1).is_ok());

    let index_store = reopen_index_store(index_store, tmp_dir.path().to_path_buf()).await;
    assert!(index_store.ensure_history_bucket(0).is_err());
    assert!(index_store.ensure_history_bucket(1).is_ok());
}

/// Raising `num_epochs_to_retain_for_indexes` across a restart must not
/// move the earliest retained epoch back down: the buckets below it are
/// already gone, so recreating them would contradict the errors queries
/// were given.
#[tokio::test]
async fn test_the_earliest_retained_epoch_never_moves_backwards() {
    let tmp_dir = iota_common::tempdir();
    let index_store = open_index_store(tmp_dir.path().to_path_buf());
    seed_history_buckets(&index_store, 4);
    assert_eq!(index_store.prune(2).unwrap(), Some(2));

    let index_store = reopen_index_store(index_store, tmp_dir.path().to_path_buf()).await;
    assert_eq!(
        index_store.prune(52).unwrap(),
        Some(2),
        "a retention reaching below the dropped epochs must not lower the floor"
    );
    assert!(index_store.ensure_history_bucket(1).is_err());
    assert!(index_store.ensure_history_bucket(2).is_ok());
}

/// The store pruner deletes a checkpoint's transactions before it
/// advances the watermark the backfill checks, so a replay can find them
/// already gone. That must end the backfill instead of failing the task
/// for the rest of the process.
#[tokio::test]
async fn test_backfill_stops_at_deleted_checkpoint_data() {
    let (authority_state, genesis_tx_digest) = genesis_authority_state().await;
    let checkpoint_store = &authority_state.checkpoint_store;
    let authority_store = authority_state.database_for_testing();
    authority_store
        .perpetual_tables
        .transactions
        .remove(&genesis_tx_digest)
        .unwrap();

    let index_dir = iota_common::tempdir();
    let index_store = open_index_store(index_dir.path().to_path_buf());
    index_store
        .tables
        .history_watermark
        .insert(&(), &1)
        .unwrap();

    index_store
        .backfill_history(&authority_store, checkpoint_store)
        .expect("deleted checkpoint data must stop the backfill, not fail it");
    assert_eq!(
        index_store.tables.history_watermark.get(&()).unwrap(),
        Some(1),
        "the checkpoint whose data is gone must not be marked as replayed"
    );
    assert_eq!(
        index_store
            .metrics
            .history_backfill_lowest_replayed_checkpoint
            .get(),
        1,
        "the gauge must report where a backfill that stopped early left off"
    );
}

/// The backfill must stop at epochs `prune` removed from the index
/// instead of replaying them. The pruned epoch's genesis checkpoint is
/// fully replayable, so only the stop keeps the marker in place.
#[tokio::test]
async fn test_backfill_stops_at_pruned_epochs() {
    let (authority_state, _) = genesis_authority_state().await;
    let checkpoint_store = &authority_state.checkpoint_store;

    let index_dir = iota_common::tempdir();
    let index_store = open_index_store(index_dir.path().to_path_buf());
    seed_history_buckets(&index_store, 2);
    assert_eq!(index_store.prune(1).unwrap(), Some(1));
    index_store
        .tables
        .history_watermark
        .insert(&(), &1)
        .unwrap();

    index_store
        .backfill_history(&authority_state.database_for_testing(), checkpoint_store)
        .expect("the backfill must stop at the pruned epoch, not replay it");
    assert_eq!(
        index_store.tables.history_watermark.get(&()).unwrap(),
        Some(1),
        "the pruned genesis epoch must not be replayed"
    );
}

/// With index pruning configured, the backfill must stop at the
/// retention horizon even before the first pruning pass persists the
/// `earliest_retained_epoch` floor: replaying below it would index
/// epochs that pass drops again. The genesis checkpoint here is fully
/// replayable, so only the stop keeps the marker in place.
#[tokio::test]
async fn test_backfill_stops_at_the_retention_horizon() {
    let (authority_state, _) = genesis_authority_state().await;
    let checkpoint_store = &authority_state.checkpoint_store;

    let index_dir = iota_common::tempdir();
    let mut index_store = open_index_store(index_dir.path().to_path_buf());
    index_store.epochs_to_retain = Some(7);
    // Buckets for epochs 0..=7: genesis' epoch 0 lies below the
    // retention horizon (epoch 1), while no pruning has run yet.
    seed_history_buckets(&index_store, 8);
    assert_eq!(index_store.history.earliest_retained(), 0);
    index_store
        .tables
        .history_watermark
        .insert(&(), &1)
        .unwrap();

    index_store
        .backfill_history(&authority_state.database_for_testing(), checkpoint_store)
        .expect("the backfill must stop at the retention horizon, not replay past it");
    assert_eq!(
        index_store.tables.history_watermark.get(&()).unwrap(),
        Some(1),
        "an epoch the next pruning pass would drop must not be replayed"
    );
}

/// `init` alone must not adopt the rebuild: the watermarks are written
/// by the caller only after the WAL-less bulk writes are flushed, so a
/// crash mid-rebuild is re-detected on the next open instead of being
/// adopted with lost data.
#[tokio::test]
async fn test_rebuild_is_not_adopted_before_the_flush() {
    let dir = iota_common::tempdir();
    let checkpoint_store = CheckpointStore::new(&dir.path().join("checkpoints"));
    mark_checkpoint_executed(&checkpoint_store, 5);
    let authority_store = open_authority_store(&dir.path().join("store"));

    let mut tables =
        super::IndexStoreTables::open_for_bulk_ingestion(dir.path().join("indexes"), 1);
    tables
        .init(
            &authority_store,
            &checkpoint_store,
            1 << 20,
            &Default::default(),
        )
        .unwrap();
    assert_eq!(tables.watermark.get(&()).unwrap(), None);
    assert_eq!(tables.history_watermark.get(&()).unwrap(), None);
    assert!(
        tables
            .needs_to_do_initialization(&checkpoint_store)
            .unwrap(),
        "a store whose rebuild was not adopted must be wiped and rebuilt on the next open"
    );
}

/// A rebuild on a node with no executed checkpoints must not write a
/// watermark: an absent watermark already means "nothing indexed", and
/// writing checkpoint 0 would shift the numbering anchor past the
/// genesis transaction.
#[tokio::test]
async fn test_rebuild_with_nothing_executed_writes_no_watermark() {
    let dir = iota_common::tempdir();
    let checkpoint_store = CheckpointStore::new(&dir.path().join("checkpoints"));
    let index_dir = dir.path().join("indexes");

    // A pre-upgrade database (data but no `meta` row) triggers the wipe
    // and rebuild even though nothing is executed yet.
    {
        let index_store = open_index_store(index_dir.clone());
        let owner = iota_types::base_types::dbg_addr(1);
        let object =
            iota_types::object::Object::with_id_owner_for_testing(ObjectId::random(), owner);
        index_store
            .tables
            .owner_index
            .insert(
                &(owner, object.id()),
                &iota_types::base_types::ObjectInfo::from_object(&object),
            )
            .unwrap();
        close_index_store(index_store).await;
    }

    let authority_store = open_authority_store(&dir.path().join("store"));
    let index_store = IndexStore::new(
        index_dir,
        &Registry::default(),
        Some(128),
        None,
        &authority_store,
        &checkpoint_store,
        Default::default(),
    )
    .await
    .unwrap();

    assert_eq!(
        index_store.tables.history_watermark.get(&()).unwrap(),
        Some(0),
        "the rebuild must have run and seeded the backfill marker"
    );
    assert_eq!(index_store.tables.watermark.get(&()).unwrap(), None);
    assert_eq!(
        index_store.next_sequence_number(),
        0,
        "the genesis transaction must later be numbered 0"
    );
}

/// `CoinInfo::from_object` must reject non-coin objects even when their
/// BCS contents happen to match `Coin`'s `{UID, u64}` layout.
#[test]
fn test_coin_info_from_object_requires_coin_type() {
    use iota_sdk_types::{Address, MoveStruct, Owner, TransactionDigest, Version};
    use iota_types::object::{MoveStructExt, Object};

    let owner = Owner::Address(Address::ZERO);
    let id = ObjectId::random();
    let contents = iota_types::coin::Coin::new(id, 42).to_bcs_bytes();

    let coin = Object::new_move(
        MoveStruct::new_coin(GAS::type_tag(), Version::MIN_VALID_INCL, id, 42),
        owner,
        TransactionDigest::ZERO,
    );
    assert_eq!(super::CoinInfo::from_object(&coin).unwrap().balance, 42);

    let fake = Object::new_move(
        MoveStruct::new_from_execution_with_limit(
            "0x2::not_coin::NotCoin".parse::<StructTag>().unwrap(),
            Version::MIN_VALID_INCL,
            contents,
            256,
        )
        .unwrap(),
        owner,
        TransactionDigest::ZERO,
    );
    assert_eq!(super::CoinInfo::from_object(&fake), None);
}

/// When a store must be wiped and rebuilt, as one decision table: a
/// pre-upgrade database (data, no `meta` row) is never seeded and always
/// rebuilt; a brand-new store needs no rebuild until the executed
/// watermark passes the indexed one; a store holding data but no
/// watermark is always rebuilt; a watermark at or ahead of the executed
/// checkpoint (crash between index commit and executed bump) needs none;
/// a schema version bump always does.
#[tokio::test]
async fn test_needs_to_do_initialization_cases() {
    let tmp_dir = iota_common::tempdir();
    let cp_dir = iota_common::tempdir();
    let checkpoint_store = CheckpointStore::new(&cp_dir.path().join("checkpoints"));
    let index_store = open_index_store(tmp_dir.path().to_path_buf());

    // A database from before per-checkpoint indexing must stay unseeded:
    // nodes restored from a formal snapshot wrote a corrupted owner
    // index into it, and without a watermark it cannot prove otherwise.
    let owner = iota_types::base_types::dbg_addr(1);
    let object = iota_types::object::Object::with_id_owner_for_testing(ObjectId::random(), owner);
    index_store
        .tables
        .owner_index
        .insert(
            &(owner, object.id()),
            &iota_types::base_types::ObjectInfo::from_object(&object),
        )
        .unwrap();
    index_store.tables.seed_meta().unwrap();
    assert_eq!(
        index_store.tables.meta.get(&()).unwrap(),
        None,
        "a database with data but no `meta` row must not be seeded"
    );
    assert!(
        index_store
            .tables
            .needs_to_do_initialization(&checkpoint_store)
            .unwrap(),
        "a database from before per-checkpoint indexing must be rebuilt"
    );

    index_store
        .tables
        .owner_index
        .remove(&(owner, object.id()))
        .unwrap();
    index_store.tables.seed_meta().unwrap();
    assert!(
        !index_store
            .tables
            .needs_to_do_initialization(&checkpoint_store)
            .unwrap(),
        "a brand-new store on a node with no executed checkpoints needs no rebuild"
    );

    // A rebuild or restore that crashed before writing the watermark
    // leaves data behind; with nothing executed, comparing the
    // watermarks alone would adopt it.
    index_store
        .tables
        .owner_index
        .insert(
            &(owner, object.id()),
            &iota_types::base_types::ObjectInfo::from_object(&object),
        )
        .unwrap();
    assert!(
        index_store
            .tables
            .needs_to_do_initialization(&checkpoint_store)
            .unwrap(),
        "a store holding data but no watermark must be rebuilt"
    );
    index_store
        .tables
        .owner_index
        .remove(&(owner, object.id()))
        .unwrap();

    mark_checkpoint_executed(&checkpoint_store, 5);
    assert!(
        index_store
            .tables
            .needs_to_do_initialization(&checkpoint_store)
            .unwrap(),
        "an executed checkpoint past the indexed watermark must trigger a rebuild"
    );

    index_store.tables.watermark.insert(&(), &5).unwrap();
    assert!(
        !index_store
            .tables
            .needs_to_do_initialization(&checkpoint_store)
            .unwrap()
    );

    // A watermark ahead of the executed checkpoint still needs the
    // checkpoint it anchors to.
    checkpoint_store
        .insert_verified_checkpoint(&executed_checkpoint(0, 6))
        .unwrap();
    index_store.tables.watermark.insert(&(), &6).unwrap();
    assert!(
        !index_store
            .tables
            .needs_to_do_initialization(&checkpoint_store)
            .unwrap(),
        "an index watermark ahead of the executed watermark must not trigger a rebuild"
    );

    // A schema version bump also triggers a rebuild.
    index_store
        .tables
        .meta
        .insert(
            &(),
            &super::MetadataInfo {
                version: super::CURRENT_DB_VERSION + 1,
            },
        )
        .unwrap();
    assert!(
        index_store
            .tables
            .needs_to_do_initialization(&checkpoint_store)
            .unwrap()
    );
}

/// The JSON-RPC index database of releases that stored it under
/// `indexes` is removed; its content cannot be adopted anyway.
#[test]
fn test_remove_legacy_jsonrpc_indexes_dir() {
    let db_path = iota_common::tempdir();
    let legacy_dir = db_path.path().join("indexes");
    std::fs::create_dir(&legacy_dir).unwrap();
    std::fs::write(legacy_dir.join("CURRENT"), b"stale").unwrap();

    super::remove_legacy_jsonrpc_indexes_dir(db_path.path()).unwrap();
    assert!(!legacy_dir.exists());

    // A second call is a no-op.
    super::remove_legacy_jsonrpc_indexes_dir(db_path.path()).unwrap();
}

/// After a rebuild, the history tables are filled by a background replay
/// that works downwards from the watermark and records its progress
/// atomically with each checkpoint's rows, so an interrupted replay
/// resumes where it stopped instead of starting over.
#[tokio::test]
async fn test_history_backfill_after_rebuild() {
    let (authority_state, genesis_tx_digest) = genesis_authority_state().await;
    let checkpoint_store = &authority_state.checkpoint_store;
    let genesis_checkpoint = checkpoint_store
        .get_checkpoint_by_sequence_number(0)
        .unwrap()
        .unwrap();

    let index_dir = iota_common::tempdir();
    let index_store = IndexStore::new(
        index_dir.path().to_path_buf(),
        &Registry::default(),
        Some(128),
        None,
        &authority_state.database_for_testing(),
        checkpoint_store,
        Default::default(),
    )
    .await
    .unwrap();
    index_store.wait_for_history_backfill_for_testing().await;

    assert_eq!(
        index_store.get_transaction_seq(&genesis_tx_digest).unwrap(),
        Some(0)
    );
    assert_eq!(
        index_store.tables.history_watermark.get(&()).unwrap(),
        Some(0),
        "the backfill must have reached the lowest replayable checkpoint"
    );
    // The two numbering schemes meet: the backfill numbered the replayed
    // transactions by network position, and the live counter continues
    // exactly one past them — which is also the reported total.
    assert_eq!(
        index_store.next_sequence_number(),
        genesis_checkpoint.network_total_transactions
    );

    // Simulate a replay interrupted before reaching checkpoint 0:
    // resuming replays it and lowers the marker again.
    index_store
        .tables
        .history_watermark
        .insert(&(), &1)
        .unwrap();
    index_store
        .backfill_history(&authority_state.database_for_testing(), checkpoint_store)
        .unwrap();
    assert_eq!(
        index_store.tables.history_watermark.get(&()).unwrap(),
        Some(0)
    );
    assert_eq!(
        index_store.get_transaction_seq(&genesis_tx_digest).unwrap(),
        Some(0)
    );
    assert_eq!(
        index_store
            .metrics
            .history_backfill_lowest_replayed_checkpoint
            .get(),
        0,
        "the gauge must report how far down the replay got"
    );
}

/// A formal-snapshot restore builds the JSON-RPC index from the restored
/// live object set (`JsonRpcIndexRestorer`); a node then opens it in
/// place instead of rebuilding, and the history backfill has nothing to
/// do. Dynamic fields are indexed by key only, so the tee needs no
/// layouts and no particular object order.
#[tokio::test]
async fn test_restore_built_store_is_adopted_on_open() {
    use iota_sdk_types::{MoveStruct, Owner, TransactionDigest, Version};
    use iota_types::{
        base_types::dbg_addr,
        object::{MoveStructExt, Object},
    };

    let dir = iota_common::tempdir();
    let checkpoint_store = CheckpointStore::new(&dir.path().join("checkpoints"));
    // The restore marks the restore checkpoint both executed and pruned.
    let restore_checkpoint = executed_checkpoint(0, 5);
    checkpoint_store
        .insert_verified_checkpoint(&restore_checkpoint)
        .unwrap();
    checkpoint_store
        .update_highest_executed_checkpoint(&restore_checkpoint)
        .unwrap();
    checkpoint_store
        .update_highest_pruned_checkpoint(&restore_checkpoint)
        .unwrap();

    let owner = dbg_addr(1);
    let gas_object = Object::new_gas_with_balance_and_owner_for_testing(100, owner);
    let parent = ObjectId::random();
    let field_id = ObjectId::random();
    let mut field_contents = field_id.into_bytes().to_vec();
    field_contents.extend_from_slice(&7u64.to_le_bytes()); // name
    field_contents.extend_from_slice(&8u64.to_le_bytes()); // value
    let field_object = Object::new_move(
        MoveStruct::new_from_execution_with_limit(
            "0x2::dynamic_field::Field<u64,u64>"
                .parse::<StructTag>()
                .unwrap(),
            Version::MIN_VALID_INCL,
            field_contents,
            256,
        )
        .unwrap(),
        Owner::Object(parent),
        TransactionDigest::ZERO,
    );

    // Tee the objects into the restorer, as the snapshot's partition
    // downloads do.
    let index_dir = dir.path().join(super::JSONRPC_INDEXES_DIR);
    let restorer = super::JsonRpcIndexRestorer::open(index_dir.clone()).unwrap();
    let mut partition = restorer.partition_indexer();
    partition.index_object(&gas_object).unwrap();
    partition.index_object(&field_object).unwrap();
    partition.finish().unwrap();
    restorer.finalize(5).await.unwrap();

    // Plant a sentinel row: if it survives the open below, the store was
    // adopted rather than wiped and rebuilt into equal-looking data.
    let sentinel = (ObjectId::random(), ObjectId::random());
    {
        let built = open_index_store(index_dir.clone());
        assert!(
            !built
                .tables
                .needs_to_do_initialization(&checkpoint_store)
                .unwrap(),
            "a restore-built store must need no rebuild"
        );
        built
            .tables
            .dynamic_field_index
            .insert(&sentinel, &())
            .unwrap();
        close_index_store(built).await;
    }

    let authority_store = open_authority_store(&dir.path().join("store"));
    let index_store = IndexStore::new(
        index_dir,
        &Registry::default(),
        Some(128),
        None,
        &authority_store,
        &checkpoint_store,
        Default::default(),
    )
    .await
    .unwrap();
    index_store.wait_for_history_backfill_for_testing().await;

    assert!(
        index_store
            .dynamic_field_exists(sentinel.0, sentinel.1)
            .unwrap(),
        "the restored database must be opened in place, not rebuilt"
    );

    // The owner and coin tables were built from the teed objects.
    let owned: Vec<_> = index_store
        .get_owner_objects(owner, None, 10, None)
        .unwrap();
    assert_eq!(owned.len(), 1);
    assert_eq!(owned[0].object_id, gas_object.id());
    let balance = index_store
        .get_balance(owner, iota_types::gas_coin::GAS::type_tag())
        .unwrap();
    assert_eq!(balance.num_coins, 1);

    // The dynamic field was indexed by key, without layout resolution.
    let field_ids: Vec<_> = index_store
        .get_dynamic_field_ids_iterator(parent, None)
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(field_ids, vec![field_id]);

    // Watermark at the restore checkpoint, history one past it — nothing
    // for the backfill to replay.
    assert_eq!(index_store.tables.watermark.get(&()).unwrap(), Some(5));
    assert_eq!(
        index_store.tables.history_watermark.get(&()).unwrap(),
        Some(6)
    );
}

/// A stale database (here: written by another schema version) is wiped
/// and rebuilt through the full open path — bulk-ingestion open, flush,
/// reopen with default options — and none of its rows survive.
#[tokio::test]
async fn test_stale_database_is_wiped_and_rebuilt_on_open() {
    let (authority_state, genesis_tx_digest) = genesis_authority_state().await;
    let checkpoint_store = &authority_state.checkpoint_store;

    let index_dir = iota_common::tempdir();
    let index_store = IndexStore::new(
        index_dir.path().to_path_buf(),
        &Registry::default(),
        Some(128),
        None,
        &authority_state.database_for_testing(),
        checkpoint_store,
        Default::default(),
    )
    .await
    .unwrap();
    index_store.wait_for_history_backfill_for_testing().await;

    // Poison the store and mark it as written by another schema version.
    let poison_field = (ObjectId::random(), ObjectId::random());
    index_store
        .tables
        .dynamic_field_index
        .insert(&poison_field, &())
        .unwrap();
    index_store
        .tables
        .meta
        .insert(
            &(),
            &super::MetadataInfo {
                version: super::CURRENT_DB_VERSION + 1,
            },
        )
        .unwrap();

    // Release the database before reopening the same path.
    close_index_store(index_store).await;

    let index_store = IndexStore::new(
        index_dir.path().to_path_buf(),
        &Registry::default(),
        Some(128),
        None,
        &authority_state.database_for_testing(),
        checkpoint_store,
        Default::default(),
    )
    .await
    .unwrap();
    index_store.wait_for_history_backfill_for_testing().await;

    assert!(
        !index_store
            .dynamic_field_exists(poison_field.0, poison_field.1)
            .unwrap(),
        "stale rows must not survive the rebuild"
    );
    assert_eq!(
        index_store.get_transaction_seq(&genesis_tx_digest).unwrap(),
        Some(0)
    );
    assert_eq!(
        index_store
            .tables
            .meta
            .get(&())
            .unwrap()
            .map(|meta| meta.version),
        Some(super::CURRENT_DB_VERSION)
    );
    assert_eq!(index_store.tables.watermark.get(&()).unwrap(), Some(0));
    assert_eq!(
        index_store.tables.history_watermark.get(&()).unwrap(),
        Some(0)
    );
}

#[tokio::test]
async fn test_index_cache() -> anyhow::Result<()> {
    // This test indexes a checkpoint where 10 coins each with balance 100
    // are created for an address. The balance is then going to be read
    // from the db and the cache. It should be 1000. Then, a second
    // checkpoint deletes 3 of those coins, and the balance should be 700,
    // verified from both db and cache. This tests make sure we are
    // invalidating entries in the cache and always reading latest balance.
    let tmp_dir = iota_common::tempdir();
    let index_store = open_index_store(tmp_dir.path().to_path_buf());
    let address = TestCheckpointDataBuilder::derive_address(1);

    let mut builder = TestCheckpointDataBuilder::new(0).start_transaction(0);
    for object_idx in 0..10 {
        builder = builder.create_coin_object(object_idx, 1, 100, GAS::type_tag());
    }
    let mut builder = builder.finish_transaction();
    let checkpoint = builder.build_checkpoint();
    index_store.index_checkpoint(&checkpoint)?;
    index_store.commit_update_for_checkpoint(0)?;

    let balance_from_db = IndexStore::get_balance_from_db(
        index_store.metrics.clone(),
        index_store.tables.coin_index.clone(),
        address,
        GAS::type_tag(),
    )?;
    let balance = index_store.get_balance(address, GAS::type_tag())?;
    assert_eq!(balance, balance_from_db);
    assert_eq!(balance.balance, 1000);
    assert_eq!(balance.num_coins, 10);

    let all_balance = index_store.get_all_balance(address)?;
    let balance = all_balance.get(&GAS::type_tag()).unwrap();
    assert_eq!(*balance, balance_from_db);
    assert_eq!(balance.balance, 1000);
    assert_eq!(balance.num_coins, 10);

    let mut builder = builder.start_transaction(0);
    for object_idx in 0..3 {
        builder = builder.delete_object(object_idx);
    }
    let mut builder = builder.finish_transaction();
    let checkpoint = builder.build_checkpoint();
    index_store.index_checkpoint(&checkpoint)?;
    index_store.commit_update_for_checkpoint(1)?;

    let balance_from_db = IndexStore::get_balance_from_db(
        index_store.metrics.clone(),
        index_store.tables.coin_index.clone(),
        address,
        GAS::type_tag(),
    )?;
    let balance = index_store.get_balance(address, GAS::type_tag())?;
    assert_eq!(balance, balance_from_db);
    assert_eq!(balance.balance, 700);
    assert_eq!(balance.num_coins, 7);
    // Invalidate per coin type balance cache and read from all balance cache to
    // ensure the balance matches
    index_store
        .caches
        .per_coin_type_balance
        .invalidate(&(address, GAS::type_tag()));
    let all_balance = index_store.get_all_balance(address)?;
    assert_eq!(all_balance.get(&GAS::type_tag()).unwrap().balance, 700);
    assert_eq!(all_balance.get(&GAS::type_tag()).unwrap().num_coins, 7);
    let balance = index_store.get_balance(address, GAS::type_tag())?;
    assert_eq!(balance, balance_from_db);
    assert_eq!(balance.balance, 700);
    assert_eq!(balance.num_coins, 7);

    Ok(())
}

/// A cache-miss repopulation racing a commit must not double-apply the
/// checkpoint's delta: the committer holds the owner's lock from the
/// delta computation through the cache merge, and cache-miss reads take
/// the same lock, so a value read between the batch write and the merge
/// can never be merged onto.
#[tokio::test]
async fn test_balance_cache_repopulation_cannot_race_a_commit() -> anyhow::Result<()> {
    let tmp_dir = iota_common::tempdir();
    let index_store = std::sync::Arc::new(open_index_store(tmp_dir.path().to_path_buf()));
    let address = TestCheckpointDataBuilder::derive_address(1);

    let mut builder = TestCheckpointDataBuilder::new(0)
        .start_transaction(0)
        .create_coin_object(0, 1, 100, GAS::type_tag())
        .finish_transaction();
    let checkpoint = builder.build_checkpoint();
    index_store.index_checkpoint(&checkpoint)?;
    index_store.commit_update_for_checkpoint(0)?;

    // A second coin for the same owner in checkpoint 1.
    let mut builder = builder
        .start_transaction(0)
        .create_coin_object(1, 1, 100, GAS::type_tag())
        .finish_transaction();
    let checkpoint = builder.build_checkpoint();
    index_store.index_checkpoint(&checkpoint)?;

    // Replay the commit by hand, pausing between the batch write and the
    // cache merge — the window where an unlocked reader used to cache
    // the post-write value the merge was then applied on top of.
    let reader = {
        let (staged_seq, update) = index_store.pending_updates.lock().pop_first().unwrap();
        assert_eq!(staged_seq, 1);
        let cache_updates = index_store.balance_cache_updates(update.coin_changes)?;
        update.batch.write()?;

        let reader = std::thread::spawn({
            let index_store = index_store.clone();
            move || index_store.get_balance(address, GAS::type_tag()).unwrap()
        });
        // Give the reader time to reach the owner's lock. The sleep only
        // makes the race likely: a slow reader arrives after the merge
        // and the test passes without exercising it.
        std::thread::sleep(std::time::Duration::from_millis(50));

        index_store.update_per_coin_type_cache(cache_updates.per_coin_type_balance_changes)?;
        index_store.update_all_balance_cache(cache_updates.all_balance_changes)?;
        reader
        // The owner locks in `cache_updates` release here.
    };

    assert_eq!(reader.join().unwrap().balance, 200);
    let cached = index_store.get_balance(address, GAS::type_tag())?;
    assert_eq!(cached.balance, 200);
    assert_eq!(cached.num_coins, 2);
    Ok(())
}

/// A cancelled rebuild fails the open instead of serving the truncated
/// store it leaves behind, and the next open rebuilds it.
#[tokio::test]
async fn test_a_cancelled_rebuild_fails_the_open() {
    let dir = iota_common::tempdir();
    let index_dir = dir.path().join(super::JSONRPC_INDEXES_DIR);
    let checkpoint_store = CheckpointStore::new(&dir.path().join("checkpoints"));
    mark_checkpoint_executed(&checkpoint_store, 5);
    let authority_store = open_authority_store(&dir.path().join("store"));

    let cancelled = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true));
    let opened = IndexStore::new(
        index_dir.clone(),
        &Registry::default(),
        Some(128),
        None,
        &authority_store,
        &checkpoint_store,
        cancelled,
    )
    .await;
    let Err(error) = opened else {
        panic!("a cancelled rebuild must not return a usable store");
    };
    assert!(
        error.to_string().contains("cancelled by shutdown"),
        "unexpected error: {error}"
    );
    assert!(
        crate::index_rebuild_cancellation::is_cancelled(&error),
        "the node's exit path must still recognize the rewrapped cancellation"
    );

    let index_store = IndexStore::new(
        index_dir,
        &Registry::default(),
        Some(128),
        None,
        &authority_store,
        &checkpoint_store,
        Default::default(),
    )
    .await
    .expect("the next open must rebuild the store the cancelled one left behind");
    assert_eq!(index_store.tables.watermark.get(&()).unwrap(), Some(5));
    assert!(
        !index_store
            .tables
            .needs_to_do_initialization(&checkpoint_store)
            .unwrap(),
        "the rebuilt store must open in place"
    );
}

/// An unclean stop leaves the watermark ahead of the executed checkpoint
/// by up to the execution concurrency, which is no reason to rebuild.
#[tokio::test]
async fn test_a_watermark_far_ahead_of_the_executed_checkpoint_is_not_fatal() {
    let tmp_dir = iota_common::tempdir();
    let cp_dir = iota_common::tempdir();
    let checkpoint_store = CheckpointStore::new(&cp_dir.path().join("checkpoints"));
    let index_store = open_index_store(tmp_dir.path().to_path_buf());
    index_store.tables.seed_meta().unwrap();
    mark_checkpoint_executed(&checkpoint_store, 5);
    checkpoint_store
        .insert_verified_checkpoint(&executed_checkpoint(0, 7))
        .unwrap();
    index_store.tables.watermark.insert(&(), &7).unwrap();

    assert!(
        !index_store
            .tables
            .needs_to_do_initialization(&checkpoint_store)
            .unwrap()
    );
}

/// Numbering anchors to the watermark's checkpoint, so a watermark whose
/// checkpoint the store no longer holds is rebuilt from scratch.
#[tokio::test]
async fn test_a_watermark_without_its_checkpoint_rebuilds_the_index() {
    let dir = iota_common::tempdir();
    let index_dir = dir.path().join(super::JSONRPC_INDEXES_DIR);
    let checkpoint_store = CheckpointStore::new(&dir.path().join("checkpoints"));
    {
        let index_store = open_index_store(index_dir.clone());
        index_store.tables.seed_meta().unwrap();
        index_store.tables.watermark.insert(&(), &5).unwrap();
        close_index_store(index_store).await;
    }

    let authority_store = open_authority_store(&dir.path().join("store"));
    let index_store = IndexStore::new(
        index_dir,
        &Registry::default(),
        Some(128),
        None,
        &authority_store,
        &checkpoint_store,
        Default::default(),
    )
    .await
    .expect("a missing anchor must rebuild instead of failing the open");

    assert_eq!(
        index_store.tables.watermark.get(&()).unwrap(),
        None,
        "the rebuild must drop the watermark it could not anchor"
    );
    assert_eq!(index_store.next_sequence_number(), 0);
}

/// Concurrent misses on one owner cost a single coin scan.
#[tokio::test]
async fn test_a_balance_miss_takes_the_value_cached_while_it_waited() {
    let tmp_dir = iota_common::tempdir();
    let index_store = std::sync::Arc::new(open_index_store(tmp_dir.path().to_path_buf()));
    let address = TestCheckpointDataBuilder::derive_address(1);
    let cached = super::TotalBalance {
        balance: 42,
        num_coins: 7,
    };

    let lock = index_store.caches.locks.acquire_lock(address);
    let reader = std::thread::spawn({
        let index_store = index_store.clone();
        move || index_store.get_balance(address, GAS::type_tag()).unwrap()
    });
    // Give the reader time to reach the owner's lock. A slow one passes
    // without exercising the race.
    std::thread::sleep(std::time::Duration::from_millis(50));

    index_store
        .caches
        .per_coin_type_balance
        .get_with((address, GAS::type_tag()), || Ok(cached))
        .unwrap();
    drop(lock);

    assert_eq!(reader.join().unwrap(), cached);
    assert_eq!(
        index_store.metrics.balance_lookup_from_db.get(),
        0,
        "the waiting reader must not repeat the coin scan"
    );
}

/// As above, for the all-balances cache.
#[tokio::test]
async fn test_an_all_balance_miss_takes_the_value_cached_while_it_waited() {
    let tmp_dir = iota_common::tempdir();
    let index_store = std::sync::Arc::new(open_index_store(tmp_dir.path().to_path_buf()));
    let address = TestCheckpointDataBuilder::derive_address(1);
    let cached = std::sync::Arc::new(std::collections::HashMap::from([(
        GAS::type_tag(),
        super::TotalBalance {
            balance: 42,
            num_coins: 7,
        },
    )]));

    let lock = index_store.caches.locks.acquire_lock(address);
    let reader = std::thread::spawn({
        let index_store = index_store.clone();
        move || index_store.get_all_balance(address).unwrap()
    });
    // See above: the sleep only makes the race likely.
    std::thread::sleep(std::time::Duration::from_millis(50));

    index_store
        .caches
        .all_balances
        .get_with(address, || Ok(cached.clone()))
        .unwrap();
    drop(lock);

    assert_eq!(reader.join().unwrap(), cached);
    assert_eq!(
        index_store.metrics.all_balance_lookup_from_db.get(),
        0,
        "the waiting reader must not repeat the coin scan"
    );
}

/// Replaying a committed checkpoint (crash recovery before the executed
/// watermark advanced, or the upgrade to per-checkpoint indexing) must
/// skip its already-indexed transactions: no new sequence numbers, no
/// duplicate rows, no double-counted balances.
#[tokio::test]
async fn test_index_checkpoint_skips_already_indexed() -> anyhow::Result<()> {
    let tmp_dir = iota_common::tempdir();
    let index_store = open_index_store(tmp_dir.path().to_path_buf());
    let address = TestCheckpointDataBuilder::derive_address(1);

    let mut builder = TestCheckpointDataBuilder::new(0)
        .start_transaction(0)
        .create_coin_object(0, 1, 100, GAS::type_tag())
        .finish_transaction();
    let checkpoint = builder.build_checkpoint();
    let digest = *checkpoint.transactions[0].effects.transaction_digest();

    index_store.index_checkpoint(&checkpoint)?;
    index_store.commit_update_for_checkpoint(0)?;
    assert_eq!(index_store.get_transaction_seq(&digest)?, Some(0));
    assert_eq!(index_store.tables.watermark.get(&())?, Some(0));

    // Replay the same checkpoint.
    index_store.index_checkpoint(&checkpoint)?;
    index_store.commit_update_for_checkpoint(0)?;

    assert_eq!(index_store.get_transaction_seq(&digest)?, Some(0));
    assert_eq!(
        index_store.get_transactions(None, None, None, false)?,
        vec![digest]
    );
    let balance = index_store.get_balance(address, GAS::type_tag())?;
    assert_eq!(balance.balance, 100);
    assert_eq!(balance.num_coins, 1);

    Ok(())
}

/// Checkpoints of different epochs land in separate history buckets:
/// queries and cursors chain across them in order, reopening rediscovers
/// the buckets from the column-family names, and pruning drops whole
/// epochs wholesale.
#[tokio::test]
async fn test_history_epoch_buckets_chain_and_prune() -> anyhow::Result<()> {
    let tmp_dir = iota_common::tempdir();
    let index_store = open_index_store(tmp_dir.path().to_path_buf());

    // One transaction in epoch 0, one in epoch 1.
    let mut builder = TestCheckpointDataBuilder::new(0)
        .with_epoch(0)
        .start_transaction(0)
        .create_coin_object(0, 1, 100, GAS::type_tag())
        .finish_transaction();
    let checkpoint_epoch_0 = builder.build_checkpoint();
    let tx_0 = *checkpoint_epoch_0.transactions[0]
        .effects
        .transaction_digest();
    index_store.index_checkpoint(&checkpoint_epoch_0)?;
    index_store.commit_update_for_checkpoint(0)?;

    let mut builder = builder
        .with_epoch(1)
        .start_transaction(1)
        .create_coin_object(1, 1, 100, GAS::type_tag())
        .finish_transaction();
    let checkpoint_epoch_1 = builder.build_checkpoint();
    let tx_1 = *checkpoint_epoch_1.transactions[0]
        .effects
        .transaction_digest();
    index_store.index_checkpoint(&checkpoint_epoch_1)?;
    index_store.commit_update_for_checkpoint(1)?;

    // Forward and reverse iteration chain across the buckets in order.
    assert_eq!(
        index_store.get_transactions(None, None, None, false)?,
        vec![tx_0, tx_1]
    );
    assert_eq!(
        index_store.get_transactions(None, None, None, true)?,
        vec![tx_1, tx_0]
    );
    // An exclusive cursor crosses the bucket boundary.
    assert_eq!(
        index_store.get_transactions(None, Some(tx_1), None, true)?,
        vec![tx_0]
    );
    assert_eq!(
        index_store.get_transactions(None, Some(tx_0), None, false)?,
        vec![tx_1]
    );
    // A limit landing exactly on the bucket boundary stops there.
    assert_eq!(
        index_store.get_transactions(None, None, Some(1), false)?,
        vec![tx_0]
    );
    assert_eq!(
        index_store.get_transactions(None, None, Some(1), true)?,
        vec![tx_1]
    );

    // Reopening rediscovers the buckets from the column-family names.
    let index_store = reopen_index_store(index_store, tmp_dir.path().to_path_buf()).await;
    assert_eq!(
        index_store.get_transactions(None, None, None, false)?,
        vec![tx_0, tx_1]
    );

    // Pruning to one retained epoch drops epoch 0's bucket wholesale,
    // and pruning again is a no-op.
    assert_eq!(index_store.prune(1)?, Some(1));
    assert_eq!(index_store.get_transaction_seq(&tx_0)?, None);
    assert_eq!(
        index_store.get_transactions(None, None, None, false)?,
        vec![tx_1]
    );
    assert_eq!(index_store.prune(1)?, Some(1));

    // A cursor pointing into the pruned epoch reports the transaction as
    // gone instead of silently re-serving the first page.
    assert!(matches!(
        index_store.get_transactions(None, Some(tx_0), None, false),
        Err(iota_types::error::IotaError::TransactionNotFound { .. })
    ));

    Ok(())
}

/// Tables of one bucket share a column family, separated only by their
/// tag byte: a full-range scan of one table must not yield a neighboring
/// table's rows, whose bytes do not deserialize under its types.
#[tokio::test]
async fn test_history_tables_do_not_bleed_across_tags() {
    use iota_sdk_types::TransactionDigest;

    let tmp_dir = iota_common::tempdir();
    let index_store = open_index_store(tmp_dir.path().to_path_buf());
    let bucket = index_store.ensure_history_bucket(0).unwrap();

    let digest = TransactionDigest::random();
    let mut batch = index_store.tables.meta.batch();
    // Adjacent tags: `tx_order` and `txs_seq`.
    batch
        .insert_batch_tagged(&bucket.tx_order, [(7u64, digest)])
        .unwrap();
    batch
        .insert_batch_tagged(&bucket.txs_seq, [(digest, 7u64)])
        .unwrap();
    batch.write().unwrap();

    let rows: Vec<_> = bucket
        .tx_order
        .safe_range_iter(u64::MIN..=u64::MAX)
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(rows, vec![(7, digest)]);
    let rows: Vec<_> = bucket
        .tx_order
        .safe_range_iter_reversed(u64::MIN..=u64::MAX)
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(rows, vec![(7, digest)]);
    let rows: Vec<_> = bucket
        .txs_seq
        .safe_range_iter_reversed(TransactionDigest::ZERO..=[0xff; 32].into())
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(rows, vec![(digest, 7)]);
}

#[tokio::test]
async fn test_get_transaction_by_move_function() {
    use iota_sdk_types::TransactionDigest;

    let tmp_dir = iota_common::tempdir();
    let index_store = open_index_store(tmp_dir.path().to_path_buf());
    let bucket = index_store.ensure_history_bucket(0).unwrap();
    let mut batch = index_store.tables.meta.batch();
    batch
        .insert_batch_tagged(
            &bucket.txs_by_move_function,
            [
                (
                    (
                        ObjectId::new([1; 32]),
                        "mod".to_string(),
                        "f".to_string(),
                        0,
                    ),
                    TransactionDigest::from([0; 32]),
                ),
                (
                    (
                        ObjectId::new([1; 32]),
                        "mod".to_string(),
                        "Z".repeat(128),
                        0,
                    ),
                    TransactionDigest::from([1; 32]),
                ),
                (
                    (
                        ObjectId::new([1; 32]),
                        "mod".to_string(),
                        "f".repeat(128),
                        0,
                    ),
                    TransactionDigest::from([2; 32]),
                ),
                (
                    (
                        ObjectId::new([1; 32]),
                        "mod".to_string(),
                        "z".repeat(128),
                        0,
                    ),
                    TransactionDigest::from([3; 32]),
                ),
            ],
        )
        .unwrap();
    batch.write().unwrap();

    let mut v = index_store
        .get_transactions_by_move_function(
            ObjectId::new([1; 32]),
            Some("mod".to_string()),
            None,
            None,
            None,
            false,
        )
        .unwrap();
    let v_rev = index_store
        .get_transactions_by_move_function(
            ObjectId::new([1; 32]),
            Some("mod".to_string()),
            None,
            None,
            None,
            true,
        )
        .unwrap();
    assert_eq!(
        v.len(),
        4,
        "an unset function must span the whole identifier range"
    );
    v.reverse();
    assert_eq!(v, v_rev);
}

/// Events chain across epoch buckets in global sequence order: with all
/// checkpoint timestamps equal, ordering falls through to the sequence
/// key, so correctness depends entirely on scanning the buckets in epoch
/// order.
#[tokio::test]
async fn test_events_chain_across_epoch_buckets() -> anyhow::Result<()> {
    use iota_sdk_types::Event;

    let tmp_dir = iota_common::tempdir();
    let index_store = open_index_store(tmp_dir.path().to_path_buf());
    let event = || Event {
        package_id: ObjectId::ZERO,
        module: iota_sdk_types::Identifier::from_static("test"),
        sender: TestCheckpointDataBuilder::derive_address(0),
        type_: StructTag::new_gas(),
        contents: vec![],
    };

    let mut builder = TestCheckpointDataBuilder::new(0)
        .with_epoch(0)
        .start_transaction(0)
        .with_events(vec![event()])
        .finish_transaction();
    let checkpoint_epoch_0 = builder.build_checkpoint();
    index_store.index_checkpoint(&checkpoint_epoch_0)?;
    index_store.commit_update_for_checkpoint(0)?;

    let mut builder = builder
        .with_epoch(1)
        .start_transaction(1)
        .with_events(vec![event()])
        .finish_transaction();
    let checkpoint_epoch_1 = builder.build_checkpoint();
    index_store.index_checkpoint(&checkpoint_epoch_1)?;
    index_store.commit_update_for_checkpoint(1)?;

    let forward = index_store.event_iterator(0, u64::MAX, 0, 0, 10, false)?;
    assert_eq!(forward.len(), 2);
    let descending = index_store.event_iterator(0, u64::MAX, u64::MAX, usize::MAX, 10, true)?;
    assert_eq!(
        descending,
        forward.iter().rev().cloned().collect::<Vec<_>>(),
        "descending must mirror the forward chain across the buckets"
    );

    Ok(())
}

/// An empty newest bucket (a crash between `create_cf` and its first
/// committed batch) must not reset the numbering: the floor scan reads
/// the older buckets.
#[tokio::test]
async fn test_numbering_floor_skips_an_empty_newest_bucket() {
    let tmp_dir = iota_common::tempdir();
    let index_store = open_index_store(tmp_dir.path().to_path_buf());
    seed_history_buckets(&index_store, 1);
    index_store.ensure_history_bucket(1).unwrap();

    let index_store = reopen_index_store(index_store, tmp_dir.path().to_path_buf()).await;
    assert_eq!(
        index_store.next_sequence_number(),
        1,
        "numbering must continue after the last row of the older buckets"
    );
}

/// After `shutdown`, the backfill stops before replaying anything, so
/// shutdown does not block on a full replay.
#[tokio::test]
async fn test_shutdown_stops_the_backfill() {
    let (authority_state, _) = genesis_authority_state().await;
    let checkpoint_store = &authority_state.checkpoint_store;
    let index_dir = iota_common::tempdir();
    let index_store = open_index_store(index_dir.path().to_path_buf());
    index_store
        .tables
        .history_watermark
        .insert(&(), &1)
        .unwrap();

    index_store.shutdown().await;
    index_store
        .backfill_history(&authority_state.database_for_testing(), checkpoint_store)
        .unwrap();
    assert_eq!(
        index_store.tables.history_watermark.get(&()).unwrap(),
        Some(1),
        "a cancelled backfill must not replay"
    );
}

/// An unopenable database is wiped and rebuilt instead of crash-looping
/// the node.
#[tokio::test]
async fn test_unopenable_database_is_wiped_and_rebuilt() {
    let (authority_state, genesis_tx_digest) = genesis_authority_state().await;
    let checkpoint_store = &authority_state.checkpoint_store;
    let index_dir = iota_common::tempdir();
    std::fs::write(index_dir.path().join("CURRENT"), b"bogus").unwrap();

    let index_store = IndexStore::new(
        index_dir.path().to_path_buf(),
        &Registry::default(),
        Some(128),
        None,
        &authority_state.database_for_testing(),
        checkpoint_store,
        Default::default(),
    )
    .await
    .unwrap();
    index_store.wait_for_history_backfill_for_testing().await;
    assert_eq!(
        index_store.get_transaction_seq(&genesis_tx_digest).unwrap(),
        Some(0)
    );
}

/// A read error in the rebuild predicate propagates instead of silently
/// deciding to wipe or to adopt.
#[tokio::test]
async fn test_rebuild_predicate_propagates_read_errors() {
    let dir = iota_common::tempdir();
    let checkpoint_store = CheckpointStore::new(&dir.path().join("checkpoints"));
    let index_store = open_index_store(dir.path().join("indexes"));
    index_store.database_for_testing().drop_cf("meta").unwrap();
    assert!(
        index_store
            .tables
            .needs_to_do_initialization(&checkpoint_store)
            .is_err()
    );

    // The watermark-less arm reads the owner index to tell a build that
    // was cut short from a fresh store.
    let index_store = open_index_store(dir.path().join("owner-index-error"));
    index_store.tables.seed_meta().unwrap();
    index_store
        .database_for_testing()
        .drop_cf("owner_index")
        .unwrap();
    assert!(
        index_store
            .tables
            .needs_to_do_initialization(&checkpoint_store)
            .is_err()
    );
}

/// Leftover files under the index directory are cleared before a
/// bulk-ingestion open instead of failing the recovery.
#[tokio::test]
async fn test_bulk_ingestion_open_clears_leftover_files() {
    let dir = iota_common::tempdir();
    let index_dir = dir.path().join("indexes");
    std::fs::create_dir_all(&index_dir).unwrap();
    std::fs::write(index_dir.join("stray"), b"leftover").unwrap();

    let tables = super::IndexStoreTables::open_for_bulk_ingestion(index_dir.clone(), 1);
    assert_eq!(tables.meta.get(&()).unwrap(), None);
    assert!(!index_dir.join("stray").exists());
}

/// The caching layout resolver resolves each struct tag once.
#[test]
fn test_caching_layout_resolver_memoizes_by_tag() {
    use iota_types::layout_resolver::LayoutResolver;
    use move_core_types::annotated_value::{MoveDatatypeLayout, MoveStructLayout};

    struct Counting {
        calls: u32,
    }
    impl LayoutResolver for Counting {
        fn get_annotated_layout(
            &mut self,
            _struct_tag: &StructTag,
        ) -> Result<MoveDatatypeLayout, iota_types::error::IotaError> {
            self.calls += 1;
            Ok(MoveDatatypeLayout::Struct(Box::new(MoveStructLayout {
                type_: "0x2::coin::Coin".parse().unwrap(),
                fields: vec![],
            })))
        }
    }

    let mut inner = Counting { calls: 0 };
    let mut caching = super::CachingLayoutResolver::new(&mut inner);
    let coin: StructTag = "0x2::coin::Coin<0x2::iota::IOTA>".parse().unwrap();
    let cap: StructTag = "0x2::coin::TreasuryCap<0x2::iota::IOTA>".parse().unwrap();
    caching.get_annotated_layout(&coin).unwrap();
    caching.get_annotated_layout(&coin).unwrap();
    caching.get_annotated_layout(&cap).unwrap();
    drop(caching);
    assert_eq!(inner.calls, 2, "one resolution per distinct tag");
}

/// Four dynamic field ids of `parent`, in the order the index stores them.
fn seed_dynamic_fields(index_store: &IndexStore, parent: ObjectId) -> Vec<ObjectId> {
    let mut field_ids: Vec<ObjectId> = (0..4).map(|_| ObjectId::random()).collect();
    field_ids.sort();
    let table = &index_store.tables.dynamic_field_index;
    let mut batch = table.batch();
    batch
        .insert_batch(table, field_ids.iter().map(|id| ((parent, *id), ())))
        .unwrap();
    batch.write().unwrap();
    field_ids
}

fn dynamic_field_page(
    index_store: &IndexStore,
    parent: ObjectId,
    cursor: Option<ObjectId>,
) -> Vec<ObjectId> {
    index_store
        .get_dynamic_field_ids_iterator(parent, cursor)
        .unwrap()
        .map(Result::unwrap)
        .collect()
}

/// A page starts after the cursor's id, even when its row is gone.
#[tokio::test]
async fn test_dynamic_field_page_excludes_only_the_cursor() {
    let tmp_dir = iota_common::tempdir();
    let index_store = open_index_store(tmp_dir.path().to_path_buf());
    let parent = ObjectId::random();
    let field_ids = seed_dynamic_fields(&index_store, parent);

    assert_eq!(dynamic_field_page(&index_store, parent, None), field_ids);
    assert_eq!(
        dynamic_field_page(&index_store, parent, Some(field_ids[0])),
        &field_ids[1..]
    );

    // The field the cursor points at can be removed between two pages.
    let table = &index_store.tables.dynamic_field_index;
    let mut batch = table.batch();
    batch.delete_batch(table, [(parent, field_ids[0])]).unwrap();
    batch.write().unwrap();

    assert_eq!(
        dynamic_field_page(&index_store, parent, Some(field_ids[0])),
        &field_ids[1..],
        "the field after the cursor must not be lost with the cursor's row"
    );
}

/// Four object ids owned by `owner`, in the order the index stores them.
fn seed_owner_objects(index_store: &IndexStore, owner: Address) -> Vec<ObjectId> {
    let mut object_ids: Vec<ObjectId> = (0..4).map(|_| ObjectId::random()).collect();
    object_ids.sort();
    let table = &index_store.tables.owner_index;
    let mut batch = table.batch();
    batch
        .insert_batch(
            table,
            object_ids.iter().map(|id| {
                (
                    (owner, *id),
                    ObjectInfo {
                        object_id: *id,
                        version: Version::OBJECT_START,
                        digest: ObjectDigest::ZERO,
                        type_: ObjectType::Package,
                        owner: Owner::Address(owner),
                        previous_transaction: TransactionDigest::ZERO,
                    },
                )
            }),
        )
        .unwrap();
    batch.write().unwrap();
    object_ids
}

fn owner_object_page(index_store: &IndexStore, owner: Address, cursor: ObjectId) -> Vec<ObjectId> {
    index_store
        .get_owner_objects_iterator(owner, cursor, None)
        .unwrap()
        .map(|info| info.object_id)
        .collect()
}

/// A page starts after the cursor's id, even when its row is gone.
#[tokio::test]
async fn test_owner_objects_page_excludes_only_the_cursor() {
    let tmp_dir = iota_common::tempdir();
    let index_store = open_index_store(tmp_dir.path().to_path_buf());
    let owner = Address::random();
    let object_ids = seed_owner_objects(&index_store, owner);

    assert_eq!(
        owner_object_page(&index_store, owner, ObjectId::ZERO),
        object_ids
    );
    assert_eq!(
        owner_object_page(&index_store, owner, object_ids[0]),
        &object_ids[1..]
    );

    // The cursor's object can be transferred away between two pages.
    let table = &index_store.tables.owner_index;
    let mut batch = table.batch();
    batch.delete_batch(table, [(owner, object_ids[0])]).unwrap();
    batch.write().unwrap();

    assert_eq!(
        owner_object_page(&index_store, owner, object_ids[0]),
        &object_ids[1..],
        "the object after the cursor must not be lost with the cursor's row"
    );
}
