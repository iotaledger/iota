// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

include!("../../../generated/iota.grpc.v1.transaction_execution_service.rs");
include!("../../../generated/iota.grpc.v1.transaction_execution_service.field_info.rs");
include!("../../../generated/iota.grpc.v1.transaction_execution_service.accessors.rs");

use crate::proto::TryFromProtoError;

// ExecuteTransactionResult
//

impl ExecuteTransactionResult {
    /// Get the executed transaction from the result, if it succeeded.
    ///
    /// The read mask paths in the request apply directly to
    /// [`ExecutedTransaction`](crate::v1::transaction::ExecutedTransaction)
    /// fields (e.g. `"effects"`, not `"executed_transaction.effects"`).
    pub fn executed_transaction(&self) -> Option<&crate::v1::transaction::ExecutedTransaction> {
        match &self.result {
            Some(execute_transaction_result::Result::ExecutedTransaction(tx)) => Some(tx),
            _ => None,
        }
    }

    /// Get the error from the result, if execution failed.
    pub fn error(&self) -> Option<&crate::google::rpc::Status> {
        match &self.result {
            Some(execute_transaction_result::Result::Error(e)) => Some(e),
            _ => None,
        }
    }
}

// ExecutionError
//

impl ExecutionError {
    /// Deserialize the execution error kind from BCS.
    ///
    /// **Read mask:** `"execution_result.execution_error.bcs_kind"` (see
    /// [`EXECUTION_ERROR_BCS_KIND`]).
    ///
    /// [`EXECUTION_ERROR_BCS_KIND`]: crate::read_masks::EXECUTION_ERROR_BCS_KIND
    pub fn error_kind(&self) -> Result<iota_sdk_types::ExecutionError, TryFromProtoError> {
        self.bcs_kind
            .as_ref()
            .ok_or_else(|| TryFromProtoError::missing(Self::BCS_KIND_FIELD.name))
            .and_then(|bcs| {
                bcs.deserialize()
                    .map_err(|e| TryFromProtoError::invalid(Self::BCS_KIND_FIELD.name, e))
            })
    }

    /// Get the error source (human-readable description).
    ///
    /// **Read mask:** `"execution_result.execution_error.source"` (see
    /// [`EXECUTION_ERROR_SOURCE`]).
    ///
    /// [`EXECUTION_ERROR_SOURCE`]: crate::read_masks::EXECUTION_ERROR_SOURCE
    pub fn error_source(&self) -> Result<String, TryFromProtoError> {
        self.source
            .clone()
            .ok_or_else(|| TryFromProtoError::missing(Self::SOURCE_FIELD.name))
    }

    /// Get the index of the command that caused the error.
    ///
    /// **Read mask:** `"execution_result.execution_error.command_index"` (see
    /// [`EXECUTION_ERROR_COMMAND_INDEX`]).
    ///
    /// [`EXECUTION_ERROR_COMMAND_INDEX`]: crate::read_masks::EXECUTION_ERROR_COMMAND_INDEX
    pub fn error_command_index(&self) -> Result<u64, TryFromProtoError> {
        self.command_index
            .ok_or_else(|| TryFromProtoError::missing(Self::COMMAND_INDEX_FIELD.name))
    }
}

// SimulatedTransaction
//

impl SimulatedTransaction {
    /// Get the simulated executed transaction.
    ///
    /// Returns the
    /// [`ExecutedTransaction`](crate::v1::transaction::ExecutedTransaction)
    /// with sub-fields populated according to the read mask. Use paths like
    /// `"executed_transaction.effects"` to request specific sub-fields.
    ///
    /// **Read mask:** `"executed_transaction"` (see
    /// [`SIMULATED_TRANSACTION_EXECUTED_TRANSACTION`]).
    ///
    /// [`SIMULATED_TRANSACTION_EXECUTED_TRANSACTION`]: crate::read_masks::SIMULATED_TRANSACTION_EXECUTED_TRANSACTION
    pub fn executed_transaction(
        &self,
    ) -> Result<&crate::v1::transaction::ExecutedTransaction, TryFromProtoError> {
        self.executed_transaction
            .as_ref()
            .ok_or_else(|| TryFromProtoError::missing(Self::EXECUTED_TRANSACTION_FIELD.name))
    }

    /// Get the suggested gas price (in NANOS).
    ///
    /// **Read mask:** `"suggested_gas_price"` (see
    /// [`SIMULATED_TRANSACTION_SUGGESTED_GAS_PRICE`]).
    ///
    /// [`SIMULATED_TRANSACTION_SUGGESTED_GAS_PRICE`]: crate::read_masks::SIMULATED_TRANSACTION_SUGGESTED_GAS_PRICE
    pub fn gas_price_suggested(&self) -> Result<u64, TryFromProtoError> {
        self.suggested_gas_price
            .ok_or_else(|| TryFromProtoError::missing(Self::SUGGESTED_GAS_PRICE_FIELD.name))
    }

    /// Get the execution result (command results on success, execution error on
    /// failure).
    ///
    /// **Read mask:** `"execution_result"` (see
    /// [`SIMULATED_TRANSACTION_EXECUTION_RESULT`]).
    ///
    /// [`SIMULATED_TRANSACTION_EXECUTION_RESULT`]: crate::read_masks::SIMULATED_TRANSACTION_EXECUTION_RESULT
    pub fn execution_result(
        &self,
    ) -> Result<
        &crate::v1::transaction_execution_service::simulated_transaction::ExecutionResult,
        TryFromProtoError,
    > {
        self.execution_result
            .as_ref()
            .ok_or_else(|| TryFromProtoError::missing(Self::EXECUTION_RESULT_ONEOF))
    }

    /// Returns `Some` if the simulation succeeded with command results, `None`
    /// otherwise.
    ///
    /// **Read mask:** `"execution_result"` (see
    /// [`SIMULATED_TRANSACTION_EXECUTION_RESULT`]).
    ///
    /// [`SIMULATED_TRANSACTION_EXECUTION_RESULT`]: crate::read_masks::SIMULATED_TRANSACTION_EXECUTION_RESULT
    pub fn command_results(&self) -> Option<&crate::v1::command::CommandResults> {
        match &self.execution_result {
            Some(crate::v1::transaction_execution_service::simulated_transaction::ExecutionResult::CommandResults(r)) => Some(r),
            _ => None,
        }
    }

    /// Returns `Some` if the simulation failed with an execution error, `None`
    /// otherwise.
    ///
    /// **Read mask:** `"execution_result"` (see
    /// [`SIMULATED_TRANSACTION_EXECUTION_RESULT`]).
    ///
    /// [`SIMULATED_TRANSACTION_EXECUTION_RESULT`]: crate::read_masks::SIMULATED_TRANSACTION_EXECUTION_RESULT
    pub fn execution_error(
        &self,
    ) -> Option<&crate::v1::transaction_execution_service::ExecutionError> {
        match &self.execution_result {
            Some(crate::v1::transaction_execution_service::simulated_transaction::ExecutionResult::ExecutionError(e)) => Some(e),
            _ => None,
        }
    }
}

// SimulateTransactionResult
//

impl SimulateTransactionResult {
    /// Get the simulated transaction from the result, if it succeeded.
    pub fn simulated_transaction(&self) -> Option<&SimulatedTransaction> {
        match &self.result {
            Some(simulate_transaction_result::Result::SimulatedTransaction(tx)) => Some(tx),
            _ => None,
        }
    }

    /// Get the error from the result, if simulation failed.
    pub fn error(&self) -> Option<&crate::google::rpc::Status> {
        match &self.result {
            Some(simulate_transaction_result::Result::Error(e)) => Some(e),
            _ => None,
        }
    }
}
