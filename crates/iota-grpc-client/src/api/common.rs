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

    /// Error from the gRPC server.
    #[error("server error: {0}")]
    Server(String),

    /// Error converting signatures.
    #[error("signature conversion error: {0}")]
    Signature(String),

    /// gRPC transport or protocol error.
    #[error("grpc error: {0}")]
    Grpc(#[from] tonic::Status),
}

impl Error {
    /// Create a new server error.
    pub fn server<T: AsRef<str>>(msg: T) -> Self {
        Error::Server(msg.as_ref().to_string())
    }
}

impl From<Error> for tonic::Status {
    fn from(err: Error) -> Self {
        match err {
            Error::ProtoConversion(e) => {
                tonic::Status::internal(format!("proto conversion error: {e}"))
            }
            Error::Server(msg) => tonic::Status::internal(format!("server error: {msg}")),
            Error::Signature(msg) => {
                tonic::Status::internal(format!("signature conversion error: {msg}"))
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
//
// These masks specify which fields to request from the server. Users can
// provide custom masks to optimize bandwidth, but must include the required
// fields for SDK type deserialization.
//
// If `None` is passed, these defaults are used. If a custom mask is provided,
// it completely replaces the default (no merging).

/// Default field mask for [`crate::Client::get_transactions`].
///
/// **Required fields for `TransactionResponse` deserialization:**
/// - `transaction.bcs` - Transaction data (required)
/// - `signatures` - User signatures (required)
/// - `effects.bcs` - Transaction effects (required)
///
/// **Optional fields:**
/// - `events` - Transaction events
/// - `checkpoint` - Checkpoint sequence number
/// - `timestamp` - Execution timestamp
///
/// If you provide a custom mask, you must include at least `transaction.bcs`,
/// `signatures`, and `effects.bcs`, or deserialization will fail.
pub const TRANSACTIONS_READ_MASK: &str =
    "transaction.bcs,signatures,effects.bcs,events,checkpoint,timestamp";

/// Default field mask for [`crate::Client::get_objects`].
///
/// **Required fields for `Object` deserialization:**
/// - `bcs` - Object BCS data (required)
///
/// If you provide a custom mask, you must include `bcs`, or deserialization
/// will fail.
pub const OBJECTS_READ_MASK: &str = "bcs";

/// Default field mask for checkpoint data queries.
///
/// **Required fields for `CheckpointResponse` deserialization:**
/// - `summary` - Checkpoint summary (required)
///
/// **Optional fields:**
/// - `contents` - Checkpoint contents
/// - `signature` - Checkpoint signature
/// - `transactions` - Executed transactions
/// - `events` - Transaction events
///
/// If you provide a custom mask, you must include at least `summary`,
/// or deserialization will fail.
pub const CHECKPOINT_DATA_READ_MASK: &str = "summary,contents,signature,transactions,events";

/// Default field mask for [`crate::Client::execute_transaction`] and
/// [`crate::Client::simulate_transaction`].
///
/// **Required fields for response deserialization:**
/// - `transaction.effects` - Transaction effects (required)
///
/// **Optional fields:**
/// - `transaction.events` - Transaction events
/// - `transaction.input_objects` - Input objects used
/// - `transaction.output_objects` - Output objects created/modified
///
/// If you provide a custom mask, you must include at least
/// `transaction.effects`, or deserialization will fail.
pub const EXECUTION_READ_MASK: &str =
    "transaction.effects,transaction.events,transaction.input_objects,transaction.output_objects";

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
            Some(object_result::Result::Error(e)) => Err(Error::Server(e.message)),
            None => Err(TryFromProtoError::missing("result").into()),
        }
    }
}

impl ProtoResult for TransactionResult {
    type Value = ExecutedTransaction;

    fn into_result(self) -> Result<Self::Value> {
        match self.result {
            Some(transaction_result::Result::Transaction(tx)) => Ok(tx),
            Some(transaction_result::Result::Error(e)) => Err(Error::Server(e.message)),
            None => Err(TryFromProtoError::missing("result").into()),
        }
    }
}

/// Build a proto Transaction from serializable transaction data and digest.
pub fn build_proto_transaction<T: Serialize>(data: &T, digest: Digest) -> Result<ProtoTransaction> {
    let bcs = BcsData::serialize(data)
        .map_err(|e| Error::from(TryFromProtoError::invalid("transaction", e)))?;

    Ok(ProtoTransaction {
        digest: Some(digest.into()),
        bcs: Some(bcs),
    })
}

/// Build a field mask with a custom value or default.
///
/// This is a convenience helper that handles the common pattern of using
/// a user-provided field mask or falling back to a default.
pub fn field_mask_with_default(custom: Option<&str>, default: &str) -> FieldMask {
    FieldMask::from_str(custom.unwrap_or(default))
}
