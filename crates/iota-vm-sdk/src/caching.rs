// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Shared on-demand caching layer for the networked stores.
//!
//! [`GrpcStore`](crate::grpc::GrpcStore) and
//! [`GraphqlStore`](crate::graphql::GraphqlStore) differ only in how they fetch
//! objects from a node; the cache and the on-demand [`Store`] resolution are
//! identical and live here. A backend implements [`ObjectFetcher`] and wraps it
//! in a [`CachingStore`].
//!
//! The [`Store`] trait is synchronous, but the Move VM resolves objects on
//! demand mid-execution, so a cache miss blocks on the fetcher via
//! [`block_in_place`]; [`LocalVm::execute`](crate::LocalVm::execute) must run
//! inside a multi-threaded Tokio runtime.

use std::{
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
    inner: Arc<Mutex<InMemoryStore>>,
    fetcher: F,
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
            inner: Arc::new(Mutex::new(InMemoryStore::with_framework())),
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
        self.inner.lock().expect("store lock poisoned").clone()
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
            let inner = self.inner.lock().expect("store lock poisoned");
            if let Some(obj) = inner.get_object(id, version)? {
                return Ok(Some(obj));
            }
        }
        let fetched = self.fetch_blocking(&[(*id, version)])?;
        let mut inner = self.inner.lock().expect("store lock poisoned");
        let mut requested = None;
        for obj in fetched {
            if obj.id() == *id {
                requested = Some(obj.clone());
            }
            // The cache holds one version per id, so skip the insert when it
            // would replace a newer cached version with this older fetch. The
            // requested version is still returned from the fetch result below.
            let downgrades = inner
                .get_object(&obj.id(), None)?
                .is_some_and(|cached| cached.version() > obj.version());
            if !downgrades {
                inner.insert(obj);
            }
        }
        match version {
            Some(v) => Ok(requested.filter(|o| o.version() == v)),
            None => inner.get_object(id, None),
        }
    }

    fn get_child_object(
        &self,
        parent: &ObjectId,
        child: &ObjectId,
        version_upper_bound: Version,
    ) -> Result<Option<Object>, StoreError> {
        {
            let inner = self.inner.lock().expect("store lock poisoned");
            if let Some(obj) = inner.get_child_object(parent, child, version_upper_bound)? {
                return Ok(Some(obj));
            }
        }
        // Fetch the child at its latest version; the upper-bound check is
        // re-applied below once it is cached.
        let fetched = self.fetch_blocking(&[(*child, None)])?;
        let mut inner = self.inner.lock().expect("store lock poisoned");
        for obj in fetched {
            inner.insert(obj);
        }
        inner.get_child_object(parent, child, version_upper_bound)
    }

    fn insert(&mut self, object: Object) {
        self.inner
            .lock()
            .expect("store lock poisoned")
            .insert(object);
    }

    fn remove(&mut self, id: &ObjectId) {
        self.inner.lock().expect("store lock poisoned").remove(id);
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
