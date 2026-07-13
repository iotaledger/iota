// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Object storage surface for the local VM.
//!
//! [`InMemoryStore`] is the default, fully-offline [`Store`] implementation.
//! The networked stores (`GrpcStore` / `GraphQLStore`) front an
//! `InMemoryStore` cache and resolve cache misses on demand, blocking on the
//! node, so the trait stays synchronous everywhere.

use std::collections::BTreeMap;

use iota_framework::BuiltInFramework;
use iota_sdk_types::{ObjectId, Version};
use iota_types::{
    base_types::VersionNumber,
    committee::EpochId,
    error::{IotaError, IotaResult},
    object::Object,
    storage::{
        BackingPackageStore, ChildObjectResolver, ObjectStore, PackageObject,
        error::Error as StorageError,
    },
};

use crate::error::StoreError;

/// A synchronous object store the local VM reads from and (on
/// [`ExecutionMode::Execute`](crate::ExecutionMode::Execute)) writes back to.
///
/// Implementors only need to provide these four methods; the SDK adapts them
/// to the storage traits the Move execution engine requires.
pub trait Store {
    /// Look up an object by ID. When `version` is `Some`, return the object
    /// only if it is at exactly that version; when `None`, return whatever
    /// version is held. `Ok(None)` means the object is absent; an `Err` means
    /// the lookup itself failed (e.g. a networked store's fetch).
    fn get_object(
        &self,
        id: &ObjectId,
        version: Option<Version>,
    ) -> Result<Option<Object>, StoreError>;

    /// Resolve a dynamic-field child object owned by `parent`, returning the
    /// child only if its version is `<= version_upper_bound`.
    ///
    /// The networked stores resolve a miss by fetching the child's *latest*
    /// version and applying this bound as a filter, so a child newer than the
    /// bound (historical replay against a pinned older `parent`, or a subtree
    /// under active mutation) reads as absent rather than at an older version.
    fn get_child_object(
        &self,
        parent: &ObjectId,
        child: &ObjectId,
        version_upper_bound: Version,
    ) -> Result<Option<Object>, StoreError>;

    /// Insert (or overwrite) an object.
    fn insert(&mut self, object: Object);

    /// Remove an object by ID.
    fn remove(&mut self, id: &ObjectId);
}

/// A simple in-memory [`Store`] backed by a `BTreeMap`.
///
/// All objects must be provided upfront via [`Self::insert`] (or pre-seeded
/// with [`Self::with_framework`]). No network access is performed.
#[derive(Clone, Default)]
pub struct InMemoryStore {
    objects: BTreeMap<ObjectId, Object>,
}

impl InMemoryStore {
    /// An empty store.
    pub fn new() -> Self {
        Self::default()
    }

    /// A store pre-seeded with every built-in framework package object
    /// ([`BuiltInFramework::genesis_objects`]). The standard starting point for
    /// offline execution, since any non-trivial Move call needs the framework.
    pub fn with_framework() -> Self {
        let mut store = Self::new();
        for obj in BuiltInFramework::genesis_objects() {
            store.insert(obj);
        }
        store
    }

    /// Iterate over all `(id, object)` pairs currently held.
    pub fn iter(&self) -> impl Iterator<Item = (&ObjectId, &Object)> {
        self.objects.iter()
    }

    /// Number of objects held.
    pub fn len(&self) -> usize {
        self.objects.len()
    }

    /// Whether the store holds no objects.
    pub fn is_empty(&self) -> bool {
        self.objects.is_empty()
    }
}

impl Store for InMemoryStore {
    fn get_object(
        &self,
        id: &ObjectId,
        version: Option<Version>,
    ) -> Result<Option<Object>, StoreError> {
        Ok(self
            .objects
            .get(id)
            .filter(|obj| version.is_none_or(|v| obj.version() == v))
            .cloned())
    }

    fn get_child_object(
        &self,
        parent: &ObjectId,
        child: &ObjectId,
        version_upper_bound: Version,
    ) -> Result<Option<Object>, StoreError> {
        // This store keeps one version per id; the bound applies to that
        // version.
        Ok(self
            .objects
            .get(child)
            .filter(|o| o.version() <= version_upper_bound)
            .filter(|o| o.owner == iota_sdk_types::Owner::Object(*parent))
            .cloned())
    }

    fn insert(&mut self, object: Object) {
        self.objects.insert(object.id(), object);
    }

    fn remove(&mut self, id: &ObjectId) {
        self.objects.remove(id);
    }
}

/// Adapts any `&dyn Store` into the [`BackingStore`] the Move execution engine
/// requires.
///
/// The engine reads objects through three traits ([`ObjectStore`],
/// [`BackingPackageStore`], [`ChildObjectResolver`]); this wrapper implements
/// all three in terms of the four public [`Store`] methods, and `BackingStore`
/// follows from its blanket impl.
pub(crate) struct StoreBackend<'a> {
    inner: &'a dyn Store,
}

impl<'a> StoreBackend<'a> {
    pub(crate) fn new(inner: &'a dyn Store) -> Self {
        Self { inner }
    }
}

impl ObjectStore for StoreBackend<'_> {
    fn try_get_object(&self, object_id: &ObjectId) -> Result<Option<Object>, StorageError> {
        self.inner
            .get_object(object_id, None)
            .map_err(StorageError::custom)
    }

    fn try_get_object_by_key(
        &self,
        object_id: &ObjectId,
        version: VersionNumber,
    ) -> Result<Option<Object>, StorageError> {
        self.inner
            .get_object(object_id, Some(version))
            .map_err(StorageError::custom)
    }
}

impl BackingPackageStore for StoreBackend<'_> {
    fn get_package_object(&self, package_id: &ObjectId) -> IotaResult<Option<PackageObject>> {
        // Rejects a non-package object at `package_id` with a typed
        // `IotaError::BadObjectType`, as the node does.
        iota_types::storage::load_package_object_from_object_store(self, package_id)
    }
}

impl ChildObjectResolver for StoreBackend<'_> {
    fn read_child_object(
        &self,
        parent: &ObjectId,
        child: &ObjectId,
        child_version_upper_bound: Version,
    ) -> IotaResult<Option<Object>> {
        self.inner
            .get_child_object(parent, child, child_version_upper_bound)
            .map_err(|e| IotaError::Storage(e.to_string()))
    }

    fn get_object_received_at_version(
        &self,
        owner: &ObjectId,
        receiving_object_id: &ObjectId,
        receive_object_at_version: Version,
        _epoch_id: EpochId,
    ) -> IotaResult<Option<Object>> {
        // Resolve the store's *current* version and require it to equal the
        // declared one — never a pinned lookup, which a networked store could
        // satisfy from historical chain state. The current-version check
        // stands in for the node's received-object marker: once a receive
        // bumps the object past the declared version, that version reads as
        // absent and the receive aborts like it does on-chain. The object must
        // also be address-owned by `owner`; anything else reads as absent too.
        Ok(self
            .inner
            .get_object(receiving_object_id, None)
            .map_err(|e| IotaError::Storage(e.to_string()))?
            .filter(|obj| {
                obj.version() == receive_object_at_version
                    && obj.owner == iota_sdk_types::Owner::Address((*owner).into())
            }))
    }
}

#[cfg(test)]
mod tests {
    use iota_sdk_types::{ObjectId, Owner, Version};
    use iota_types::{
        digests::TransactionDigest,
        object::{MoveObject, MoveObjectExt, Object},
        storage::ChildObjectResolver,
    };

    use super::{InMemoryStore, Store, StoreBackend};

    #[test]
    fn get_child_object_requires_parent_ownership() {
        let parent = ObjectId::random();
        let stranger = ObjectId::random();
        let child = Object::new_move(
            MoveObject::new_gas_coin(Version::from(3), ObjectId::random(), 1),
            Owner::Object(parent),
            TransactionDigest::ZERO,
        );
        let child_id = child.id();

        let mut store = InMemoryStore::new();
        store.insert(child);

        let high = Version::from(10);
        assert!(
            store
                .get_child_object(&parent, &child_id, high)
                .unwrap()
                .is_some()
        );
        // A different parent must not be able to read the child.
        assert!(
            store
                .get_child_object(&stranger, &child_id, high)
                .unwrap()
                .is_none()
        );
        // A version bound below the child's version hides it.
        let low = Version::from(2);
        assert!(
            store
                .get_child_object(&parent, &child_id, low)
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn receiving_resolves_only_the_current_version_for_the_owner() {
        let parent = ObjectId::random();
        let stranger = ObjectId::random();
        let current = Version::from(5);
        let child = Object::new_move(
            MoveObject::new_gas_coin(current, ObjectId::random(), 1),
            Owner::Address(parent.into()),
            TransactionDigest::ZERO,
        );
        let child_id = child.id();

        let mut store = InMemoryStore::new();
        store.insert(child);
        let backend = StoreBackend::new(&store);

        // The current version, address-owned by the parent, is receivable.
        assert!(
            backend
                .get_object_received_at_version(&parent, &child_id, current, 0)
                .unwrap()
                .is_some()
        );
        // An older declared version reads as absent: the object was received
        // past it, so the receive must abort like it does on-chain.
        assert!(
            backend
                .get_object_received_at_version(&parent, &child_id, Version::from(4), 0)
                .unwrap()
                .is_none()
        );
        // A parent that does not own the object cannot receive it.
        assert!(
            backend
                .get_object_received_at_version(&stranger, &child_id, current, 0)
                .unwrap()
                .is_none()
        );
    }
}
