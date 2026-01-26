// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Common utilities shared across API modules.

use iota_grpc_types::v0::{
    bcs::BcsData,
    ledger_service::{ObjectResult, TransactionResult, object_result, transaction_result},
    transaction::{
        ExecutedTransaction, Transaction as ProtoTransaction,
        TransactionEvents as ProtoTransactionEvents,
    },
};
pub use iota_grpc_types::{
    field::{FieldMask, FieldMaskUtil},
    proto::TryFromProtoError,
};
use iota_sdk_types::{Digest, Event, Object, TransactionEffects, TransactionEvents};
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
    type Value = Box<ExecutedTransaction>;

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

/// Convert proto TransactionEffects BCS to SDK TransactionEffects.
fn convert_effects(
    proto: &iota_grpc_types::v0::transaction::TransactionEffects,
) -> Result<TransactionEffects> {
    let bcs = proto
        .bcs
        .as_ref()
        .ok_or(TryFromProtoError::missing("effects.bcs"))?;

    bcs.deserialize()
        .map_err(|e| TryFromProtoError::invalid("effects.bcs", e).into())
}

/// Convert proto TransactionEvents to SDK TransactionEvents.
fn convert_events(proto: &ProtoTransactionEvents) -> Result<Option<TransactionEvents>> {
    let Some(events) = proto.events.as_ref() else {
        return Ok(None);
    };

    let sdk_events: Vec<Event> = events
        .events
        .iter()
        .map(|e| {
            let bcs = e
                .bcs
                .as_ref()
                .ok_or(TryFromProtoError::missing("event.bcs"))?;
            bcs.deserialize()
                .map_err(|e| TryFromProtoError::invalid("event.bcs", e).into())
        })
        .collect::<Result<Vec<_>>>()?;

    Ok(Some(TransactionEvents(sdk_events)))
}

/// Convert a single proto Object to SDK Object.
pub fn convert_object(
    proto: &iota_grpc_types::v0::object::Object,
    field_name: &str,
) -> Result<Object> {
    let bcs = proto
        .bcs
        .as_ref()
        .ok_or(TryFromProtoError::missing(field_name))?;

    bcs.deserialize()
        .map_err(|e| TryFromProtoError::invalid(field_name, e).into())
}

/// Extract only effects and events from a proto ExecutedTransaction.
///
/// This is a lighter alternative to [`extract_execution_data`] for cases
/// where input/output objects are not needed (e.g., transaction queries).
pub fn extract_effects_and_events(
    proto: &ExecutedTransaction,
) -> Result<(TransactionEffects, Option<TransactionEvents>)> {
    let effects = proto
        .effects
        .as_ref()
        .map(convert_effects)
        .transpose()?
        .ok_or(TryFromProtoError::missing("effects"))?;

    let events = proto
        .events
        .as_ref()
        .map(convert_events)
        .transpose()?
        .flatten();

    Ok((effects, events))
}

/// Response for transaction execution.
///
/// Contains the effects, optional events, and optional objects.
#[derive(Debug, Clone)]
pub struct TransactionExecutionResponse {
    /// The effects of executing this transaction.
    pub effects: TransactionEffects,
    /// Events emitted during execution (if requested).
    pub events: Option<TransactionEvents>,
    /// Input objects used by the transaction (if requested).
    pub input_objects: Option<Vec<Object>>,
    /// Output objects created/modified by the transaction (if requested).
    pub output_objects: Option<Vec<Object>>,
}

/// Build a field mask with a custom value or default.
///
/// This is a convenience helper that handles the common pattern of using
/// a user-provided field mask or falling back to a default.
pub fn field_mask_with_default(custom: Option<&str>, default: &str) -> FieldMask {
    FieldMask::from_str(custom.unwrap_or(default))
}

/// Extract execution data from an optional ExecutedTransaction response.
///
/// This is a convenience wrapper that handles the common pattern of extracting
/// the transaction from a response and converting it to SDK types.
pub fn extract_execution_response(
    transaction: Option<ExecutedTransaction>,
) -> Result<TransactionExecutionResponse> {
    let executed = transaction.ok_or(TryFromProtoError::missing("transaction"))?;
    extract_execution_data(&executed)
}

/// Extract execution data from a proto ExecutedTransaction.
pub fn extract_execution_data(proto: &ExecutedTransaction) -> Result<TransactionExecutionResponse> {
    let (effects, events) = extract_effects_and_events(proto)?;

    let input_objects = proto
        .input_objects
        .as_ref()
        .map(|objs| {
            objs.objects
                .iter()
                .map(|o| convert_object(o, "input_object"))
                .collect::<Result<Vec<_>>>()
        })
        .transpose()?;

    let output_objects = proto
        .output_objects
        .as_ref()
        .map(|objs| {
            objs.objects
                .iter()
                .map(|o| convert_object(o, "output_object"))
                .collect::<Result<Vec<_>>>()
        })
        .transpose()?;

    Ok(TransactionExecutionResponse {
        effects,
        events,
        input_objects,
        output_objects,
    })
}
