// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::sync::Arc;

use iota_sdk_types::{ObjectId, Owner};
use iota_types::{committee::EpochId, object::Object, storage::ObjectKey};
use prometheus_filtered::Registry;
use tempfile::TempDir;
use typed_store::{database::wait_for_database_close, traits::Map};

use super::{ObjectBacklogSweep, ObjectBacklogSweepProgress};
use crate::authority::{
    AuthorityStore,
    authority_store_tables::AuthorityPerpetualTables,
    authority_store_types::{StoreObject, StoreObjectWrapper, get_store_object},
};

/// The epoch that is current while the sweep runs, and whose bucket it
/// records the tombstones it finds in.
const SWEEP_EPOCH: EpochId = 7;

/// An object with three live versions, walked first.
fn live_id() -> ObjectId {
    ObjectId::new([1; 32])
}

/// An object deleted at its third version, walked second.
fn deleted_id() -> ObjectId {
    ObjectId::new([2; 32])
}

/// An object wrapped at its second version and unwrapped at its third,
/// walked last.
fn wrapped_id() -> ObjectId {
    ObjectId::new([3; 32])
}

fn open_store(dir: &TempDir) -> Arc<AuthorityStore> {
    let (perpetual, historic) =
        AuthorityPerpetualTables::open_with_historic_objects(dir.path(), None).unwrap();
    AuthorityStore::open_no_genesis(
        Arc::new(perpetual),
        Arc::new(historic),
        false,
        &Registry::new(),
    )
    .unwrap()
}

fn value(id: ObjectId, version: u64) -> (ObjectKey, StoreObjectWrapper) {
    (
        ObjectKey(id, version.into()),
        get_store_object(
            Object::with_id_owner_version_for_testing(id, version.into(), Owner::Immutable),
            None,
        ),
    )
}

fn tombstone(id: ObjectId, version: u64, row: StoreObject) -> (ObjectKey, StoreObjectWrapper) {
    (ObjectKey(id, version.into()), StoreObjectWrapper::from(row))
}

/// Writes the live table an earlier build would have left behind: every
/// object still carries the versions superseded before the upgrade next to
/// its newest row.
fn seed(store: &AuthorityStore) {
    store
        .perpetual_tables
        .objects
        .multi_insert([
            value(live_id(), 1),
            value(live_id(), 2),
            value(live_id(), 3),
            value(deleted_id(), 1),
            value(deleted_id(), 2),
            tombstone(deleted_id(), 3, StoreObject::Deleted),
            value(wrapped_id(), 1),
            tombstone(wrapped_id(), 2, StoreObject::Wrapped),
            value(wrapped_id(), 3),
        ])
        .unwrap();
}

fn sweeper(store: &AuthorityStore, keys_per_slice: usize) -> ObjectBacklogSweep {
    ObjectBacklogSweep {
        perpetual_tables: store.perpetual_tables.clone(),
        historic_objects: store.get_historic_objects().clone(),
        keys_per_slice,
    }
}

/// Runs the whole walk, in slices of `keys_per_slice`.
fn sweep_all(store: &AuthorityStore, keys_per_slice: usize) {
    let sweep = sweeper(store, keys_per_slice);
    while sweep.sweep_slice(SWEEP_EPOCH).unwrap() {}
}

fn live_keys(store: &AuthorityStore) -> Vec<ObjectKey> {
    store
        .perpetual_tables
        .objects
        .safe_iter()
        .map(|row| row.unwrap().0)
        .collect()
}

fn recorded_tombstones(store: &AuthorityStore, epoch: EpochId) -> Vec<ObjectKey> {
    store
        .get_historic_objects()
        .ensure(epoch)
        .unwrap()
        .tombstones
        .safe_iter()
        .map(|row| row.unwrap().0)
        .collect()
}

fn progress(store: &AuthorityStore) -> Option<ObjectBacklogSweepProgress> {
    store
        .perpetual_tables
        .object_backlog_sweep_progress
        .get(&())
        .unwrap()
}

/// The live table is left with the newest version of every object and with
/// every tombstone, including the one an unwrap left below a newer version.
/// Each tombstone is recorded in the current epoch's bucket, so that
/// ordinary retention deletes it later, and nothing is relocated into that
/// bucket.
#[tokio::test]
async fn the_sweep_keeps_the_latest_version_and_the_tombstones() {
    let dir = iota_common::tempdir();
    let store = open_store(&dir);
    seed(&store);

    sweep_all(&store, 5_000);

    assert_eq!(
        live_keys(&store),
        vec![
            ObjectKey(live_id(), 3.into()),
            ObjectKey(deleted_id(), 3.into()),
            ObjectKey(wrapped_id(), 2.into()),
            ObjectKey(wrapped_id(), 3.into()),
        ]
    );
    assert_eq!(
        recorded_tombstones(&store, SWEEP_EPOCH),
        vec![
            ObjectKey(deleted_id(), 3.into()),
            ObjectKey(wrapped_id(), 2.into()),
        ]
    );
    assert!(
        store
            .get_historic_objects()
            .ensure(SWEEP_EPOCH)
            .unwrap()
            .objects
            .is_empty()
    );
    assert_eq!(progress(&store), Some(ObjectBacklogSweepProgress::Done));
}

/// A walk stopped part-way resumes from the key it recorded, across a
/// restart, and leaves the same table an uninterrupted walk does.
#[tokio::test]
async fn the_sweep_resumes_from_its_watermark() {
    let uninterrupted_dir = iota_common::tempdir();
    let uninterrupted = open_store(&uninterrupted_dir);
    seed(&uninterrupted);
    sweep_all(&uninterrupted, 5_000);

    let dir = iota_common::tempdir();
    let interrupted = open_store(&dir);
    seed(&interrupted);
    let sweep = sweeper(&interrupted, 1);
    assert!(sweep.sweep_slice(SWEEP_EPOCH).unwrap());
    // One row decided, the first version of the first object id, which the
    // second version supersedes.
    assert_eq!(
        progress(&interrupted),
        Some(ObjectBacklogSweepProgress::SweptThrough(ObjectKey(
            live_id(),
            1.into()
        )))
    );
    assert_eq!(live_keys(&interrupted).len(), 8);

    // Release every handle on the database before reopening the same path,
    // as a restart does.
    let weak_db = Arc::downgrade(&interrupted.perpetual_tables.objects.db);
    drop(sweep);
    drop(interrupted);
    assert!(wait_for_database_close(weak_db).await);

    let resumed = open_store(&dir);
    sweep_all(&resumed, 1);

    assert_eq!(live_keys(&resumed), live_keys(&uninterrupted));
    assert_eq!(
        recorded_tombstones(&resumed, SWEEP_EPOCH),
        recorded_tombstones(&uninterrupted, SWEEP_EPOCH)
    );
    assert_eq!(progress(&resumed), Some(ObjectBacklogSweepProgress::Done));
}

/// Once the walk has reached the end of the table, a later start does
/// nothing: from then on a superseded version leaves the live table in the
/// batch that supersedes it, and there is no backlog left to drain.
#[tokio::test]
async fn a_finished_sweep_leaves_later_starts_nothing_to_do() {
    let dir = iota_common::tempdir();
    let store = open_store(&dir);
    seed(&store);
    sweep_all(&store, 5_000);

    let (key, row) = value(live_id(), 4);
    store.perpetual_tables.objects.insert(&key, &row).unwrap();

    sweep_all(&store, 5_000);

    let superseded = ObjectKey(live_id(), 3.into());
    assert!(
        store
            .perpetual_tables
            .objects
            .get(&superseded)
            .unwrap()
            .is_some()
    );
}
