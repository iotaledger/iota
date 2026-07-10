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
/// JS function `fetch_object(id_hex: string, version: number | null) ->
/// string | null` returning the base-64 BCS of the full [`Object`] at the given
/// version (latest when `version` is null), or null when the object doesn't
/// exist. The Move VM is synchronous, so the callback must return synchronously
/// (e.g. a blocking `XMLHttpRequest`). A callback that throws, or returns
/// anything other than a base-64 `Object` or null, fails the lookup with a
/// [`StoreError`] — only null reads as "object doesn't exist".
pub(crate) struct CallbackStore {
    cache: RefCell<InMemoryStore>,
    /// Lookups the callback returned null for; skip re-fetching them.
    unresolved: RefCell<HashSet<(ObjectId, Option<u64>)>>,
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

    /// Fetch `id` (at `version`, or latest) via the callback and insert it
    /// into the cache. No-op if the callback already returned null for this
    /// lookup.
    fn fetch(&self, id: &ObjectId, version: Option<Version>) -> Result<(), StoreError> {
        let key = (*id, version.map(|v| v.as_u64()));
        if self.unresolved.borrow().contains(&key) {
            return Ok(());
        }
        let version_arg = match version {
            Some(v) => JsValue::from_f64(v.as_u64() as f64),
            None => JsValue::NULL,
        };
        let returned = self
            .fetch_object
            .call2(
                &JsValue::NULL,
                &JsValue::from_str(&id.to_string()),
                &version_arg,
            )
            .map_err(|e| StoreError::new(format!("fetch object {id}"), format!("{e:?}")))?;
        if returned.is_null() || returned.is_undefined() {
            self.unresolved.borrow_mut().insert(key);
            return Ok(());
        }
        let b64 = returned.as_string().ok_or_else(|| {
            StoreError::new(
                format!("fetch object {id}"),
                "callback must return a base-64 string or null",
            )
        })?;
        let bytes = BASE64
            .decode(b64.trim())
            .map_err(|e| StoreError::new(format!("decode object {id} base64"), e))?;
        let obj: Object = bcs::from_bytes(&bytes)
            .map_err(|e| StoreError::new(format!("decode object {id} bcs"), e))?;
        self.cache.borrow_mut().insert(obj);
        Ok(())
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
        self.fetch(id, version)?;
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
        // The bound is a ceiling, not an exact version, so fetch the latest.
        self.fetch(child, None)?;
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
