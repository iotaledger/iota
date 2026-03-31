// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

include!("../../../generated/iota.grpc.v1.ledger_service.rs");
include!("../../../generated/iota.grpc.v1.ledger_service.field_info.rs");
include!("../../../generated/iota.grpc.v1.ledger_service.accessors.rs");

use crate::proto::{TryFromProtoError, get_inner_field};

// GetServiceInfoResponse
//

impl GetServiceInfoResponse {
    /// Get the chain identifier (digest of genesis checkpoint).
    pub fn chain_identifier(&self) -> Result<iota_sdk_types::Digest, TryFromProtoError> {
        get_inner_field!(self.chain_id, Self::CHAIN_ID_FIELD, try_into)
    }

    /// Get the human-readable chain name (e.g., "mainnet", "testnet").
    pub fn chain_name(&self) -> Result<&str, TryFromProtoError> {
        self.chain
            .as_deref()
            .ok_or_else(|| TryFromProtoError::missing(Self::CHAIN_FIELD.name))
    }

    /// Get the current epoch ID.
    pub fn epoch_id(&self) -> Result<iota_sdk_types::EpochId, TryFromProtoError> {
        self.epoch
            .ok_or_else(|| TryFromProtoError::missing(Self::EPOCH_FIELD.name))
    }

    /// Get the checkpoint height of the most recently executed checkpoint.
    pub fn checkpoint_executed(
        &self,
    ) -> Result<iota_sdk_types::CheckpointSequenceNumber, TryFromProtoError> {
        self.executed_checkpoint_height
            .ok_or_else(|| TryFromProtoError::missing(Self::EXECUTED_CHECKPOINT_HEIGHT_FIELD.name))
    }

    /// Get the Unix timestamp of the most recently executed checkpoint in
    /// milliseconds.
    pub fn checkpoint_executed_timestamp_ms(
        &self,
    ) -> Result<iota_sdk_types::CheckpointTimestamp, TryFromProtoError> {
        let ts = self.executed_checkpoint_timestamp.ok_or_else(|| {
            TryFromProtoError::missing(Self::EXECUTED_CHECKPOINT_TIMESTAMP_FIELD.name)
        })?;
        crate::proto::proto_to_timestamp_ms(ts)
            .map_err(|e| e.nested(Self::EXECUTED_CHECKPOINT_TIMESTAMP_FIELD.name))
    }

    /// Get the lowest checkpoint for which checkpoints and transaction data are
    /// available.
    pub fn checkpoint_lowest(
        &self,
    ) -> Result<iota_sdk_types::CheckpointSequenceNumber, TryFromProtoError> {
        self.lowest_available_checkpoint
            .ok_or_else(|| TryFromProtoError::missing(Self::LOWEST_AVAILABLE_CHECKPOINT_FIELD.name))
    }

    /// Get the lowest checkpoint for which object data is available.
    pub fn checkpoint_objects_lowest(
        &self,
    ) -> Result<iota_sdk_types::CheckpointSequenceNumber, TryFromProtoError> {
        self.lowest_available_checkpoint_objects.ok_or_else(|| {
            TryFromProtoError::missing(Self::LOWEST_AVAILABLE_CHECKPOINT_OBJECTS_FIELD.name)
        })
    }

    /// Get the software version of the service.
    pub fn server_version(&self) -> Result<&str, TryFromProtoError> {
        self.server
            .as_deref()
            .ok_or_else(|| TryFromProtoError::missing(Self::SERVER_FIELD.name))
    }
}

// ObjectResult
//

impl ObjectResult {
    /// Get the object if this result is an object.
    pub fn object(&self) -> Result<Option<&super::object::Object>, TryFromProtoError> {
        match &self.result {
            Some(object_result::Result::Object(obj)) => Ok(Some(obj)),
            _ => Ok(None),
        }
    }

    /// Get the error code if this result is an error.
    pub fn error_code(&self) -> Option<i32> {
        match &self.result {
            Some(object_result::Result::Error(status)) => Some(status.code),
            _ => None,
        }
    }

    /// Get the error message if this result is an error.
    pub fn error_message(&self) -> Option<&str> {
        match &self.result {
            Some(object_result::Result::Error(status)) => Some(&status.message),
            _ => None,
        }
    }
}

// TransactionResult
//

impl TransactionResult {
    /// Get the executed transaction if this result is a transaction.
    pub fn executed_transaction(
        &self,
    ) -> Result<Option<&super::transaction::ExecutedTransaction>, TryFromProtoError> {
        match &self.result {
            Some(transaction_result::Result::ExecutedTransaction(tx)) => Ok(Some(tx)),
            _ => Ok(None),
        }
    }

    /// Get the error code if this result is an error.
    pub fn error_code(&self) -> Option<i32> {
        match &self.result {
            Some(transaction_result::Result::Error(status)) => Some(status.code),
            _ => None,
        }
    }

    /// Get the error message if this result is an error.
    pub fn error_message(&self) -> Option<&str> {
        match &self.result {
            Some(transaction_result::Result::Error(status)) => Some(&status.message),
            _ => None,
        }
    }
}

// GetEpochResponse
//

impl GetEpochResponse {
    pub fn epoch(&self) -> Result<&crate::v1::epoch::Epoch, TryFromProtoError> {
        self.epoch
            .as_ref()
            .ok_or_else(|| TryFromProtoError::missing(Self::EPOCH_FIELD.name))
    }
}
