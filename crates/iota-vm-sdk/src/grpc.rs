// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! gRPC-backed store population (`feature = "grpc"`, native only).
//!
//! [`GrpcStore`] wraps an [`InMemoryStore`] and a gRPC [`Client`]. Object
//! fetching is async and confined to this module: callers `prefetch` the
//! objects a transaction references (and the [`ChainContext`]) up front, then
//! hand the populated store to a [`LocalVm`](crate::LocalVm). Because the
//! resulting store is a plain synchronous [`Store`], the executor itself never
//! does network I/O.

use iota_grpc_client::Client;
use iota_sdk_types::{ObjectId, Version};
use iota_types::{
    object::Object,
    transaction::{TransactionData, TransactionDataAPI},
};

use crate::{
    error::{ValidationError, VmSdkError},
    executor::ChainContext,
    store::{InMemoryStore, Store},
};

/// A [`Store`] populated from a remote node over gRPC.
#[derive(Clone)]
pub struct GrpcStore {
    inner: InMemoryStore,
    client: Client,
}

impl GrpcStore {
    /// Wrap an existing client. The store is seeded with the built-in
    /// framework packages so Move calls resolve.
    pub fn new(client: Client) -> Self {
        Self {
            inner: InMemoryStore::with_framework(),
            client,
        }
    }

    /// Connect to a gRPC endpoint (by URL) and create an empty,
    /// framework-seeded store.
    pub fn connect(url: &str) -> Result<Self, VmSdkError> {
        let client = Client::new(url).map_err(|e| ValidationError::new("connect gRPC", e))?;
        Ok(Self::new(client))
    }

    /// Read-only access to the wrapped in-memory store, e.g. to snapshot the
    /// objects fetched so far.
    pub fn store(&self) -> &InMemoryStore {
        &self.inner
    }

    /// Fetch the chain parameters a [`LocalVm`](crate::LocalVm) needs.
    pub async fn fetch_chain_context(&self) -> Result<ChainContext, VmSdkError> {
        let epoch = self
            .client
            .get_epoch(None, None)
            .await
            .map_err(|e| ValidationError::new("fetch epoch", e))?
            .into_inner();
        let epoch_id = epoch
            .epoch_id()
            .map_err(|e| ValidationError::new("epoch id", e))?;
        let reference_gas_price = epoch
            .gas_price()
            .map_err(|e| ValidationError::new("gas price", e))?;
        let epoch_timestamp_ms = epoch
            .start_ms()
            .map_err(|e| ValidationError::new("epoch start", e))?;
        let protocol_version = epoch
            .protocol_config()
            .and_then(|pc| pc.version())
            .map_err(|e| ValidationError::new("protocol version", e))?;
        Ok(ChainContext {
            protocol_version: iota_protocol_config::ProtocolVersion::new(protocol_version),
            reference_gas_price,
            epoch_id,
            epoch_timestamp_ms,
            chain: iota_protocol_config::Chain::Unknown,
        })
    }

    /// Fetch every object the transaction references and insert it into the
    /// store. Owned/immutable objects are fetched at their transaction
    /// versions; shared objects and packages at the latest version.
    pub async fn prefetch(&mut self, transaction: &TransactionData) -> Result<(), VmSdkError> {
        let mut refs: Vec<(ObjectId, Option<Version>)> = Vec::new();
        let input_object_kinds = transaction
            .input_objects()
            .map_err(|e| ValidationError::new("collect input objects", e))?;
        use iota_types::transaction::InputObjectKind;
        for kind in &input_object_kinds {
            match kind {
                InputObjectKind::ImmOrOwnedMoveObject(objref) => {
                    refs.push((objref.object_id, Some(objref.version)))
                }
                // Shared objects and packages: latest version.
                InputObjectKind::SharedMoveObject { id, .. } => refs.push((*id, None)),
                InputObjectKind::MovePackage(id) => refs.push((*id, None)),
            }
        }
        for gas_ref in transaction.gas() {
            refs.push((gas_ref.object_id, Some(gas_ref.version)));
        }
        for objref in transaction.receiving_objects() {
            refs.push((objref.object_id, Some(objref.version)));
        }
        if refs.is_empty() {
            return Ok(());
        }
        self.fetch_and_insert(&refs).await
    }

    /// Recursively fetch the dynamic-field children of every object already in
    /// the store and insert them too. Move calls that read tables/bags need
    /// these children present to execute offline — e.g. staking walks the
    /// validator set stored as a dynamic field inside `IotaSystemState`, and
    /// `request_add_stake` aborts in `dynamic_field::remove_child_object`
    /// without them. Call after [`prefetch`](Self::prefetch); children are
    /// fetched at their latest version, matching how shared objects are loaded.
    pub async fn prefetch_dynamic_fields(&mut self) -> Result<(), VmSdkError> {
        let mut visited: std::collections::HashSet<ObjectId> = std::collections::HashSet::new();
        let mut queue: Vec<ObjectId> = self.inner.iter().map(|(id, _)| *id).collect();
        while let Some(parent) = queue.pop() {
            if !visited.insert(parent) {
                continue;
            }
            let fields = self
                .client
                .list_dynamic_fields(parent, None, None, None)
                .collect(None)
                .await
                .map_err(|e| ValidationError::new("list dynamic fields", e))?
                .into_inner();
            let mut refs: Vec<(ObjectId, Option<Version>)> = Vec::new();
            for field in fields {
                // The field wrapper object is always needed; for dynamic
                // *object* fields the value lives in a separate child object.
                if let Some(id) = field.field_id {
                    let id = id
                        .object_id()
                        .map_err(|e| ValidationError::new("decode dynamic field id", e))?;
                    refs.push((id, None));
                }
                if let Some(id) = field.child_id {
                    let id = id
                        .object_id()
                        .map_err(|e| ValidationError::new("decode dynamic child id", e))?;
                    refs.push((id, None));
                }
            }
            if refs.is_empty() {
                continue;
            }
            self.fetch_and_insert(&refs).await?;
            // Recurse into the newly fetched children to find their descendants.
            queue.extend(refs.into_iter().map(|(id, _)| id));
        }
        Ok(())
    }

    async fn fetch_and_insert(
        &mut self,
        refs: &[(ObjectId, Option<Version>)],
    ) -> Result<(), VmSdkError> {
        let proto_objects = self
            .client
            .get_objects(refs, None)
            .await
            .map_err(|e| ValidationError::new("fetch objects via gRPC", e))?
            .into_inner();
        for proto_obj in proto_objects {
            // The proto helper yields the SDK `Object`; round-trip through BCS
            // into the node's `iota_types::object::Object` (identical layout).
            let sdk_obj = proto_obj
                .object()
                .map_err(|e| ValidationError::new("decode gRPC object", e))?;
            let bytes =
                bcs::to_bytes(&sdk_obj).map_err(|e| ValidationError::new("re-encode object", e))?;
            let obj: Object =
                bcs::from_bytes(&bytes).map_err(|e| ValidationError::new("decode object", e))?;
            self.inner.insert(obj);
        }
        Ok(())
    }
}

impl Store for GrpcStore {
    fn get_object(&self, id: &ObjectId, version: Option<Version>) -> Option<Object> {
        self.inner.get_object(id, version)
    }

    fn get_child_object(
        &self,
        parent: &ObjectId,
        child: &ObjectId,
        version_upper_bound: Version,
    ) -> Option<Object> {
        self.inner
            .get_child_object(parent, child, version_upper_bound)
    }

    fn insert(&mut self, object: Object) {
        self.inner.insert(object);
    }

    fn remove(&mut self, id: &ObjectId) {
        self.inner.remove(id);
    }
}
