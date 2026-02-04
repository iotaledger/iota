// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

include!("../../../generated/iota.grpc.v0.ledger_service.rs");
include!("../../../generated/iota.grpc.v0.ledger_service.field_info.rs");

use crate::proto::TryFromProtoError;

// GetServiceInfoResponse
//

impl GetServiceInfoResponse {
    /// Get the chain identifier (digest of genesis checkpoint).
    pub fn chain_identifier(&self) -> Option<&str> {
        self.chain_id.as_deref()
    }

    /// Get the human-readable chain name (e.g., "mainnet", "testnet").
    pub fn chain_name(&self) -> Option<&str> {
        self.chain.as_deref()
    }

    /// Get the current epoch number.
    pub fn epoch_number(&self) -> Option<u64> {
        self.epoch
    }

    /// Get the checkpoint height of the most recently executed checkpoint.
    pub fn checkpoint_height(&self) -> Option<u64> {
        self.executed_checkpoint_height
    }

    /// Get the Unix timestamp of the most recently executed checkpoint in
    /// milliseconds.
    pub fn checkpoint_timestamp_ms(&self) -> Result<Option<u64>, TryFromProtoError> {
        self.executed_checkpoint_timestamp
            .map(|ts| {
                crate::proto::proto_to_timestamp_ms(ts)
                    .map_err(|e| e.nested(Self::EXECUTED_CHECKPOINT_TIMESTAMP_FIELD.name))
            })
            .transpose()
    }

    /// Get the lowest checkpoint for which checkpoints and transaction data are
    /// available.
    pub fn lowest_checkpoint(&self) -> Option<u64> {
        self.lowest_available_checkpoint
    }

    /// Get the lowest checkpoint for which object data is available.
    pub fn lowest_checkpoint_objects(&self) -> Option<u64> {
        self.lowest_available_checkpoint_objects
    }

    /// Get the software version of the service.
    pub fn server_version(&self) -> Option<&str> {
        self.server.as_deref()
    }
}

// ObjectResult
//

impl ObjectResult {
    /// Get the object if this result is an object.
    pub fn object(&self) -> Result<Option<iota_sdk_types::Object>, TryFromProtoError> {
        match &self.result {
            Some(object_result::Result::Object(obj)) => Ok(Some(obj.object()?)),
            _ => Ok(None),
        }
    }

    /// Get the object ID if this result is an object.
    pub fn object_id(&self) -> Result<Option<iota_sdk_types::ObjectId>, TryFromProtoError> {
        match &self.result {
            Some(object_result::Result::Object(obj)) => obj.object_id(),
            _ => Ok(None),
        }
    }

    /// Get the raw BCS bytes if this result is an object.
    pub fn object_bcs(&self) -> Option<&[u8]> {
        match &self.result {
            Some(object_result::Result::Object(obj)) => obj.object_bcs(),
            _ => None,
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

// GetObjectsResponse
//

impl GetObjectsResponse {
    /// Check if there are more results available.
    pub fn has_more(&self) -> bool {
        self.has_next
    }

    /// Get all successful objects.
    pub fn objects(&self) -> Result<Vec<iota_sdk_types::Object>, TryFromProtoError> {
        self.objects
            .iter()
            .enumerate()
            .filter_map(|(i, r)| match r.object() {
                Ok(Some(obj)) => Some(Ok(obj)),
                Ok(None) => None,
                Err(e) => Some(Err(e.nested_at(Self::OBJECTS_FIELD.name, i))),
            })
            .collect()
    }
}

// TransactionResult
//

impl TransactionResult {
    /// Get the transaction if this result is a transaction.
    pub fn transaction(&self) -> Result<Option<iota_sdk_types::Transaction>, TryFromProtoError> {
        match &self.result {
            Some(transaction_result::Result::Transaction(tx)) => Ok(Some(tx.transaction()?)),
            _ => Ok(None),
        }
    }

    /// Get the transaction digest if this result is a transaction.
    pub fn digest(&self) -> Result<Option<iota_sdk_types::Digest>, TryFromProtoError> {
        match &self.result {
            Some(transaction_result::Result::Transaction(tx)) => Ok(Some(tx.digest()?)),
            _ => Ok(None),
        }
    }

    /// Get the effects if this result is a transaction.
    pub fn effects(&self) -> Result<Option<iota_sdk_types::TransactionEffects>, TryFromProtoError> {
        match &self.result {
            Some(transaction_result::Result::Transaction(tx)) => Ok(Some(tx.effects()?)),
            _ => Ok(None),
        }
    }

    /// Get the raw BCS bytes of the transaction if this result is a
    /// transaction.
    pub fn transaction_bcs(&self) -> Option<&[u8]> {
        match &self.result {
            Some(transaction_result::Result::Transaction(tx)) => tx.transaction_bcs(),
            _ => None,
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

// GetTransactionsResponse
//

impl GetTransactionsResponse {
    /// Check if there are more results available.
    pub fn has_more(&self) -> bool {
        self.has_next
    }

    /// Get all transaction digests (successful results only).
    pub fn digests(&self) -> Result<Vec<iota_sdk_types::Digest>, TryFromProtoError> {
        self.transactions
            .iter()
            .enumerate()
            .filter_map(|(i, r)| match r.digest() {
                Ok(Some(digest)) => Some(Ok(digest)),
                Ok(None) => None,
                Err(e) => Some(Err(e.nested_at(Self::TRANSACTIONS_FIELD.name, i))),
            })
            .collect()
    }
}

// CheckpointData
//

impl CheckpointData {
    /// Get the checkpoint sequence number if this is a checkpoint payload.
    pub fn checkpoint_sequence_number(&self) -> Option<u64> {
        match &self.payload {
            Some(checkpoint_data::Payload::Checkpoint(cp)) => cp.checkpoint_sequence_number(),
            _ => None,
        }
    }

    /// Get the checkpoint summary if this is a checkpoint payload.
    pub fn checkpoint_summary(
        &self,
    ) -> Result<Option<iota_sdk_types::CheckpointSummary>, TryFromProtoError> {
        match &self.payload {
            Some(checkpoint_data::Payload::Checkpoint(cp)) => cp.summary(),
            _ => Ok(None),
        }
    }

    /// Get all transactions if this is a transactions payload.
    pub fn transactions(
        &self,
    ) -> Result<Option<Vec<iota_sdk_types::Transaction>>, TryFromProtoError> {
        match &self.payload {
            Some(checkpoint_data::Payload::Transactions(txs)) => Ok(Some(txs.transactions()?)),
            _ => Ok(None),
        }
    }

    /// Get all events if this is an events payload.
    pub fn events(&self) -> Result<Option<Vec<iota_sdk_types::Event>>, TryFromProtoError> {
        match &self.payload {
            Some(checkpoint_data::Payload::Events(events)) => Ok(Some(events.events()?)),
            _ => Ok(None),
        }
    }

    /// Get the end marker sequence number if this is an end marker payload.
    pub fn end_marker_sequence_number(&self) -> Option<u64> {
        match &self.payload {
            Some(checkpoint_data::Payload::EndMarker(marker)) => marker.sequence_number,
            _ => None,
        }
    }
}

// checkpoint_data::EndMarker
//

impl checkpoint_data::EndMarker {
    /// Get the checkpoint sequence number.
    pub fn checkpoint_sequence_number(&self) -> Option<u64> {
        self.sequence_number
    }
}

// GetEpochResponse
//

impl GetEpochResponse {
    /// Get the epoch number.
    pub fn epoch_number(&self) -> Option<u64> {
        self.epoch.as_ref().and_then(|e| e.epoch_number())
    }

    /// Get the validator committee.
    pub fn committee(
        &self,
    ) -> Result<Option<iota_sdk_types::ValidatorCommittee>, TryFromProtoError> {
        self.epoch
            .as_ref()
            .map(|e| {
                e.committee()
                    .map_err(|err| err.nested(Self::EPOCH_FIELD.name))
            })
            .transpose()
    }

    /// Get the first checkpoint sequence number in this epoch.
    pub fn first_checkpoint(&self) -> Option<u64> {
        self.epoch
            .as_ref()
            .and_then(|e| e.first_checkpoint_sequence_number())
    }

    /// Get the last checkpoint sequence number in this epoch.
    pub fn last_checkpoint(&self) -> Option<u64> {
        self.epoch
            .as_ref()
            .and_then(|e| e.last_checkpoint_sequence_number())
    }

    /// Get the epoch start time in milliseconds.
    pub fn start_ms(&self) -> Result<Option<u64>, TryFromProtoError> {
        match &self.epoch {
            Some(e) => e
                .start_ms()
                .map_err(|err| err.nested(Self::EPOCH_FIELD.name)),
            None => Ok(None),
        }
    }

    /// Get the epoch end time in milliseconds.
    pub fn end_ms(&self) -> Result<Option<u64>, TryFromProtoError> {
        match &self.epoch {
            Some(e) => e.end_ms().map_err(|err| err.nested(Self::EPOCH_FIELD.name)),
            None => Ok(None),
        }
    }

    /// Get the reference gas price in NANOS.
    pub fn gas_price(&self) -> Option<u64> {
        self.epoch.as_ref().and_then(|e| e.gas_price())
    }

    /// Get the protocol version.
    pub fn protocol_version(&self) -> Option<u64> {
        self.epoch.as_ref().and_then(|e| e.protocol_version())
    }
}
