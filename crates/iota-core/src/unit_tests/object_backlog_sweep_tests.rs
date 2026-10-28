// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::sync::Arc;

use iota_sdk_types::{ObjectId, Owner, Version};
use iota_types::{
    committee::EpochId, messages_checkpoint::CheckpointSequenceNumber, object::Object,
    storage::ObjectKey,
};
use prometheus_filtered::Registry;
use tempfile::TempDir;
use typed_store::{database::wait_for_database_close, traits::Map};

use super::{KEYS_PER_SLICE, ObjectBacklogSweep, ObjectBacklogSweepProgress, WALK_RANGES, sweep};
use crate::authority::{
    AuthorityStore,
    authority_store_tables::AuthorityPerpetualTables,
    authority_store_types::{StoreObject, StoreObjectWrapper, get_store_object},
    ledger_backlog_migration::RangeProgress,
};

/// The epoch that is current while the sweep runs.
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
    let (perpetual, historic, historic_ledger, epoch_markers) =
        AuthorityPerpetualTables::open_with_historic_objects(dir.path(), None).unwrap();
    AuthorityStore::open_no_genesis(
        Arc::new(perpetual),
        Arc::new(historic),
        Arc::new(historic_ledger),
        Arc::new(epoch_markers),
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

/// Writes a live table in which every object still has its superseded
/// versions next to its newest row.
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
    sweeper(store, keys_per_slice)
        .walk_in_parallel(SWEEP_EPOCH)
        .unwrap();
}

/// The walk range holding `id`'s versions.
fn walk_range_of(id: ObjectId) -> usize {
    usize::from(id.as_bytes()[0]) * WALK_RANGES / 256
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

/// The keys of the versions relocated into `epoch`'s bucket.
fn relocated_keys(store: &AuthorityStore, epoch: EpochId) -> Vec<ObjectKey> {
    store
        .get_historic_objects()
        .ensure(epoch)
        .unwrap()
        .objects
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

/// Records an objects pruner watermark, as the copy out of the dropped
/// `pruned_checkpoint` column family leaves it.
fn seed_pruner_watermark(store: &AuthorityStore, watermark: CheckpointSequenceNumber) {
    store
        .perpetual_tables
        .object_backlog_sweep_bound
        .insert(&(), &watermark)
        .unwrap();
}

/// The live table keeps the newest version of every object and every
/// tombstone, including one an unwrap left below a newer version. Superseded
/// versions are relocated into the current epoch's bucket, where each
/// tombstone is recorded too.
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
        relocated_keys(&store, SWEEP_EPOCH),
        vec![
            ObjectKey(live_id(), 1.into()),
            ObjectKey(live_id(), 2.into()),
            ObjectKey(deleted_id(), 1.into()),
            ObjectKey(deleted_id(), 2.into()),
            ObjectKey(wrapped_id(), 1.into()),
        ]
    );
    assert_eq!(
        recorded_tombstones(&store, SWEEP_EPOCH),
        vec![
            ObjectKey(deleted_id(), 3.into()),
            ObjectKey(wrapped_id(), 2.into()),
        ]
    );
    assert_eq!(progress(&store), Some(ObjectBacklogSweepProgress::Done));
}

/// A relocated version is served by the bounded read from the bucket, and one
/// relocated from under a tombstone is never served above that tombstone.
#[tokio::test]
async fn a_relocated_version_is_readable_from_the_current_epoch_bucket() {
    let dir = iota_common::tempdir();
    let store = open_store(&dir);
    seed(&store);

    sweep_all(&store, 5_000);

    let key = ObjectKey(live_id(), 2.into());
    let object = Object::with_id_owner_version_for_testing(live_id(), 2.into(), Owner::Immutable);
    assert_eq!(
        store
            .get_historic_objects()
            .ensure(SWEEP_EPOCH)
            .unwrap()
            .objects
            .get(&key)
            .unwrap(),
        Some(object.clone())
    );
    assert_eq!(
        store.get_historic_objects().get(&key).unwrap(),
        Some(object)
    );

    for (id, bound, expected) in [
        (live_id(), 1, Some(1)),
        (live_id(), 2, Some(2)),
        (live_id(), 3, Some(3)),
        (deleted_id(), 2, Some(2)),
        // At and above the tombstone.
        (deleted_id(), 3, None),
        (deleted_id(), 4, None),
    ] {
        assert_eq!(
            store
                .find_object_lt_or_eq_version_with_historic_fallback(id, bound.into())
                .unwrap()
                .map(|object| object.version()),
            expected.map(Version::from),
            "object {id} bounded at {bound}"
        );
    }
}

/// Every range of the walk sweeps the objects whose ids fall in it,
/// including the first and the last range.
#[tokio::test]
async fn every_walk_range_sweeps_its_own_objects() {
    let dir = iota_common::tempdir();
    let store = open_store(&dir);
    let ids: Vec<ObjectId> = [0x00, 0x07, 0x08, 0x80, 0xff]
        .into_iter()
        .map(|first| {
            let mut bytes = [0x11; 32];
            bytes[0] = first;
            ObjectId::new(bytes)
        })
        .collect();
    store
        .perpetual_tables
        .objects
        .multi_insert(
            ids.iter()
                .flat_map(|id| [value(*id, 1), value(*id, 2), value(*id, 3)]),
        )
        .unwrap();

    sweep_all(&store, 1);

    assert_eq!(
        live_keys(&store),
        ids.iter()
            .map(|id| ObjectKey(*id, 3.into()))
            .collect::<Vec<_>>()
    );
    assert_eq!(
        relocated_keys(&store, SWEEP_EPOCH),
        ids.iter()
            .flat_map(|id| [ObjectKey(*id, 1.into()), ObjectKey(*id, 2.into())])
            .collect::<Vec<_>>()
    );
    assert_eq!(progress(&store), Some(ObjectBacklogSweepProgress::Done));
}

/// One call walks the whole table, however many slices that takes.
#[tokio::test]
async fn one_call_drives_the_walk_past_the_slice_boundary() {
    let dir = iota_common::tempdir();
    let store = open_store(&dir);
    let last_version = KEYS_PER_SLICE as u64 + 10;
    store
        .perpetual_tables
        .objects
        .multi_insert((1..=last_version).map(|version| value(live_id(), version)))
        .unwrap();

    sweep(store.clone(), SWEEP_EPOCH).await.unwrap();

    assert_eq!(
        live_keys(&store),
        vec![ObjectKey(live_id(), last_version.into())]
    );
    assert_eq!(
        relocated_keys(&store, SWEEP_EPOCH),
        (1..last_version)
            .map(|version| ObjectKey(live_id(), version.into()))
            .collect::<Vec<_>>()
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
    let range = walk_range_of(live_id());
    let (mut batch, decided, next) = sweep.sweep_slice(range, None, SWEEP_EPOCH).unwrap();
    assert_eq!(decided, 1);
    let mut positions = vec![RangeProgress::At(None); WALK_RANGES];
    positions[range] = next;
    batch
        .insert_batch(
            &interrupted.perpetual_tables.object_backlog_sweep_progress,
            [((), ObjectBacklogSweepProgress::Walking(positions))],
        )
        .unwrap();
    batch.write().unwrap();
    let Some(ObjectBacklogSweepProgress::Walking(recorded)) = progress(&interrupted) else {
        panic!("the interrupted walk must have recorded its ranges");
    };
    assert_eq!(
        recorded[range],
        RangeProgress::At(Some(ObjectKey(live_id(), 1.into())))
    );
    assert_eq!(live_keys(&interrupted).len(), 8);
    assert_eq!(
        relocated_keys(&interrupted, SWEEP_EPOCH),
        vec![ObjectKey(live_id(), 1.into())]
    );

    let weak_db = Arc::downgrade(&interrupted.perpetual_tables.objects.db);
    drop(sweep);
    drop(interrupted);
    assert!(wait_for_database_close(weak_db).await);

    let resumed = open_store(&dir);
    sweep_all(&resumed, 1);

    assert_eq!(live_keys(&resumed), live_keys(&uninterrupted));
    assert_eq!(
        relocated_keys(&resumed, SWEEP_EPOCH),
        relocated_keys(&uninterrupted, SWEEP_EPOCH)
    );
    assert_eq!(
        recorded_tombstones(&resumed, SWEEP_EPOCH),
        recorded_tombstones(&uninterrupted, SWEEP_EPOCH)
    );
    assert_eq!(progress(&resumed), Some(ObjectBacklogSweepProgress::Done));
}

/// Once the walk has reached the end of the table, a later start does
/// nothing: from then on, commit relocates superseded versions itself.
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
    assert!(
        !relocated_keys(&store, SWEEP_EPOCH).contains(&superseded),
        "a version superseded after the walk finished is the commit's to relocate"
    );
}

/// A finished sweep records the epoch whose bucket it moved the backlog into,
/// and a reopen reads it back.
#[tokio::test]
async fn a_finished_sweep_records_the_epoch_it_swept_into() {
    let dir = iota_common::tempdir();
    {
        let store = open_store(&dir);
        assert_eq!(store.get_historic_objects().backlog_sweep_epoch(), None);
        seed(&store);
        sweep_all(&store, 5_000);
        assert_eq!(
            store.get_historic_objects().backlog_sweep_epoch(),
            Some(SWEEP_EPOCH)
        );
        let weak_db = Arc::downgrade(&store.perpetual_tables.objects.db);
        drop(store);
        assert!(wait_for_database_close(weak_db).await);
    }

    let store = open_store(&dir);
    assert_eq!(
        store.get_historic_objects().backlog_sweep_epoch(),
        Some(SWEEP_EPOCH)
    );
}

/// An objects pruner watermark does not narrow the walk: every superseded
/// version left in the live table is relocated, whatever checkpoint
/// superseded it. A sweep that trusted the watermark would read only the
/// checkpoints above it, which this database does not hold, and relocate
/// nothing.
#[tokio::test]
async fn a_pruner_watermark_does_not_narrow_the_walk() {
    let dir = iota_common::tempdir();
    let store = open_store(&dir);

    seed(&store);
    // A ledger bucket and a low watermark: what made the watermark look
    // trustworthy to a sweep reading checkpoints.
    store.get_historic_ledger().ensure(0).unwrap();
    seed_pruner_watermark(&store, 5);

    sweep(store.clone(), SWEEP_EPOCH).await.unwrap();

    assert_eq!(
        relocated_keys(&store, SWEEP_EPOCH),
        vec![
            ObjectKey(live_id(), 1.into()),
            ObjectKey(live_id(), 2.into()),
            ObjectKey(deleted_id(), 1.into()),
            ObjectKey(deleted_id(), 2.into()),
            ObjectKey(wrapped_id(), 1.into()),
        ]
    );
}

/// The walked fraction counts a finished range whole, an unstarted one as
/// nothing, and a range at the middle of its share of the id space as half.
#[test]
fn the_walked_fraction_follows_each_range_through_its_share_of_the_id_space() {
    let mut ranges = vec![RangeProgress::At(None); WALK_RANGES];
    assert_eq!(super::walked_fraction(&ranges), 0.0);

    ranges[0] = RangeProgress::Done;
    // Range 1 covers the first bytes from one share to two; this key is half
    // way through.
    let share = 256 / WALK_RANGES;
    let mut middle = [0; ObjectId::LENGTH];
    middle[0] = (share + share / 2) as u8;
    ranges[1] = RangeProgress::At(Some(ObjectKey(ObjectId::new(middle), 1.into())));
    let fraction = super::walked_fraction(&ranges);
    assert!(
        (fraction - 1.5 / WALK_RANGES as f64).abs() < 1e-9,
        "{fraction}"
    );

    let all_done = vec![RangeProgress::Done; WALK_RANGES];
    assert_eq!(super::walked_fraction(&all_done), 1.0);
}
