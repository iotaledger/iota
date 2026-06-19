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
use tokio::{runtime::Handle, task::block_in_place};

use crate::{
    error::VmSdkError,
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
    ) -> impl Future<Output = Result<Vec<Object>, VmSdkError>>;
}

/// An [`InMemoryStore`] cache fronting an [`ObjectFetcher`], resolving misses
/// on demand. Clones share the same cache and fetcher.
pub(crate) struct CachingStore<F> {
    inner: Arc<Mutex<InMemoryStore>>,
    fetcher: F,
    last_fetch_error: Arc<Mutex<Option<String>>>,
}

impl<F: Clone> Clone for CachingStore<F> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
            fetcher: self.fetcher.clone(),
            last_fetch_error: self.last_fetch_error.clone(),
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
            last_fetch_error: Arc::new(Mutex::new(None)),
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

    /// The most recent on-demand fetch failure, if any.
    ///
    /// The synchronous [`Store`] surface cannot return an error from a cache
    /// miss, so a failed on-demand fetch collapses to "object absent" and later
    /// surfaces as [`VmSdkError::MissingObject`]. When a run fails that way,
    /// check this to tell a transient transport or decode failure apart from a
    /// genuinely missing object. Set by a failing on-demand fetch and cleared
    /// by the next successful one; shared across clones.
    pub(crate) fn last_fetch_error(&self) -> Option<String> {
        self.last_fetch_error
            .lock()
            .expect("error lock poisoned")
            .clone()
    }

    /// Fetch `refs` synchronously from within the executor by blocking on the
    /// fetcher. A fetch error collapses to an empty result, leaving the object
    /// absent so the VM treats it as missing, but is stashed in
    /// [`last_fetch_error`](Self::last_fetch_error) first. Must run inside a
    /// multi-threaded Tokio runtime.
    fn fetch_blocking(&self, refs: &[(ObjectId, Option<Version>)]) -> Vec<Object> {
        match block_in_place(|| Handle::current().block_on(self.fetcher.fetch_objects(refs))) {
            Ok(objects) => {
                // A successful fetch clears any stale error from an earlier one,
                // so a later genuine miss is not misread as a transport failure.
                *self.last_fetch_error.lock().expect("error lock poisoned") = None;
                objects
            }
            Err(e) => {
                *self.last_fetch_error.lock().expect("error lock poisoned") = Some(e.to_string());
                Vec::new()
            }
        }
    }
}

impl<F: ObjectFetcher> Store for CachingStore<F> {
    fn get_object(&self, id: &ObjectId, version: Option<Version>) -> Option<Object> {
        // Scope the read lock so it is released before the blocking fetch
        // re-acquires it (a std `Mutex` is not reentrant).
        {
            let inner = self.inner.lock().expect("store lock poisoned");
            if let Some(obj) = inner.get_object(id, version) {
                return Some(obj);
            }
        }
        let fetched = self.fetch_blocking(&[(*id, version)]);
        let mut inner = self.inner.lock().expect("store lock poisoned");
        for obj in fetched {
            inner.insert(obj);
        }
        inner.get_object(id, version)
    }

    fn get_child_object(
        &self,
        parent: &ObjectId,
        child: &ObjectId,
        version_upper_bound: Version,
    ) -> Option<Object> {
        {
            let inner = self.inner.lock().expect("store lock poisoned");
            if let Some(obj) = inner.get_child_object(parent, child, version_upper_bound) {
                return Some(obj);
            }
        }
        // Fetch the child at its latest version; the upper-bound check is
        // re-applied below once it is cached.
        let fetched = self.fetch_blocking(&[(*child, None)]);
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
