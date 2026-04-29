// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{collections::HashSet, path::Path, sync::Arc, time::Duration};

use futures::FutureExt;
use iota_framework::BuiltInFramework;
use iota_move_build::BuildConfig;
use iota_swarm_config::network_config_builder::ConfigBuilder;
use iota_types::{
    IOTA_FRAMEWORK_PACKAGE_ID,
    base_types::{IotaAddress, ObjectID, SequenceNumber},
    digests::TransactionDigest,
    object::{Object, Owner},
    storage::{InputKey, MarkerValue, ObjectKey},
};
use tempfile::tempdir;
use tokio::time::timeout;

use super::{
    ObjectCacheRead, passthrough_cache::PassthroughCache, writeback_cache::WritebackCache,
};
use crate::authority::{AuthorityStore, authority_store_tables::AuthorityPerpetualTables};

/// Trait abstracting cache operations needed for notify_read_input_objects
/// tests, so the same test logic can run against both WritebackCache and
/// PassthroughCache.
trait NotifyReadTestCache: ObjectCacheRead + Send + Sync + 'static {
    fn write_object_for_testing(&self, object: Object);
    fn write_marker_for_testing(
        &self,
        epoch_id: u64,
        object_key: &ObjectKey,
        marker_value: MarkerValue,
    );
}

impl NotifyReadTestCache for WritebackCache {
    fn write_object_for_testing(&self, object: Object) {
        WritebackCache::write_object_for_testing(self, object);
    }

    fn write_marker_for_testing(
        &self,
        epoch_id: u64,
        object_key: &ObjectKey,
        marker_value: MarkerValue,
    ) {
        WritebackCache::write_marker_for_testing(self, epoch_id, object_key, marker_value);
    }
}

impl NotifyReadTestCache for PassthroughCache {
    fn write_object_for_testing(&self, object: Object) {
        PassthroughCache::write_object_for_testing(self, object);
    }

    fn write_marker_for_testing(
        &self,
        epoch_id: u64,
        object_key: &ObjectKey,
        marker_value: MarkerValue,
    ) {
        PassthroughCache::write_marker_for_testing(self, epoch_id, object_key, marker_value);
    }
}

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

async fn create_passthrough_cache() -> Arc<PassthroughCache> {
    let store = create_store().await;
    let registry = prometheus::Registry::new();
    Arc::new(PassthroughCache::new_for_tests(store, &registry))
}

// ---------------------------------------------------------------------------
// Generic test functions
// ---------------------------------------------------------------------------

async fn test_immediate_return_canceled_shared_impl(cache: &impl NotifyReadTestCache) {
    let canceled_key = InputKey::VersionedObject {
        id: ObjectID::random(),
        version: SequenceNumber::CANCELLED_READ,
    };
    let receiving_keys = HashSet::new();
    let epoch = &0;

    let result = cache
        .notify_read_input_objects(&[canceled_key], &receiving_keys, epoch)
        .now_or_never()
        .unwrap();
    assert_eq!(result.len(), 1);

    let congested_key = InputKey::VersionedObject {
        id: ObjectID::random(),
        version: SequenceNumber::CONGESTED_PRIOR_TO_GAS_PRICE_FEEDBACK,
    };

    let result = cache
        .notify_read_input_objects(&[congested_key], &receiving_keys, epoch)
        .now_or_never()
        .unwrap();
    assert_eq!(result.len(), 1);

    let randomness_unavailable_key = InputKey::VersionedObject {
        id: ObjectID::random(),
        version: SequenceNumber::RANDOMNESS_UNAVAILABLE,
    };

    let result = cache
        .notify_read_input_objects(&[randomness_unavailable_key], &receiving_keys, epoch)
        .now_or_never()
        .unwrap();
    assert_eq!(result.len(), 1);
}

async fn test_immediate_return_cached_object_impl(cache: &impl NotifyReadTestCache) {
    let object_id = ObjectID::random();
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
    let result = cache
        .notify_read_input_objects(&input_keys, &receiving_keys, epoch)
        .now_or_never()
        .unwrap();

    assert_eq!(result.len(), 1);
}

async fn test_immediate_return_cached_package_impl(cache: &impl NotifyReadTestCache) {
    let input_keys = vec![InputKey::Package {
        id: IOTA_FRAMEWORK_PACKAGE_ID,
    }];
    let receiving_keys = HashSet::new();
    let epoch = &0;

    // Should return immediately since system package is available by default.
    let result = cache
        .notify_read_input_objects(&input_keys, &receiving_keys, epoch)
        .now_or_never()
        .unwrap();

    assert_eq!(result.len(), 1);
}

async fn test_immediate_return_shared_deleted_impl(cache: &impl NotifyReadTestCache) {
    let object_id = ObjectID::random();
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
    let result = cache
        .notify_read_input_objects(&input_keys, &receiving_keys, epoch)
        .now_or_never()
        .unwrap();

    assert_eq!(result.len(), 1);
}

async fn test_wait_for_object_impl<C: NotifyReadTestCache>(cache: Arc<C>) {
    let object_id = ObjectID::random();
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
                Owner::Shared {
                    initial_shared_version: version,
                },
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
                Owner::Shared {
                    initial_shared_version: version,
                },
            );
            cache.write_object_for_testing(object);
        }
    });
    let result = timeout(
        Duration::from_secs(3),
        cache.notify_read_input_objects(&input_keys, &receiving_keys, epoch),
    )
    .await
    .unwrap();
    assert_eq!(result.len(), 1);
}

async fn test_wait_for_package_impl<C: NotifyReadTestCache>(cache: Arc<C>) {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/move/basics");
    let compiled_modules = BuildConfig::new_for_testing()
        .build(&path)
        .unwrap()
        .into_modules();
    let package = Object::new_package_for_testing(
        &compiled_modules,
        TransactionDigest::genesis_marker(),
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
    let result = timeout(Duration::from_secs(1), notification).await.unwrap();

    assert_eq!(result.len(), 1);
}

async fn test_wait_for_shared_deleted_impl<C: NotifyReadTestCache>(cache: Arc<C>) {
    let object_id = ObjectID::random();
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
    let result = timeout(Duration::from_secs(1), notification).await.unwrap();

    assert_eq!(result.len(), 1);
}

async fn test_receiving_object_higher_version_impl(cache: &impl NotifyReadTestCache) {
    let object_id = ObjectID::random();
    let requested_version = SequenceNumber::from(1);
    let higher_version = SequenceNumber::from(2);
    let object = Object::with_id_owner_version_for_testing(
        object_id,
        higher_version,
        Owner::AddressOwner(IotaAddress::default()),
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
    let result = cache
        .notify_read_input_objects(&input_keys, &receiving_keys, epoch)
        .now_or_never()
        .unwrap();

    assert_eq!(result.len(), 1);
}

// ---------------------------------------------------------------------------
// WritebackCache tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_writeback_immediate_return_canceled_shared() {
    let cache = create_writeback_cache().await;
    test_immediate_return_canceled_shared_impl(&*cache).await;
}

#[tokio::test]
async fn test_writeback_immediate_return_cached_object() {
    let cache = create_writeback_cache().await;
    test_immediate_return_cached_object_impl(&*cache).await;
}

#[tokio::test]
async fn test_writeback_immediate_return_cached_package() {
    let cache = create_writeback_cache().await;
    test_immediate_return_cached_package_impl(&*cache).await;
}

#[tokio::test]
async fn test_writeback_immediate_return_shared_deleted() {
    let cache = create_writeback_cache().await;
    test_immediate_return_shared_deleted_impl(&*cache).await;
}

#[tokio::test]
async fn test_writeback_wait_for_object() {
    let cache = create_writeback_cache().await;
    test_wait_for_object_impl(cache).await;
}

#[tokio::test]
async fn test_writeback_wait_for_package() {
    let cache = create_writeback_cache().await;
    test_wait_for_package_impl(cache).await;
}

#[tokio::test]
async fn test_writeback_wait_for_shared_deleted() {
    let cache = create_writeback_cache().await;
    test_wait_for_shared_deleted_impl(cache).await;
}

#[tokio::test]
async fn test_writeback_receiving_object_higher_version() {
    let cache = create_writeback_cache().await;
    test_receiving_object_higher_version_impl(&*cache).await;
}

// ---------------------------------------------------------------------------
// PassthroughCache tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_passthrough_immediate_return_canceled_shared() {
    let cache = create_passthrough_cache().await;
    test_immediate_return_canceled_shared_impl(&*cache).await;
}

#[tokio::test]
async fn test_passthrough_immediate_return_cached_object() {
    let cache = create_passthrough_cache().await;
    test_immediate_return_cached_object_impl(&*cache).await;
}

#[tokio::test]
async fn test_passthrough_immediate_return_cached_package() {
    let cache = create_passthrough_cache().await;
    test_immediate_return_cached_package_impl(&*cache).await;
}

#[tokio::test]
async fn test_passthrough_immediate_return_shared_deleted() {
    let cache = create_passthrough_cache().await;
    test_immediate_return_shared_deleted_impl(&*cache).await;
}

#[tokio::test]
async fn test_passthrough_wait_for_object() {
    let cache = create_passthrough_cache().await;
    test_wait_for_object_impl(cache).await;
}

#[tokio::test]
async fn test_passthrough_wait_for_package() {
    let cache = create_passthrough_cache().await;
    test_wait_for_package_impl(cache).await;
}

#[tokio::test]
async fn test_passthrough_wait_for_shared_deleted() {
    let cache = create_passthrough_cache().await;
    test_wait_for_shared_deleted_impl(cache).await;
}

#[tokio::test]
async fn test_passthrough_receiving_object_higher_version() {
    let cache = create_passthrough_cache().await;
    test_receiving_object_higher_version_impl(&*cache).await;
}
