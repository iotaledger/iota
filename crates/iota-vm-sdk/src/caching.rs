// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Shared on-demand caching layer for the networked stores.
//!
//! [`GrpcStore`](crate::grpc::GrpcStore) and
//! [`GraphQLStore`](crate::graphql::GraphQLStore) differ only in how they fetch
//! objects from a node; the cache and the on-demand [`Store`] resolution are
//! identical and live here. A backend implements [`ObjectFetcher`] and wraps it
//! in a [`CachingStore`].
//!
//! The [`Store`] trait is synchronous, but the Move VM resolves objects on
//! demand mid-execution, so a cache miss blocks on the fetcher via
//! [`block_in_place`]; [`LocalVm::execute`](crate::LocalVm::execute) must run
//! inside a multi-threaded Tokio runtime.
//!
//! Removals are remembered: a removed object (e.g. deleted by an
//! `Execute`-mode commit) reads as absent instead of being re-fetched from the
//! node, until a new version is inserted.

use std::{
    collections::BTreeSet,
    future::Future,
    sync::{Arc, Mutex},
};

use iota_sdk_types::{ObjectId, Version};
use iota_types::object::Object;
use tokio::{
    runtime::{Handle, RuntimeFlavor},
    task::block_in_place,
};

use crate::{
    error::StoreError,
    store::{InMemoryStore, Store},
};

/// A node backend that fetches and decodes objects. Implementors provide only
/// the transport; [`CachingStore`] adds caching and the synchronous [`Store`]
/// surface.
pub(crate) trait ObjectFetcher {
    /// Fetch and decode objects, each at its given version or the latest when
    /// `None`. Does not cache.
    fn fetch_objects(
        &self,
        refs: &[(ObjectId, Option<Version>)],
    ) -> impl Future<Output = Result<Vec<Object>, StoreError>>;
}

/// An [`InMemoryStore`] cache fronting an [`ObjectFetcher`], resolving misses
/// on demand. Clones share the same cache and fetcher.
pub(crate) struct CachingStore<F> {
    inner: Arc<Mutex<CacheState>>,
    fetcher: F,
}

struct CacheState {
    objects: InMemoryStore,
    /// Ids removed via [`Store::remove`]; they read as absent without a fetch
    /// until re-inserted.
    removed: BTreeSet<ObjectId>,
}

impl<F: Clone> Clone for CachingStore<F> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
            fetcher: self.fetcher.clone(),
        }
    }
}

impl<F: ObjectFetcher> CachingStore<F> {
    /// Wrap `fetcher`, starting with the built-in framework packages loaded so
    /// Move calls resolve.
    pub(crate) fn new(fetcher: F) -> Self {
        Self {
            inner: Arc::new(Mutex::new(CacheState {
                objects: InMemoryStore::with_framework(),
                removed: BTreeSet::new(),
            })),
            fetcher,
        }
    }

    /// The wrapped fetcher, for backend-specific queries.
    pub(crate) fn fetcher(&self) -> &F {
        &self.fetcher
    }

    /// A snapshot clone of the objects cached so far (framework packages plus
    /// anything fetched on demand).
    pub(crate) fn store(&self) -> InMemoryStore {
        self.inner
            .lock()
            .expect("store lock poisoned")
            .objects
            .clone()
    }

    /// Fetch `refs` by blocking on the fetcher from within the executor.
    ///
    /// Bridges the synchronous [`Store`] surface to the async fetcher via
    /// [`block_in_place`], which requires a multi-threaded Tokio runtime. A
    /// missing or current-thread runtime is reported as a [`StoreError`] up
    /// front, rather than panicking deep inside the VM's object resolution.
    fn fetch_blocking(
        &self,
        refs: &[(ObjectId, Option<Version>)],
    ) -> Result<Vec<Object>, StoreError> {
        let handle = Handle::try_current().map_err(|e| {
            StoreError::new("on-demand fetch requires a multi-threaded Tokio runtime", e)
        })?;
        if !matches!(handle.runtime_flavor(), RuntimeFlavor::MultiThread) {
            return Err(StoreError::new(
                "on-demand fetch requires a multi-threaded Tokio runtime",
                "called from a current-thread runtime, where block_in_place is unavailable",
            ));
        }
        block_in_place(|| handle.block_on(self.fetcher.fetch_objects(refs)))
    }
}

impl<F: ObjectFetcher> Store for CachingStore<F> {
    fn get_object(
        &self,
        id: &ObjectId,
        version: Option<Version>,
    ) -> Result<Option<Object>, StoreError> {
        // Scope the read lock so it is released before the blocking fetch
        // re-acquires it (a std `Mutex` is not reentrant).
        {
            let state = self.inner.lock().expect("store lock poisoned");
            if state.removed.contains(id) {
                return Ok(None);
            }
            if let Some(obj) = state.objects.get_object(id, version)? {
                return Ok(Some(obj));
            }
        }
        let fetched = self.fetch_blocking(&[(*id, version)])?;
        let mut state = self.inner.lock().expect("store lock poisoned");
        let mut requested = None;
        for obj in fetched {
            if obj.id() == *id {
                requested = Some(obj.clone());
            }
            // Only an unpinned fetch returns the node's latest version; a
            // pinned fetch may be older and must not become the cached entry
            // (which `get_object(id, None)` reports as latest). The fetch runs
            // outside the lock, so honor a newer version or a removal another
            // handle committed in the meantime.
            let downgrades = state
                .objects
                .get_object(&obj.id(), None)?
                .is_some_and(|cached| cached.version() > obj.version());
            if version.is_none() && !downgrades && !state.removed.contains(&obj.id()) {
                state.objects.insert(obj);
            }
        }
        Ok(requested.filter(|o| version.is_none_or(|v| o.version() == v)))
    }

    fn get_child_object(
        &self,
        parent: &ObjectId,
        child: &ObjectId,
        version_upper_bound: Version,
    ) -> Result<Option<Object>, StoreError> {
        {
            let state = self.inner.lock().expect("store lock poisoned");
            if state.removed.contains(child) {
                return Ok(None);
            }
            if let Some(obj) = state
                .objects
                .get_child_object(parent, child, version_upper_bound)?
            {
                return Ok(Some(obj));
            }
        }
        // Fetch the child at its latest version; the upper-bound check is
        // re-applied below once it is cached.
        let fetched = self.fetch_blocking(&[(*child, None)])?;
        let mut state = self.inner.lock().expect("store lock poisoned");
        for obj in fetched {
            // The cache holds one version per id, so keep a newer cached
            // version (e.g. committed by an `Execute` run) over the node's
            // older latest, and honor a removal another handle committed while
            // the fetch was in flight.
            let downgrades = state
                .objects
                .get_object(&obj.id(), None)?
                .is_some_and(|cached| cached.version() > obj.version());
            if !downgrades && !state.removed.contains(&obj.id()) {
                state.objects.insert(obj);
            }
        }
        state
            .objects
            .get_child_object(parent, child, version_upper_bound)
    }

    fn insert(&mut self, object: Object) {
        let mut state = self.inner.lock().expect("store lock poisoned");
        state.removed.remove(&object.id());
        state.objects.insert(object);
    }

    fn remove(&mut self, id: &ObjectId) {
        let mut state = self.inner.lock().expect("store lock poisoned");
        state.objects.remove(id);
        state.removed.insert(*id);
    }
}

#[cfg(test)]
mod tests {
    use iota_sdk_types::{ObjectId, Owner, Version};
    use iota_types::{
        digests::TransactionDigest,
        object::{MoveObject, MoveObjectExt, Object},
    };

    use super::{CachingStore, ObjectFetcher};
    use crate::{error::StoreError, store::Store};

    /// The newest version the fetcher reports for a `None` (latest) request.
    const LATEST: u64 = 8;

    /// A fetcher that serves any object at exactly the requested version,
    /// answering a `None` (latest) request with [`LATEST`].
    #[derive(Clone)]
    struct VersionedFetcher;

    impl ObjectFetcher for VersionedFetcher {
        async fn fetch_objects(
            &self,
            refs: &[(ObjectId, Option<Version>)],
        ) -> Result<Vec<Object>, StoreError> {
            Ok(refs
                .iter()
                .map(|(id, version)| coin(*id, version.unwrap_or_else(|| Version::from(LATEST))))
                .collect())
        }
    }

    fn coin(id: ObjectId, version: Version) -> Object {
        // Owner is irrelevant for `get_object`; only the version matters here.
        Object::new_move(
            MoveObject::new_gas_coin(version, id, 1),
            Owner::Object(ObjectId::random()),
            TransactionDigest::ZERO,
        )
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn pinned_fetch_on_cold_cache_does_not_become_latest() {
        let id = ObjectId::random();
        let store = CachingStore::new(VersionedFetcher);

        // First access is version-pinned to an old version.
        let pinned = store
            .get_object(&id, Some(Version::from(5)))
            .unwrap()
            .expect("object present at v5");
        assert_eq!(pinned.version(), Version::from(5));

        // A latest read must fetch the node's latest (v8), not report the
        // pinned v5 from the cache.
        let latest = store
            .get_object(&id, None)
            .unwrap()
            .expect("object present");
        assert_eq!(
            latest.version(),
            Version::from(LATEST),
            "a version-pinned fetch must not be served as the latest version"
        );
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn removed_objects_read_absent_until_reinserted() {
        let id = ObjectId::random();
        let mut store = CachingStore::new(VersionedFetcher);

        // Cache the object at latest, then remove it (as an `Execute`-mode
        // deletion commit does).
        assert!(store.get_object(&id, None).unwrap().is_some());
        store.remove(&id);

        // The fetcher still serves the object, but the removal must win: the
        // object reads as absent for plain, pinned, and child lookups.
        assert!(store.get_object(&id, None).unwrap().is_none());
        assert!(
            store
                .get_object(&id, Some(Version::from(LATEST)))
                .unwrap()
                .is_none()
        );
        assert!(
            store
                .get_child_object(&ObjectId::random(), &id, Version::from(u64::MAX))
                .unwrap()
                .is_none()
        );

        // Re-inserting makes the object visible again.
        store.insert(coin(id, Version::from(9)));
        let after = store.get_object(&id, None).unwrap().expect("re-inserted");
        assert_eq!(after.version(), Version::from(9));
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn child_fetch_keeps_newer_cached_version() {
        let parent = ObjectId::random();
        let child_id = ObjectId::random();
        let mut store = CachingStore::new(VersionedFetcher);

        // A locally committed child at v10, newer than the node's latest (v8).
        let child = Object::new_move(
            MoveObject::new_gas_coin(Version::from(10), child_id, 1),
            Owner::Object(parent),
            TransactionDigest::ZERO,
        );
        store.insert(child);

        // The bound excludes v10, and the node's v8 must not clobber it.
        assert!(
            store
                .get_child_object(&parent, &child_id, Version::from(9))
                .unwrap()
                .is_none()
        );
        let cached = store
            .get_object(&child_id, None)
            .unwrap()
            .expect("child present");
        assert_eq!(
            cached.version(),
            Version::from(10),
            "a child fetch must not replace a newer cached version with the node's older latest"
        );
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn pinned_older_fetch_does_not_evict_newer_cached_version() {
        let id = ObjectId::random();
        let store = CachingStore::new(VersionedFetcher);

        // Resolve the object at latest (v8) into the cache.
        let latest = store
            .get_object(&id, None)
            .unwrap()
            .expect("object present");
        assert_eq!(latest.version(), Version::from(LATEST));

        // A version-pinned read of an older version still returns that version,
        let pinned = store
            .get_object(&id, Some(Version::from(5)))
            .unwrap()
            .expect("object present at v5");
        assert_eq!(pinned.version(), Version::from(5));

        // but must not have evicted the newer cached version: a later latest
        // read still returns v8, not the older v5 just fetched.
        let after = store
            .get_object(&id, None)
            .unwrap()
            .expect("object present");
        assert_eq!(
            after.version(),
            Version::from(LATEST),
            "an older version-pinned fetch must not downgrade the cached object"
        );
    }
}
