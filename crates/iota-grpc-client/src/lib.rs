// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! gRPC client for IOTA node operations.
//!
//! This crate provides a high-level client for interacting with IOTA nodes
//! via gRPC. It wraps the low-level proto types and provides ergonomic APIs
//! using SDK types from `iota_sdk_types`.
//!
//! # Example
//!
//! ```no_run
//! use iota_grpc_client::Client;
//! use iota_sdk_types::{Digest, ObjectId};
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let client = Client::connect("http://localhost:9000").await?;
//!
//! // Get a transaction with full details (None = use default field mask)
//! let digest: Digest = todo!();
//! let txs = client.get_transactions(&[digest], None).await?;
//! if let Some(tx) = txs.body().first() {
//!     println!("Transaction digest: {:?}", tx.transaction()?.digest()?);
//! }
//!
//! // Get an object (None = use default field mask)
//! let object_id: ObjectId = "0x2".parse()?;
//! let objects = client.get_objects(&[(object_id, None)], None).await?;
//! if let Some(object) = objects.body().first() {
//!     println!("Object version: {:?}", object.object_reference()?.version());
//! }
//! # Ok(())
//! # }
//! ```

pub mod api;

// Re-export all read mask constants (per-method fields)
pub use api::{
    // CheckpointResponse per-method masks
    CHECKPOINT_CONTENTS_BCS,
    CHECKPOINT_CONTENTS_DIGEST,
    CHECKPOINT_RESPONSE_CHECKPOINT_DATA,
    CHECKPOINT_RESPONSE_CONTENTS,
    CHECKPOINT_RESPONSE_EVENTS,
    CHECKPOINT_RESPONSE_EXECUTED_TRANSACTIONS,
    CHECKPOINT_RESPONSE_SIGNATURE,
    CHECKPOINT_RESPONSE_SIGNED_SUMMARY,
    CHECKPOINT_RESPONSE_SUMMARY,
    CHECKPOINT_SUMMARY_BCS,
    CHECKPOINT_SUMMARY_DIGEST,
    // Event per-method masks
    EVENT_BCS,
    EVENT_BCS_CONTENTS,
    EVENT_JSON_CONTENTS,
    EVENT_MODULE,
    EVENT_PACKAGE_ID,
    EVENT_SENDER,
    EVENT_TYPE,
    // ExecutedTransaction per-method masks
    EXECUTED_TRANSACTION_CHECKPOINT,
    EXECUTED_TRANSACTION_EFFECTS,
    EXECUTED_TRANSACTION_EVENTS,
    EXECUTED_TRANSACTION_INPUT_OBJECTS,
    EXECUTED_TRANSACTION_OUTPUT_OBJECTS,
    EXECUTED_TRANSACTION_SIGNATURES,
    EXECUTED_TRANSACTION_TIMESTAMP,
    EXECUTED_TRANSACTION_TRANSACTION,
    // ExecutionError sub-fields
    EXECUTION_ERROR_BCS_KIND,
    EXECUTION_ERROR_COMMAND_INDEX,
    EXECUTION_ERROR_SOURCE,
    // Object per-method masks
    OBJECT_BCS,
    OBJECT_REFERENCE,
    // SimulatedTransaction per-method masks
    SIMULATED_TRANSACTION_EXECUTED_TRANSACTION,
    SIMULATED_TRANSACTION_EXECUTION_RESULT,
    SIMULATED_TRANSACTION_SUGGESTED_GAS_PRICE,
    // Transaction / Effects / Events sub-fields
    TRANSACTION_BCS,
    TRANSACTION_DIGEST,
    TRANSACTION_EFFECTS_BCS,
    TRANSACTION_EFFECTS_DIGEST,
    TRANSACTION_EVENTS_BCS,
    TRANSACTION_EVENTS_DIGEST,
};
// Re-export types for convenience
pub use api::{
    CheckpointResponse, CheckpointStreamItem, Error, MetadataEnvelope, Page, Result, RpcStatus,
    execution::simulate::SimulateTransactionInput,
};
// Re-export all read mask constants (endpoint defaults)
pub use api::{
    // Endpoint defaults
    EXECUTE_TRANSACTIONS_READ_MASK,
    GET_CHECKPOINT_READ_MASK,
    GET_EPOCH_READ_MASK,
    GET_OBJECTS_READ_MASK,
    GET_SERVICE_INFO_READ_MASK,
    GET_TRANSACTIONS_READ_MASK,
    LIST_DYNAMIC_FIELDS_READ_MASK,
    LIST_OWNED_OBJECTS_READ_MASK,
    SIMULATE_TRANSACTIONS_READ_MASK,
};
// Re-export query builders for convenience
pub use api::{
    move_package::package_versions::ListPackageVersionsQuery,
    state::{dynamic_fields::ListDynamicFieldsQuery, owned_objects::ListOwnedObjectsQuery},
};

mod client;
pub use client::{Client, InterceptedChannel};

mod response_ext;
pub use response_ext::ResponseExt;

mod interceptors;
pub use interceptors::HeadersInterceptor;
