// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::collections::{BTreeMap, BTreeSet, HashMap};

use iota_sdk_types::{
    Address, Event, Identifier, MoveStruct, ObjectData, ObjectId, Owner, StructTag,
    TransactionDigest, TypeTag, Version, move_package::MovePackage,
};
use iota_types::{
    effects::TransactionEffectsAPI,
    error::{IotaError, UserInputError},
    full_checkpoint_content::CheckpointData,
    gas_coin::GAS,
    messages_checkpoint::CheckpointContentsExt,
    object::{MoveStructExt, Object},
    storage::{DynamicFieldKey, error::Error as StorageError},
    test_checkpoint_data_builder::TestCheckpointDataBuilder,
};
use prometheus_filtered::Registry;
use typed_store::Map;

use super::{
    IndexGroup, RpcIndexesStore, jsonrpc_api::CachingLayoutResolver, schema::OwnerIndexKey,
};
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
    let (owner_key, owner_info) = super::schema::OwnerIndexKey::for_object(owner, &object).unwrap();
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

/// Indexes one checkpoint's transactions into its epoch's history bucket,
/// standing in for the staging/commit pipeline a later task adds: this task
/// owns the read surface the history tables feed, not the write path that
/// fills them from checkpoints.
fn index_checkpoint_for_testing(store: &RpcIndexesStore, checkpoint: &CheckpointData) {
    let summary = &checkpoint.checkpoint_summary;
    let bucket = store.ensure_history_bucket(summary.epoch).unwrap();
    let mut batch = store.tables.meta.batch();
    for tx in &checkpoint.transactions {
        let sequence = store
            .next_sequence_number
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let data =
            super::schema::transaction_index_data(&tx.transaction, &tx.effects, tx.events.as_ref())
                .unwrap();
        bucket
            .index_tx(
                &mut batch,
                sequence,
                summary.sequence_number,
                summary.timestamp_ms,
                data,
            )
            .unwrap();
    }
    batch.write().unwrap();
}

#[tokio::test]
async fn test_get_transaction_by_move_function() {
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

/// Transactions and their positions chain across epoch buckets in global
/// sequence order, and a cursor into a pruned epoch is refused rather than
/// silently restarting the scan.
#[tokio::test]
async fn test_history_epoch_buckets_chain_and_prune() {
    let tmp_dir = iota_common::tempdir();
    let index_store = open_index_store(tmp_dir.path().to_path_buf());

    let mut builder = TestCheckpointDataBuilder::new(0)
        .with_epoch(0)
        .start_transaction(0)
        .finish_transaction();
    let checkpoint_epoch_0 = builder.build_checkpoint();
    let tx_0 = *checkpoint_epoch_0.transactions[0]
        .effects
        .transaction_digest();
    index_checkpoint_for_testing(&index_store, &checkpoint_epoch_0);

    let mut builder = builder
        .with_epoch(1)
        .start_transaction(1)
        .finish_transaction();
    let checkpoint_epoch_1 = builder.build_checkpoint();
    let tx_1 = *checkpoint_epoch_1.transactions[0]
        .effects
        .transaction_digest();
    index_checkpoint_for_testing(&index_store, &checkpoint_epoch_1);

    // Forward and reverse iteration chain across the buckets in order.
    assert_eq!(
        index_store
            .get_transactions(None, None, None, false)
            .unwrap(),
        vec![tx_0, tx_1]
    );
    assert_eq!(
        index_store
            .get_transactions(None, None, None, true)
            .unwrap(),
        vec![tx_1, tx_0]
    );
    // An exclusive cursor crosses the bucket boundary.
    assert_eq!(
        index_store
            .get_transactions(None, Some(tx_1), None, true)
            .unwrap(),
        vec![tx_0]
    );
    assert_eq!(
        index_store
            .get_transactions(None, Some(tx_0), None, false)
            .unwrap(),
        vec![tx_1]
    );
    // A limit landing exactly on the bucket boundary stops there.
    assert_eq!(
        index_store
            .get_transactions(None, None, Some(1), false)
            .unwrap(),
        vec![tx_0]
    );
    assert_eq!(
        index_store
            .get_transactions(None, None, Some(1), true)
            .unwrap(),
        vec![tx_1]
    );

    // Reopening rediscovers the buckets from the column-family names.
    let mut index_store = reopen_index_store(index_store, tmp_dir.path().to_path_buf()).await;
    assert_eq!(
        index_store
            .get_transactions(None, None, None, false)
            .unwrap(),
        vec![tx_0, tx_1]
    );

    // Pruning to one retained epoch drops epoch 0's bucket wholesale,
    // and pruning again is a no-op.
    index_store.epochs_to_retain = Some(1);
    assert_eq!(index_store.prune().unwrap(), Some(1));
    assert_eq!(index_store.lookup_digest(&tx_0).unwrap(), None);
    assert_eq!(
        index_store
            .get_transactions(None, None, None, false)
            .unwrap(),
        vec![tx_1]
    );
    assert_eq!(index_store.prune().unwrap(), Some(1));

    // A cursor pointing into the pruned epoch reports the transaction as
    // gone instead of silently re-serving the first page.
    assert!(matches!(
        index_store.get_transactions(None, Some(tx_0), None, false),
        Err(IotaError::TransactionNotFound { .. })
    ));
}

/// Events chain across epoch buckets in global sequence order: with all
/// checkpoint timestamps equal, ordering falls through to the sequence
/// key, so correctness depends entirely on scanning the buckets in epoch
/// order.
#[tokio::test]
async fn test_events_chain_across_epoch_buckets() {
    let index_store = open_index_store(iota_common::tempdir().path().to_path_buf());
    let event = || Event {
        package_id: ObjectId::ZERO,
        module: Identifier::from_static("test"),
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
    index_checkpoint_for_testing(&index_store, &checkpoint_epoch_0);

    let mut builder = builder
        .with_epoch(1)
        .start_transaction(1)
        .with_events(vec![event()])
        .finish_transaction();
    let checkpoint_epoch_1 = builder.build_checkpoint();
    index_checkpoint_for_testing(&index_store, &checkpoint_epoch_1);

    let forward = index_store
        .event_iterator(0, u64::MAX, 0, 0, 10, false)
        .unwrap();
    assert_eq!(forward.len(), 2);
    let descending = index_store
        .event_iterator(0, u64::MAX, u64::MAX, usize::MAX, 10, true)
        .unwrap();
    assert_eq!(
        descending,
        forward.iter().rev().cloned().collect::<Vec<_>>(),
        "descending must mirror the forward chain across the buckets"
    );
}

/// Writes `balance`'s coin for `owner` directly into the owner index, as
/// the live indexer would from a freshly created `Coin<IOTA>` object.
/// Returns the row's key, so a test can remove it again by hand.
fn insert_gas_coin(index_store: &RpcIndexesStore, owner: Address, balance: u64) -> OwnerIndexKey {
    let object = Object::new_move(
        MoveStruct::new_coin(
            GAS::type_tag(),
            Version::MIN_VALID_INCL,
            ObjectId::random(),
            balance,
        ),
        Owner::Address(owner),
        TransactionDigest::GENESIS_MARKER,
    );
    let (key, info) = OwnerIndexKey::for_object(owner, &object).unwrap();
    index_store.tables.owner.insert(&key, &info).unwrap();
    key
}

/// A checkpoint's coin changes are staged elsewhere; here the cache is
/// exercised directly against the owner index, invalidating by hand where a
/// real commit would merge a delta computed from the checkpoint.
#[tokio::test]
async fn test_index_cache() {
    let index_store = open_index_store(iota_common::tempdir().path().to_path_buf());
    let address = Address::random();

    let keys: Vec<_> = (0..10)
        .map(|_| insert_gas_coin(&index_store, address, 100))
        .collect();

    let balance_from_db = index_store
        .get_balance_from_db(address, &GAS::type_tag())
        .unwrap();
    let balance = index_store.get_balance(address, GAS::type_tag()).unwrap();
    assert_eq!(balance, balance_from_db);
    assert_eq!(balance.balance, 1000);
    assert_eq!(balance.num_coins, 10);

    let all_balance = index_store.get_all_balance(address).unwrap();
    let balance = all_balance.get(&GAS::type_tag()).unwrap();
    assert_eq!(*balance, balance_from_db);
    assert_eq!(balance.balance, 1000);
    assert_eq!(balance.num_coins, 10);

    for key in &keys[0..3] {
        index_store.tables.owner.remove(key).unwrap();
    }
    // A real commit's cache maintenance runs here; simulate it by hand.
    index_store
        .caches
        .per_coin_type_balance
        .invalidate(&(address, GAS::type_tag()));
    index_store.caches.all_balances.invalidate(&address);

    let balance_from_db = index_store
        .get_balance_from_db(address, &GAS::type_tag())
        .unwrap();
    let balance = index_store.get_balance(address, GAS::type_tag()).unwrap();
    assert_eq!(balance, balance_from_db);
    assert_eq!(balance.balance, 700);
    assert_eq!(balance.num_coins, 7);

    // Invalidate per coin type balance cache and read from all balance cache to
    // ensure the balance matches
    index_store
        .caches
        .per_coin_type_balance
        .invalidate(&(address, GAS::type_tag()));
    let all_balance = index_store.get_all_balance(address).unwrap();
    assert_eq!(all_balance.get(&GAS::type_tag()).unwrap().balance, 700);
    assert_eq!(all_balance.get(&GAS::type_tag()).unwrap().num_coins, 7);
    let balance = index_store.get_balance(address, GAS::type_tag()).unwrap();
    assert_eq!(balance, balance_from_db);
    assert_eq!(balance.balance, 700);
    assert_eq!(balance.num_coins, 7);
}

/// A cache-miss repopulation racing a commit must not read a half-committed
/// state: the committer holds the owner's lock across its write, and
/// cache-miss reads take the same lock, so a repopulation started before the
/// write can only finish reading the table after it.
#[tokio::test]
async fn test_balance_cache_repopulation_cannot_race_a_commit() {
    let index_store = std::sync::Arc::new(open_index_store(
        iota_common::tempdir().path().to_path_buf(),
    ));
    let address = Address::random();
    insert_gas_coin(&index_store, address, 100);

    // Stand in for a commit in progress: the owner's lock held across the
    // second coin's write, the way a real commit holds it across the
    // checkpoint's batch write and the cache merge that follows.
    let lock = index_store.caches.locks.acquire_lock(address);
    let reader = std::thread::spawn({
        let index_store = index_store.clone();
        move || index_store.get_balance(address, GAS::type_tag()).unwrap()
    });
    // Give the reader time to reach the owner's lock. The sleep only makes
    // the race likely: a slow reader arrives after the write and the test
    // passes without exercising it.
    std::thread::sleep(std::time::Duration::from_millis(50));

    // The second coin lands on disk while the lock is still held; the
    // reader must not be able to observe this half-committed state.
    insert_gas_coin(&index_store, address, 100);
    drop(lock);

    let balance = reader.join().unwrap();
    assert_eq!(
        balance.balance, 200,
        "the repopulation must see both coins, never just one"
    );
    assert_eq!(balance.num_coins, 2);
}

#[tokio::test]
async fn test_a_balance_miss_takes_the_value_cached_while_it_waited() {
    let index_store = std::sync::Arc::new(open_index_store(
        iota_common::tempdir().path().to_path_buf(),
    ));
    let address = Address::random();
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
        index_store.jsonrpc_metrics.balance_lookup_from_db.get(),
        0,
        "the waiting reader must not repeat the coin scan"
    );
}

/// As above, for the all-balances cache.
#[tokio::test]
async fn test_an_all_balance_miss_takes_the_value_cached_while_it_waited() {
    let index_store = std::sync::Arc::new(open_index_store(
        iota_common::tempdir().path().to_path_buf(),
    ));
    let address = Address::random();
    let cached = std::sync::Arc::new(HashMap::from([(
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
        index_store.jsonrpc_metrics.all_balance_lookup_from_db.get(),
        0,
        "the waiting reader must not repeat the coin scan"
    );
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
        ) -> Result<MoveDatatypeLayout, IotaError> {
            self.calls += 1;
            Ok(MoveDatatypeLayout::Struct(Box::new(MoveStructLayout {
                type_: "0x2::coin::Coin".parse().unwrap(),
                fields: vec![],
            })))
        }
    }

    let mut inner = Counting { calls: 0 };
    let mut caching = CachingLayoutResolver::new(&mut inner);
    let coin: StructTag = "0x2::coin::Coin<0x2::iota::IOTA>".parse().unwrap();
    let cap: StructTag = "0x2::coin::TreasuryCap<0x2::iota::IOTA>".parse().unwrap();
    caching.get_annotated_layout(&coin).unwrap();
    caching.get_annotated_layout(&coin).unwrap();
    caching.get_annotated_layout(&cap).unwrap();
    drop(caching);
    assert_eq!(inner.calls, 2, "one resolution per distinct tag");
}

/// Four dynamic field ids of `parent`, in the order the index stores them.
fn seed_dynamic_fields(index_store: &RpcIndexesStore, parent: ObjectId) -> Vec<ObjectId> {
    let mut field_ids: Vec<ObjectId> = (0..4).map(|_| ObjectId::random()).collect();
    field_ids.sort();
    let table = &index_store.tables.dynamic_field;
    let mut batch = table.batch();
    batch
        .insert_batch(
            table,
            field_ids
                .iter()
                .map(|id| (DynamicFieldKey::new(parent, *id), ())),
        )
        .unwrap();
    batch.write().unwrap();
    field_ids
}

fn dynamic_field_page(
    index_store: &RpcIndexesStore,
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
    let index_store = open_index_store(iota_common::tempdir().path().to_path_buf());
    let parent = ObjectId::random();
    let field_ids = seed_dynamic_fields(&index_store, parent);

    assert_eq!(dynamic_field_page(&index_store, parent, None), field_ids);
    assert_eq!(
        dynamic_field_page(&index_store, parent, Some(field_ids[0])),
        &field_ids[1..]
    );

    // The field the cursor points at can be removed between two pages.
    let table = &index_store.tables.dynamic_field;
    let mut batch = table.batch();
    batch
        .delete_batch(table, [DynamicFieldKey::new(parent, field_ids[0])])
        .unwrap();
    batch.write().unwrap();

    assert_eq!(
        dynamic_field_page(&index_store, parent, Some(field_ids[0])),
        &field_ids[1..],
        "the field after the cursor must not be lost with the cursor's row"
    );
}

/// Four objects of two coin types for `owner`, inserted into both the owner
/// index and `object_store`, as the live indexer would.
fn seed_owner_objects_of_two_types(
    index_store: &RpcIndexesStore,
    object_store: &mut BTreeMap<ObjectId, Object>,
    owner: Address,
) {
    let coins = [
        (GAS::type_tag(), 300u64),
        (GAS::type_tag(), 100u64),
        (TypeTag::U64, 500u64),
        (TypeTag::U64, 50u64),
    ];
    for (coin_type, balance) in coins {
        let object = Object::new_move(
            MoveStruct::new_coin(
                coin_type,
                Version::MIN_VALID_INCL,
                ObjectId::random(),
                balance,
            ),
            Owner::Address(owner),
            TransactionDigest::GENESIS_MARKER,
        );
        let (key, info) = OwnerIndexKey::for_object(owner, &object).unwrap();
        index_store.tables.owner.insert(&key, &info).unwrap();
        object_store.insert(object.id(), object);
    }
}

/// Pages follow the unified key order and the ObjectId cursor continues
/// exactly where the previous page stopped.
#[tokio::test]
async fn test_owner_pages_follow_the_unified_key_order() {
    let index_store = open_index_store(iota_common::tempdir().path().to_path_buf());
    let owner = Address::random();
    let mut object_store = BTreeMap::new();
    seed_owner_objects_of_two_types(&index_store, &mut object_store, owner);

    let full = index_store
        .get_owner_objects(owner, None, 4, None, &object_store)
        .unwrap();
    assert_eq!(full.len(), 4, "all four seeded objects must resolve");

    let page_1 = index_store
        .get_owner_objects(owner, None, 2, None, &object_store)
        .unwrap();
    let cursor = page_1.last().unwrap().object_id;
    let page_2 = index_store
        .get_owner_objects(owner, Some(cursor), 2, None, &object_store)
        .unwrap();

    assert_eq!(page_1.len(), 2);
    assert_eq!(page_2.len(), 2);
    assert_eq!(
        [page_1, page_2]
            .concat()
            .iter()
            .map(|o| o.object_id)
            .collect::<Vec<_>>(),
        full.iter().map(|o| o.object_id).collect::<Vec<_>>(),
        "two pages of 2 must partition the full scan in the same order"
    );

    assert_eq!(
        index_store
            .get_owner_objects(owner, None, 0, None, &object_store)
            .unwrap(),
        vec![],
        "a zero limit must return no rows, not the first matching one"
    );
}

/// A cursor whose object was deleted between pages is refused instead of
/// silently restarting the scan.
#[tokio::test]
async fn test_owner_cursor_of_a_deleted_object_is_refused() {
    let index_store = open_index_store(iota_common::tempdir().path().to_path_buf());
    let owner = Address::random();
    let mut object_store = BTreeMap::new();
    seed_owner_objects_of_two_types(&index_store, &mut object_store, owner);

    let page_1 = index_store
        .get_owner_objects(owner, None, 2, None, &object_store)
        .unwrap();
    let cursor = page_1.last().unwrap().object_id;
    object_store.remove(&cursor);

    let result = index_store.get_owner_objects(owner, Some(cursor), 2, None, &object_store);
    assert!(
        matches!(
            result,
            Err(IotaError::UserInput {
                error: UserInputError::ObjectNotFound { .. }
            })
        ),
        "unexpected result: {result:?}"
    );
}

/// A cursor whose object exists but carries no Move type (a package) is
/// refused instead of panicking: `OwnerIndexKey::for_object` returns `None`
/// for it exactly as it would for a deleted object, and the cursor rebuild
/// must treat the two cases alike.
#[tokio::test]
async fn test_owner_cursor_of_a_package_is_refused() {
    let index_store = open_index_store(iota_common::tempdir().path().to_path_buf());
    let owner = Address::random();
    let mut object_store = BTreeMap::new();
    seed_owner_objects_of_two_types(&index_store, &mut object_store, owner);

    let package = MovePackage::new(
        ObjectId::random(),
        Version::MIN_VALID_INCL,
        BTreeMap::new(),
        u64::MAX,
        Vec::new(),
        BTreeMap::new(),
    )
    .unwrap();
    let package_id = package.id;
    let package_object = Object::new_package_from_data(
        ObjectData::Package(package),
        TransactionDigest::GENESIS_MARKER,
    );
    object_store.insert(package_id, package_object);

    let result = index_store.get_owner_objects(owner, Some(package_id), 2, None, &object_store);
    assert!(
        matches!(
            result,
            Err(IotaError::UserInput {
                error: UserInputError::ObjectNotFound { .. }
            })
        ),
        "unexpected result: {result:?}"
    );
}

/// Coin pages follow the unified key's balance-descending order, both
/// narrowed to one coin type and across every coin type, and the cursor
/// continues exactly where the previous page stopped.
#[tokio::test]
async fn test_owned_coins_pages_follow_the_unified_key_order() {
    let index_store = open_index_store(iota_common::tempdir().path().to_path_buf());
    let owner = Address::random();
    let mut object_store = BTreeMap::new();
    seed_owner_objects_of_two_types(&index_store, &mut object_store, owner);

    // Narrowed to one coin type: only the two `Coin<IOTA>` rows, richest
    // first, the `Coin<u64>` rows excluded entirely.
    let gas_coin_type = StructTag::new_coin(GAS::type_tag());
    let one_type = index_store
        .get_owned_coins_iterator_with_cursor(owner, None, Some(gas_coin_type), 10, &object_store)
        .unwrap();
    assert_eq!(
        one_type
            .iter()
            .map(|(_, _, coin)| coin.balance)
            .collect::<Vec<_>>(),
        vec![300, 100],
        "narrowing to one coin type must exclude the other and stay balance-descending"
    );

    // Every coin type, paginated in two pages of two, must partition the
    // full scan in the same order.
    let full = index_store
        .get_owned_coins_iterator_with_cursor(owner, None, None, 4, &object_store)
        .unwrap();
    assert_eq!(full.len(), 4, "all four seeded coins must resolve");

    let page_1 = index_store
        .get_owned_coins_iterator_with_cursor(owner, None, None, 2, &object_store)
        .unwrap();
    let cursor = page_1.last().unwrap().1;
    let page_2 = index_store
        .get_owned_coins_iterator_with_cursor(owner, Some(cursor), None, 2, &object_store)
        .unwrap();

    assert_eq!(page_1.len(), 2);
    assert_eq!(page_2.len(), 2);
    assert_eq!(
        [page_1, page_2]
            .concat()
            .iter()
            .map(|(_, id, _)| *id)
            .collect::<Vec<_>>(),
        full.iter().map(|(_, id, _)| *id).collect::<Vec<_>>(),
        "two pages of 2 must partition the full scan in the same order"
    );

    assert_eq!(
        index_store
            .get_owned_coins_iterator_with_cursor(owner, None, None, 0, &object_store)
            .unwrap(),
        vec![],
        "a zero limit must return no rows, not the first matching one"
    );
}

/// `get_balance` excludes other coin types and `get_all_balance` groups by
/// the exact `Coin<T>` — the two pieces of logic that replaced the deleted
/// `coin_index` table's per-type keying.
#[tokio::test]
async fn test_balance_reads_narrow_and_group_by_coin_type() {
    let index_store = open_index_store(iota_common::tempdir().path().to_path_buf());
    let owner = Address::random();
    let mut object_store = BTreeMap::new();
    seed_owner_objects_of_two_types(&index_store, &mut object_store, owner);

    let gas_balance = index_store.get_balance(owner, GAS::type_tag()).unwrap();
    assert_eq!(
        gas_balance.balance, 400,
        "must sum only the Coin<IOTA> rows, not the Coin<u64> ones"
    );
    assert_eq!(gas_balance.num_coins, 2);

    let u64_balance = index_store.get_balance(owner, TypeTag::U64).unwrap();
    assert_eq!(u64_balance.balance, 550);
    assert_eq!(u64_balance.num_coins, 2);

    let all_balances = index_store.get_all_balance(owner).unwrap();
    assert_eq!(all_balances.len(), 2, "one entry per coin type");
    assert_eq!(all_balances.get(&GAS::type_tag()).unwrap().balance, 400);
    assert_eq!(all_balances.get(&GAS::type_tag()).unwrap().num_coins, 2);
    assert_eq!(all_balances.get(&TypeTag::U64).unwrap().balance, 550);
    assert_eq!(all_balances.get(&TypeTag::U64).unwrap().num_coins, 2);
}
