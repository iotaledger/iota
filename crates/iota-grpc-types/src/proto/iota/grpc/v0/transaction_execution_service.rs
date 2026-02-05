// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

include!("../../../generated/iota.grpc.v0.transaction_execution_service.rs");
include!("../../../generated/iota.grpc.v0.transaction_execution_service.field_info.rs");

use crate::proto::TryFromProtoError;

/// Macro to implement common transaction response methods for types that wrap
/// an `ExecutedTransaction`.
///
/// Both `ExecuteTransactionResponse` and `SimulateTransactionResponse` have a
/// `transaction: Option<ExecutedTransaction>` field and expose nearly identical
/// accessor methods. This macro generates those common implementations.
macro_rules! impl_transaction_response_getters {
    ($response_type:ty) => {
        impl $response_type {
            /// Get the transaction digest.
            pub fn digest(&self) -> Result<iota_sdk_types::Digest, TryFromProtoError> {
                self.transaction
                    .as_ref()
                    .ok_or_else(|| TryFromProtoError::missing(Self::TRANSACTION_FIELD.name))?
                    .digest()
                    .map_err(|e| e.nested(Self::TRANSACTION_FIELD.name))
            }

            /// Deserialize the transaction.
            pub fn transaction(&self) -> Result<iota_sdk_types::Transaction, TryFromProtoError> {
                self.transaction
                    .as_ref()
                    .ok_or_else(|| TryFromProtoError::missing(Self::TRANSACTION_FIELD.name))?
                    .transaction()
                    .map_err(|e| e.nested(Self::TRANSACTION_FIELD.name))
            }

            /// Deserialize the transaction effects.
            pub fn effects(&self) -> Result<iota_sdk_types::TransactionEffects, TryFromProtoError> {
                self.transaction
                    .as_ref()
                    .ok_or_else(|| TryFromProtoError::missing(Self::TRANSACTION_FIELD.name))?
                    .effects()
                    .map_err(|e| e.nested(Self::TRANSACTION_FIELD.name))
            }

            /// Get the effects digest.
            pub fn effects_digest(&self) -> Result<iota_sdk_types::Digest, TryFromProtoError> {
                self.transaction
                    .as_ref()
                    .ok_or_else(|| TryFromProtoError::missing(Self::TRANSACTION_FIELD.name))?
                    .effects_digest()
                    .map_err(|e| e.nested(Self::TRANSACTION_FIELD.name))
            }

            /// Deserialize the transaction events.
            ///
            /// Returns `Ok(None)` if events were not included in the response.
            pub fn events(
                &self,
            ) -> Result<Option<iota_sdk_types::TransactionEvents>, TryFromProtoError> {
                self.transaction
                    .as_ref()
                    .ok_or_else(|| TryFromProtoError::missing(Self::TRANSACTION_FIELD.name))?
                    .events()
                    .map_err(|e| e.nested(Self::TRANSACTION_FIELD.name))
            }

            /// Get the events digest directly.
            ///
            /// Returns `Ok(None)` if events were not included in the response.
            pub fn events_digest(
                &self,
            ) -> Result<Option<iota_sdk_types::Digest>, TryFromProtoError> {
                self.transaction
                    .as_ref()
                    .ok_or_else(|| TryFromProtoError::missing(Self::TRANSACTION_FIELD.name))?
                    .events_digest()
                    .map_err(|e| e.nested(Self::TRANSACTION_FIELD.name))
            }

            /// Get checkpoint sequence number.
            pub fn checkpoint_sequence_number(&self) -> Result<u64, TryFromProtoError> {
                self.transaction
                    .as_ref()
                    .ok_or_else(|| TryFromProtoError::missing(Self::TRANSACTION_FIELD.name))?
                    .checkpoint_sequence_number()
                    .map_err(|e| e.nested(Self::TRANSACTION_FIELD.name))
            }

            /// Get timestamp in milliseconds.
            pub fn timestamp_ms(&self) -> Result<u64, TryFromProtoError> {
                self.transaction
                    .as_ref()
                    .ok_or_else(|| TryFromProtoError::missing(Self::TRANSACTION_FIELD.name))?
                    .timestamp_ms()
                    .map_err(|e| e.nested(Self::TRANSACTION_FIELD.name))
            }

            /// Get the raw BCS bytes of the transaction.
            pub fn transaction_bcs(&self) -> Result<&[u8], TryFromProtoError> {
                self.transaction
                    .as_ref()
                    .ok_or_else(|| TryFromProtoError::missing(Self::TRANSACTION_FIELD.name))?
                    .transaction_bcs()
                    .map_err(|e| e.nested(Self::TRANSACTION_FIELD.name))
            }

            /// Get the raw BCS bytes of the transaction effects.
            pub fn effects_bcs(&self) -> Result<&[u8], TryFromProtoError> {
                self.transaction
                    .as_ref()
                    .ok_or_else(|| TryFromProtoError::missing(Self::TRANSACTION_FIELD.name))?
                    .effects_bcs()
                    .map_err(|e| e.nested(Self::TRANSACTION_FIELD.name))
            }

            /// Deserialize input objects.
            ///
            /// Returns `Ok(None)` if input objects were not included in the response.
            pub fn input_objects(
                &self,
            ) -> Result<Option<Vec<iota_sdk_types::Object>>, TryFromProtoError> {
                self.transaction
                    .as_ref()
                    .ok_or_else(|| TryFromProtoError::missing(Self::TRANSACTION_FIELD.name))?
                    .input_objects()
                    .map_err(|e| e.nested(Self::TRANSACTION_FIELD.name))
            }

            /// Deserialize output objects.
            ///
            /// Returns `Ok(None)` if output objects were not included in the response.
            pub fn output_objects(
                &self,
            ) -> Result<Option<Vec<iota_sdk_types::Object>>, TryFromProtoError> {
                self.transaction
                    .as_ref()
                    .ok_or_else(|| TryFromProtoError::missing(Self::TRANSACTION_FIELD.name))?
                    .output_objects()
                    .map_err(|e| e.nested(Self::TRANSACTION_FIELD.name))
            }
        }
    };
}

// ExecuteTransactionResponse
//

impl_transaction_response_getters!(ExecuteTransactionResponse);

impl ExecuteTransactionResponse {
    /// Deserialize user signatures.
    ///
    /// Note: This method is only available on `ExecuteTransactionResponse`,
    /// not on `SimulateTransactionResponse`, because simulated transactions
    /// do not include signatures.
    pub fn signatures(&self) -> Result<Vec<iota_sdk_types::UserSignature>, TryFromProtoError> {
        self.transaction
            .as_ref()
            .ok_or_else(|| TryFromProtoError::missing(Self::TRANSACTION_FIELD.name))?
            .signatures()
            .map_err(|e| e.nested(Self::TRANSACTION_FIELD.name))
    }
}

// SimulateTransactionResponse
//

impl_transaction_response_getters!(SimulateTransactionResponse);

impl SimulateTransactionResponse {
    /// Get all mutated-by-reference arguments from command results.
    ///
    /// Returns intermediate results from executing each command in a
    /// programmable transaction. Each outer vector element corresponds to a
    /// command, and each inner vector contains the arguments mutated by that
    /// command.
    pub fn command_mutated_by_ref_arguments(
        &self,
    ) -> Result<Vec<Vec<iota_sdk_types::transaction::Argument>>, TryFromProtoError> {
        self.command_results
            .as_ref()
            .ok_or_else(|| TryFromProtoError::missing(Self::COMMAND_RESULTS_FIELD.name))?
            .all_mutated_by_ref_arguments()
            .map_err(|e| e.nested(Self::COMMAND_RESULTS_FIELD.name))
    }

    /// Get all return value arguments from command results.
    ///
    /// Returns the return values from executing each command in a programmable
    /// transaction. Each outer vector element corresponds to a command.
    pub fn command_return_values_arguments(
        &self,
    ) -> Result<Vec<Vec<iota_sdk_types::transaction::Argument>>, TryFromProtoError> {
        self.command_results
            .as_ref()
            .ok_or_else(|| TryFromProtoError::missing(Self::COMMAND_RESULTS_FIELD.name))?
            .all_return_values_arguments()
            .map_err(|e| e.nested(Self::COMMAND_RESULTS_FIELD.name))
    }

    /// Get all mutated-by-reference type tags from command results.
    pub fn command_mutated_by_ref_type_tags(
        &self,
    ) -> Result<Vec<Vec<iota_sdk_types::TypeTag>>, TryFromProtoError> {
        self.command_results
            .as_ref()
            .ok_or_else(|| TryFromProtoError::missing(Self::COMMAND_RESULTS_FIELD.name))?
            .all_mutated_by_ref_type_tags()
            .map_err(|e| e.nested(Self::COMMAND_RESULTS_FIELD.name))
    }

    /// Get all return value type tags from command results.
    pub fn command_return_values_type_tags(
        &self,
    ) -> Result<Vec<Vec<iota_sdk_types::TypeTag>>, TryFromProtoError> {
        self.command_results
            .as_ref()
            .ok_or_else(|| TryFromProtoError::missing(Self::COMMAND_RESULTS_FIELD.name))?
            .all_return_values_type_tags()
            .map_err(|e| e.nested(Self::COMMAND_RESULTS_FIELD.name))
    }
}
