// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::sync::Arc;

use iota_sdk_types::ObjectId;
use iota_types::{
    committee::EpochId,
    storage::{MarkerValue, ObjectKey},
};
use tempfile::TempDir;
use typed_store::traits::Map;

use super::EpochMarkers;
use crate::authority::authority_store_tables::AuthorityPerpetualTables;

/// A perpetual store and the marker buckets over it. The directory is returned
/// so it outlives the database.
fn test_markers() -> (Arc<AuthorityPerpetualTables>, Arc<EpochMarkers>, TempDir) {
    let dir = iota_common::tempdir();
    let (perpetual, _historic, _historic_ledger, markers) =
        AuthorityPerpetualTables::open_with_historic_objects(dir.path(), None).unwrap();
    (Arc::new(perpetual), Arc::new(markers), dir)
}

fn write_marker(markers: &EpochMarkers, epoch: EpochId, id: ObjectId, version: u64) {
    markers
        .ensure(epoch)
        .unwrap()
        .markers
        .insert(&ObjectKey(id, version.into()), &MarkerValue::OwnedDeleted)
        .unwrap();
}

/// A marker is read back from the epoch that wrote it, and is invisible to
/// every other epoch: the bucket is the epoch, so no key carries one.
#[tokio::test]
async fn a_marker_is_read_back_from_the_epoch_that_wrote_it() {
    let (_perpetual, markers, _dir) = test_markers();
    let id = ObjectId::random();
    write_marker(&markers, 7, id, 3);

    assert_eq!(
        markers.get_marker_value(&id, &3u64.into(), 7).unwrap(),
        Some(MarkerValue::OwnedDeleted)
    );
    assert_eq!(
        markers.get_marker_value(&id, &3u64.into(), 6).unwrap(),
        None
    );
    assert_eq!(
        markers.get_marker_value(&id, &3u64.into(), 8).unwrap(),
        None
    );
}

/// The newest marked version of one object id wins, and a neighbouring id in
/// the same bucket does not answer for it.
#[tokio::test]
async fn the_latest_marker_is_the_newest_version_of_that_object() {
    let (_perpetual, markers, _dir) = test_markers();
    let id = ObjectId::random();
    let other = ObjectId::random();
    write_marker(&markers, 4, id, 1);
    write_marker(&markers, 4, id, 9);
    write_marker(&markers, 4, other, 42);

    let (version, marker) = markers.get_latest_marker(&id, 4).unwrap().unwrap();
    assert_eq!(version, 9u64);
    assert_eq!(marker, MarkerValue::OwnedDeleted);
    assert_eq!(markers.get_latest_marker(&id, 5).unwrap(), None);
}

/// Entering an epoch keeps that epoch's bucket and drops every earlier one,
/// so the markers of a finished epoch stop answering.
#[tokio::test]
async fn entering_an_epoch_drops_every_earlier_bucket() {
    let (_perpetual, markers, _dir) = test_markers();
    let id = ObjectId::random();
    write_marker(&markers, 1, id, 1);
    write_marker(&markers, 2, id, 2);

    markers.expire(3).unwrap();

    assert_eq!(
        markers.get_marker_value(&id, &1u64.into(), 1).unwrap(),
        None
    );
    assert_eq!(
        markers.get_marker_value(&id, &2u64.into(), 2).unwrap(),
        None
    );
    // The epoch being entered keeps its bucket, empty as it is.
    write_marker(&markers, 3, id, 3);
    assert_eq!(
        markers.get_marker_value(&id, &3u64.into(), 3).unwrap(),
        Some(MarkerValue::OwnedDeleted)
    );
}

/// The migration files the running epoch's flat rows in its bucket, deletes
/// the rows of epochs that are already over, and empties the flat table either
/// way.
#[tokio::test]
async fn the_migration_files_the_running_epoch_and_drops_the_rest() {
    let (perpetual, markers, _dir) = test_markers();
    let running = ObjectId::random();
    let over = ObjectId::random();
    let flat = &perpetual.object_per_epoch_marker_table;
    flat.insert(
        &(5, ObjectKey(running, 1u64.into())),
        &MarkerValue::OwnedDeleted,
    )
    .unwrap();
    flat.insert(
        &(4, ObjectKey(over, 1u64.into())),
        &MarkerValue::OwnedDeleted,
    )
    .unwrap();

    markers.migrate_flat_markers(flat, 5).unwrap();

    assert_eq!(
        markers.get_marker_value(&running, &1u64.into(), 5).unwrap(),
        Some(MarkerValue::OwnedDeleted)
    );
    assert_eq!(
        markers.get_marker_value(&over, &1u64.into(), 4).unwrap(),
        None
    );
    assert!(flat.safe_iter().next().is_none());
}

/// Running the migration again on a drained table is a no-op, so a start that
/// follows a finished one costs nothing.
#[tokio::test]
async fn the_migration_is_idempotent() {
    let (perpetual, markers, _dir) = test_markers();
    let id = ObjectId::random();
    let flat = &perpetual.object_per_epoch_marker_table;
    flat.insert(&(2, ObjectKey(id, 1u64.into())), &MarkerValue::OwnedDeleted)
        .unwrap();

    markers.migrate_flat_markers(flat, 2).unwrap();
    markers.migrate_flat_markers(flat, 2).unwrap();

    assert_eq!(
        markers.get_marker_value(&id, &1u64.into(), 2).unwrap(),
        Some(MarkerValue::OwnedDeleted)
    );
}
