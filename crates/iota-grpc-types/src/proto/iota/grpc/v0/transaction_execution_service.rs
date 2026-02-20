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
    pub fn executed_transaction(
        &self,
    ) -> Result<&crate::v0::transaction::ExecutedTransaction, TryFromProtoError> {
        self.transaction
            .as_ref()
            .ok_or_else(|| TryFromProtoError::missing(Self::TRANSACTION_FIELD.name))
    }
}

// SimulateTransactionResponse
//

impl SimulateTransactionResponse {
    pub fn executed_transaction(
        &self,
    ) -> Result<&crate::v0::transaction::ExecutedTransaction, TryFromProtoError> {
        self.transaction
            .as_ref()
            .ok_or_else(|| TryFromProtoError::missing(Self::TRANSACTION_FIELD.name))
    }

    /// Get the suggested gas price.
    pub fn gas_price_suggested(&self) -> Result<u64, TryFromProtoError> {
        self.suggested_gas_price
            .ok_or_else(|| TryFromProtoError::missing(Self::SUGGESTED_GAS_PRICE_FIELD.name))
    }

    /// Get the execution result.
    pub fn execution_result(
        &self,
    ) -> Result<&crate::v0::transaction::ExecutionResult, TryFromProtoError> {
        self.result
            .as_ref()
            .ok_or_else(|| TryFromProtoError::missing(Self::RESULT_FIELD.name))
    }
}
