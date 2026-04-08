// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Common utilities shared across API modules.

use iota_grpc_types::v1::{
    bcs::BcsData,
    ledger_service::{ObjectResult, TransactionResult, object_result, transaction_result},
    transaction::{ExecutedTransaction, Transaction as ProtoTransaction},
    transaction_execution_service::{
        ExecuteTransactionResult, SimulateTransactionResult, SimulatedTransaction,
        execute_transaction_result, simulate_transaction_result,
    },
    types::ObjectId as ProtoObjectId,
};
pub use iota_grpc_types::{
    field::{FieldMask, FieldMaskUtil},
    google::rpc::Status as RpcStatus,
    proto::TryFromProtoError,
};
use iota_sdk_types::{Digest, ObjectId};
use serde::Serialize;

use super::MetadataEnvelope;

/// Errors that can occur during gRPC client API operations.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// Error converting proto types to SDK types.
    #[error("proto conversion error: {0}")]
    ProtoConversion(#[from] TryFromProtoError),

    /// Per-item error returned by the server (preserves code, message,
    /// details).
    #[error("server error (code {code}): {msg}", code = .0.code, msg = .0.message)]
    Server(RpcStatus),

    /// Client-side protocol error (e.g. checkpoint stream reassembly).
    #[error("protocol error: {0}")]
    Protocol(String),

    /// Error converting signatures.
    #[error("signature conversion error: {0}")]
    Signature(String),

    /// The caller passed an empty request (e.g. no object IDs or digests).
    #[error("empty request: at least one item must be provided")]
    EmptyRequest,

    /// The server stream ended unexpectedly while `has_next` was still true.
    #[error("stream ended unexpectedly: server indicated more results with has_next=true")]
    UnexpectedEndOfStream,

    /// gRPC transport or protocol error.
    #[error("grpc error: {0}")]
    Grpc(#[from] tonic::Status),
}

impl From<Error> for tonic::Status {
    fn from(err: Error) -> Self {
        match err {
            Error::ProtoConversion(e) => {
                tonic::Status::internal(format!("proto conversion error: {e}"))
            }
            Error::Server(status) => status.to_tonic_status(),
            Error::Protocol(msg) => tonic::Status::internal(format!("protocol error: {msg}")),
            Error::Signature(msg) => {
                tonic::Status::internal(format!("signature conversion error: {msg}"))
            }
            Error::EmptyRequest => {
                tonic::Status::invalid_argument("empty request: at least one item must be provided")
            }
            Error::UnexpectedEndOfStream => {
                tonic::Status::internal("stream ended unexpectedly: has_next was true")
            }
            Error::Grpc(status) => status,
        }
    }
}

/// Result type alias for API operations.
pub type Result<T> = std::result::Result<T, Error>;

// =============================================================================
// Field Masks
// =============================================================================

/// Build a field mask with a custom value or default.
///
/// This is a convenience helper that handles the common pattern of using
/// a user-provided field mask or falling back to a default.
pub fn field_mask_with_default(custom: Option<&str>, default: &str) -> FieldMask {
    FieldMask::from_str(custom.unwrap_or(default))
}

/// Safely convert a `usize` to `u32`, saturating at `u32::MAX` instead of
/// silently truncating on 64-bit platforms.
pub fn saturating_usize_to_u32(value: usize) -> u32 {
    u32::try_from(value).unwrap_or(u32::MAX)
}

/// A trait for proto result types that follow the pattern of having
/// `Some(Result::Value)`, `Some(Result::Error)`, or `None`.
///
/// This allows generic handling of gRPC response results that can be either
/// a success value, a server error, or missing.
pub trait ProtoResult {
    /// The success value type.
    type Value;

    /// Extract the result, converting to our error types.
    fn into_result(self) -> Result<Self::Value>;
}

impl ProtoResult for ObjectResult {
    type Value = iota_grpc_types::v1::object::Object;

    fn into_result(self) -> Result<Self::Value> {
        match self.result {
            Some(object_result::Result::Object(obj)) => Ok(obj),
            Some(object_result::Result::Error(e)) => Err(Error::Server(e)),
            None => Err(TryFromProtoError::missing("result").into()),
            Some(_) => Err(Error::Protocol("Unknown object result type".into())),
        }
    }
}

impl ProtoResult for TransactionResult {
    type Value = ExecutedTransaction;

    fn into_result(self) -> Result<Self::Value> {
        match self.result {
            Some(transaction_result::Result::ExecutedTransaction(tx)) => Ok(tx),
            Some(transaction_result::Result::Error(e)) => Err(Error::Server(e)),
            None => Err(TryFromProtoError::missing("result").into()),
            Some(_) => Err(Error::Protocol("Unknown transaction result type".into())),
        }
    }
}

impl ProtoResult for ExecuteTransactionResult {
    type Value = ExecutedTransaction;

    fn into_result(self) -> Result<Self::Value> {
        match self.result {
            Some(execute_transaction_result::Result::ExecutedTransaction(tx)) => Ok(tx),
            Some(execute_transaction_result::Result::Error(e)) => Err(Error::Server(e)),
            None => Err(TryFromProtoError::missing("result").into()),
            Some(_) => Err(Error::Protocol(
                "Unknown execute transaction result type".into(),
            )),
        }
    }
}

impl ProtoResult for SimulateTransactionResult {
    type Value = SimulatedTransaction;

    fn into_result(self) -> Result<Self::Value> {
        match self.result {
            Some(simulate_transaction_result::Result::SimulatedTransaction(tx)) => Ok(tx),
            Some(simulate_transaction_result::Result::Error(e)) => Err(Error::Server(e)),
            None => Err(TryFromProtoError::missing("result").into()),
            Some(_) => Err(Error::Protocol(
                "Unknown simulate transaction result type".into(),
            )),
        }
    }
}

/// Collect all items from a paginated gRPC stream into a single `Vec`.
///
/// This handles the common pattern of iterating over a `tonic::Streaming<T>`,
/// extracting items from each message via the `extract` closure, and checking
/// that the stream was not truncated (i.e. `has_next` is `false` on the last
/// message).
///
/// The `extract` closure receives each stream message and must return
/// `(has_next, items)`.  Because some streams require fallible per-item
/// conversion (e.g. via [`ProtoResult`]), the closure itself returns
/// `Result<…>`.
pub async fn collect_stream<T, I, F>(
    mut stream: tonic::Streaming<T>,
    metadata: tonic::metadata::MetadataMap,
    extract: F,
) -> Result<MetadataEnvelope<Vec<I>>>
where
    F: Fn(T) -> Result<(bool, Vec<I>)>,
{
    let mut results = Vec::new();
    let mut has_next = false;

    while let Some(response) = stream.message().await? {
        let (next, items) = extract(response)?;
        has_next = next;
        results.extend(items);
    }

    if has_next {
        return Err(Error::UnexpectedEndOfStream);
    }

    Ok(MetadataEnvelope::new(results, metadata))
}

/// A single page of results from a paginated list endpoint.
///
/// Returned when awaiting a list query builder directly (single-page mode).
/// Contains the items from this page plus an optional continuation token.
#[derive(Debug, Clone)]
pub struct Page<T> {
    /// The items returned in this page.
    pub items: Vec<T>,
    /// Token to retrieve the next page. `None` when this is the last page.
    pub next_page_token: Option<::prost::bytes::Bytes>,
}

/// Generate a paginated query builder for a list endpoint.
///
/// The generated struct implements [`IntoFuture`](std::future::IntoFuture) for
/// single-page retrieval and provides a [`collect`] method for auto-pagination.
///
/// # Parameters
///
/// - `$query_name` — name of the generated builder struct
/// - `$service_client_type` — the tonic service client type
/// - `$item_type` — the item type in the response vec
/// - `$rpc_method` — the RPC method name on the service client
/// - `$items_field` — the field name on the response containing the items vec
///
/// # Example
///
/// ```ignore
/// define_list_query! {
///     pub struct ListOwnedObjectsQuery {
///         service_client: StateServiceClient<InterceptedChannel>,
///         request: ListOwnedObjectsRequest,
///         item: Object,
///         rpc_method: list_owned_objects,
///         items_field: objects,
///     }
/// }
/// ```
macro_rules! define_list_query {
    (
        $(#[$meta:meta])*
        pub struct $query_name:ident {
            service_client: $service_client_type:ty,
            request: $request_type:ty,
            item: $item_type:ty,
            rpc_method: $rpc_method:ident,
            items_field: $items_field:ident,
        }
    ) => {
        $(#[$meta])*
        pub struct $query_name {
            service_client: $service_client_type,
            base_request: $request_type,
            max_message_size: Option<usize>,
            page_size: Option<u32>,
            page_token: Option<::prost::bytes::Bytes>,
        }

        impl $query_name {
            pub(crate) fn new(
                service_client: $service_client_type,
                base_request: $request_type,
                max_message_size: Option<usize>,
                page_size: Option<u32>,
                page_token: Option<::prost::bytes::Bytes>,
            ) -> Self {
                Self {
                    service_client,
                    base_request,
                    max_message_size,
                    page_size,
                    page_token,
                }
            }

            /// Auto-paginate through all pages, collecting up to `limit` items.
            ///
            /// If `limit` is `None`, collects all items across all pages.
            pub async fn collect(
                self,
                limit: Option<u32>,
            ) -> $crate::api::Result<$crate::api::MetadataEnvelope<Vec<$item_type>>> {
                let mut all_items = Vec::new();
                let mut next_page_token = self.page_token;
                let mut result_metadata = None;
                let mut service_client = self.service_client;

                loop {
                    let mut request = self.base_request.clone();

                    // Cap page_size to the remaining items needed when a
                    // limit is set, so we don't over-fetch from the server.
                    let effective_page_size = match (self.page_size, limit) {
                        (Some(ps), Some(l)) => {
                            let remaining = (l as usize).saturating_sub(all_items.len());
                            Some(ps.min(remaining as u32))
                        }
                        (Some(ps), None) => Some(ps),
                        (None, Some(l)) => {
                            let remaining = (l as usize).saturating_sub(all_items.len());
                            Some(remaining as u32)
                        }
                        (None, None) => None,
                    };
                    if let Some(ps) = effective_page_size {
                        request = request.with_page_size(ps);
                    }
                    if let Some(token) = next_page_token.take() {
                        request = request.with_page_token(token);
                    }
                    if let Some(max_size) = self.max_message_size {
                        request = request.with_max_message_size_bytes(
                            $crate::api::saturating_usize_to_u32(max_size),
                        );
                    }

                    let response = service_client.$rpc_method(request).await?;
                    let (body, metadata) =
                        $crate::api::MetadataEnvelope::from(response).into_parts();
                    if result_metadata.is_none() {
                        result_metadata = Some(metadata);
                    }

                    all_items.extend(body.$items_field);

                    match body.next_page_token {
                        Some(token) => next_page_token = Some(token),
                        None => break,
                    }

                    if limit.is_some_and(|l| all_items.len() >= l as usize) {
                        break;
                    }
                }

                Ok($crate::api::MetadataEnvelope::new(
                    all_items,
                    result_metadata.unwrap_or_default(),
                ))
            }
        }

        impl ::std::future::IntoFuture for $query_name {
            type Output = $crate::api::Result<
                $crate::api::MetadataEnvelope<$crate::api::Page<$item_type>>,
            >;
            type IntoFuture = ::std::pin::Pin<
                Box<dyn ::std::future::Future<Output = Self::Output> + Send>,
            >;

            fn into_future(self) -> Self::IntoFuture {
                Box::pin(async move {
                    let mut service_client = self.service_client;
                    let mut request = self.base_request;

                    if let Some(ps) = self.page_size {
                        request = request.with_page_size(ps);
                    }
                    if let Some(token) = self.page_token {
                        request = request.with_page_token(token);
                    }
                    if let Some(max_size) = self.max_message_size {
                        request = request.with_max_message_size_bytes(
                            $crate::api::saturating_usize_to_u32(max_size),
                        );
                    }

                    let response = service_client.$rpc_method(request).await?;
                    let (body, metadata) =
                        $crate::api::MetadataEnvelope::from(response).into_parts();

                    Ok($crate::api::MetadataEnvelope::new(
                        $crate::api::Page {
                            items: body.$items_field,
                            next_page_token: body.next_page_token,
                        },
                        metadata,
                    ))
                })
            }
        }
    };
}

pub(crate) use define_list_query;

/// Convert an `ObjectId` to the gRPC proto `ObjectId` type.
pub fn proto_object_id(id: ObjectId) -> ProtoObjectId {
    ProtoObjectId::default().with_object_id(id.inner().to_vec())
}

/// Build a proto Transaction from serializable transaction data and digest.
pub fn build_proto_transaction<T: Serialize>(data: &T, digest: Digest) -> Result<ProtoTransaction> {
    let bcs = BcsData::serialize(data)
        .map_err(|e| Error::from(TryFromProtoError::invalid("transaction", e)))?;

    let proto_transaction = ProtoTransaction::default()
        .with_digest(digest)
        .with_bcs(bcs);

    Ok(proto_transaction)
}
