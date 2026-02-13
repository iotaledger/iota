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
//! if let Some(tx) = txs.first() {
//!     println!("Transaction digest: {:?}", tx.transaction()?.digest()?);
//! }
//!
//! // Get an object (None = use default field mask)
//! let object_id: ObjectId = "0x2".parse()?;
//! let objects = client.get_objects(&[(object_id, None)], None).await?;
//! if let Some(object) = objects.first() {
//!     println!("Object version: {:?}", object.object_reference()?.version());
//! }
//! # Ok(())
//! # }
//! ```

pub mod api;

// Re-export types for convenience
pub use api::{
    CHECKPOINT_READ_MASK, CheckpointResponse, EPOCH_READ_MASK, EXECUTION_READ_MASK, Error,
    OBJECTS_READ_MASK, Result, SERVICE_INFO_READ_MASK, TRANSACTIONS_READ_MASK,
};

mod client;
pub use client::{Client, InterceptedChannel};

mod response_ext;
pub use response_ext::ResponseExt;

mod interceptors;
pub use interceptors::HeadersInterceptor;
