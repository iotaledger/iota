// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{collections::HashSet, path::Path, sync::Arc, time::Duration};

use futures::FutureExt;
use iota_framework::BuiltInFramework;
use iota_move_build::BuildConfig;
use iota_sdk_types::{Address, ObjectId, Owner};
use iota_swarm_config::network_config_builder::ConfigBuilder;
use iota_types::{
    IOTA_FRAMEWORK_PACKAGE_ID,
    base_types::SequenceNumber,
    digests::TransactionDigest,
    object::Object,
    storage::{InputKey, MarkerValue, ObjectKey},
};
use tempfile::tempdir;
use tokio::time::timeout;

use super::{ObjectCacheRead, writeback_cache::WritebackCache};
use crate::authority::{AuthorityStore, authority_store_tables::AuthorityPerpetualTables};

async fn create_store() -> Arc<AuthorityStore> {
    let path = tempdir().unwrap();
    let tables = Arc::new(AuthorityPerpetualTables::open(path.path(), None));
    let config = ConfigBuilder::new_with_temp_dir().build();
    AuthorityStore::open_with_committee_for_testing(
        tables,
        config.committee_with_network().committee(),
        &config.genesis,
    )
    .await
    .unwrap()
}

async fn create_writeback_cache() -> Arc<WritebackCache> {
    Arc::new(WritebackCache::new_for_tests(create_store().await))
}

#[tokio::test]
async fn test_writeback_immediate_return_canceled_shared() {
    let cache = create_writeback_cache().await;
    let canceled_key = InputKey::VersionedObject {
        id: ObjectId::random(),
        version: SequenceNumber::CANCELLED_READ,
    };
    let receiving_keys = HashSet::new();
    let epoch = &0;

    cache
        .notify_read_input_objects(&[canceled_key], &receiving_keys, epoch)
        .now_or_never()
        .unwrap();

    let congested_key = InputKey::VersionedObject {
        id: ObjectId::random(),
        version: SequenceNumber::CONGESTED_PRIOR_TO_GAS_PRICE_FEEDBACK,
    };

    cache
        .notify_read_input_objects(&[congested_key], &receiving_keys, epoch)
        .now_or_never()
        .unwrap();

    let randomness_unavailable_key = InputKey::VersionedObject {
        id: ObjectId::random(),
        version: SequenceNumber::RANDOMNESS_UNAVAILABLE,
    };

    cache
        .notify_read_input_objects(&[randomness_unavailable_key], &receiving_keys, epoch)
        .now_or_never()
        .unwrap();
}

#[tokio::test]
async fn test_writeback_immediate_return_cached_object() {
    let cache = create_writeback_cache().await;
    let object_id = ObjectId::random();
    let version = SequenceNumber::from(1);
    let object = Object::with_id_owner_version_for_testing(object_id, version, Owner::Immutable);

    cache.write_object_for_testing(object);

    let input_keys = vec![InputKey::VersionedObject {
        id: object_id,
        version,
    }];
    let receiving_keys = HashSet::new();
    let epoch = &0;

    // Should return immediately since object is in cache/store
    cache
        .notify_read_input_objects(&input_keys, &receiving_keys, epoch)
        .now_or_never()
        .unwrap();
}

#[tokio::test]
async fn test_writeback_immediate_return_cached_package() {
    let cache = create_writeback_cache().await;
    let input_keys = vec![InputKey::Package {
        id: IOTA_FRAMEWORK_PACKAGE_ID,
    }];
    let receiving_keys = HashSet::new();
    let epoch = &0;

    // Should return immediately since system package is available by default.
    cache
        .notify_read_input_objects(&input_keys, &receiving_keys, epoch)
        .now_or_never()
        .unwrap();
}

#[tokio::test]
async fn test_writeback_immediate_return_shared_deleted() {
    let cache = create_writeback_cache().await;
    let object_id = ObjectId::random();
    let version = SequenceNumber::from(1);
    let epoch_id = 0;

    // Write a SharedDeleted marker
    cache.write_marker_for_testing(
        epoch_id,
        &ObjectKey(object_id, version),
        MarkerValue::SharedDeleted(TransactionDigest::random()),
    );

    let input_keys = vec![InputKey::VersionedObject {
        id: object_id,
        version,
    }];
    let receiving_keys = HashSet::new();
    let epoch = &epoch_id;

    // Should return immediately since the shared object was deleted
    cache
        .notify_read_input_objects(&input_keys, &receiving_keys, epoch)
        .now_or_never()
        .unwrap();
}

#[tokio::test]
async fn test_writeback_wait_for_object() {
    let cache = create_writeback_cache().await;
    let object_id = ObjectId::random();
    let version = SequenceNumber::from(1);

    let input_keys = vec![InputKey::VersionedObject {
        id: object_id,
        version,
    }];
    let receiving_keys = HashSet::new();
    let epoch = &0;

    let result = timeout(
        Duration::from_secs(3),
        cache.notify_read_input_objects(&input_keys, &receiving_keys, epoch),
    )
    .await;
    assert!(result.is_err());

    // Write an older version of the object - should NOT unblock.
    tokio::spawn({
        let cache = cache.clone();
        async move {
            tokio::time::sleep(Duration::from_millis(100)).await;
            let object = Object::with_id_owner_version_for_testing(
                object_id,
                SequenceNumber::from(0),
                Owner::Shared(version),
            );
            cache.write_object_for_testing(object);
        }
    });
    let result = timeout(
        Duration::from_secs(3),
        cache.notify_read_input_objects(&input_keys, &receiving_keys, epoch),
    )
    .await;
    assert!(result.is_err());

    // Write the correct version of the object.
    tokio::spawn({
        let cache = cache.clone();
        async move {
            tokio::time::sleep(Duration::from_millis(100)).await;
            let object = Object::with_id_owner_version_for_testing(
                object_id,
                version,
                Owner::Shared(version),
            );
            cache.write_object_for_testing(object);
        }
    });
    timeout(
        Duration::from_secs(3),
        cache.notify_read_input_objects(&input_keys, &receiving_keys, epoch),
    )
    .await
    .unwrap();
}

#[tokio::test]
async fn test_writeback_wait_for_package() {
    let cache = create_writeback_cache().await;
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/move/basics");
    let compiled_modules = BuildConfig::new_for_testing()
        .build(&path)
        .unwrap()
        .into_modules();
    let package = Object::new_package_for_testing(
        &compiled_modules,
        TransactionDigest::GENESIS_MARKER,
        BuiltInFramework::genesis_move_packages(),
    )
    .unwrap();
    let package_id = package.id();

    let input_keys = vec![InputKey::Package { id: package_id }];
    let receiving_keys = HashSet::new();
    let epoch = &0;

    // Start notification future
    let notification = cache.notify_read_input_objects(&input_keys, &receiving_keys, epoch);

    // Write package after small delay
    tokio::spawn({
        let cache = cache.clone();
        async move {
            tokio::time::sleep(Duration::from_millis(100)).await;
            cache.write_object_for_testing(package);
        }
    });

    // Should complete once package is written
    timeout(Duration::from_secs(1), notification).await.unwrap();
}

#[tokio::test]
async fn test_writeback_wait_for_shared_deleted() {
    let cache = create_writeback_cache().await;
    let object_id = ObjectId::random();
    let version = SequenceNumber::from(1);
    let epoch_id = 0;

    let input_keys = vec![InputKey::VersionedObject {
        id: object_id,
        version,
    }];
    let receiving_keys = HashSet::new();
    let epoch = &epoch_id;

    // Start notification future
    let notification = cache.notify_read_input_objects(&input_keys, &receiving_keys, epoch);

    // Write SharedDeleted marker after small delay
    tokio::spawn({
        let cache = cache.clone();
        async move {
            tokio::time::sleep(Duration::from_millis(100)).await;
            cache.write_marker_for_testing(
                epoch_id,
                &ObjectKey(object_id, version),
                MarkerValue::SharedDeleted(TransactionDigest::random()),
            );
        }
    });

    // Should complete once SharedDeleted marker is written
    timeout(Duration::from_secs(1), notification).await.unwrap();
}

#[tokio::test]
async fn test_writeback_receiving_object_higher_version() {
    let cache = create_writeback_cache().await;
    let object_id = ObjectId::random();
    let requested_version = SequenceNumber::from(1);
    let higher_version = SequenceNumber::from(2);
    let object = Object::with_id_owner_version_for_testing(
        object_id,
        higher_version,
        Owner::Address(Address::ZERO),
    );

    // Write higher version to cache
    cache.write_object_for_testing(object);

    let input_keys = vec![InputKey::VersionedObject {
        id: object_id,
        version: requested_version,
    }];
    let mut receiving_keys = HashSet::new();
    receiving_keys.insert(input_keys[0]);
    let epoch = &0;

    // Should return immediately since a higher version exists for receiving object
    cache
        .notify_read_input_objects(&input_keys, &receiving_keys, epoch)
        .now_or_never()
        .unwrap();
}

/// A received-then-deleted owned object can never be written at the awaited
/// version, so the scheduler's `notify_read_input_objects` wait must still
/// resolve for it — via the `OwnedDeleted` marker — rather than hang forever;
/// the transaction then proceeds to fail at execution. This pins the exact
/// closed hang-bug through the availability path the scheduler actually uses,
/// not just the standalone helper.
#[tokio::test]
async fn notify_read_resolves_received_then_deleted_owned_input() {
    let cache = create_writeback_cache().await;
    let object_id = ObjectId::random();
    let version = SequenceNumber::from(1);
    let epoch_id = 0;

    // The owned object was received and then deleted at `version`; only a marker
    // remains — the object itself is never written at that version.
    cache.write_marker_for_testing(
        epoch_id,
        &ObjectKey(object_id, version),
        MarkerValue::OwnedDeleted,
    );

    let input_key = InputKey::VersionedObject {
        id: object_id,
        version,
    };

    // As a receiving input, the deleted-owned marker makes it available.
    let mut receiving_keys = HashSet::new();
    receiving_keys.insert(input_key);
    assert_eq!(
        cache.multi_input_objects_available(&[input_key], &receiving_keys, &epoch_id),
        vec![true],
    );
    cache
        .notify_read_input_objects(&[input_key], &receiving_keys, &epoch_id)
        .now_or_never()
        .expect("received-then-deleted owned input must resolve, not hang");

    // Negative control: the same deleted-owned key that is NOT a receiving input
    // must stay unavailable — an `OwnedDeleted` marker only releases a *receiving*
    // input; a plain owned input at a deleted version keeps waiting.
    let no_receiving = HashSet::new();
    assert_eq!(
        cache.multi_input_objects_available(&[input_key], &no_receiving, &epoch_id),
        vec![false],
    );
    assert!(
        cache
            .notify_read_input_objects(&[input_key], &no_receiving, &epoch_id)
            .now_or_never()
            .is_none(),
        "a non-receiving deleted-owned input must not resolve"
    );
}

/// `multi_input_objects_available_cache_only` is the scheduler's fast-path
/// admission check and MUST consult only the in-memory cache: a store-backed
/// answer here could release a transaction before its input is durably
/// available. This pins that contract — an object present only in the backing
/// store reads as unavailable via `cache_only` but available via the full
/// marker-aware path (and its `notify_read` resolves) — plus the cancelled
/// sentinel short-circuit and a package absent from the cache.
#[tokio::test]
async fn cache_only_availability_ignores_store_but_full_path_falls_back() {
    let store = create_store().await;

    // An object written straight to the store, never through the cache.
    let store_only_id = ObjectId::random();
    let version = SequenceNumber::from(1);
    let store_only =
        Object::with_id_owner_version_for_testing(store_only_id, version, Owner::Immutable);
    store.bulk_insert_genesis_objects(&[store_only]).unwrap();

    let cache = Arc::new(WritebackCache::new_for_tests(store));
    let epoch = &0;
    let no_receiving = HashSet::new();
    let store_only_key = InputKey::VersionedObject {
        id: store_only_id,
        version,
    };

    // cache_only must NOT see the store-only object...
    assert_eq!(
        cache.multi_input_objects_available_cache_only(&[store_only_key]),
        vec![false],
    );
    // ...but the full marker-aware path finds it via the store fallback.
    assert_eq!(
        cache.multi_input_objects_available(&[store_only_key], &no_receiving, epoch),
        vec![true],
    );
    cache
        .notify_read_input_objects(&[store_only_key], &no_receiving, epoch)
        .now_or_never()
        .expect("store-backed input must resolve via the full path");

    // An object actually in the cache reads as available on the fast path.
    let cached_id = ObjectId::random();
    let cached = Object::with_id_owner_version_for_testing(cached_id, version, Owner::Immutable);
    cache.write_object_for_testing(cached);
    let cached_key = InputKey::VersionedObject {
        id: cached_id,
        version,
    };
    assert_eq!(
        cache.multi_input_objects_available_cache_only(&[cached_key]),
        vec![true],
    );

    // A cancelled sentinel version short-circuits to available.
    let cancelled_key = InputKey::VersionedObject {
        id: ObjectId::random(),
        version: SequenceNumber::CANCELLED_READ,
    };
    assert_eq!(
        cache.multi_input_objects_available_cache_only(&[cancelled_key]),
        vec![true],
    );

    // A package absent from the cache reads as unavailable on the fast path.
    let absent_package = InputKey::Package {
        id: ObjectId::random(),
    };
    assert_eq!(
        cache.multi_input_objects_available_cache_only(&[absent_package]),
        vec![false],
    );

    // Result alignment for a mixed key list.
    assert_eq!(
        cache.multi_input_objects_available_cache_only(&[
            cached_key,
            store_only_key,
            cancelled_key,
        ]),
        vec![true, false, true],
    );
}
