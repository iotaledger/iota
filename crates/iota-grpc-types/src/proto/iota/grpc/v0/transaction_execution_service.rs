// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

include!("../../../generated/iota.grpc.v0.transaction_execution_service.rs");
include!("../../../generated/iota.grpc.v0.transaction_execution_service.field_info.rs");
include!("../../../generated/iota.grpc.v0.transaction_execution_service.accessors.rs");

use crate::proto::{TryFromProtoError, get_inner_field};

// ExecuteTransactionResponse
//

impl ExecuteTransactionResponse {
    /// Get the transaction digest.
    pub fn digest(&self) -> Result<iota_sdk_types::Digest, TryFromProtoError> {
        get_inner_field!(self.transaction, Self::TRANSACTION_FIELD, digest)
    }

    /// Deserialize the transaction.
    pub fn transaction(&self) -> Result<iota_sdk_types::Transaction, TryFromProtoError> {
        get_inner_field!(self.transaction, Self::TRANSACTION_FIELD, transaction)
    }

    /// Deserialize the transaction effects.
    pub fn effects(&self) -> Result<iota_sdk_types::TransactionEffects, TryFromProtoError> {
        get_inner_field!(self.transaction, Self::TRANSACTION_FIELD, effects)
    }

    /// Get the effects digest.
    pub fn effects_digest(&self) -> Result<iota_sdk_types::Digest, TryFromProtoError> {
        get_inner_field!(self.transaction, Self::TRANSACTION_FIELD, effects_digest)
    }

    /// Deserialize the transaction events.
    pub fn events(&self) -> Result<iota_sdk_types::TransactionEvents, TryFromProtoError> {
        get_inner_field!(self.transaction, Self::TRANSACTION_FIELD, events)
    }

    /// Get the events digest directly.
    pub fn events_digest(&self) -> Result<iota_sdk_types::Digest, TryFromProtoError> {
        get_inner_field!(self.transaction, Self::TRANSACTION_FIELD, events_digest)
    }

    /// Get checkpoint sequence number.
    pub fn checkpoint_sequence_number(&self) -> Result<u64, TryFromProtoError> {
        get_inner_field!(
            self.transaction,
            Self::TRANSACTION_FIELD,
            checkpoint_sequence_number
        )
    }

    /// Get timestamp in milliseconds.
    pub fn timestamp_ms(&self) -> Result<u64, TryFromProtoError> {
        get_inner_field!(self.transaction, Self::TRANSACTION_FIELD, timestamp_ms)
    }

    /// Get the raw BCS bytes of the transaction.
    pub fn transaction_bcs(&self) -> Result<&[u8], TryFromProtoError> {
        get_inner_field!(self.transaction, Self::TRANSACTION_FIELD, transaction_bcs)
    }

    /// Get the raw BCS bytes of the transaction effects.
    pub fn effects_bcs(&self) -> Result<&[u8], TryFromProtoError> {
        get_inner_field!(self.transaction, Self::TRANSACTION_FIELD, effects_bcs)
    }

    /// Get input objects.
    pub fn input_objects(&self) -> Result<&super::object::Objects, TryFromProtoError> {
        get_inner_field!(self.transaction, Self::TRANSACTION_FIELD, input_objects)
    }

    /// Get output objects.
    pub fn output_objects(&self) -> Result<&super::object::Objects, TryFromProtoError> {
        get_inner_field!(self.transaction, Self::TRANSACTION_FIELD, output_objects)
    }

    /// Deserialize user signatures.
    pub fn signatures(&self) -> Result<Vec<iota_sdk_types::UserSignature>, TryFromProtoError> {
        get_inner_field!(self.transaction, Self::TRANSACTION_FIELD, signatures)
    }
}

// SimulateTransactionResponse
//

impl SimulateTransactionResponse {
    pub fn executed_transaction(
        &self,
    ) -> Result<&super::transaction::ExecutedTransaction, TryFromProtoError> {
        self.transaction
            .as_ref()
            .ok_or_else(|| TryFromProtoError::missing(Self::TRANSACTION_FIELD.name))
    }

    pub fn command_results(&self) -> Result<&super::command::CommandResults, TryFromProtoError> {
        self.command_results
            .as_ref()
            .ok_or_else(|| TryFromProtoError::missing(Self::COMMAND_RESULTS_FIELD.name))
    }

    /// Get all mutated-by-reference arguments from command results.
    ///
    /// Returns intermediate results from executing each command in a
    /// programmable transaction. Each outer vector element corresponds to a
    /// command, and each inner vector contains the arguments mutated by that
    /// command.
    pub fn command_mutated_by_ref_arguments(
        &self,
    ) -> Result<Vec<Vec<iota_sdk_types::transaction::Argument>>, TryFromProtoError> {
        get_inner_field!(
            self.command_results,
            Self::COMMAND_RESULTS_FIELD,
            all_mutated_by_ref_arguments
        )
    }

    /// Get all return value arguments from command results.
    ///
    /// Returns the return values from executing each command in a programmable
    /// transaction. Each outer vector element corresponds to a command.
    pub fn command_return_values_arguments(
        &self,
    ) -> Result<Vec<Vec<iota_sdk_types::transaction::Argument>>, TryFromProtoError> {
        get_inner_field!(
            self.command_results,
            Self::COMMAND_RESULTS_FIELD,
            all_return_values_arguments
        )
    }

    /// Get all mutated-by-reference type tags from command results.
    pub fn command_mutated_by_ref_type_tags(
        &self,
    ) -> Result<Vec<Vec<iota_sdk_types::TypeTag>>, TryFromProtoError> {
        get_inner_field!(
            self.command_results,
            Self::COMMAND_RESULTS_FIELD,
            all_mutated_by_ref_type_tags
        )
    }

    /// Get all return value type tags from command results.
    pub fn command_return_values_type_tags(
        &self,
    ) -> Result<Vec<Vec<iota_sdk_types::TypeTag>>, TryFromProtoError> {
        get_inner_field!(
            self.command_results,
            Self::COMMAND_RESULTS_FIELD,
            all_return_values_type_tags
        )
    }
}
