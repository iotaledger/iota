// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Node-internal implementation of the SDK's
//! [`TransactionBuilderLedgerClient`].
//!
//! [`NodeTransactionBuilderLedgerClient`] backs the SDK
//! [`TransactionBuilder`](iota_sdk_transaction_builder::TransactionBuilder)
//! with a node's local state instead of a remote gRPC or GraphQL endpoint.
//! All reads go through [`GrpcStateReader`] — the same interface the gRPC
//! server uses — so the client is available wherever that store is
//! (fullnodes via `GrpcReadStore`, simulacrum in tests).
//!
//! The client is ledger-only: it resolves objects, gas, and protocol
//! parameters, but does not simulate or execute, so transactions are built
//! with an explicit gas budget via
//! [`TransactionBuilder::finish_with_budget`](iota_sdk_transaction_builder::TransactionBuilder::finish_with_budget).

use std::sync::Arc;

use iota_node_storage::GrpcStateReader;
use iota_protocol_config::{ProtocolConfig as NodeProtocolConfig, ProtocolVersion};
use iota_sdk_transaction_builder::{
    ObjectsPage, ProtocolConfig, TransactionBuilderClientBase, TransactionBuilderLedgerClient,
};
use iota_sdk_types::{Address, Object, ObjectId, StructTag, Version};
use iota_types::{
    error::IotaError,
    iota_sdk_types_conversions::SdkTypeConversionError,
    iota_system_state::{IotaSystemStateTrait, get_iota_system_state},
    storage::OwnedObjectCursor,
};
use serde::{Deserialize, Serialize};
use typed_store_error::TypedStoreError;

/// Default number of objects returned by
/// [`TransactionBuilderLedgerClient::objects`] when no limit (or a zero
/// limit) is given.
const DEFAULT_OBJECTS_PAGE_SIZE: usize = 50;

/// Upper bound on the number of objects returned by
/// [`TransactionBuilderLedgerClient::objects`].
const MAX_OBJECTS_PAGE_SIZE: usize = 1000;

/// Payload of the opaque [`TransactionBuilderLedgerClient::objects`] cursor.
///
/// Binds the index position to the request that produced it, mirroring the
/// gRPC server's page token, so a cursor replayed with a different `owner`
/// or `struct_tag` is rejected instead of silently resuming from a
/// meaningless position.
#[derive(Serialize, Deserialize)]
struct PageToken {
    owner: Address,
    struct_tag: Option<StructTag>,
    cursor: OwnedObjectCursor,
}

impl PageToken {
    /// Decodes a cursor and checks that it was produced by a request with the
    /// same `owner` and `struct_tag`.
    fn decode(
        bytes: &[u8],
        owner: Address,
        struct_tag: Option<&StructTag>,
    ) -> Result<OwnedObjectCursor, Error> {
        let token: PageToken = bcs::from_bytes(bytes).map_err(|e| Error::Cursor(e.to_string()))?;
        if token.owner != owner || token.struct_tag.as_ref() != struct_tag {
            return Err(Error::Cursor(
                "does not match the request parameters".to_string(),
            ));
        }
        Ok(token.cursor)
    }

    fn encode(&self) -> Vec<u8> {
        bcs::to_bytes(self).expect("page token serialization cannot fail")
    }
}

/// Error type for [`NodeTransactionBuilderLedgerClient`].
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Storage(#[from] iota_types::storage::error::Error),
    #[error(transparent)]
    Store(#[from] TypedStoreError),
    #[error(transparent)]
    Conversion(#[from] SdkTypeConversionError),
    #[error(transparent)]
    Node(#[from] IotaError),
    #[error("invalid objects page cursor: {0}")]
    Cursor(String),
    #[error("gRPC indexes are disabled on this node")]
    IndexesDisabled,
    #[error("protocol version {0} is not supported by this node")]
    UnsupportedProtocolVersion(u64),
}

/// A [`TransactionBuilderLedgerClient`] backed directly by a node's local
/// state.
///
/// [`objects`](TransactionBuilderLedgerClient::objects) requires the gRPC
/// indexes (the owned-object index) to be enabled and returns
/// [`Error::IndexesDisabled`] otherwise.
#[derive(Clone)]
pub struct NodeTransactionBuilderLedgerClient {
    reader: Arc<dyn GrpcStateReader>,
}

impl NodeTransactionBuilderLedgerClient {
    pub fn new(reader: Arc<dyn GrpcStateReader>) -> Self {
        Self { reader }
    }

    /// Protocol config of the current epoch.
    pub fn node_protocol_config(&self) -> Result<NodeProtocolConfig, Error> {
        let system_state = get_iota_system_state(self.reader.as_ref())?;
        let chain = self.reader.get_chain_identifier()?.chain();
        let protocol_version = system_state.protocol_version();
        NodeProtocolConfig::get_for_version_if_supported(
            ProtocolVersion::new(protocol_version),
            chain,
        )
        .ok_or(Error::UnsupportedProtocolVersion(protocol_version))
    }
}

impl TransactionBuilderClientBase for NodeTransactionBuilderLedgerClient {
    type Error = Error;
}

impl TransactionBuilderLedgerClient for NodeTransactionBuilderLedgerClient {
    async fn object(
        &self,
        object_id: ObjectId,
        version: impl Into<Option<Version>>,
    ) -> Result<Option<Object>, Self::Error> {
        let object = match version.into() {
            Some(version) => self.reader.try_get_object_by_key(&object_id, version)?,
            None => self.reader.try_get_object(&object_id)?,
        };
        object.map(Object::try_from).transpose().map_err(Into::into)
    }

    async fn objects(
        &self,
        struct_tag: Option<StructTag>,
        owner: Address,
        cursor: Option<Vec<u8>>,
        limit: Option<usize>,
    ) -> Result<ObjectsPage, Self::Error> {
        let limit = match limit {
            None | Some(0) => DEFAULT_OBJECTS_PAGE_SIZE,
            Some(limit) => limit.min(MAX_OBJECTS_PAGE_SIZE),
        };
        let cursor = cursor
            .map(|bytes| PageToken::decode(&bytes, owner, struct_tag.as_ref()))
            .transpose()?;

        let indexes = self.reader.grpc_indexes().ok_or(Error::IndexesDisabled)?;
        // The index iterator's cursor bound is inclusive, so skip the cursor
        // item itself to advance past the previous page.
        let skip = usize::from(cursor.is_some());
        let mut iter = indexes
            .account_owned_objects_info_iter(owner, cursor.as_ref(), struct_tag.clone())?
            .skip(skip);

        let mut data = Vec::with_capacity(limit);
        let mut last_cursor = None;
        for item in iter.by_ref() {
            let (info, item_cursor) = item?;
            let Some(object) = self
                .reader
                .try_get_object_by_key(&info.object_id, info.version)?
            else {
                // The object is no longer at the indexed version (e.g. mutated
                // between the index scan and the fetch).
                tracing::debug!(
                    object_id = %info.object_id,
                    version = %info.version,
                    "object not found while iterating owned objects, skipping",
                );
                continue;
            };
            data.push(Object::try_from(object)?);
            last_cursor = Some(item_cursor);
            if data.len() >= limit {
                break;
            }
        }

        let has_more = iter.next().transpose()?.is_some();
        let next_cursor = last_cursor.filter(|_| has_more).map(|cursor| {
            PageToken {
                owner,
                struct_tag,
                cursor,
            }
            .encode()
        });

        Ok(ObjectsPage { data, next_cursor })
    }

    async fn protocol_config(&self) -> Result<ProtocolConfig, Self::Error> {
        let attributes = self
            .node_protocol_config()?
            .attr_map()
            .into_iter()
            .filter_map(|(name, value)| value.map(|value| (name, value.to_string())))
            .collect();
        Ok(ProtocolConfig { attributes })
    }

    async fn reference_gas_price(
        &self,
        epoch: impl Into<Option<u64>>,
    ) -> Result<Option<u64>, Self::Error> {
        match epoch.into() {
            None => {
                let system_state = get_iota_system_state(self.reader.as_ref())?;
                Ok(Some(system_state.reference_gas_price()))
            }
            Some(epoch) => Ok(self
                .reader
                .get_epoch_info(epoch)?
                .map(|info| info.reference_gas_price())),
        }
    }
}
