// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

/// Implementation of the BigTableDB client and its R&W operations.
pub(crate) mod client;
/// KV operations metrics.
mod metrics;
/// Data ingestion core ~ProgressStore` implementation.
pub(crate) mod progress_store;
/// Proto definition for BigTableDB communincation trough GRPC.
mod proto;
/// Data ingestion core `Worker` implementation.
pub(crate) mod worker;
