// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Common utilities shared across API modules.

use iota_grpc_types::v0::{
    bcs::BcsData,
    ledger_service::{ObjectResult, TransactionResult, object_result, transaction_result},
    transaction::{ExecutedTransaction, Transaction as ProtoTransaction},
};
pub use iota_grpc_types::{
    field::{FieldMask, FieldMaskUtil},
    google::rpc::Status as RpcStatus,
    proto::TryFromProtoError,
};
use iota_sdk_types::Digest;
use serde::Serialize;

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
    type Value = iota_grpc_types::v0::object::Object;

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

/// Build a proto Transaction from serializable transaction data and digest.
pub fn build_proto_transaction<T: Serialize>(data: &T, digest: Digest) -> Result<ProtoTransaction> {
    let bcs = BcsData::serialize(data)
        .map_err(|e| Error::from(TryFromProtoError::invalid("transaction", e)))?;

    let proto_transaction = ProtoTransaction::default()
        .with_digest(digest)
        .with_bcs(bcs);

    Ok(proto_transaction)
}
