// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::collections::BTreeSet;

use iota_sdk_types::TransactionDigest;
use iota_types::{
    messages_checkpoint::CheckpointContentsExt, storage::error::Error as StorageError,
};
use prometheus_filtered::Registry;
use typed_store::Map;

use super::{IndexGroup, RpcIndexesStore};
use crate::{
    checkpoints::CheckpointStore,
    par_index_live_object_set::{LiveObjectIndexer, ParMakeLiveObjectIndexer},
    test_utils::executed_checkpoint,
};

/// Opens an `RpcIndexesStore` at `path` without running the rebuild path,
/// serving every group.
fn open_index_store(path: std::path::PathBuf) -> RpcIndexesStore {
    RpcIndexesStore::new_without_init(
        path,
        BTreeSet::from([IndexGroup::JsonRpc, IndexGroup::Grpc]),
    )
}

/// Closes the store's database, waiting until every handle is released
/// so the same path can be reopened.
async fn close_index_store(index_store: RpcIndexesStore) {
    let weak_db = std::sync::Arc::downgrade(&index_store.tables.meta.db);
    drop(index_store);
    assert!(super::wait_for_database_close(weak_db).await);
}

/// Closes the store and reopens the same path, as a restart does.
async fn reopen_index_store(
    index_store: RpcIndexesStore,
    path: std::path::PathBuf,
) -> RpcIndexesStore {
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
fn seed_history_buckets(index_store: &RpcIndexesStore, epochs: u64) {
    for epoch in 0..epochs {
        let bucket = index_store.ensure_history_bucket(epoch).unwrap();
        let mut batch = index_store.tables.meta.batch();
        batch
            .insert_batch_tagged(&bucket.tx_order, [(epoch, TransactionDigest::random())])
            .unwrap();
        batch.write().unwrap();
    }
}

/// A live-object indexer that fills nothing, standing in for the real one
/// (wired in a later task) so `init` can be exercised directly.
struct NoOpIndexer;

impl ParMakeLiveObjectIndexer for NoOpIndexer {
    type ObjectIndexer<'a> = NoOpObjectIndexer;

    fn make_live_object_indexer(&self) -> Self::ObjectIndexer<'_> {
        NoOpObjectIndexer
    }
}

struct NoOpObjectIndexer;

impl LiveObjectIndexer for NoOpObjectIndexer {
    fn index_object(&mut self, _object: &iota_types::object::Object) -> Result<(), StorageError> {
        Ok(())
    }

    fn finish(self) -> Result<(), StorageError> {
        Ok(())
    }
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
    let groups = BTreeSet::from([IndexGroup::JsonRpc, IndexGroup::Grpc]);

    let mut tables =
        super::IndexStoreTables::open_for_bulk_ingestion(dir.path().join("indexes"), 1);
    tables
        .init(
            &authority_store,
            &checkpoint_store,
            &NoOpIndexer,
            &groups,
            &Default::default(),
        )
        .unwrap();
    assert_eq!(tables.watermark.get(&()).unwrap(), None);
    assert_eq!(tables.history_watermark.get(&()).unwrap(), None);
    assert!(
        tables
            .needs_to_do_initialization(&checkpoint_store, &groups)
            .unwrap(),
        "a store whose rebuild was not adopted must be wiped and rebuilt on the next open"
    );
}

/// When a store must be wiped and rebuilt, as one decision table: a
/// pre-upgrade database (data, no `meta` row) is never seeded and always
/// rebuilt; a brand-new store needs no rebuild until the executed
/// watermark passes the indexed one; a store holding data but no
/// watermark is always rebuilt; a watermark at or ahead of the executed
/// checkpoint (crash between index commit and executed bump) needs none;
/// a schema version bump always does; and a group missing from the
/// recorded set, once the rest of the store looks healthy, does too.
#[tokio::test]
async fn test_needs_to_do_initialization_cases() {
    let tmp_dir = iota_common::tempdir();
    let cp_dir = iota_common::tempdir();
    let checkpoint_store = CheckpointStore::new(&cp_dir.path().join("checkpoints"));
    let index_store = open_index_store(tmp_dir.path().to_path_buf());
    let groups = BTreeSet::from([IndexGroup::JsonRpc]);

    // A database from before per-checkpoint indexing must stay unseeded:
    // nodes restored from a formal snapshot wrote a corrupted owner
    // index into it, and without a watermark it cannot prove otherwise.
    let owner = iota_types::base_types::dbg_addr(1);
    let object = iota_types::object::Object::with_id_owner_for_testing(
        iota_sdk_types::ObjectId::random(),
        owner,
    );
    let (owner_key, owner_info) = super::schema::make_owner_key(owner, &object).unwrap();
    index_store
        .tables
        .owner
        .insert(&owner_key, &owner_info)
        .unwrap();
    index_store.tables.seed_meta(&groups).unwrap();
    assert_eq!(
        index_store.tables.meta.get(&()).unwrap(),
        None,
        "a database with data but no `meta` row must not be seeded"
    );
    assert!(
        index_store
            .tables
            .needs_to_do_initialization(&checkpoint_store, &groups)
            .unwrap(),
        "a database from before per-checkpoint indexing must be rebuilt"
    );

    index_store.tables.owner.remove(&owner_key).unwrap();
    index_store.tables.seed_meta(&groups).unwrap();
    assert!(
        !index_store
            .tables
            .needs_to_do_initialization(&checkpoint_store, &groups)
            .unwrap(),
        "a brand-new store on a node with no executed checkpoints needs no rebuild"
    );

    // A rebuild or restore that crashed before writing the watermark
    // leaves data behind; with nothing executed, comparing the
    // watermarks alone would adopt it.
    index_store
        .tables
        .owner
        .insert(&owner_key, &owner_info)
        .unwrap();
    assert!(
        index_store
            .tables
            .needs_to_do_initialization(&checkpoint_store, &groups)
            .unwrap(),
        "a store holding data but no watermark must be rebuilt"
    );
    index_store.tables.owner.remove(&owner_key).unwrap();

    mark_checkpoint_executed(&checkpoint_store, 5);
    assert!(
        index_store
            .tables
            .needs_to_do_initialization(&checkpoint_store, &groups)
            .unwrap(),
        "an executed checkpoint past the indexed watermark must trigger a rebuild"
    );

    index_store.tables.watermark.insert(&(), &5).unwrap();
    assert!(
        !index_store
            .tables
            .needs_to_do_initialization(&checkpoint_store, &groups)
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
            .needs_to_do_initialization(&checkpoint_store, &groups)
            .unwrap(),
        "an index watermark ahead of the executed watermark must not trigger a rebuild"
    );

    // A group the recorded metadata never covered turning on must trigger
    // a rebuild, even though nothing else about the store changed.
    assert!(
        index_store
            .tables
            .needs_to_do_initialization(
                &checkpoint_store,
                &BTreeSet::from([IndexGroup::JsonRpc, IndexGroup::Grpc])
            )
            .unwrap(),
        "a store built without the Grpc group must rebuild when the group turns on"
    );

    // A schema version bump also triggers a rebuild.
    index_store
        .tables
        .meta
        .insert(
            &(),
            &super::MetadataInfo {
                version: super::CURRENT_DB_VERSION + 1,
                groups: groups.clone(),
            },
        )
        .unwrap();
    assert!(
        index_store
            .tables
            .needs_to_do_initialization(&checkpoint_store, &groups)
            .unwrap()
    );
}

#[tokio::test]
async fn test_a_cancelled_rebuild_fails_the_open() {
    let dir = iota_common::tempdir();
    let index_dir = dir.path().join(super::schema::RPC_INDEXES_DIR);
    let checkpoint_store = CheckpointStore::new(&dir.path().join("checkpoints"));
    mark_checkpoint_executed(&checkpoint_store, 5);
    let authority_store = open_authority_store(&dir.path().join("store"));
    let groups = BTreeSet::from([IndexGroup::JsonRpc, IndexGroup::Grpc]);

    let cancelled = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true));
    let opened = RpcIndexesStore::new(
        index_dir.clone(),
        &Registry::default(),
        groups.clone(),
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

    let index_store = RpcIndexesStore::new(
        index_dir,
        &Registry::default(),
        groups.clone(),
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
            .needs_to_do_initialization(&checkpoint_store, &groups)
            .unwrap(),
        "the rebuilt store must open in place"
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

    let index_store = RpcIndexesStore::new(
        index_dir.path().to_path_buf(),
        &Registry::default(),
        BTreeSet::from([IndexGroup::JsonRpc, IndexGroup::Grpc]),
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
        index_store
            .lookup_digest(&genesis_tx_digest)
            .unwrap()
            .map(|(sequence, _)| sequence),
        Some(0)
    );
}

/// Pruned epochs must not be recreated by `ensure`, before or after a
/// reopen; the retention floor persists across restarts.
#[tokio::test]
async fn test_pruned_epochs_are_not_recreated() {
    let tmp_dir = iota_common::tempdir();
    let mut index_store = open_index_store(tmp_dir.path().to_path_buf());
    index_store.epochs_to_retain = Some(1);
    seed_history_buckets(&index_store, 2);
    assert_eq!(index_store.prune().unwrap(), Some(1));
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
    let mut index_store = open_index_store(tmp_dir.path().to_path_buf());
    index_store.epochs_to_retain = Some(2);
    seed_history_buckets(&index_store, 4);
    assert_eq!(index_store.prune().unwrap(), Some(2));

    let mut index_store = reopen_index_store(index_store, tmp_dir.path().to_path_buf()).await;
    index_store.epochs_to_retain = Some(52);
    assert_eq!(
        index_store.prune().unwrap(),
        Some(2),
        "a retention reaching below the dropped epochs must not lower the floor"
    );
    assert!(index_store.ensure_history_bucket(1).is_err());
    assert!(index_store.ensure_history_bucket(2).is_ok());
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

/// A store maintaining only the gRPC group still fills the digest table,
/// from contents alone, and the JSON-RPC-only tables stay empty.
#[tokio::test]
async fn test_grpc_only_backfill_fills_digests_from_contents() {
    let (authority_state, genesis_tx_digest) = genesis_authority_state().await;
    let checkpoint_store = &authority_state.checkpoint_store;
    let tmp_dir = iota_common::tempdir();
    let store = RpcIndexesStore::new_without_init(
        tmp_dir.path().to_path_buf(),
        BTreeSet::from([IndexGroup::Grpc]),
    );
    store.tables.history_watermark.insert(&(), &1).unwrap();

    store
        .backfill_history(&authority_state.database_for_testing(), checkpoint_store)
        .unwrap();

    assert_eq!(store.tables.history_watermark.get(&()).unwrap(), Some(0));
    assert_eq!(
        store.lookup_digest(&genesis_tx_digest).unwrap(),
        Some((0, 0)),
        "the digest table must fill without the JSON-RPC group"
    );
    let bucket = store.history.ensure(0).unwrap();
    assert!(
        bucket.tx_order.safe_iter().next().is_none(),
        "the JSON-RPC history tables must stay empty"
    );
}
