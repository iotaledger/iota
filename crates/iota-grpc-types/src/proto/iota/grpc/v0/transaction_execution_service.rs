// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

include!("../../../generated/iota.grpc.v0.transaction_execution_service.rs");
include!("../../../generated/iota.grpc.v0.transaction_execution_service.field_info.rs");
include!("../../../generated/iota.grpc.v0.transaction_execution_service.accessors.rs");

use crate::proto::TryFromProtoError;

// ExecuteTransactionResponse
//

impl ExecuteTransactionResponse {
    /// Get the executed transaction.
    pub fn executed_transaction(
        &self,
    ) -> Result<&crate::v0::transaction::ExecutedTransaction, TryFromProtoError> {
        self.executed_transaction
            .as_ref()
            .ok_or_else(|| TryFromProtoError::missing(Self::EXECUTED_TRANSACTION_FIELD.name))
    }
}

// ExecutionError
//

impl ExecutionError {
    /// Deserialize the execution error kind from BCS.
    ///
    /// Requires `bcs_kind` in the read_mask.
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
    /// Requires `source` in the read_mask.
    pub fn error_source(&self) -> Result<String, TryFromProtoError> {
        self.source
            .clone()
            .ok_or_else(|| TryFromProtoError::missing(Self::SOURCE_FIELD.name))
    }

    /// Get the index of the command that caused the error.
    ///
    /// Requires `command_index` in the read_mask.
    pub fn error_command_index(&self) -> Result<u64, TryFromProtoError> {
        self.command_index
            .ok_or_else(|| TryFromProtoError::missing(Self::COMMAND_INDEX_FIELD.name))
    }
}

// SimulateTransactionResponse
//

impl SimulateTransactionResponse {
    /// Get the simulated executed transaction.
    ///
    /// Requires `executed_transaction` in the read_mask.
    pub fn executed_transaction(
        &self,
    ) -> Result<&crate::v0::transaction::ExecutedTransaction, TryFromProtoError> {
        self.executed_transaction
            .as_ref()
            .ok_or_else(|| TryFromProtoError::missing(Self::EXECUTED_TRANSACTION_FIELD.name))
    }

    /// Get the suggested gas price.
    ///
    /// Requires `suggested_gas_price` in the read_mask.
    pub fn gas_price_suggested(&self) -> Result<u64, TryFromProtoError> {
        self.suggested_gas_price
            .ok_or_else(|| TryFromProtoError::missing(Self::SUGGESTED_GAS_PRICE_FIELD.name))
    }

    /// Get the execution result (command results on success, execution error on
    /// failure).
    ///
    /// Requires `execution_result` in the read_mask.
    pub fn execution_result(
        &self,
    ) -> Result<
        &crate::v0::transaction_execution_service::simulate_transaction_response::ExecutionResult,
        TryFromProtoError,
    > {
        self.execution_result
            .as_ref()
            .ok_or_else(|| TryFromProtoError::missing(Self::EXECUTION_RESULT_ONEOF))
    }

    /// Returns `Some` if the simulation succeeded with command results, `None`
    /// otherwise.
    pub fn command_results(&self) -> Option<&crate::v0::command::CommandResults> {
        match &self.execution_result {
            Some(crate::v0::transaction_execution_service::simulate_transaction_response::ExecutionResult::CommandResults(r)) => Some(r),
            _ => None,
        }
    }

    /// Returns `Some` if the simulation failed with an execution error, `None`
    /// otherwise.
    pub fn execution_error(
        &self,
    ) -> Option<&crate::v0::transaction_execution_service::ExecutionError> {
        match &self.execution_result {
            Some(crate::v0::transaction_execution_service::simulate_transaction_response::ExecutionResult::ExecutionError(e)) => Some(e),
            _ => None,
        }
    }
}
