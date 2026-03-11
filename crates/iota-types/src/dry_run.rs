// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Native dry-run transaction response type.
//!
//! This replaces `iota-json-rpc-types::DryRunTransactionBlockResponse` so that
//! `iota-core` does not need to depend on JSON-RPC presentation types.
//! Conversion to RPC-specific response formats (JSON-RPC, gRPC) happens at the
//! API boundary layer.

use serde::{Deserialize, Serialize};

use crate::{
    effects::{TransactionEffects, TransactionEvents},
    transaction::TransactionData,
};

/// Result of a dry-run transaction execution.
///
/// Fields like `object_changes` and `balance_changes` are deliberately omitted
/// because they are computed from effects at the RPC layer (see the original
/// comment in authority.rs: "Returning empty vector here because we recalculate
/// changes in the rpc layer").
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DryRunTransactionBlockResponse {
    pub effects: TransactionEffects,
    pub events: TransactionEvents,
    pub input: TransactionData,
    /// If an input object is congested, suggest a gas price to use.
    pub suggested_gas_price: Option<u64>,
    /// Source of execution error if the transaction failed.
    pub execution_error_source: Option<String>,
}
