// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::collections::{BTreeMap, BTreeSet, HashMap};

use iota_json_rpc_types::IotaObjectDataFilter;
use iota_sdk_types::{
    Address, Event, Identifier, MoveStruct, ObjectData, ObjectId, Owner, StructTag,
    TransactionDigest, TypeTag, Version, move_package::MovePackage,
};
use iota_types::{
    base_types::ObjectInfo,
    effects::TransactionEffectsAPI,
    error::IotaError,
    full_checkpoint_content::CheckpointData,
    messages_checkpoint::CheckpointContentsExt,
    object::{MoveStructExt, Object},
    storage::{DynamicFieldKey, OwnedObjectCursor, PackageVersionInfo, PackageVersionKey},
    test_checkpoint_data_builder::TestCheckpointDataBuilder,
};
use prometheus_filtered::Registry;
use typed_store::Map;

use super::{
    IndexGroup, RpcIndexesStore,
    jsonrpc_api::CachingLayoutResolver,
    live_scan::RpcIndexesRestorer,
    schema::{CoinIndexInfo, CoinIndexKey, OwnerIndexKey},
};
use crate::{checkpoints::CheckpointStore, test_utils::executed_checkpoint};

/// Prunes anchored on the store's newest history bucket, which is the epoch
/// the node is entering when it prunes at a reconfiguration.
fn prune_at_newest_epoch(store: &RpcIndexesStore) -> iota_types::error::IotaResult<Option<u64>> {
    let newest = store.retained_history_epochs().last().copied().unwrap_or(0);
    store.prune(newest)
}

/// Opens an `RpcIndexesStore` at `path` without running the rebuild path,
/// serving every group.
fn open_index_store(path: std::path::PathBuf) -> RpcIndexesStore {
    RpcIndexesStore::new_without_init(
        path,
        BTreeSet::from([IndexGroup::JsonRpc, IndexGroup::Grpc]),
    )
}

/// Opens an `RpcIndexesStore` at `path` without running the rebuild path,
/// serving every group, with an explicit epoch retention.
fn open_index_store_with_retention(
    path: &std::path::Path,
    epochs_to_retain: Option<u64>,
) -> RpcIndexesStore {
    RpcIndexesStore::new_without_init_with_retention(
        path.to_path_buf(),
        BTreeSet::from([IndexGroup::JsonRpc, IndexGroup::Grpc]),
        epochs_to_retain,
    )
}

/// Closes the store's database, waiting until every handle is released
/// so the same path can be reopened.
async fn close_index_store(index_store: impl std::borrow::Borrow<RpcIndexesStore>) {
    let weak_db = std::sync::Arc::downgrade(&index_store.borrow().tables.meta.db);
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
    let (perpetual_tables, historic_objects, historic_ledger, epoch_markers) =
        crate::authority::authority_store_tables::AuthorityPerpetualTables::
            open_with_historic_objects(dir, None)
            .unwrap();
    crate::authority::AuthorityStore::open_no_genesis(
        std::sync::Arc::new(perpetual_tables),
        std::sync::Arc::new(historic_objects),
        std::sync::Arc::new(historic_ledger),
        std::sync::Arc::new(epoch_markers),
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

/// A `Field` object of a dynamic field of `parent`, the only kind of object
/// the dynamic-field index stores.
fn dynamic_field_object(parent: ObjectId, field_id: ObjectId) -> Object {
    let mut contents = field_id.into_bytes().to_vec();
    contents.extend_from_slice(&7u64.to_le_bytes()); // name
    contents.extend_from_slice(&8u64.to_le_bytes()); // value
    Object::new_move(
        MoveStruct::new_from_execution_with_limit(
            "0x2::dynamic_field::Field<u64,u64>"
                .parse::<StructTag>()
                .unwrap(),
            Version::MIN_VALID_INCL,
            contents,
            256,
        )
        .unwrap(),
        Owner::Object(parent),
        TransactionDigest::ZERO,
    )
}

/// An object of `object_type` with a random id. The ingest reads only the
/// type of the objects whose rows depend on it, so the contents are just the
/// object's own id.
fn typed_object_for_testing(object_type: StructTag, owner: Owner) -> Object {
    let object_id = ObjectId::random();
    Object::new_move(
        MoveStruct::new_from_execution_with_limit(
            object_type,
            Version::MIN_VALID_INCL,
            object_id.into_bytes().to_vec(),
            256,
        )
        .unwrap(),
        owner,
        TransactionDigest::ZERO,
    )
}

/// A Move package object, the only kind of object the package-version index
/// stores.
fn package_object_for_testing() -> Object {
    // The first version of a package is its own original package, so the
    // index needs no module bytes to derive the key from.
    let package = MovePackage::new(
        ObjectId::random(),
        Version::OBJECT_START,
        BTreeMap::new(),
        u64::MAX,
        Vec::new(),
        BTreeMap::new(),
    )
    .unwrap();
    Object::new_package_from_data(
        ObjectData::Package(package),
        TransactionDigest::GENESIS_MARKER,
    )
}

/// Gives the object the checkpoint created under `object_idx` the type
/// `object_type`, keeping its id and version so the transaction's effects
/// still resolve it, and returns its id. The checkpoint builder creates only
/// coins, while the gRPC group's tables are filled from the type of a
/// created object.
fn replace_created_object_type(
    checkpoint: &mut CheckpointData,
    object_idx: u64,
    object_type: StructTag,
) -> ObjectId {
    let object_id = TestCheckpointDataBuilder::derive_object_id(object_idx);
    let mut replaced = false;
    for tx in &mut checkpoint.transactions {
        for object in &mut tx.output_objects {
            if object.id() != object_id {
                continue;
            }
            *object = Object::new_move(
                MoveStruct::new_from_execution_with_limit(
                    object_type.clone(),
                    object.version(),
                    object_id.into_bytes().to_vec(),
                    256,
                )
                .unwrap(),
                object.owner,
                object.previous_transaction,
            );
            replaced = true;
        }
    }
    assert!(replaced, "the checkpoint created no object {object_idx}");
    object_id
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

    let mut tables = super::IndexStoreTables::open_for_bulk_ingestion(dir.path().join("indexes"));
    tables
        .init(
            &authority_store,
            &checkpoint_store,
            &groups,
            1024,
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
        index_store.lookup_digest(&genesis_tx_digest).unwrap(),
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
    // One historic epoch retained on top of the current one (epoch 2), so
    // three epochs must exist for the oldest, epoch 0, to fall out of it.
    seed_history_buckets(&index_store, 3);
    assert_eq!(prune_at_newest_epoch(&index_store).unwrap(), Some(1));
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
    assert_eq!(prune_at_newest_epoch(&index_store).unwrap(), Some(1));

    let mut index_store = reopen_index_store(index_store, tmp_dir.path().to_path_buf()).await;
    index_store.epochs_to_retain = Some(52);
    assert_eq!(
        prune_at_newest_epoch(&index_store).unwrap(),
        Some(1),
        "a retention reaching below the dropped epochs must not lower the floor"
    );
    assert!(index_store.ensure_history_bucket(0).is_err());
    assert!(index_store.ensure_history_bucket(1).is_ok());
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
    index_store.epochs_to_retain = Some(6);
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
        Some(0),
        "the digest table must fill without the JSON-RPC group"
    );
    let bucket = store.history.ensure(0).unwrap();
    assert!(
        bucket.tx_order.safe_iter().next().is_none(),
        "the JSON-RPC history tables must stay empty"
    );
}

/// Stages and commits one checkpoint's index update, the way the node's
/// execution path does.
fn index_checkpoint_for_testing(store: &RpcIndexesStore, checkpoint: &CheckpointData) {
    store.index_checkpoint(checkpoint).unwrap();
    store
        .commit_update_for_checkpoint(checkpoint.checkpoint_summary.sequence_number)
        .unwrap();
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

    // Pruning with no historic epochs retained keeps the current epoch
    // only, dropping epoch 0's bucket wholesale; pruning again is a no-op.
    index_store.epochs_to_retain = Some(0);
    assert_eq!(prune_at_newest_epoch(&index_store).unwrap(), Some(1));
    assert_eq!(index_store.lookup_digest(&tx_0).unwrap(), None);
    assert_eq!(
        index_store
            .get_transactions(None, None, None, false)
            .unwrap(),
        vec![tx_1]
    );
    assert_eq!(prune_at_newest_epoch(&index_store).unwrap(), Some(1));

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
        struct_tag: StructTag::new_gas(),
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
            TypeTag::from(StructTag::new_gas()),
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
        .get_balance_from_db(address, &TypeTag::from(StructTag::new_gas()))
        .unwrap();
    let balance = index_store
        .get_balance(address, TypeTag::from(StructTag::new_gas()))
        .unwrap();
    assert_eq!(balance, balance_from_db);
    assert_eq!(balance.balance, 1000);
    assert_eq!(balance.num_coins, 10);

    let all_balance = index_store.get_all_balance(address).unwrap();
    let balance = all_balance
        .get(&TypeTag::from(StructTag::new_gas()))
        .unwrap();
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
        .invalidate(&(address, TypeTag::from(StructTag::new_gas())));
    index_store.caches.all_balances.invalidate(&address);

    let balance_from_db = index_store
        .get_balance_from_db(address, &TypeTag::from(StructTag::new_gas()))
        .unwrap();
    let balance = index_store
        .get_balance(address, TypeTag::from(StructTag::new_gas()))
        .unwrap();
    assert_eq!(balance, balance_from_db);
    assert_eq!(balance.balance, 700);
    assert_eq!(balance.num_coins, 7);

    // Invalidate per coin type balance cache and read from all balance cache to
    // ensure the balance matches
    index_store
        .caches
        .per_coin_type_balance
        .invalidate(&(address, TypeTag::from(StructTag::new_gas())));
    let all_balance = index_store.get_all_balance(address).unwrap();
    assert_eq!(
        all_balance
            .get(&TypeTag::from(StructTag::new_gas()))
            .unwrap()
            .balance,
        700
    );
    assert_eq!(
        all_balance
            .get(&TypeTag::from(StructTag::new_gas()))
            .unwrap()
            .num_coins,
        7
    );
    let balance = index_store
        .get_balance(address, TypeTag::from(StructTag::new_gas()))
        .unwrap();
    assert_eq!(balance, balance_from_db);
    assert_eq!(balance.balance, 700);
    assert_eq!(balance.num_coins, 7);
}

/// A cache-miss repopulation racing a commit must not double-apply the
/// checkpoint's delta: the committer holds the owner's lock from the delta
/// computation through the cache merge, and cache-miss reads take the same
/// lock, so a value read between the batch write and the merge can never be
/// merged onto.
#[tokio::test]
async fn test_balance_cache_repopulation_cannot_race_a_commit() {
    let index_store = std::sync::Arc::new(open_index_store(
        iota_common::tempdir().path().to_path_buf(),
    ));
    let address = TestCheckpointDataBuilder::derive_address(1);

    let mut builder = TestCheckpointDataBuilder::new(0)
        .start_transaction(0)
        .create_coin_object(0, 1, 100, TypeTag::from(StructTag::new_gas()))
        .finish_transaction();
    let checkpoint = builder.build_checkpoint();
    index_checkpoint_for_testing(&index_store, &checkpoint);

    // A second coin for the same owner in checkpoint 1.
    let mut builder = builder
        .start_transaction(0)
        .create_coin_object(1, 1, 100, TypeTag::from(StructTag::new_gas()))
        .finish_transaction();
    let checkpoint = builder.build_checkpoint();
    index_store.index_checkpoint(&checkpoint).unwrap();

    // Replay the commit by hand, pausing between the batch write and the
    // cache merge — the window where an unlocked reader used to cache the
    // post-write value the merge was then applied on top of.
    let reader = {
        let (staged_seq, update) = index_store.pending_updates.lock().pop_first().unwrap();
        assert_eq!(staged_seq, 1);
        let cache_updates = index_store.balance_cache_updates(update.coin_changes);
        update.batch.write().unwrap();

        let reader = std::thread::spawn({
            let index_store = index_store.clone();
            move || {
                index_store
                    .get_balance(address, TypeTag::from(StructTag::new_gas()))
                    .unwrap()
            }
        });
        // Give the reader time to reach the owner's lock. The sleep only
        // makes the race likely: a slow reader arrives after the merge and
        // the test passes without exercising it.
        std::thread::sleep(std::time::Duration::from_millis(50));

        index_store.merge_balance_cache_updates(cache_updates);
        reader
        // The owner locks in `cache_updates` release here.
    };

    assert_eq!(reader.join().unwrap().balance, 200);
    let cached = index_store
        .get_balance(address, TypeTag::from(StructTag::new_gas()))
        .unwrap();
    assert_eq!(cached.balance, 200);
    assert_eq!(cached.num_coins, 2);
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
        move || {
            index_store
                .get_balance(address, TypeTag::from(StructTag::new_gas()))
                .unwrap()
        }
    });
    // Give the reader time to reach the owner's lock. A slow one passes
    // without exercising the race.
    std::thread::sleep(std::time::Duration::from_millis(50));

    index_store
        .caches
        .per_coin_type_balance
        .get_with((address, TypeTag::from(StructTag::new_gas())), || {
            Ok(cached)
        })
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
        TypeTag::from(StructTag::new_gas()),
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
        (TypeTag::from(StructTag::new_gas()), 300u64),
        (TypeTag::from(StructTag::new_gas()), 100u64),
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
    let cursor = page_1.last().unwrap().1;
    let page_2 = index_store
        .get_owner_objects(owner, Some(&cursor), 2, None, &object_store)
        .unwrap();

    assert_eq!(page_1.len(), 2);
    assert_eq!(page_2.len(), 2);
    assert_eq!(
        [page_1, page_2]
            .concat()
            .iter()
            .map(|(o, _)| o.object_id)
            .collect::<Vec<_>>(),
        full.iter().map(|(o, _)| o.object_id).collect::<Vec<_>>(),
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

/// A filter that pins the object type narrows the index scan, so the page
/// must still hold exactly what the unfiltered scan filtered in memory holds,
/// both in one page and paged one row at a time. Filters that pin no type
/// keep walking everything the owner holds.
#[tokio::test]
async fn test_filtered_owner_pages_match_the_unfiltered_scan() {
    let index_store = open_index_store(iota_common::tempdir().path().to_path_buf());
    let owner = Address::random();
    let mut object_store = BTreeMap::new();
    seed_owner_objects_of_two_types(&index_store, &mut object_store, owner);
    // Objects of a type no coin filter matches, so narrowing has rows to
    // leave out.
    for _ in 0..3 {
        let object =
            typed_object_for_testing("0x42::thing::Thing".parse().unwrap(), Owner::Address(owner));
        let (key, info) = OwnerIndexKey::for_object(owner, &object).unwrap();
        index_store.tables.owner.insert(&key, &info).unwrap();
        object_store.insert(object.id(), object);
    }

    let unfiltered = index_store
        .get_owner_objects(owner, None, 100, None, &object_store)
        .unwrap();
    assert_eq!(unfiltered.len(), 7, "all seeded objects must resolve");

    let coin_of_iota: StructTag = "0x2::coin::Coin<0x2::iota::IOTA>".parse().unwrap();
    let filters = [
        IotaObjectDataFilter::StructType(coin_of_iota.clone()),
        // No type parameters: every coin type.
        IotaObjectDataFilter::StructType("0x2::coin::Coin".parse().unwrap()),
        IotaObjectDataFilter::StructType("0x42::thing::Thing".parse().unwrap()),
        // Narrowed through the `MatchAll`, and still checked per row.
        IotaObjectDataFilter::MatchAll(vec![
            IotaObjectDataFilter::StructType(coin_of_iota.clone()),
            IotaObjectDataFilter::AddressOwner(owner),
        ]),
        // Pins no type: the full scan, filtered per row.
        IotaObjectDataFilter::AddressOwner(owner),
        IotaObjectDataFilter::MoveModule {
            package: ObjectId::from(coin_of_iota.address()),
            module: coin_of_iota.module().to_owned(),
        },
    ];
    for filter in filters {
        let expected = unfiltered
            .iter()
            .filter(|(object_info, _)| filter.matches(object_info))
            .cloned()
            .collect::<Vec<_>>();
        assert!(
            !expected.is_empty(),
            "the filter {filter:?} must match some of the seeded objects"
        );
        assert_eq!(
            index_store
                .get_owner_objects(owner, None, 100, Some(filter.clone()), &object_store)
                .unwrap(),
            expected,
            "the filter {filter:?} must page like the in-memory filter"
        );

        // The same rows, one page at a time: the cursor of a narrowed scan
        // has to resume inside the narrowed bounds.
        let mut paged = Vec::new();
        let mut cursor = None;
        loop {
            let page = index_store
                .get_owner_objects(
                    owner,
                    cursor.as_ref(),
                    1,
                    Some(filter.clone()),
                    &object_store,
                )
                .unwrap();
            let Some(last) = page.last() else { break };
            cursor = Some(last.1);
            paged.extend(page);
        }
        assert_eq!(
            paged, expected,
            "paging the filter {filter:?} one row at a time must yield the same rows"
        );
    }
}

/// A cursor carries the position of the row it came from, so a page resumes
/// after an object that has since been spent. The position used to be rebuilt
/// by reading the object, which made a spent cursor fail the whole page — and
/// a page of a wallet's coins is exactly where an object goes missing between
/// two reads.
#[tokio::test]
async fn test_owner_cursor_of_a_deleted_object_still_resumes() {
    let index_store = open_index_store(iota_common::tempdir().path().to_path_buf());
    let owner = Address::random();
    let mut object_store = BTreeMap::new();
    seed_owner_objects_of_two_types(&index_store, &mut object_store, owner);

    let full = index_store
        .get_owner_objects(owner, None, 4, None, &object_store)
        .unwrap();
    let page_1 = index_store
        .get_owner_objects(owner, None, 2, None, &object_store)
        .unwrap();
    let cursor = page_1.last().unwrap().1;

    // The cursor's object goes away between the two pages.
    object_store.remove(&cursor.object_id);

    let page_2 = index_store
        .get_owner_objects(owner, Some(&cursor), 2, None, &object_store)
        .unwrap();
    assert_eq!(
        page_2.iter().map(|(o, _)| o.object_id).collect::<Vec<_>>(),
        full[2..]
            .iter()
            .map(|(o, _)| o.object_id)
            .collect::<Vec<_>>(),
        "the page after a spent cursor must be the rest of the scan"
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
    let one_type = index_store
        .get_owned_coins(
            owner,
            None,
            Some(TypeTag::from(StructTag::new_gas())),
            10,
            &object_store,
        )
        .unwrap();
    assert_eq!(
        one_type
            .iter()
            .map(|(_, _, coin, _)| coin.balance)
            .collect::<Vec<_>>(),
        vec![300, 100],
        "narrowing to one coin type must exclude the other and stay balance-descending"
    );
    // The reported type is the coin's own `T`, not the `Coin<T>` the object
    // is: it is what the JSON-RPC `coinType` field carries and what
    // `get_all_balance` keys on.
    assert!(
        one_type
            .iter()
            .all(|(coin_type, _, _, _)| *coin_type == TypeTag::from(StructTag::new_gas())),
        "the coin type must be the inner T, found {:?}",
        one_type.iter().map(|(t, _, _, _)| t).collect::<Vec<_>>()
    );
    // Across every coin type, the reported types agree with the ones
    // `get_all_balance` groups by.
    let all_balance_types: BTreeSet<TypeTag> = index_store
        .get_all_balance(owner)
        .unwrap()
        .keys()
        .cloned()
        .collect();
    let page_types: BTreeSet<TypeTag> = index_store
        .get_owned_coins(owner, None, None, 4, &object_store)
        .unwrap()
        .into_iter()
        .map(|(coin_type, _, _, _)| coin_type)
        .collect();
    assert_eq!(
        page_types, all_balance_types,
        "the coin page and the balance map must report the same coin types"
    );

    // Every coin type, paginated in two pages of two, must partition the
    // full scan in the same order.
    let full = index_store
        .get_owned_coins(owner, None, None, 4, &object_store)
        .unwrap();
    assert_eq!(full.len(), 4, "all four seeded coins must resolve");

    let page_1 = index_store
        .get_owned_coins(owner, None, None, 2, &object_store)
        .unwrap();
    let cursor = page_1.last().unwrap().3;
    let page_2 = index_store
        .get_owned_coins(owner, Some(&cursor), None, 2, &object_store)
        .unwrap();

    assert_eq!(page_1.len(), 2);
    assert_eq!(page_2.len(), 2);
    assert_eq!(
        [page_1, page_2]
            .concat()
            .iter()
            .map(|(_, id, _, _)| *id)
            .collect::<Vec<_>>(),
        full.iter().map(|(_, id, _, _)| *id).collect::<Vec<_>>(),
        "two pages of 2 must partition the full scan in the same order"
    );

    assert_eq!(
        index_store
            .get_owned_coins(owner, None, None, 0, &object_store)
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

    let gas_balance = index_store
        .get_balance(owner, TypeTag::from(StructTag::new_gas()))
        .unwrap();
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
    assert_eq!(
        all_balances
            .get(&TypeTag::from(StructTag::new_gas()))
            .unwrap()
            .balance,
        400
    );
    assert_eq!(
        all_balances
            .get(&TypeTag::from(StructTag::new_gas()))
            .unwrap()
            .num_coins,
        2
    );
    assert_eq!(all_balances.get(&TypeTag::U64).unwrap().balance, 550);
    assert_eq!(all_balances.get(&TypeTag::U64).unwrap().num_coins, 2);
}

// ---------------------------------------------------------------------------
// gRPC read surface
// ---------------------------------------------------------------------------

/// `lookup_digest` probes every retained epoch's bucket, newest first.
#[tokio::test]
async fn test_lookup_digest_probes_across_epoch_buckets() {
    let index_store = open_index_store(iota_common::tempdir().path().to_path_buf());
    let (old_digest, new_digest) = (TransactionDigest::random(), TransactionDigest::random());

    let old_bucket = index_store.ensure_history_bucket(0).unwrap();
    let mut batch = index_store.tables.meta.batch();
    batch
        .insert_batch_tagged(&old_bucket.txs_seq, [(old_digest, 0)])
        .unwrap();
    batch.write().unwrap();

    let new_bucket = index_store.ensure_history_bucket(1).unwrap();
    let mut batch = index_store.tables.meta.batch();
    batch
        .insert_batch_tagged(&new_bucket.txs_seq, [(new_digest, 1)])
        .unwrap();
    batch.write().unwrap();

    assert_eq!(index_store.lookup_digest(&old_digest).unwrap(), Some(0));
    assert_eq!(index_store.lookup_digest(&new_digest).unwrap(), Some(1));
    assert!(
        index_store
            .lookup_digest(&TransactionDigest::random())
            .unwrap()
            .is_none()
    );
}

/// Buckets are rediscovered from the on-disk column-family names on
/// reopen, and their digest rows survive with them.
#[tokio::test]
async fn test_digest_buckets_survive_a_reopen() {
    let tmp_dir = iota_common::tempdir();
    let index_store = open_index_store(tmp_dir.path().to_path_buf());
    let digest = TransactionDigest::random();
    let bucket = index_store.ensure_history_bucket(3).unwrap();
    let mut batch = index_store.tables.meta.batch();
    batch
        .insert_batch_tagged(&bucket.txs_seq, [(digest, 2)])
        .unwrap();
    batch.write().unwrap();
    drop(bucket); // release the database handle before closing it below

    let index_store = reopen_index_store(index_store, tmp_dir.path().to_path_buf()).await;
    assert_eq!(index_store.history.newest_epoch(), Some(3));
    assert_eq!(index_store.lookup_digest(&digest).unwrap(), Some(2));
}

/// Pruning with no historic epochs retained drops whole epoch buckets below
/// the current one, digests included, and the floor survives a reopen.
#[tokio::test]
async fn test_digest_pruning_drops_expired_epoch_buckets() {
    let tmp_dir = iota_common::tempdir();
    let mut index_store = open_index_store(tmp_dir.path().to_path_buf());
    index_store.epochs_to_retain = Some(0);
    let old_digest = TransactionDigest::random();
    let old_bucket = index_store.ensure_history_bucket(0).unwrap();
    let mut batch = index_store.tables.meta.batch();
    batch
        .insert_batch_tagged(&old_bucket.txs_seq, [(old_digest, 0)])
        .unwrap();
    batch.write().unwrap();
    let new_bucket = index_store.ensure_history_bucket(1).unwrap();
    let mut batch = index_store.tables.meta.batch();
    batch
        .insert_batch_tagged(&new_bucket.txs_seq, [(TransactionDigest::random(), 1)])
        .unwrap();
    batch.write().unwrap();
    drop(old_bucket); // release the database handles before closing it below
    drop(new_bucket);

    assert_eq!(prune_at_newest_epoch(&index_store).unwrap(), Some(1));
    assert_eq!(index_store.lookup_digest(&old_digest).unwrap(), None);
    assert!(
        index_store.ensure_history_bucket(0).is_err(),
        "a pruned epoch must not be recreated"
    );

    let index_store = reopen_index_store(index_store, tmp_dir.path().to_path_buf()).await;
    assert!(
        index_store.ensure_history_bucket(0).is_err(),
        "the retention floor must survive a reopen"
    );
}

/// Every gRPC read must fail explicitly instead of silently answering from
/// an unmaintained table when this store does not serve the gRPC group.
#[tokio::test]
async fn test_grpc_reads_fail_when_the_group_is_disabled() {
    use iota_node_storage::GrpcIndexes;

    let index_store = RpcIndexesStore::new_without_init(
        iota_common::tempdir().path().to_path_buf(),
        BTreeSet::from([IndexGroup::JsonRpc]),
    );

    assert!(
        index_store
            .dynamic_field_iter(ObjectId::random(), None)
            .is_err()
    );
    assert!(index_store.get_coin_info(&StructTag::new_gas()).is_err());
    assert!(
        index_store
            .package_versions_iter(ObjectId::random(), None)
            .is_err()
    );
    assert!(
        index_store
            .account_owned_objects_info_iter(Address::random(), None, None)
            .is_err()
    );
}

/// Regulated coin metadata round-trips through `get_coin_info`, on both the
/// inherent method and the `GrpcIndexes` trait's conversion to the public
/// `CoinInfo` type.
#[tokio::test]
async fn test_get_coin_info_reads_regulated_metadata() {
    use iota_node_storage::GrpcIndexes;

    let index_store = open_index_store(iota_common::tempdir().path().to_path_buf());
    let coin_type = StructTag::new_gas();
    let info = CoinIndexInfo {
        coin_metadata_object_id: Some(ObjectId::random()),
        treasury_object_id: Some(ObjectId::random()),
        regulated_coin_metadata_object_id: Some(ObjectId::random()),
    };
    index_store
        .tables
        .coin
        .insert(
            &CoinIndexKey {
                coin_type: coin_type.clone(),
            },
            &info,
        )
        .unwrap();

    assert_eq!(
        index_store.get_coin_info(&coin_type).unwrap(),
        Some(info.clone())
    );
    assert_eq!(
        GrpcIndexes::get_coin_info(&index_store, &coin_type).unwrap(),
        Some(info.into())
    );
    assert_eq!(
        index_store
            .get_coin_info(&StructTag::new_coin(TypeTag::from(StructTag::new_gas())))
            .unwrap(),
        None,
        "a coin type with no regulated metadata must miss"
    );
}

/// Package versions round-trip through `package_versions_iter`, and the
/// cursor bound is inclusive.
#[tokio::test]
async fn test_package_versions_iter_pages_from_the_cursor() {
    let index_store = open_index_store(iota_common::tempdir().path().to_path_buf());
    let original_package_id = ObjectId::random();
    let storage_ids: Vec<_> = (0..3).map(|_| ObjectId::random()).collect();
    let table = &index_store.tables.package_version;
    let mut batch = table.batch();
    batch
        .insert_batch(
            table,
            storage_ids.iter().enumerate().map(|(version, storage_id)| {
                (
                    PackageVersionKey {
                        original_package_id,
                        version: version as u64,
                    },
                    PackageVersionInfo {
                        storage_id: *storage_id,
                    },
                )
            }),
        )
        .unwrap();
    batch.write().unwrap();

    let all: Vec<_> = index_store
        .package_versions_iter(original_package_id, None)
        .unwrap()
        .map(Result::unwrap)
        .collect();
    assert_eq!(all.len(), 3, "all three seeded versions must resolve");

    let from_cursor: Vec<_> = index_store
        .package_versions_iter(original_package_id, Some(1))
        .unwrap()
        .map(Result::unwrap)
        .collect();
    assert_eq!(from_cursor.len(), 2, "the cursor bound is inclusive");
}

/// `dynamic_field_iter` returns the full key, unlike the JSON-RPC surface's
/// `get_dynamic_field_ids_iterator`, which returns only the field id.
#[tokio::test]
async fn test_dynamic_field_iter_returns_the_full_key() {
    let index_store = open_index_store(iota_common::tempdir().path().to_path_buf());
    let parent = ObjectId::random();
    let field_ids = seed_dynamic_fields(&index_store, parent);

    let keys: Vec<_> = index_store
        .dynamic_field_iter(parent, None)
        .unwrap()
        .map(Result::unwrap)
        .collect();
    assert_eq!(
        keys,
        field_ids
            .iter()
            .map(|id| DynamicFieldKey::new(parent, *id))
            .collect::<Vec<_>>()
    );

    let from_cursor: Vec<_> = index_store
        .dynamic_field_iter(parent, Some(field_ids[1]))
        .unwrap()
        .map(Result::unwrap)
        .collect();
    assert_eq!(
        from_cursor,
        field_ids[1..]
            .iter()
            .map(|id| DynamicFieldKey::new(parent, *id))
            .collect::<Vec<_>>(),
        "the cursor bound is inclusive"
    );
}

/// The gRPC surface's owned-objects iterator narrows by type and pages with
/// an `OwnedObjectCursor`, which carries no owner of its own: the trait
/// method must rebuild the full `OwnerIndexKey` from `owner` and the
/// cursor's other fields.
#[tokio::test]
async fn test_account_owned_objects_info_iter_narrows_and_pages() {
    use iota_node_storage::GrpcIndexes;

    let index_store = open_index_store(iota_common::tempdir().path().to_path_buf());
    let owner = Address::random();
    let coins = [
        (TypeTag::from(StructTag::new_gas()), 300u64),
        (TypeTag::from(StructTag::new_gas()), 100u64),
        (TypeTag::U64, 500u64),
    ];
    let mut gas_ids = BTreeSet::new();
    for (coin_type, balance) in coins {
        let object = Object::new_move(
            MoveStruct::new_coin(
                coin_type.clone(),
                Version::MIN_VALID_INCL,
                ObjectId::random(),
                balance,
            ),
            Owner::Address(owner),
            TransactionDigest::GENESIS_MARKER,
        );
        let (key, info) = OwnerIndexKey::for_object(owner, &object).unwrap();
        index_store.tables.owner.insert(&key, &info).unwrap();
        if coin_type == TypeTag::from(StructTag::new_gas()) {
            gas_ids.insert(object.id());
        }
    }

    let gas_coin_type = StructTag::new_coin(TypeTag::from(StructTag::new_gas()));
    let narrowed: BTreeSet<_> = index_store
        .account_owned_objects_info_iter(owner, None, Some(gas_coin_type))
        .unwrap()
        .map(|item| item.unwrap().0.object_id)
        .collect();
    assert_eq!(
        narrowed, gas_ids,
        "narrowing to one coin type must exclude the other"
    );

    let full: Vec<_> = index_store
        .account_owned_objects_info_iter(owner, None, None)
        .unwrap()
        .map(Result::unwrap)
        .collect();
    assert_eq!(full.len(), 3, "all three seeded objects must resolve");

    let cursor = full[1].1;
    let from_cursor: Vec<_> = index_store
        .account_owned_objects_info_iter(owner, Some(&cursor), None)
        .unwrap()
        .map(|item| item.unwrap().0.object_id)
        .collect();
    assert_eq!(
        from_cursor,
        full[1..]
            .iter()
            .map(|(info, _)| info.object_id)
            .collect::<Vec<_>>(),
        "the cursor bound is inclusive"
    );
}

/// A checkpoint replayed after a crash (or one the history backfill already
/// covered) must skip its indexed transactions: no new sequence numbers, no
/// duplicate rows, no double-counted balances.
#[tokio::test]
async fn test_index_checkpoint_skips_already_indexed() {
    let index_store = open_index_store(iota_common::tempdir().path().to_path_buf());
    let address = TestCheckpointDataBuilder::derive_address(1);

    let mut builder = TestCheckpointDataBuilder::new(0)
        .start_transaction(0)
        .create_coin_object(0, 1, 100, TypeTag::from(StructTag::new_gas()))
        .finish_transaction();
    let checkpoint = builder.build_checkpoint();
    let digest = *checkpoint.transactions[0].effects.transaction_digest();

    index_checkpoint_for_testing(&index_store, &checkpoint);
    assert_eq!(index_store.lookup_digest(&digest).unwrap(), Some(0));
    assert_eq!(index_store.tables.watermark.get(&()).unwrap(), Some(0));

    // Replay the same checkpoint.
    index_checkpoint_for_testing(&index_store, &checkpoint);

    assert_eq!(index_store.lookup_digest(&digest).unwrap(), Some(0));
    assert_eq!(
        index_store
            .get_transactions(None, None, None, false)
            .unwrap(),
        vec![digest]
    );
    let balance = index_store
        .get_balance(address, TypeTag::from(StructTag::new_gas()))
        .unwrap();
    assert_eq!(balance.balance, 100);
    assert_eq!(balance.num_coins, 1);
}

/// The balance caches follow the coin changes of every committed checkpoint
/// without ever reading a coin table: creations, spends, transfers between
/// owners, and a coin that changes hands twice inside one checkpoint must
/// all leave the cached balances equal to the owner index's own sums.
#[tokio::test]
async fn test_balance_caches_follow_the_checkpoint_coin_changes() {
    let index_store = open_index_store(iota_common::tempdir().path().to_path_buf());
    let alice = TestCheckpointDataBuilder::derive_address(1);
    let bob = TestCheckpointDataBuilder::derive_address(2);
    let carol = TestCheckpointDataBuilder::derive_address(3);
    let cached_and_stored = |owner| {
        let cached = index_store
            .get_balance(owner, TypeTag::from(StructTag::new_gas()))
            .unwrap();
        let stored = index_store
            .get_balance_from_db(owner, &TypeTag::from(StructTag::new_gas()))
            .unwrap();
        assert_eq!(cached, stored, "the cache must match the owner index");
        cached
    };

    // Ten coins of 100 for alice.
    let mut builder = TestCheckpointDataBuilder::new(0).start_transaction(0);
    for object_idx in 0..10 {
        builder =
            builder.create_coin_object(object_idx, 1, 100, TypeTag::from(StructTag::new_gas()));
    }
    let mut builder = builder.finish_transaction();
    let checkpoint = builder.build_checkpoint();
    index_checkpoint_for_testing(&index_store, &checkpoint);
    assert_eq!(cached_and_stored(alice).balance, 1000);
    assert_eq!(cached_and_stored(alice).num_coins, 10);

    // Three of them are spent. Every transaction is sent by address 0, whose
    // gas coin the builder mutates into the checkpoint: a sender under test
    // would see its balance move with the gas.
    let mut builder = builder.start_transaction(0);
    for object_idx in 0..3 {
        builder = builder.delete_object(object_idx);
    }
    let mut builder = builder.finish_transaction();
    let checkpoint = builder.build_checkpoint();
    index_checkpoint_for_testing(&index_store, &checkpoint);
    assert_eq!(cached_and_stored(alice).balance, 700);
    assert_eq!(cached_and_stored(alice).num_coins, 7);

    // One coin moves to bob, and half of another's balance is split off into
    // a new coin for bob: alice loses a whole coin and gains a smaller one.
    let mut builder = builder
        .start_transaction(0)
        .transfer_object(3, 2)
        .transfer_coin_balance(4, 10, 2, 40)
        .finish_transaction();
    let checkpoint = builder.build_checkpoint();
    index_checkpoint_for_testing(&index_store, &checkpoint);
    assert_eq!(cached_and_stored(alice).balance, 560);
    assert_eq!(cached_and_stored(alice).num_coins, 6);
    assert_eq!(cached_and_stored(bob).balance, 140);
    assert_eq!(cached_and_stored(bob).num_coins, 2);

    // Within one checkpoint a coin passes from bob to carol and on to alice:
    // only the first change of each key sees the state the checkpoint
    // started from, so bob must end up with no coin counted twice.
    let mut builder = builder
        .start_transaction(0)
        .transfer_object(3, 3)
        .finish_transaction()
        .start_transaction(0)
        .transfer_object(3, 1)
        .finish_transaction();
    let checkpoint = builder.build_checkpoint();
    assert_eq!(
        checkpoint.transactions.len(),
        2,
        "both hops must land in the same checkpoint"
    );
    index_checkpoint_for_testing(&index_store, &checkpoint);
    assert_eq!(cached_and_stored(alice).balance, 660);
    assert_eq!(cached_and_stored(alice).num_coins, 7);
    assert_eq!(cached_and_stored(bob).balance, 40);
    assert_eq!(cached_and_stored(bob).num_coins, 1);
    assert_eq!(cached_and_stored(carol).balance, 0);
    assert_eq!(cached_and_stored(carol).num_coins, 0);
}

/// A store built by a formal-snapshot restore is opened in place: the
/// markers a node checks are stamped, its live-state tables carry the teed
/// objects, and the history backfill has nothing to replay.
#[tokio::test]
async fn test_restore_built_store_is_adopted_on_open() {
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

    let owner = iota_types::base_types::dbg_addr(1);
    let gas_object = Object::new_gas_with_balance_and_owner_for_testing(100, owner);
    let parent = ObjectId::random();
    let field_id = ObjectId::random();
    let field_object = dynamic_field_object(parent, field_id);

    // Tee the objects into the restorer, as the snapshot's partition
    // downloads do.
    let index_dir = dir.path().join(super::schema::RPC_INDEXES_DIR);
    let groups = BTreeSet::from([IndexGroup::JsonRpc, IndexGroup::Grpc]);
    let restorer = RpcIndexesRestorer::open(index_dir.clone(), groups.clone()).unwrap();
    let mut partition = restorer.partition_indexer();
    partition.index_object(&gas_object).unwrap();
    partition.index_object(&field_object).unwrap();
    partition.finish().unwrap();
    restorer.finalize(5).await.unwrap();
    RpcIndexesRestorer::verify_restored(&index_dir, 5, 2)
        .await
        .unwrap();

    // Plant a sentinel row: if it survives the open below, the store was
    // adopted rather than wiped and rebuilt into equal-looking data.
    let sentinel = DynamicFieldKey::new(ObjectId::random(), ObjectId::random());
    {
        let built = open_index_store(index_dir.clone());
        assert!(
            !built
                .tables
                .needs_to_do_initialization(&checkpoint_store, &groups)
                .unwrap(),
            "a restore-built store must need no rebuild"
        );
        built.tables.dynamic_field.insert(&sentinel, &()).unwrap();
        close_index_store(built).await;
    }

    let authority_store = open_authority_store(&dir.path().join("store"));
    let index_store = RpcIndexesStore::new(
        index_dir,
        &Registry::default(),
        groups,
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
            .dynamic_field_exists(sentinel.parent, sentinel.field_id)
            .unwrap(),
        "the restored database must be opened in place, not rebuilt"
    );

    // The owner index was built from the teed objects, balances included.
    let object_store = BTreeMap::from([(gas_object.id(), gas_object.clone())]);
    let owned = index_store
        .get_owner_objects(owner, None, 10, None, &object_store)
        .unwrap();
    assert_eq!(owned.len(), 1);
    assert_eq!(owned[0].0.object_id, gas_object.id());
    let balance = index_store
        .get_balance(owner, TypeTag::from(StructTag::new_gas()))
        .unwrap();
    assert_eq!(balance.num_coins, 1);
    assert_eq!(balance.balance, 100);

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

/// The finalize writes the version and the watermark whether or not any
/// object landed, so a store the node would wipe — or one that carries no
/// restored objects — must fail the verification instead.
#[tokio::test]
async fn test_verify_restored_rejects_an_unusable_store() {
    let dir = iota_common::tempdir();
    let index_dir = dir.path().join(super::schema::RPC_INDEXES_DIR);
    let groups = BTreeSet::from([IndexGroup::Grpc]);

    let restorer = RpcIndexesRestorer::open(index_dir.clone(), groups.clone()).unwrap();
    restorer.finalize(5).await.unwrap();
    let error = RpcIndexesRestorer::verify_restored(&index_dir, 5, 1)
        .await
        .expect_err("an empty restore must not pass verification");
    assert!(
        error.to_string().contains("empty owner index"),
        "unexpected error: {error}"
    );

    let error = RpcIndexesRestorer::verify_restored(&index_dir, 6, 0)
        .await
        .expect_err("a watermark below the restore checkpoint must not pass verification");
    assert!(
        error.to_string().contains("watermarked at"),
        "unexpected error: {error}"
    );
}

/// A stale database (here: written by another schema version) is wiped and
/// rebuilt through the full open path — bulk-ingestion open, live object
/// scan, flush, reopen with default options — and none of its rows survive.
#[tokio::test]
async fn test_stale_database_is_wiped_and_rebuilt_on_open() {
    let (authority_state, genesis_tx_digest) = genesis_authority_state().await;
    let checkpoint_store = &authority_state.checkpoint_store;
    let groups = BTreeSet::from([IndexGroup::JsonRpc, IndexGroup::Grpc]);
    let index_dir = iota_common::tempdir();
    let authority_store = authority_state.database_for_testing();

    let index_store = RpcIndexesStore::new(
        index_dir.path().to_path_buf(),
        &Registry::default(),
        groups.clone(),
        Some(128),
        None,
        &authority_store,
        checkpoint_store,
        Default::default(),
    )
    .await
    .unwrap();
    index_store.wait_for_history_backfill_for_testing().await;
    // The genesis objects were indexed by the rebuild's live object scan.
    let indexed_objects = index_store.tables.owner.safe_iter().count();
    assert!(indexed_objects > 0, "the scan must fill the owner index");

    // Poison the store and mark it as written by another schema version.
    let poison_field = DynamicFieldKey::new(ObjectId::random(), ObjectId::random());
    index_store
        .tables
        .dynamic_field
        .insert(&poison_field, &())
        .unwrap();
    index_store
        .tables
        .meta
        .insert(
            &(),
            &super::schema::MetadataInfo {
                version: super::CURRENT_DB_VERSION + 1,
                groups: groups.clone(),
            },
        )
        .unwrap();
    close_index_store(index_store).await;

    // A fresh registry: the rebuilt store registers the same metrics again.
    let index_store = RpcIndexesStore::new(
        index_dir.path().to_path_buf(),
        &Registry::default(),
        groups.clone(),
        Some(128),
        None,
        &authority_store,
        checkpoint_store,
        Default::default(),
    )
    .await
    .unwrap();
    index_store.wait_for_history_backfill_for_testing().await;

    assert!(
        !index_store
            .dynamic_field_exists(poison_field.parent, poison_field.field_id)
            .unwrap(),
        "stale rows must not survive the rebuild"
    );
    assert_eq!(
        index_store.tables.owner.safe_iter().count(),
        indexed_objects,
        "the rebuild must fill the owner index again"
    );
    assert_eq!(
        index_store.lookup_digest(&genesis_tx_digest).unwrap(),
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

/// The restore must derive from an external object stream what the rebuild
/// derives from a scan of the local store: the shared owner and
/// dynamic-field rows, and the gRPC group's coin metadata and package
/// versions. The coin metadata of one coin type is spread over separate
/// objects that may land in different partitions, so it is gathered across
/// them and written once, on the finalize.
#[tokio::test]
async fn test_restore_builds_the_same_live_state_as_the_rebuild() {
    let dir = iota_common::tempdir();
    let owner = iota_types::base_types::dbg_addr(1);
    let gas_object = Object::new_gas_with_balance_and_owner_for_testing(100, owner);
    let parent = ObjectId::random();
    let field_id = ObjectId::random();
    let field_object = dynamic_field_object(parent, field_id);
    // A coin type of its own, so the genesis objects the rebuild scans
    // cannot contribute to the same row.
    let coin_type: StructTag = "0x42::test_coin::TEST_COIN".parse().unwrap();
    let coin_metadata = typed_object_for_testing(
        format!("0x2::coin::CoinMetadata<{coin_type}>")
            .parse()
            .unwrap(),
        Owner::Immutable,
    );
    let treasury_cap = typed_object_for_testing(
        format!("0x2::coin::TreasuryCap<{coin_type}>")
            .parse()
            .unwrap(),
        Owner::Address(owner),
    );
    let package_object = package_object_for_testing();
    let objects = [
        gas_object.clone(),
        field_object.clone(),
        coin_metadata.clone(),
        treasury_cap.clone(),
        package_object.clone(),
    ];
    let groups = BTreeSet::from([IndexGroup::JsonRpc, IndexGroup::Grpc]);

    // The restore tees the objects in, one partition each for the two coin
    // metadata objects of the same coin type.
    let restored_dir = dir.path().join("restored");
    let restorer = RpcIndexesRestorer::open(restored_dir.clone(), groups.clone()).unwrap();
    for object in &objects {
        let mut partition = restorer.partition_indexer();
        partition.index_object(object).unwrap();
        partition.finish().unwrap();
    }
    restorer.finalize(0).await.unwrap();
    let restored = open_index_store(restored_dir);

    // The rebuild scans the same objects out of a live authority store.
    let authority_state = crate::authority::test_authority_builder::TestAuthorityBuilder::new()
        .insert_genesis_checkpoint()
        .build()
        .await;
    authority_state.insert_genesis_objects(&objects);
    let checkpoint_store = &authority_state.checkpoint_store;
    let genesis_checkpoint = checkpoint_store
        .get_checkpoint_by_sequence_number(0)
        .unwrap()
        .unwrap();
    checkpoint_store
        .update_highest_executed_checkpoint(&genesis_checkpoint)
        .unwrap();
    let rebuilt = RpcIndexesStore::new(
        dir.path().join("rebuilt"),
        &Registry::default(),
        groups,
        Some(128),
        None,
        &authority_state.database_for_testing(),
        checkpoint_store,
        Default::default(),
    )
    .await
    .unwrap();
    rebuilt.wait_for_history_backfill_for_testing().await;

    for store in [&restored, &*rebuilt] {
        let (owner_key, owner_info) = OwnerIndexKey::for_object(owner, &gas_object).unwrap();
        assert_eq!(
            store.tables.owner.get(&owner_key).unwrap(),
            Some(owner_info),
            "the address-owned coin must be owner-indexed with its balance"
        );
        assert!(
            store
                .dynamic_field_exists(parent, field_id)
                .expect("the dynamic field must be indexed by key")
        );
        assert_eq!(
            store.get_coin_info(&coin_type).unwrap(),
            Some(CoinIndexInfo {
                coin_metadata_object_id: Some(coin_metadata.id()),
                treasury_object_id: Some(treasury_cap.id()),
                regulated_coin_metadata_object_id: None,
            }),
            "the coin metadata of the partitions must be merged into one row"
        );
        let versions: Vec<_> = store
            .package_versions_iter(package_object.id(), None)
            .unwrap()
            .map(Result::unwrap)
            .collect();
        assert_eq!(
            versions,
            vec![(
                PackageVersionKey {
                    original_package_id: package_object.id(),
                    version: package_object.version().as_u64(),
                },
                PackageVersionInfo {
                    storage_id: package_object.id(),
                },
            )],
            "the package version must be indexed"
        );
    }
}

/// A store maintaining only one group indexes only that group's tables,
/// and enabling the other group later triggers a rebuild.
#[tokio::test]
async fn test_groups_gate_the_ingest_and_toggle_triggers_rebuild() {
    let dir = iota_common::tempdir();
    let index_dir = dir.path().join(super::schema::RPC_INDEXES_DIR);
    let checkpoint_store = CheckpointStore::new(&dir.path().join("checkpoints"));
    let owner = TestCheckpointDataBuilder::derive_address(1);
    let grpc_only = BTreeSet::from([IndexGroup::Grpc]);
    let index_store = RpcIndexesStore::new_without_init(index_dir, grpc_only.clone());
    index_store.tables.seed_meta(&grpc_only).unwrap();

    // One coin for the owner, plus a created coin object turned into the
    // coin metadata of its type.
    let mut builder = TestCheckpointDataBuilder::new(0)
        .start_transaction(0)
        .create_coin_object(0, 1, 100, TypeTag::from(StructTag::new_gas()))
        .create_coin_object(1, 1, 1, TypeTag::from(StructTag::new_gas()))
        .finish_transaction();
    let mut checkpoint = builder.build_checkpoint();
    let metadata_id = replace_created_object_type(
        &mut checkpoint,
        1,
        "0x2::coin::CoinMetadata<0x2::iota::IOTA>".parse().unwrap(),
    );
    let digest = *checkpoint.transactions[0].effects.transaction_digest();
    index_checkpoint_for_testing(&index_store, &checkpoint);

    // The digest row and the shared live state are always written.
    assert_eq!(index_store.lookup_digest(&digest).unwrap(), Some(0));
    let coin_object = checkpoint.transactions[0]
        .output_objects
        .iter()
        .find(|object| object.id() == TestCheckpointDataBuilder::derive_object_id(0))
        .unwrap();
    let (owner_key, owner_info) = OwnerIndexKey::for_object(owner, coin_object).unwrap();
    assert_eq!(
        index_store.tables.owner.get(&owner_key).unwrap(),
        Some(owner_info.clone()),
        "the owner index is shared by both groups"
    );
    // The gRPC group's own tables are filled.
    assert_eq!(
        index_store.get_coin_info(&StructTag::new_gas()).unwrap(),
        Some(CoinIndexInfo {
            coin_metadata_object_id: Some(metadata_id),
            ..Default::default()
        })
    );
    // The JSON-RPC group's own tables stay empty, and its reads refuse to
    // answer from them.
    let bucket = index_store.history.ensure(0).unwrap();
    assert!(
        bucket.tx_order.safe_iter().next().is_none(),
        "the JSON-RPC history tables must stay empty"
    );
    assert!(matches!(
        index_store.get_balance(owner, TypeTag::from(StructTag::new_gas())),
        Err(IotaError::IndexStoreNotAvailable)
    ));

    // The same checkpoint the other way round: a JSON-RPC-only store fills
    // the history tables and the balances, and leaves the gRPC group's own
    // tables empty.
    let jsonrpc_store = RpcIndexesStore::new_without_init(
        dir.path().join("jsonrpc_only"),
        BTreeSet::from([IndexGroup::JsonRpc]),
    );
    index_checkpoint_for_testing(&jsonrpc_store, &checkpoint);

    assert_eq!(jsonrpc_store.lookup_digest(&digest).unwrap(), Some(0));
    assert_eq!(
        jsonrpc_store.tables.owner.get(&owner_key).unwrap(),
        Some(owner_info),
        "the owner index is shared by both groups"
    );
    assert_eq!(
        jsonrpc_store
            .get_transactions(None, None, None, false)
            .unwrap(),
        vec![digest],
        "the JSON-RPC history tables must be filled"
    );
    assert_eq!(
        jsonrpc_store
            .get_balance(owner, TypeTag::from(StructTag::new_gas()))
            .unwrap()
            .balance,
        100
    );
    assert!(
        jsonrpc_store.tables.coin.safe_iter().next().is_none(),
        "the gRPC group's coin table must stay empty"
    );
    assert!(
        jsonrpc_store
            .tables
            .package_version
            .safe_iter()
            .next()
            .is_none(),
        "the gRPC group's package table must stay empty"
    );

    // Enabling the JSON-RPC group makes the store stale: its tables were
    // never filled, so the whole store must be rebuilt.
    let both = BTreeSet::from([IndexGroup::JsonRpc, IndexGroup::Grpc]);
    mark_checkpoint_executed(&checkpoint_store, 0);
    assert!(
        !index_store
            .tables
            .needs_to_do_initialization(&checkpoint_store, &grpc_only)
            .unwrap(),
        "the store the checkpoint was indexed into is healthy for its own groups"
    );
    assert!(
        index_store
            .tables
            .needs_to_do_initialization(&checkpoint_store, &both)
            .unwrap(),
        "a newly enabled group must trigger a rebuild"
    );
}

/// Reopening with fewer groups records the narrowed set, so enabling the
/// dropped group again rebuilds instead of adopting tables that stopped being
/// maintained at the reopen.
#[tokio::test]
async fn test_reopening_with_fewer_groups_makes_re_enabling_them_rebuild() {
    let (authority_state, _) = genesis_authority_state().await;
    let checkpoint_store = &authority_state.checkpoint_store;
    let authority_store = authority_state.database_for_testing();
    let index_dir = iota_common::tempdir();
    let both = BTreeSet::from([IndexGroup::JsonRpc, IndexGroup::Grpc]);
    let jsonrpc_only = BTreeSet::from([IndexGroup::JsonRpc]);
    let open = async |groups: BTreeSet<IndexGroup>| {
        // A fresh registry: every open registers the same metrics again.
        let store = RpcIndexesStore::new(
            index_dir.path().to_path_buf(),
            &Registry::default(),
            groups,
            Some(128),
            None,
            &authority_store,
            checkpoint_store,
            Default::default(),
        )
        .await
        .unwrap();
        store.wait_for_history_backfill_for_testing().await;
        store
    };

    let index_store = open(both.clone()).await;
    assert_eq!(
        index_store.tables.meta.get(&()).unwrap().unwrap().groups,
        both
    );
    close_index_store(index_store).await;

    // Dropping a group must not rebuild — its tables are still complete as
    // of this open — but the store must record that it stops maintaining it.
    let index_store = open(jsonrpc_only.clone()).await;
    assert_eq!(
        index_store.tables.meta.get(&()).unwrap().unwrap().groups,
        jsonrpc_only,
        "the narrowed group set must be recorded"
    );
    assert!(
        !index_store
            .tables
            .needs_to_do_initialization(checkpoint_store, &jsonrpc_only)
            .unwrap(),
        "dropping a group must not rebuild the store"
    );
    assert!(
        index_store
            .tables
            .needs_to_do_initialization(checkpoint_store, &both)
            .unwrap(),
        "re-enabling the dropped group must rebuild the store"
    );
    // A row no rebuild would write, to tell the rebuilt store from this one.
    let poison_field = DynamicFieldKey::new(ObjectId::random(), ObjectId::random());
    index_store
        .tables
        .dynamic_field
        .insert(&poison_field, &())
        .unwrap();
    close_index_store(index_store).await;

    let index_store = open(both.clone()).await;
    assert!(
        !index_store
            .dynamic_field_exists(poison_field.parent, poison_field.field_id)
            .unwrap(),
        "re-enabling the dropped group must have rebuilt the store"
    );
    assert_eq!(
        index_store.tables.meta.get(&()).unwrap().unwrap().groups,
        both
    );
}

/// A coin type's metadata, treasury cap and regulated metadata are separate
/// objects of one row, created together by one transaction: indexing that
/// checkpoint must leave a row carrying all three, and a later checkpoint
/// contributing another of them must merge onto the row instead of replacing
/// it.
#[tokio::test]
async fn test_coin_metadata_objects_merge_into_one_row() {
    let index_store = open_index_store(iota_common::tempdir().path().to_path_buf());
    let together: StructTag = "0x42::together::TOGETHER".parse().unwrap();
    let apart: StructTag = "0x42::apart::APART".parse().unwrap();
    let coin_metadata_type = |coin_type: &StructTag| {
        format!("0x2::coin::CoinMetadata<{coin_type}>")
            .parse()
            .unwrap()
    };
    let treasury_type = |coin_type: &StructTag| {
        format!("0x2::coin::TreasuryCap<{coin_type}>")
            .parse()
            .unwrap()
    };
    let regulated_type = |coin_type: &StructTag| {
        format!("0x2::coin::RegulatedCoinMetadata<{coin_type}>")
            .parse()
            .unwrap()
    };

    // One transaction creates all three objects of `together`, and the two
    // that a currency's creation always pairs for `apart`.
    let mut builder = TestCheckpointDataBuilder::new(0).start_transaction(0);
    for object_idx in 0..5 {
        builder = builder.create_coin_object(object_idx, 1, 1, TypeTag::from(StructTag::new_gas()));
    }
    let mut builder = builder.finish_transaction();
    let mut checkpoint = builder.build_checkpoint();
    let together_metadata =
        replace_created_object_type(&mut checkpoint, 0, coin_metadata_type(&together));
    let together_treasury =
        replace_created_object_type(&mut checkpoint, 1, treasury_type(&together));
    let together_regulated =
        replace_created_object_type(&mut checkpoint, 2, regulated_type(&together));
    let apart_metadata =
        replace_created_object_type(&mut checkpoint, 3, coin_metadata_type(&apart));
    let apart_treasury = replace_created_object_type(&mut checkpoint, 4, treasury_type(&apart));
    index_checkpoint_for_testing(&index_store, &checkpoint);

    assert_eq!(
        index_store.get_coin_info(&together).unwrap(),
        Some(CoinIndexInfo {
            coin_metadata_object_id: Some(together_metadata),
            treasury_object_id: Some(together_treasury),
            regulated_coin_metadata_object_id: Some(together_regulated),
        }),
        "objects of one checkpoint must not overwrite each other's fields"
    );

    // A later checkpoint contributes the regulated metadata of `apart`.
    let mut builder = builder
        .start_transaction(0)
        .create_coin_object(5, 1, 1, TypeTag::from(StructTag::new_gas()))
        .finish_transaction();
    let mut checkpoint = builder.build_checkpoint();
    let apart_regulated = replace_created_object_type(&mut checkpoint, 5, regulated_type(&apart));
    index_checkpoint_for_testing(&index_store, &checkpoint);

    assert_eq!(
        index_store.get_coin_info(&apart).unwrap(),
        Some(CoinIndexInfo {
            coin_metadata_object_id: Some(apart_metadata),
            treasury_object_id: Some(apart_treasury),
            regulated_coin_metadata_object_id: Some(apart_regulated),
        }),
        "a later checkpoint must merge onto the row instead of replacing it"
    );
}

/// The live scan writes only the tables of the enabled groups: a
/// JSON-RPC-only store leaves the gRPC group's coin and package tables empty,
/// whatever the object stream carries, while the shared tables fill as usual.
#[tokio::test]
async fn test_live_scan_gates_the_grpc_tables() {
    let dir = iota_common::tempdir();
    let index_dir = dir.path().join(super::schema::RPC_INDEXES_DIR);
    let owner = iota_types::base_types::dbg_addr(1);
    let gas_object = Object::new_gas_with_balance_and_owner_for_testing(100, owner);
    let coin_metadata = typed_object_for_testing(
        "0x2::coin::CoinMetadata<0x2::iota::IOTA>".parse().unwrap(),
        Owner::Immutable,
    );
    let package_object = package_object_for_testing();

    let restorer =
        RpcIndexesRestorer::open(index_dir.clone(), BTreeSet::from([IndexGroup::JsonRpc])).unwrap();
    let mut partition = restorer.partition_indexer();
    for object in [&gas_object, &coin_metadata, &package_object] {
        partition.index_object(object).unwrap();
    }
    partition.finish().unwrap();
    restorer.finalize(0).await.unwrap();

    // Opened serving both groups, so the reads answer from the tables the
    // restore filled rather than refusing.
    let store = open_index_store(index_dir);
    let (owner_key, owner_info) = OwnerIndexKey::for_object(owner, &gas_object).unwrap();
    assert_eq!(
        store.tables.owner.get(&owner_key).unwrap(),
        Some(owner_info),
        "the owner index is shared by both groups"
    );
    assert!(
        store.tables.coin.safe_iter().next().is_none(),
        "the gRPC group's coin table must stay empty"
    );
    assert!(
        store.tables.package_version.safe_iter().next().is_none(),
        "the gRPC group's package table must stay empty"
    );
}

/// A query that snapshotted the history buckets before a `prune` must
/// report an error for the dropped epoch's rows, as [`RpcIndexesStore::prune`]
/// documents, rather than panicking.
#[tokio::test]
async fn test_prune_racing_a_reader_reports_an_error() {
    let tmp_dir = iota_common::tempdir();
    let mut index_store = open_index_store(tmp_dir.path().to_path_buf());
    index_store.epochs_to_retain = Some(0);
    seed_history_buckets(&index_store, 2);

    // Every digest probe and range scan reads through such a snapshot.
    let snapshot = index_store.history.iter(false);
    assert_eq!(snapshot.len(), 2);

    assert_eq!(prune_at_newest_epoch(&index_store).unwrap(), Some(1));

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
    assert_eq!(index_store.history.iter(false).len(), 1);
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
    let mut index_store = open_index_store(tmp_dir.path().to_path_buf());
    index_store.epochs_to_retain = Some(0);
    seed_history_buckets(&index_store, 2);

    // Makes the pruner's own drop fail: the column family is already gone.
    index_store
        .tables
        .meta
        .db
        .drop_cf(&super::history_cf_name(0))
        .unwrap();

    assert_eq!(prune_at_newest_epoch(&index_store).unwrap(), Some(1));
    assert_eq!(index_store.history.iter(false).len(), 1);
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
    let mut index_store = open_index_store(tmp_dir.path().to_path_buf());
    index_store.epochs_to_retain = Some(0);
    seed_history_buckets(&index_store, 2);
    assert_eq!(prune_at_newest_epoch(&index_store).unwrap(), Some(1));

    // Stands in for a drop that failed: the column family is on disk
    // below the persisted floor.
    index_store
        .tables
        .meta
        .db
        .create_cf(
            &super::history_cf_name(0),
            &typed_store::rocksdb::Options::default(),
        )
        .unwrap();

    let index_store = reopen_index_store(index_store, tmp_dir.path().to_path_buf()).await;
    assert_eq!(index_store.history.iter(false).len(), 1);
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
            .contains(&super::history_cf_name(0))
    );
}

/// A retention floor that cannot be read fails the open, which a restart
/// retries: the database itself is intact, so it must not reach the
/// wipe-and-rebuild path `RpcIndexesStore::new` takes for an unopenable one.
#[tokio::test]
async fn test_a_failed_floor_read_fails_the_open() {
    let tmp_dir = iota_common::tempdir();
    let opened =
        RpcIndexesStore::open_index_db(&tmp_dir.path().join(super::schema::RPC_INDEXES_DIR))
            .unwrap();

    // Makes the floor read fail: RocksDB unregisters the column family.
    opened.db.drop_cf("earliest_retained_epoch").unwrap();

    assert!(
        RpcIndexesStore::finish_open(
            opened,
            &Registry::default(),
            BTreeSet::from([IndexGroup::JsonRpc]),
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
                    let _ = index_store.lookup_digest(&Default::default());
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
        index_store
            .history
            .prune(EPOCHS - 1, retained - 1, |_, _| Ok(()))
            .unwrap();
    }
    stop.store(true, Ordering::Relaxed);
    for worker in workers {
        worker.join().expect("a worker thread panicked");
    }

    // No bucket left in the map may point at a dropped column family.
    for bucket in index_store.history.iter(false) {
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

/// Expiry drops a checkpoint's transactions before it advances the watermark
/// the backfill checks, so a replay can find them already gone. That must end
/// the backfill instead of failing the task for the rest of the process.
#[tokio::test]
async fn test_backfill_stops_at_deleted_checkpoint_data() {
    let (authority_state, genesis_tx_digest) = genesis_authority_state().await;
    let checkpoint_store = &authority_state.checkpoint_store;
    let authority_store = authority_state.database_for_testing();
    authority_store
        .get_historic_ledger()
        .ensure(0)
        .unwrap()
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
    let mut index_store = open_index_store(index_dir.path().to_path_buf());
    index_store.epochs_to_retain = Some(0);
    seed_history_buckets(&index_store, 2);
    assert_eq!(prune_at_newest_epoch(&index_store).unwrap(), Some(1));
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

/// A rebuild on a node with nothing executed writes the backfill marker
/// but no watermark: an absent watermark already means "nothing indexed",
/// while writing 0 would claim checkpoint 0 was indexed.
#[tokio::test]
async fn test_rebuild_with_nothing_executed_writes_no_watermark() {
    let dir = iota_common::tempdir();
    let checkpoint_store = CheckpointStore::new(&dir.path().join("checkpoints"));
    let index_dir = dir.path().join(super::schema::RPC_INDEXES_DIR);

    // A database holding data but no `meta` row triggers the wipe and
    // rebuild even though nothing is executed yet.
    {
        let index_store = open_index_store(index_dir.clone());
        let owner = iota_types::base_types::dbg_addr(1);
        let object = Object::with_id_owner_for_testing(ObjectId::random(), owner);
        let (key, info) = OwnerIndexKey::for_object(owner, &object).unwrap();
        index_store.tables.owner.insert(&key, &info).unwrap();
        close_index_store(index_store).await;
    }

    let authority_store = open_authority_store(&dir.path().join("store"));
    let index_store = RpcIndexesStore::new(
        index_dir,
        &Registry::default(),
        BTreeSet::from([IndexGroup::JsonRpc, IndexGroup::Grpc]),
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
    let owner = Owner::Address(Address::ZERO);
    let id = ObjectId::random();
    let contents = iota_types::coin::Coin::new(id, 42).to_bcs_bytes();

    let coin = Object::new_move(
        MoveStruct::new_coin(
            TypeTag::from(StructTag::new_gas()),
            Version::MIN_VALID_INCL,
            id,
            42,
        ),
        owner,
        TransactionDigest::ZERO,
    );
    assert_eq!(
        super::jsonrpc_api::CoinInfo::from_object(&coin)
            .unwrap()
            .balance,
        42
    );

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
    assert_eq!(super::jsonrpc_api::CoinInfo::from_object(&fake), None);
}

/// The index databases of earlier releases are removed; none of their
/// content can be adopted by the unified store.
#[test]
fn test_remove_legacy_index_dirs() {
    let db_path = iota_common::tempdir();
    let legacy_dirs: Vec<_> = ["indexes", "jsonrpc_indexes", "grpc_indexes"]
        .iter()
        .map(|dir| db_path.path().join(dir))
        .collect();
    for legacy_dir in &legacy_dirs {
        std::fs::create_dir(legacy_dir).unwrap();
        std::fs::write(legacy_dir.join("CURRENT"), b"stale").unwrap();
    }

    super::remove_legacy_index_dirs(db_path.path()).unwrap();
    for legacy_dir in &legacy_dirs {
        assert!(!legacy_dir.exists());
    }

    // A second call is a no-op.
    super::remove_legacy_index_dirs(db_path.path()).unwrap();
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
        index_store.lookup_digest(&genesis_tx_digest).unwrap(),
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
        index_store.lookup_digest(&genesis_tx_digest).unwrap(),
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

/// After an unclean stop the watermark can be ahead of the executed
/// checkpoint by up to the execution concurrency; replaying those
/// checkpoints writes nothing but the watermark, so no rebuild is needed.
#[tokio::test]
async fn test_a_watermark_far_ahead_of_the_executed_checkpoint_is_not_fatal() {
    let tmp_dir = iota_common::tempdir();
    let cp_dir = iota_common::tempdir();
    let groups = BTreeSet::from([IndexGroup::JsonRpc, IndexGroup::Grpc]);
    let checkpoint_store = CheckpointStore::new(&cp_dir.path().join("checkpoints"));
    let index_store = open_index_store(tmp_dir.path().to_path_buf());
    index_store.tables.seed_meta(&groups).unwrap();
    mark_checkpoint_executed(&checkpoint_store, 5);
    checkpoint_store
        .insert_verified_checkpoint(&executed_checkpoint(0, 7))
        .unwrap();
    index_store.tables.watermark.insert(&(), &7).unwrap();

    assert!(
        !index_store
            .tables
            .needs_to_do_initialization(&checkpoint_store, &groups)
            .unwrap()
    );
}

/// Numbering anchors to the watermark's checkpoint, so a watermark whose
/// checkpoint the store no longer holds is rebuilt from scratch.
#[tokio::test]
async fn test_a_watermark_without_its_checkpoint_rebuilds_the_index() {
    let dir = iota_common::tempdir();
    let index_dir = dir.path().join(super::schema::RPC_INDEXES_DIR);
    let groups = BTreeSet::from([IndexGroup::JsonRpc, IndexGroup::Grpc]);
    let checkpoint_store = CheckpointStore::new(&dir.path().join("checkpoints"));
    {
        let index_store = open_index_store(index_dir.clone());
        index_store.tables.seed_meta(&groups).unwrap();
        index_store.tables.watermark.insert(&(), &5).unwrap();
        close_index_store(index_store).await;
    }

    let authority_store = open_authority_store(&dir.path().join("store"));
    let index_store = RpcIndexesStore::new(
        index_dir,
        &Registry::default(),
        groups,
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

/// The history tables share one column family, so a scan of one must stop
/// at its own tag instead of running into the neighbouring table's rows.
#[tokio::test]
async fn test_history_tables_do_not_bleed_across_tags() {
    let tmp_dir = iota_common::tempdir();
    let index_store = open_index_store(tmp_dir.path().to_path_buf());
    let bucket = index_store.ensure_history_bucket(0).unwrap();

    let digest = TransactionDigest::random();
    let mut batch = index_store.tables.meta.batch();
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

/// A read error in the rebuild predicate propagates instead of silently
/// deciding to wipe or to adopt.
#[tokio::test]
async fn test_rebuild_predicate_propagates_read_errors() {
    let dir = iota_common::tempdir();
    let groups = BTreeSet::from([IndexGroup::JsonRpc, IndexGroup::Grpc]);
    let checkpoint_store = CheckpointStore::new(&dir.path().join("checkpoints"));
    let index_store = open_index_store(dir.path().join("meta-error"));
    index_store.tables.meta.db.drop_cf("meta").unwrap();
    assert!(
        index_store
            .tables
            .needs_to_do_initialization(&checkpoint_store, &groups)
            .is_err()
    );

    // The watermark-less arm reads the owner index to tell a build that
    // was cut short from a fresh store.
    let index_store = open_index_store(dir.path().join("owner-index-error"));
    index_store.tables.seed_meta(&groups).unwrap();
    index_store.tables.meta.db.drop_cf("owner").unwrap();
    assert!(
        index_store
            .tables
            .needs_to_do_initialization(&checkpoint_store, &groups)
            .is_err()
    );
}

/// Leftover files under the index directory are cleared before a
/// bulk-ingestion open instead of failing the recovery.
#[tokio::test]
async fn test_bulk_ingestion_open_clears_leftover_files() {
    let dir = iota_common::tempdir();
    let index_dir = dir.path().join(super::schema::RPC_INDEXES_DIR);
    std::fs::create_dir_all(&index_dir).unwrap();
    std::fs::write(index_dir.join("stray"), b"leftover").unwrap();

    let tables = super::IndexStoreTables::open_for_bulk_ingestion(index_dir.clone());
    assert_eq!(tables.meta.get(&()).unwrap(), None);
    assert!(!index_dir.join("stray").exists());
}

/// An owner page starts after the cursor's object, even when the cursor's
/// own row is gone: its position is rebuilt from the live object, which a
/// transfer leaves behind.
#[tokio::test]
async fn test_owner_objects_page_excludes_only_the_cursor() {
    let tmp_dir = iota_common::tempdir();
    let index_store = open_index_store(tmp_dir.path().to_path_buf());
    let owner = Address::random();
    let mut object_store = BTreeMap::new();
    seed_owner_objects_of_two_types(&index_store, &mut object_store, owner);

    let rows = |cursor: Option<OwnedObjectCursor>| {
        index_store
            .get_owner_objects(owner, cursor.as_ref(), 10, None, &object_store)
            .unwrap()
    };
    let ids = |rows: &[(ObjectInfo, OwnedObjectCursor)]| -> Vec<ObjectId> {
        rows.iter().map(|(info, _)| info.object_id).collect()
    };

    let all_rows = rows(None);
    let all = ids(&all_rows);
    assert_eq!(all.len(), 4);
    let first_cursor = all_rows[0].1;
    assert_eq!(ids(&rows(Some(first_cursor))), all[1..]);

    // The cursor's object can be transferred away between two pages: its
    // owner row is gone while the object itself still resolves.
    let cursor_object = object_store.get(&all[0]).unwrap();
    let (cursor_key, _) = OwnerIndexKey::for_object(owner, cursor_object).unwrap();
    let table = &index_store.tables.owner;
    let mut batch = table.batch();
    batch.delete_batch(table, [cursor_key]).unwrap();
    batch.write().unwrap();

    assert_eq!(
        ids(&rows(Some(first_cursor))),
        all[1..],
        "the objects after the cursor must not be lost with the cursor's row"
    );
}

/// A retention of one historic epoch keeps the current epoch's history and
/// the one before it, and drops everything older.
#[tokio::test]
async fn test_retention_of_one_epoch_keeps_the_previous_epoch() {
    let path = iota_common::tempdir();
    let store = open_index_store_with_retention(path.path(), Some(1));

    for epoch in 0..4 {
        store.ensure_history_bucket(epoch).unwrap();
    }
    assert_eq!(prune_at_newest_epoch(&store).unwrap(), Some(2));
    assert_eq!(store.history.earliest_retained(), 2);
}

/// A retention of two historic epochs keeps the current epoch and the two
/// before it.
#[tokio::test]
async fn test_retention_of_two_epochs_keeps_two_previous_epochs() {
    let path = iota_common::tempdir();
    let store = open_index_store_with_retention(path.path(), Some(2));

    for epoch in 0..4 {
        store.ensure_history_bucket(epoch).unwrap();
    }
    assert_eq!(prune_at_newest_epoch(&store).unwrap(), Some(1));
    assert_eq!(store.history.earliest_retained(), 1);
}

/// Pruning never drops the newest epoch's bucket, whatever a caller asks
/// for: checkpoint ingest reads its digests to tell an already-indexed
/// transaction from a new one.
#[tokio::test]
async fn test_pruning_keeps_the_newest_bucket_whatever_the_retention() {
    let path = iota_common::tempdir();
    let store = open_index_store_with_retention(path.path(), Some(0));

    for epoch in 0..3 {
        store.ensure_history_bucket(epoch).unwrap();
    }
    assert_eq!(prune_at_newest_epoch(&store).unwrap(), Some(2));
    assert_eq!(store.history.newest_epoch(), Some(2));

    // The bucket ingest depends on is still usable after the prune.
    let mut builder = TestCheckpointDataBuilder::new(0)
        .with_epoch(2)
        .start_transaction(0)
        .create_coin_object(0, 1, 100, TypeTag::from(StructTag::new_gas()))
        .finish_transaction();
    let checkpoint = builder.build_checkpoint();
    let digest = *checkpoint.transactions[0].effects.transaction_digest();
    index_checkpoint_for_testing(&store, &checkpoint);
    assert!(store.lookup_digest(&digest).unwrap().is_some());
}
