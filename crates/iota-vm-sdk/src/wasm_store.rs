// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! On-demand object store for the wasm surface (`feature = "wasm-bindgen"`,
//! `target_arch = "wasm32"`).
//!
//! [`CallbackStore`] resolves objects lazily by calling back into JavaScript.
//! A transaction's bytes only declare its *input* objects, but Move execution
//! also reads dynamic-field children (e.g. staking walks the validator set
//! inside `IotaSystemState`). Rather than have the JS side enumerate or
//! pre-fetch those — an object can have thousands of dynamic fields — the store
//! fetches whatever the synchronous VM asks for, exactly when it asks: on a
//! cache miss it calls a JS function that returns the object's base-64 BCS,
//! then decodes and caches it. IDs the callback can't resolve are remembered so
//! it isn't called again for them.

use std::{cell::RefCell, collections::HashSet};

use base64::{Engine, engine::general_purpose::STANDARD as BASE64};
use iota_sdk_types::{ObjectId, Version};
use iota_types::object::Object;
use js_sys::Function;
use wasm_bindgen::JsValue;

use crate::{
    error::StoreError,
    store::{InMemoryStore, Store},
};

/// A [`Store`] that fetches missing objects on demand via a JS callback.
///
/// Wraps an [`InMemoryStore`] cache (seeded with the framework packages) and a
/// JS function `fetch_object(id_hex: string) -> string | null` returning the
/// base-64 BCS of the full [`Object`]. The Move VM is synchronous, so the
/// callback must return synchronously (e.g. a blocking `XMLHttpRequest`).
pub(crate) struct CallbackStore {
    cache: RefCell<InMemoryStore>,
    /// IDs the callback returned null/garbage for; skip re-fetching them.
    unresolved: RefCell<HashSet<ObjectId>>,
    fetch_object: Function,
}

impl CallbackStore {
    /// Create a store that resolves misses through `fetch_object`.
    pub(crate) fn new(fetch_object: Function) -> Self {
        Self {
            cache: RefCell::new(InMemoryStore::with_framework()),
            unresolved: RefCell::new(HashSet::new()),
            fetch_object,
        }
    }

    /// Seed the cache with objects supplied up front (optional; the store can
    /// resolve everything on demand, but a caller may pre-load known objects).
    pub(crate) fn seed<I: IntoIterator<Item = Object>>(&self, objects: I) {
        let mut cache = self.cache.borrow_mut();
        for obj in objects {
            cache.insert(obj);
        }
    }

    /// Fetch `id` via the callback and insert it into the cache. No-op if the
    /// callback already failed for this ID. Returns whether the object is now
    /// cached.
    fn fetch(&self, id: &ObjectId) -> bool {
        if self.unresolved.borrow().contains(id) {
            return false;
        }
        let fetched = self
            .fetch_object
            .call1(&JsValue::NULL, &JsValue::from_str(&id.to_string()))
            .ok()
            .and_then(|v| v.as_string())
            .and_then(|b64| BASE64.decode(b64.trim()).ok())
            .and_then(|bytes| bcs::from_bytes::<Object>(&bytes).ok());
        match fetched {
            Some(obj) => {
                self.cache.borrow_mut().insert(obj);
                true
            }
            None => {
                self.unresolved.borrow_mut().insert(*id);
                false
            }
        }
    }
}

impl Store for CallbackStore {
    fn get_object(
        &self,
        id: &ObjectId,
        version: Option<Version>,
    ) -> Result<Option<Object>, StoreError> {
        if let Some(obj) = self.cache.borrow().get_object(id, version)? {
            return Ok(Some(obj));
        }
        self.fetch(id);
        self.cache.borrow().get_object(id, version)
    }

    fn get_child_object(
        &self,
        parent: &ObjectId,
        child: &ObjectId,
        version_upper_bound: Version,
    ) -> Result<Option<Object>, StoreError> {
        if let Some(obj) =
            self.cache
                .borrow()
                .get_child_object(parent, child, version_upper_bound)?
        {
            return Ok(Some(obj));
        }
        self.fetch(child);
        self.cache
            .borrow()
            .get_child_object(parent, child, version_upper_bound)
    }

    fn insert(&mut self, object: Object) {
        self.cache.borrow_mut().insert(object);
    }

    fn remove(&mut self, id: &ObjectId) {
        self.cache.borrow_mut().remove(id);
    }
}
