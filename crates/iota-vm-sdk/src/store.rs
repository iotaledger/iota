// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Object storage surface for the local VM.
//!
//! The public [`Store`] trait is a four-method surface —
//! `get_object` / `get_child_object` / `insert` / `remove`.
//!
//! [`InMemoryStore`] is the default, fully-offline implementation. The
//! networked stores ([`crate::grpc`] / [`crate::graphql`]) pre-fetch into an
//! `InMemoryStore` and hand the populated store to the VM, so the trait stays
//! synchronous everywhere.

use std::collections::BTreeMap;

use iota_framework::BuiltInFramework;
use iota_sdk_types::{ObjectId, Version};
use iota_types::{
    base_types::{SequenceNumber, VersionNumber},
    committee::EpochId,
    error::IotaResult,
    object::Object,
    storage::{
        BackingPackageStore, ChildObjectResolver, ObjectStore, PackageObject,
        error::Error as StorageError,
    },
};

/// A synchronous object store the local VM reads from and (on
/// [`ExecutionMode::Execute`](crate::ExecutionMode::Execute)) writes back to.
///
/// Implementors only need to provide these four methods; the SDK adapts them
/// to the storage traits the Move execution engine requires.
pub trait Store {
    /// Look up an object by ID. When `version` is `Some`, return the object
    /// only if it is at exactly that version; when `None`, return whatever
    /// version is held.
    fn get_object(&self, id: &ObjectId, version: Option<Version>) -> Option<Object>;

    /// Resolve a dynamic-field child object owned by `parent`, returning the
    /// child only if its version is `<= version_upper_bound`.
    fn get_child_object(
        &self,
        parent: &ObjectId,
        child: &ObjectId,
        version_upper_bound: Version,
    ) -> Option<Object>;

    /// Insert (or overwrite) an object.
    fn insert(&mut self, object: Object);

    /// Remove an object by ID.
    fn remove(&mut self, id: &ObjectId);
}

/// A simple in-memory [`Store`] backed by a `BTreeMap`.
///
/// All objects must be provided upfront via [`Self::insert`] / [`Self::extend`]
/// (or pre-seeded with [`Self::with_framework`]). No network access is
/// performed.
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
    /// ([`BuiltInFramework::genesis_objects`]). This is the standard starting
    /// point for offline execution — any non-trivial Move call needs the
    /// framework on hand.
    pub fn with_framework() -> Self {
        let mut store = Self::new();
        store.extend(BuiltInFramework::genesis_objects());
        store
    }

    /// Insert every object from `objects` into the store.
    pub fn extend<I: IntoIterator<Item = Object>>(&mut self, objects: I) {
        for obj in objects {
            self.insert(obj);
        }
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
    fn get_object(&self, id: &ObjectId, version: Option<Version>) -> Option<Object> {
        let obj = self.objects.get(id)?;
        match version {
            Some(v) if obj.version() != v => None,
            _ => Some(obj.clone()),
        }
    }

    fn get_child_object(
        &self,
        _parent: &ObjectId,
        child: &ObjectId,
        version_upper_bound: Version,
    ) -> Option<Object> {
        self.objects
            .get(child)
            .filter(|o| o.version() <= version_upper_bound)
            .cloned()
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
/// [`BackingPackageStore`], [`ChildObjectResolver`]); this thin wrapper
/// implements all three in terms of the four public [`Store`] methods.
/// `BackingStore` is then granted by its blanket impl.
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
        Ok(self.inner.get_object(object_id, None))
    }

    fn try_get_object_by_key(
        &self,
        object_id: &ObjectId,
        version: VersionNumber,
    ) -> Result<Option<Object>, StorageError> {
        Ok(self.inner.get_object(object_id, Some(version)))
    }
}

impl BackingPackageStore for StoreBackend<'_> {
    fn get_package_object(&self, package_id: &ObjectId) -> IotaResult<Option<PackageObject>> {
        Ok(self
            .inner
            .get_object(package_id, None)
            .map(PackageObject::new))
    }
}

impl ChildObjectResolver for StoreBackend<'_> {
    fn read_child_object(
        &self,
        parent: &ObjectId,
        child: &ObjectId,
        child_version_upper_bound: SequenceNumber,
    ) -> IotaResult<Option<Object>> {
        Ok(self
            .inner
            .get_child_object(parent, child, child_version_upper_bound))
    }

    fn get_object_received_at_version(
        &self,
        _owner: &ObjectId,
        receiving_object_id: &ObjectId,
        receive_object_at_version: SequenceNumber,
        _epoch_id: EpochId,
    ) -> IotaResult<Option<Object>> {
        Ok(self
            .inner
            .get_object(receiving_object_id, Some(receive_object_at_version)))
    }
}
