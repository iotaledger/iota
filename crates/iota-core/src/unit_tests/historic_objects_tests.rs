// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{collections::BTreeMap, sync::Arc};

use iota_sdk_types::{ObjectId, Owner, Version};
use iota_types::{committee::EpochId, object::Object, storage::ObjectKey};
use prometheus_filtered::Registry;
use tempfile::TempDir;
use typed_store::{
    database::wait_for_database_close,
    rocks::{DBMap, ReadWriteOptions, TaggedDBMap, default_db_options},
    traits::Map,
};

use super::{
    DB_PREFIX_HISTORIC_TOMBSTONES, EARLIEST_RETAINED_CF, HistoricObjects,
    TOMBSTONE_DELETE_BATCH_SIZE,
};
use crate::authority::{
    AuthorityStore,
    authority_store_tables::AuthorityPerpetualTables,
    authority_store_types::{StoreObject, StoreObjectWrapper, get_store_object},
};

/// A perpetual store, its historic buckets, and an [`AuthorityStore`] over
/// both, for the reads that consult the live `objects` table and the buckets
/// together. The directory is returned so it outlives the databases.
fn test_store() -> (
    Arc<AuthorityPerpetualTables>,
    Arc<HistoricObjects>,
    Arc<AuthorityStore>,
    TempDir,
) {
    let dir = iota_common::tempdir();
    let (perpetual, historic, historic_ledger) =
        AuthorityPerpetualTables::open_with_historic_objects(dir.path(), None).unwrap();
    let perpetual = Arc::new(perpetual);
    let historic = Arc::new(historic);
    let store = AuthorityStore::open_no_genesis(
        perpetual.clone(),
        historic.clone(),
        Arc::new(historic_ledger),
        false,
        &Registry::new(),
    )
    .unwrap();
    (perpetual, historic, store, dir)
}

fn object_at(id: ObjectId, version: u64) -> Object {
    Object::with_id_owner_version_for_testing(id, version.into(), Owner::Immutable)
}

/// A relocated version is readable from the bucket of the epoch it was
/// relocated into, and a version never relocated is absent.
#[tokio::test]
async fn test_relocated_version_is_readable_from_its_bucket() {
    let dir = iota_common::tempdir();
    let (perpetual, historic, _historic_ledger) =
        AuthorityPerpetualTables::open_with_historic_objects(dir.path(), None).unwrap();

    let object = Object::immutable_with_id_for_testing(ObjectId::random());
    let key = ObjectKey(object.id(), object.version());

    let bucket = historic.ensure(3).unwrap();
    let mut batch = perpetual.objects.batch();
    batch
        .insert_batch_tagged(&bucket.objects, [(key, object.clone())])
        .unwrap();
    batch.write().unwrap();

    assert_eq!(historic.get(&key).unwrap().as_ref(), Some(&object));
    assert_eq!(
        historic
            .get(&ObjectKey(ObjectId::random(), 1.into()))
            .unwrap(),
        None
    );
}

/// Buckets of different epochs are independent, and a lookup finds a key
/// in whichever bucket holds it.
#[tokio::test]
async fn test_lookup_spans_epoch_buckets() {
    let dir = iota_common::tempdir();
    let (perpetual, historic, _historic_ledger) =
        AuthorityPerpetualTables::open_with_historic_objects(dir.path(), None).unwrap();

    let older = Object::immutable_with_id_for_testing(ObjectId::random());
    let newer = Object::immutable_with_id_for_testing(ObjectId::random());
    let older_key = ObjectKey(older.id(), older.version());
    let newer_key = ObjectKey(newer.id(), newer.version());

    for (epoch, key, object) in [(1, older_key, &older), (2, newer_key, &newer)] {
        let bucket = historic.ensure(epoch).unwrap();
        let mut batch = perpetual.objects.batch();
        batch
            .insert_batch_tagged(&bucket.objects, [(key, (*object).clone())])
            .unwrap();
        batch.write().unwrap();
    }

    assert_eq!(historic.get(&older_key).unwrap().as_ref(), Some(&older));
    assert_eq!(historic.get(&newer_key).unwrap().as_ref(), Some(&newer));
}

/// A relocated version survives a restart: the next open rediscovers the
/// bucket's column family on disk instead of serving an empty store.
#[tokio::test]
async fn test_relocated_version_survives_a_reopen() {
    let dir = iota_common::tempdir();
    let (perpetual, historic, historic_ledger) =
        AuthorityPerpetualTables::open_with_historic_objects(dir.path(), None).unwrap();

    let object = Object::immutable_with_id_for_testing(ObjectId::random());
    let key = ObjectKey(object.id(), object.version());

    let bucket = historic.ensure(3).unwrap();
    let mut batch = perpetual.objects.batch();
    batch
        .insert_batch_tagged(&bucket.objects, [(key, object.clone())])
        .unwrap();
    batch.write().unwrap();

    // Release every handle on the database before reopening the same path,
    // as a restart does.
    let weak_db = Arc::downgrade(&perpetual.objects.db);
    drop(bucket);
    drop(historic);
    drop(historic_ledger);
    drop(perpetual);
    assert!(wait_for_database_close(weak_db).await);

    let (_perpetual, historic, _historic_ledger) =
        AuthorityPerpetualTables::open_with_historic_objects(dir.path(), None).unwrap();
    assert_eq!(historic.get(&key).unwrap().as_ref(), Some(&object));
}

/// A tombstone head recorded alongside a relocated version survives a
/// restart the same way the version itself does, and the bucket has no
/// expiring marker until something sets one.
#[tokio::test]
async fn test_tombstone_heads_survive_a_reopen() {
    let dir = iota_common::tempdir();
    let (perpetual, historic, historic_ledger) =
        AuthorityPerpetualTables::open_with_historic_objects(dir.path(), None).unwrap();

    let object = Object::immutable_with_id_for_testing(ObjectId::random());
    let key = ObjectKey(object.id(), object.version());

    let bucket = historic.ensure(3).unwrap();
    let mut batch = perpetual.objects.batch();
    batch
        .insert_batch_tagged(&bucket.tombstones, [(key, ())])
        .unwrap();
    batch.write().unwrap();

    let weak_db = Arc::downgrade(&perpetual.objects.db);
    drop(bucket);
    drop(historic);
    drop(historic_ledger);
    drop(perpetual);
    assert!(wait_for_database_close(weak_db).await);

    let (_perpetual, historic, _historic_ledger) =
        AuthorityPerpetualTables::open_with_historic_objects(dir.path(), None).unwrap();
    let bucket = historic.ensure(3).unwrap();
    assert!(bucket.tombstones.get(&key).unwrap().is_some());
    assert!(bucket.expiring.get(&()).unwrap().is_none());
}

/// `iota-tool`'s table dump reaches a bucket and the retention floor through
/// [`HistoricObjects::dump_column_family`], since neither is a field of
/// `AuthorityPerpetualTables`, and gets nothing for a name that belongs to
/// neither. A bucket's dump covers its relocated versions, its tombstone
/// heads and its expiring marker alike, since all three share the bucket's
/// column family.
#[tokio::test]
async fn test_dump_reads_a_bucket_and_the_retention_floor() {
    let dir = iota_common::tempdir();
    let (perpetual, historic, _historic_ledger) =
        AuthorityPerpetualTables::open_with_historic_objects(dir.path(), None).unwrap();

    let object = Object::immutable_with_id_for_testing(ObjectId::random());
    let key = ObjectKey(object.id(), object.version());
    let tombstone_key = ObjectKey(ObjectId::random(), object.version());

    let bucket = historic.ensure(3).unwrap();
    let mut batch = perpetual.objects.batch();
    batch
        .insert_batch_tagged(&bucket.objects, [(key, object)])
        .unwrap();
    batch
        .insert_batch_tagged(&bucket.tombstones, [(tombstone_key, ())])
        .unwrap();
    batch
        .insert_batch_tagged(&bucket.expiring, [((), ())])
        .unwrap();
    batch.write().unwrap();
    // The dump reads through a secondary handle, which only sees what the
    // primary has written out.
    perpetual.objects.db.flush_all().unwrap();

    let read_only = AuthorityPerpetualTables::open_readonly(dir.path());
    let db = &read_only.objects.db;

    let rows = HistoricObjects::dump_column_family(db, "hist_obj_e3", 100, 0)
        .unwrap()
        .expect("a bucket's column family is dumpable");
    assert_eq!(rows.len(), 3);
    assert!(rows.contains_key(&format!("{key:?}")));
    assert!(rows.contains_key(&format!("tombstone:{tombstone_key:?}")));
    assert!(rows.contains_key("expiring:()"));

    assert_eq!(
        HistoricObjects::dump_column_family(db, "hist_obj_retention", 100, 0).unwrap(),
        Some(BTreeMap::new())
    );
    assert_eq!(
        HistoricObjects::dump_column_family(db, "objects", 100, 0).unwrap(),
        None
    );
}

/// Expiring an epoch deletes the tombstone heads it recorded from the live
/// `objects` table together with its relocated versions, and a second prune
/// over the same retention window is harmless.
#[tokio::test]
async fn test_expiry_deletes_the_epochs_tombstone_heads() {
    let dir = iota_common::tempdir();
    let (perpetual, historic, _historic_ledger) =
        AuthorityPerpetualTables::open_with_historic_objects(dir.path(), None).unwrap();

    let object = Object::immutable_with_id_for_testing(ObjectId::random());
    let relocated = ObjectKey(object.id(), object.version());
    let deleted = ObjectKey(ObjectId::random(), 4.into());

    let bucket = historic.ensure(1).unwrap();
    let mut batch = perpetual.objects.batch();
    batch
        .insert_batch_tagged(&bucket.objects, [(relocated, object)])
        .unwrap();
    batch
        .insert_batch_tagged(&bucket.tombstones, [(deleted, ())])
        .unwrap();
    batch
        .insert_batch(
            &perpetual.objects,
            [(deleted, StoreObjectWrapper::from(StoreObject::Deleted))],
        )
        .unwrap();
    batch.write().unwrap();
    drop(bucket);

    historic.ensure(2).unwrap();
    assert_eq!(historic.prune(1).unwrap(), Some(2));
    assert!(perpetual.objects.get(&deleted).unwrap().is_none());
    assert_eq!(historic.get(&relocated).unwrap(), None);

    assert_eq!(historic.prune(1).unwrap(), Some(2));
}

/// An epoch holding more tombstone heads than fit in one write batch has all
/// of them deleted, the remainder past the last full batch included.
#[tokio::test]
async fn test_expiry_deletes_heads_past_the_batch_boundary() {
    let dir = iota_common::tempdir();
    let (perpetual, historic, _historic_ledger) =
        AuthorityPerpetualTables::open_with_historic_objects(dir.path(), None).unwrap();

    let deleted: Vec<ObjectKey> = (0..TOMBSTONE_DELETE_BATCH_SIZE + 1)
        .map(|version| ObjectKey(ObjectId::random(), (version as u64 + 1).into()))
        .collect();

    let bucket = historic.ensure(1).unwrap();
    let mut batch = perpetual.objects.batch();
    batch
        .insert_batch_tagged(&bucket.tombstones, deleted.iter().map(|key| (*key, ())))
        .unwrap();
    batch
        .insert_batch(
            &perpetual.objects,
            deleted
                .iter()
                .map(|key| (*key, StoreObjectWrapper::from(StoreObject::Deleted))),
        )
        .unwrap();
    batch.write().unwrap();
    drop(bucket);

    historic.ensure(2).unwrap();
    assert_eq!(historic.prune(1).unwrap(), Some(2));
    for key in &deleted {
        assert!(
            perpetual.objects.get(key).unwrap().is_none(),
            "{key:?} was left in the live table"
        );
    }
}

/// A bucket already marked expiring is skipped by reads before its column
/// family is dropped: its tombstone heads may be gone from the live table by
/// then, and a version served from under a deleted tombstone would resurrect
/// a deleted object.
#[tokio::test]
async fn test_a_bucket_marked_expiring_is_skipped_by_reads() {
    let dir = iota_common::tempdir();
    let (perpetual, historic, _historic_ledger) =
        AuthorityPerpetualTables::open_with_historic_objects(dir.path(), None).unwrap();

    let object = Object::immutable_with_id_for_testing(ObjectId::random());
    let key = ObjectKey(object.id(), object.version());

    let bucket = historic.ensure(1).unwrap();
    let mut batch = perpetual.objects.batch();
    batch
        .insert_batch_tagged(&bucket.objects, [(key, object.clone())])
        .unwrap();
    batch.write().unwrap();
    assert_eq!(historic.get(&key).unwrap().as_ref(), Some(&object));

    bucket.mark_expiring().unwrap();
    assert_eq!(historic.get(&key).unwrap(), None);
    // The row is still there; it is the marker that takes the bucket out of
    // the read path.
    assert!(bucket.objects.get(&key).unwrap().is_some());
}

/// An expiry interrupted after its marker was written is finished at the next
/// open: the bucket's tombstone heads are deleted from the live table and its
/// column family is dropped, before any query can reach it.
#[tokio::test]
async fn test_an_interrupted_expiry_is_finished_at_open() {
    let dir = iota_common::tempdir();
    let (perpetual, historic, historic_ledger) =
        AuthorityPerpetualTables::open_with_historic_objects(dir.path(), None).unwrap();

    let object = Object::immutable_with_id_for_testing(ObjectId::random());
    let relocated = ObjectKey(object.id(), object.version());
    let deleted = ObjectKey(ObjectId::random(), 4.into());

    let bucket = historic.ensure(1).unwrap();
    let mut batch = perpetual.objects.batch();
    batch
        .insert_batch_tagged(&bucket.objects, [(relocated, object)])
        .unwrap();
    batch
        .insert_batch_tagged(&bucket.tombstones, [(deleted, ())])
        .unwrap();
    batch
        .insert_batch_tagged(&bucket.expiring, [((), ())])
        .unwrap();
    batch
        .insert_batch(
            &perpetual.objects,
            [(deleted, StoreObjectWrapper::from(StoreObject::Deleted))],
        )
        .unwrap();
    batch.write().unwrap();

    let weak_db = Arc::downgrade(&perpetual.objects.db);
    drop(bucket);
    drop(historic);
    drop(historic_ledger);
    drop(perpetual);
    assert!(wait_for_database_close(weak_db).await);

    let (perpetual, historic, _historic_ledger) =
        AuthorityPerpetualTables::open_with_historic_objects(dir.path(), None).unwrap();
    assert!(perpetual.objects.get(&deleted).unwrap().is_none());
    assert_eq!(historic.get(&relocated).unwrap(), None);
}

/// A prune persists its retention floor before it marks the first bucket, so
/// a crash in between leaves a bucket below the floor and unmarked. Its expiry
/// is finished at the next open all the same: dropping its column family on
/// its own would leave its tombstone heads in the live `objects` table with
/// nothing left to delete them.
#[tokio::test]
async fn test_a_bucket_below_the_retention_floor_is_expired_at_open() {
    let dir = iota_common::tempdir();
    let (perpetual, historic, historic_ledger) =
        AuthorityPerpetualTables::open_with_historic_objects(dir.path(), None).unwrap();

    let object = Object::immutable_with_id_for_testing(ObjectId::random());
    let relocated = ObjectKey(object.id(), object.version());
    let deleted = ObjectKey(ObjectId::random(), 4.into());

    let bucket = historic.ensure(1).unwrap();
    let mut batch = perpetual.objects.batch();
    batch
        .insert_batch_tagged(&bucket.objects, [(relocated, object)])
        .unwrap();
    batch
        .insert_batch_tagged(&bucket.tombstones, [(deleted, ())])
        .unwrap();
    batch
        .insert_batch(
            &perpetual.objects,
            [(deleted, StoreObjectWrapper::from(StoreObject::Deleted))],
        )
        .unwrap();
    batch.write().unwrap();
    historic.ensure(2).unwrap();

    // The floor a prune persists first, without the marker it would have
    // written next.
    let earliest_retained_table: DBMap<(), EpochId> = DBMap::reopen(
        &perpetual.objects.db,
        Some(EARLIEST_RETAINED_CF),
        &ReadWriteOptions::default(),
        true,
    )
    .unwrap();
    earliest_retained_table.insert(&(), &2).unwrap();
    assert!(bucket.expiring.get(&()).unwrap().is_none());

    let weak_db = Arc::downgrade(&perpetual.objects.db);
    drop(earliest_retained_table);
    drop(bucket);
    drop(historic);
    drop(historic_ledger);
    drop(perpetual);
    assert!(wait_for_database_close(weak_db).await);

    let (perpetual, historic, _historic_ledger) =
        AuthorityPerpetualTables::open_with_historic_objects(dir.path(), None).unwrap();
    assert!(perpetual.objects.get(&deleted).unwrap().is_none());
    assert_eq!(historic.get(&relocated).unwrap(), None);
    assert_eq!(historic.earliest_bucket_epoch(), Some(2));
}

/// Recovery at open goes oldest bucket first and stops at the first bucket it
/// cannot finish, here one whose tombstone heads no longer deserialize: the
/// newer bucket's tombstone is still in the live table, because the versions
/// beneath it are still readable from the bucket below.
#[tokio::test]
async fn test_interrupted_expiries_are_resumed_oldest_first() {
    let dir = iota_common::tempdir();
    let (perpetual, historic, _historic_ledger) =
        AuthorityPerpetualTables::open_with_historic_objects(dir.path(), None).unwrap();

    let older_tombstone = ObjectKey(ObjectId::random(), 4.into());
    let newer_tombstone = ObjectKey(ObjectId::random(), 7.into());

    for (epoch, tombstone) in [(1, older_tombstone), (2, newer_tombstone)] {
        let bucket = historic.ensure(epoch).unwrap();
        let mut batch = perpetual.objects.batch();
        batch
            .insert_batch_tagged(&bucket.tombstones, [(tombstone, ())])
            .unwrap();
        batch
            .insert_batch_tagged(&bucket.expiring, [((), ())])
            .unwrap();
        batch
            .insert_batch(
                &perpetual.objects,
                [(tombstone, StoreObjectWrapper::from(StoreObject::Deleted))],
            )
            .unwrap();
        batch.write().unwrap();
    }

    // A value of another type under the tombstone tag of the older bucket, so
    // that reading its tombstone heads back fails.
    let unreadable: TaggedDBMap<ObjectKey, u64> = TaggedDBMap::reopen(
        &perpetual.objects.db,
        "hist_obj_e1",
        DB_PREFIX_HISTORIC_TOMBSTONES,
        &ReadWriteOptions::default(),
        true,
    )
    .unwrap();
    let mut batch = unreadable.batch();
    batch
        .insert_batch_tagged(&unreadable, [(older_tombstone, 7u64)])
        .unwrap();
    batch.write().unwrap();
    drop(historic);

    assert!(
        HistoricObjects::open(
            perpetual.objects.db.clone(),
            &default_db_options(),
            perpetual.objects.clone(),
        )
        .is_err()
    );
    assert!(perpetual.objects.get(&newer_tombstone).unwrap().is_some());
}

/// A tombstone at or below the bound and nothing at or below the bound are
/// different answers. The version-bounded scan keeps them apart, so a version
/// relocated under a tombstone is never served in the deleted object's place.
#[tokio::test]
async fn test_a_deleted_object_stays_deleted_across_the_buckets() {
    let (perpetual, historic, store, _dir) = test_store();
    let id = ObjectId::random();

    // Version 5 relocated into epoch 1's bucket, with the object deleted at
    // version 9 and its tombstone still in the live table.
    let bucket = historic.ensure(1).unwrap();
    let mut batch = perpetual.objects.batch();
    batch
        .insert_batch_tagged(
            &bucket.objects,
            [(ObjectKey(id, 5.into()), object_at(id, 5))],
        )
        .unwrap();
    batch
        .insert_batch(
            &perpetual.objects,
            [(
                ObjectKey(id, 9.into()),
                StoreObjectWrapper::from(StoreObject::Deleted),
            )],
        )
        .unwrap();
    batch.write().unwrap();

    // Bounded above the tombstone: the object is gone, and the relocated
    // version beneath it must not be served in its place.
    let (key, row) = perpetual
        .find_object_lt_or_eq_version(id, 12.into())
        .unwrap()
        .expect("the tombstone is in range");
    assert_eq!(key, ObjectKey(id, 9.into()));
    assert!(matches!(row.into_inner(), StoreObject::Deleted));
    assert_eq!(
        store
            .find_object_lt_or_eq_version_with_historic_fallback(id, 12.into())
            .unwrap(),
        None
    );

    // Bounded below the tombstone: the relocated version is the answer.
    assert!(
        perpetual
            .find_object_lt_or_eq_version(id, 6.into())
            .unwrap()
            .is_none()
    );
    assert_eq!(
        historic
            .find_lt_or_eq_version(id, 6.into())
            .unwrap()
            .map(|object| object.version()),
        Some(Version::from(5))
    );
    assert_eq!(
        store
            .find_object_lt_or_eq_version_with_historic_fallback(id, 6.into())
            .unwrap()
            .map(|object| object.version()),
        Some(Version::from(5))
    );
}

/// The bucket walk answers with the newest relocated version within the
/// bound, whichever bucket holds it, and the live table still answers for a
/// version that never left it.
#[tokio::test]
async fn test_the_newest_relocated_version_in_range_is_served() {
    let (perpetual, historic, store, _dir) = test_store();
    let id = ObjectId::random();

    // Versions 3 and 4 relocated in epoch 1, version 7 in epoch 2, version 11
    // still live.
    for (epoch, versions) in [(1, vec![3, 4]), (2, vec![7])] {
        let bucket = historic.ensure(epoch).unwrap();
        let mut batch = perpetual.objects.batch();
        batch
            .insert_batch_tagged(
                &bucket.objects,
                versions
                    .into_iter()
                    .map(|version| (ObjectKey(id, version.into()), object_at(id, version))),
            )
            .unwrap();
        batch.write().unwrap();
    }
    let live = object_at(id, 11);
    perpetual
        .objects
        .insert(&ObjectKey(id, 11.into()), &get_store_object(live, None))
        .unwrap();

    for (bound, expected) in [
        (11, Some(11)),
        (9, Some(7)),
        (7, Some(7)),
        (6, Some(4)),
        (3, Some(3)),
        (2, None),
    ] {
        assert_eq!(
            store
                .find_object_lt_or_eq_version_with_historic_fallback(id, bound.into())
                .unwrap()
                .map(|object| object.version()),
            expected.map(Version::from),
            "bound {bound}"
        );
    }

    // The walk on its own, without the live table in front of it.
    assert_eq!(
        historic
            .find_lt_or_eq_version(id, 9.into())
            .unwrap()
            .map(|object| object.version()),
        Some(Version::from(7))
    );
}

/// An object wrapped and later unwrapped keeps its tombstone in the live
/// table below its newer versions, and those versions relocate out from
/// between the two. The bounded read answers with the relocated version
/// above the tombstone rather than reading the tombstone as the object's
/// end.
#[tokio::test]
async fn test_a_tombstone_below_the_relocated_version_is_not_the_answer() {
    let (perpetual, historic, store, _dir) = test_store();
    let id = ObjectId::random();

    // Wrapped at version 2, unwrapped since, version 7 relocated, version 8
    // live.
    let bucket = historic.ensure(1).unwrap();
    let mut batch = perpetual.objects.batch();
    batch
        .insert_batch_tagged(
            &bucket.objects,
            [(ObjectKey(id, 7.into()), object_at(id, 7))],
        )
        .unwrap();
    batch
        .insert_batch(
            &perpetual.objects,
            [
                (
                    ObjectKey(id, 2.into()),
                    StoreObjectWrapper::from(StoreObject::Wrapped),
                ),
                (
                    ObjectKey(id, 8.into()),
                    get_store_object(object_at(id, 8), None),
                ),
            ],
        )
        .unwrap();
    batch.write().unwrap();

    for (bound, expected) in [(8, Some(8)), (7, Some(7)), (6, None), (2, None)] {
        assert_eq!(
            store
                .find_object_lt_or_eq_version_with_historic_fallback(id, bound.into())
                .unwrap()
                .map(|object| object.version()),
            expected.map(Version::from),
            "bound {bound}"
        );
    }
}

/// A bucket marked expiring is left out of the walk as it is left out of an
/// exact-key probe: its tombstone heads may already be gone from the live
/// table, and a version served from under a deleted tombstone would resurrect
/// a deleted object.
#[tokio::test]
async fn test_a_bucket_marked_expiring_is_left_out_of_the_walk() {
    let (perpetual, historic, _store, _dir) = test_store();
    let id = ObjectId::random();

    let bucket = historic.ensure(1).unwrap();
    let mut batch = perpetual.objects.batch();
    batch
        .insert_batch_tagged(
            &bucket.objects,
            [(ObjectKey(id, 5.into()), object_at(id, 5))],
        )
        .unwrap();
    batch.write().unwrap();
    assert!(
        historic
            .find_lt_or_eq_version(id, 6.into())
            .unwrap()
            .is_some()
    );

    bucket.mark_expiring().unwrap();
    assert!(
        historic
            .find_lt_or_eq_version(id, 6.into())
            .unwrap()
            .is_none()
    );
}
