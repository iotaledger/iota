// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! gRPC-backed store (`feature = "grpc"`, native only).
//!
//! [`GrpcStore`] wraps a gRPC [`Client`] and an in-memory object cache: it
//! resolves objects on demand during execution and caches them, so only the
//! objects a run actually touches are fetched.
//! [`fetch_chain_context`](GrpcStore::fetch_chain_context) runs up front, and
//! callers may [`prefetch`](GrpcStore::prefetch) to warm the cache, but neither
//! is required for correctness.
//!
//! On-demand fetching blocks the executor thread on async I/O, so
//! [`LocalVm::execute`](crate::LocalVm::execute) must run inside a
//! multi-threaded Tokio runtime (e.g. `#[tokio::main]`).

use iota_grpc_client::Client;
use iota_sdk_types::{ObjectId, Version};
use iota_types::{object::Object, transaction::TransactionData};

use crate::{
    caching::{CachingStore, ObjectFetcher},
    error::{StoreError, VmSdkError},
    executor::ChainContext,
    store::{InMemoryStore, Store},
};

/// A [`Store`] backed by a remote node over gRPC, resolving objects on demand.
///
/// Clones share the same cache and client.
///
/// # Panics
///
/// On-demand object resolution (via the synchronous [`Store`] impl) panics
/// unless called from within a multi-threaded Tokio runtime.
#[derive(Clone)]
pub struct GrpcStore {
    cache: CachingStore<GrpcFetcher>,
}

impl GrpcStore {
    /// Wrap an existing client. The store starts with the built-in framework
    /// packages already loaded so Move calls resolve.
    pub fn new(client: Client) -> Self {
        Self {
            cache: CachingStore::new(GrpcFetcher { client }),
        }
    }

    /// Connect to a gRPC endpoint (by URL) and create a store containing only
    /// the built-in framework packages.
    ///
    /// # Errors
    ///
    /// Returns [`VmSdkError::Store`] if the client cannot be created for `url`.
    pub fn connect(url: impl Into<String>) -> Result<Self, VmSdkError> {
        let url: String = url.into();
        let client = Client::new(url).map_err(|e| StoreError::new("connect gRPC", e))?;
        Ok(Self::new(client))
    }

    /// A snapshot clone of the objects cached so far (framework packages plus
    /// anything fetched on demand or pre-fetched).
    pub fn store(&self) -> InMemoryStore {
        self.cache.store()
    }

    /// The most recent on-demand fetch failure, if any.
    ///
    /// A failed cache-miss fetch collapses to "object absent" — surfacing later
    /// as [`VmSdkError::MissingObject`] — so check this to tell a transient
    /// transport or decode failure apart from a genuinely missing object.
    /// Cleared by the next successful fetch.
    pub fn last_fetch_error(&self) -> Option<String> {
        self.cache.last_fetch_error()
    }

    /// Fetch the chain parameters a [`LocalVm`](crate::LocalVm) needs.
    ///
    /// Reports [`Chain::Unknown`](crate::Chain) — the chain identity is not
    /// resolved here; this only affects chain-specific protocol behaviour.
    ///
    /// # Errors
    ///
    /// Returns [`VmSdkError::Store`] if the epoch RPC fails or can't be
    /// decoded.
    pub async fn fetch_chain_context(&self) -> Result<ChainContext, VmSdkError> {
        let epoch = self
            .cache
            .fetcher()
            .client
            .get_epoch(None, None)
            .await
            .map_err(|e| StoreError::new("fetch epoch", e))?
            .into_inner();
        let epoch_id = epoch
            .epoch_id()
            .map_err(|e| StoreError::new("epoch id", e))?;
        let reference_gas_price = epoch
            .gas_price()
            .map_err(|e| StoreError::new("gas price", e))?;
        let epoch_timestamp_ms = epoch
            .start_ms()
            .map_err(|e| StoreError::new("epoch start", e))?;
        let protocol_version = epoch
            .protocol_config()
            .and_then(|pc| pc.version())
            .map_err(|e| StoreError::new("protocol version", e))?;
        Ok(ChainContext {
            protocol_version: iota_protocol_config::ProtocolVersion::new(protocol_version),
            reference_gas_price,
            epoch_id,
            epoch_timestamp_ms,
            chain: iota_protocol_config::Chain::Unknown,
        })
    }

    /// Fetch every object the transaction references and cache them in one
    /// batched request.
    ///
    /// Optional: the store also resolves these objects on demand during
    /// execution. Pre-fetching only saves the per-object round-trips the
    /// executor would otherwise make for the transaction body.
    pub async fn prefetch(&mut self, transaction: &TransactionData) -> Result<(), VmSdkError> {
        self.cache.prefetch(transaction).await
    }
}

impl Store for GrpcStore {
    fn get_object(&self, id: &ObjectId, version: Option<Version>) -> Option<Object> {
        self.cache.get_object(id, version)
    }

    fn get_child_object(
        &self,
        parent: &ObjectId,
        child: &ObjectId,
        version_upper_bound: Version,
    ) -> Option<Object> {
        self.cache
            .get_child_object(parent, child, version_upper_bound)
    }

    fn insert(&mut self, object: Object) {
        self.cache.insert(object);
    }

    fn remove(&mut self, id: &ObjectId) {
        self.cache.remove(id);
    }
}

/// gRPC transport for [`CachingStore`].
#[derive(Clone)]
struct GrpcFetcher {
    client: Client,
}

impl ObjectFetcher for GrpcFetcher {
    async fn fetch_objects(
        &self,
        refs: &[(ObjectId, Option<Version>)],
    ) -> Result<Vec<Object>, VmSdkError> {
        let proto_objects = self
            .client
            .get_objects(refs, None)
            .await
            .map_err(|e| StoreError::new("fetch objects via gRPC", e))?
            .into_inner();
        let mut objects = Vec::with_capacity(proto_objects.len());
        for proto_obj in proto_objects {
            // The proto helper yields the SDK `Object`; round-trip through BCS
            // into the node's `iota_types::object::Object` (identical layout).
            let sdk_obj = proto_obj
                .object()
                .map_err(|e| StoreError::new("decode gRPC object", e))?;
            let bytes =
                bcs::to_bytes(&sdk_obj).map_err(|e| StoreError::new("re-encode object", e))?;
            let obj: Object =
                bcs::from_bytes(&bytes).map_err(|e| StoreError::new("decode object", e))?;
            objects.push(obj);
        }
        Ok(objects)
    }
}
