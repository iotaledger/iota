// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Native dev-inspect transaction response type.
//!
//! This replaces `iota-json-rpc-types::DevInspectResults` so that `iota-core`
//! does not need to depend on JSON-RPC presentation types. Conversion to
//! RPC-specific response formats happens at the API boundary layer.

use move_core_types::language_storage::TypeTag;
use serde::{Deserialize, Serialize};

use crate::{
    effects::{TransactionEffects, TransactionEvents},
    error::ExecutionError,
    execution::ExecutionResult,
    object::Object,
    transaction::Argument,
};

/// The response from processing a dev inspect transaction.
///
/// Unlike the JSON-RPC version, this stores native types without converting to
/// JSON-RPC wrapper types (no `IotaTypeTag`, `IotaArgument`, etc.). The
/// conversion to presentation types happens at the RPC handler level.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DevInspectResults {
    /// Summary of effects that likely would be generated if the transaction is
    /// actually run.
    pub effects: TransactionEffects,
    /// Events that likely would be generated if the transaction is actually
    /// run.
    pub events: TransactionEvents,
    /// Execution results (including return values) from executing the
    /// transactions. Each entry corresponds to one command in the
    /// programmable transaction.
    pub results: Option<Vec<DevInspectExecutionResult>>,
    /// Execution error from executing the transactions.
    pub error: Option<String>,
    /// The raw transaction data that was dev inspected.
    pub raw_txn_data: Vec<u8>,
    /// The raw effects of the transaction that was dev inspected.
    pub raw_effects: Vec<u8>,
    /// Objects written during execution, needed for resolving event layouts
    /// when newly published packages emit events.
    #[serde(skip)]
    pub output_objects: Vec<Object>,
}

/// Execution result for a single command in a dev-inspect transaction.
///
/// Stores native `Argument` + raw bytes + `TypeTag` without converting to
/// JSON-RPC wrapper types.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DevInspectExecutionResult {
    /// The value of any arguments that were mutably borrowed.
    /// Each entry is (argument, bytes, type_tag).
    pub mutable_reference_outputs: Vec<(Argument, Vec<u8>, TypeTag)>,
    /// The return values from the transaction.
    /// Each entry is (bytes, type_tag).
    pub return_values: Vec<(Vec<u8>, TypeTag)>,
}

impl DevInspectResults {
    pub fn new(
        effects: TransactionEffects,
        events: TransactionEvents,
        return_values: Result<Vec<ExecutionResult>, ExecutionError>,
        raw_txn_data: Vec<u8>,
        raw_effects: Vec<u8>,
        output_objects: Vec<Object>,
    ) -> Self {
        let (results, error) = match return_values {
            Err(e) => (None, Some(e.to_string())),
            Ok(srvs) => {
                let results = srvs
                    .into_iter()
                    .map(
                        |(mutable_reference_outputs, return_values)| DevInspectExecutionResult {
                            mutable_reference_outputs,
                            return_values,
                        },
                    )
                    .collect();
                (Some(results), None)
            }
        };
        Self {
            effects,
            events,
            results,
            error,
            raw_txn_data,
            raw_effects,
            output_objects,
        }
    }
}
