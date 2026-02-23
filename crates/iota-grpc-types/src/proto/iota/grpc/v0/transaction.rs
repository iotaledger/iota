// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

include!("../../../generated/iota.grpc.v0.transaction.rs");
include!("../../../generated/iota.grpc.v0.transaction.field_info.rs");
include!("../../../generated/iota.grpc.v0.transaction.accessors.rs");

use crate::proto::TryFromProtoError;

// TryFrom implementations for TransactionEffects
impl TryFrom<&TransactionEffects> for iota_sdk_types::TransactionEffects {
    type Error = TryFromProtoError;

    fn try_from(value: &TransactionEffects) -> Result<Self, Self::Error> {
        let bcs = value
            .bcs
            .as_ref()
            .ok_or_else(|| TryFromProtoError::missing(TransactionEffects::BCS_FIELD.name))?;

        bcs.deserialize()
            .map_err(|e| TryFromProtoError::invalid(TransactionEffects::BCS_FIELD.name, e))
    }
}

impl TryFrom<&TransactionEffects> for iota_sdk_types::Digest {
    type Error = TryFromProtoError;

    fn try_from(value: &TransactionEffects) -> Result<Self, Self::Error> {
        let digest_proto = value
            .digest
            .as_ref()
            .ok_or_else(|| TryFromProtoError::missing(TransactionEffects::DIGEST_FIELD.name))?;

        iota_sdk_types::Digest::from_bytes(&digest_proto.digest)
            .map_err(|e| TryFromProtoError::invalid(TransactionEffects::DIGEST_FIELD.name, e))
    }
}

// Convenience methods for TransactionEffects (delegate to TryFrom)
impl TransactionEffects {
    /// Get the effects digest.
    pub fn digest(&self) -> Result<iota_sdk_types::Digest, TryFromProtoError> {
        self.try_into()
    }

    /// Deserialize effects from BCS.
    pub fn effects(&self) -> Result<iota_sdk_types::TransactionEffects, TryFromProtoError> {
        self.try_into()
    }
}

// TryFrom implementations for TransactionEvents
impl TryFrom<&TransactionEvents> for iota_sdk_types::TransactionEvents {
    type Error = TryFromProtoError;

    fn try_from(value: &TransactionEvents) -> Result<Self, Self::Error> {
        let events = value
            .events
            .as_ref()
            .ok_or_else(|| TryFromProtoError::missing(TransactionEvents::EVENTS_FIELD.name))?;

        let sdk_events: Vec<iota_sdk_types::Event> = events
            .events
            .iter()
            .enumerate()
            .map(|(i, e)| {
                let bcs = e.bcs.as_ref().ok_or_else(|| {
                    TryFromProtoError::missing("event.bcs")
                        .nested_at(TransactionEvents::EVENTS_FIELD.name, i)
                })?;
                bcs.deserialize::<crate::v0::versioned::VersionedEvent>()
                    .map_err(|err| {
                        TryFromProtoError::invalid("event.bcs", err)
                            .nested_at(TransactionEvents::EVENTS_FIELD.name, i)
                    })?
                    .try_into_v1()
                    .map_err(|_| {
                        TryFromProtoError::invalid("event.bcs", "unsupported Event version")
                            .nested_at(TransactionEvents::EVENTS_FIELD.name, i)
                    })
            })
            .collect::<Result<Vec<_>, _>>()?;

        Ok(iota_sdk_types::TransactionEvents(sdk_events))
    }
}

impl TryFrom<&TransactionEvents> for iota_sdk_types::Digest {
    type Error = TryFromProtoError;

    fn try_from(value: &TransactionEvents) -> Result<Self, Self::Error> {
        let digest_proto = value
            .digest
            .as_ref()
            .ok_or_else(|| TryFromProtoError::missing(TransactionEvents::DIGEST_FIELD.name))?;

        iota_sdk_types::Digest::from_bytes(&digest_proto.digest)
            .map_err(|e| TryFromProtoError::invalid(TransactionEvents::DIGEST_FIELD.name, e))
    }
}

// Convenience methods for TransactionEvents (delegate to TryFrom)
impl TransactionEvents {
    /// Get the events digest.
    pub fn digest(&self) -> Result<iota_sdk_types::Digest, TryFromProtoError> {
        self.try_into()
    }

    /// Deserialize all events from BCS.
    pub fn events(&self) -> Result<iota_sdk_types::TransactionEvents, TryFromProtoError> {
        self.try_into()
    }
}

// ExecutedTransaction
//

// Lazy conversion methods for ExecutedTransaction
impl ExecutedTransaction {
    pub fn transaction(&self) -> Result<&super::transaction::Transaction, TryFromProtoError> {
        self.transaction
            .as_ref()
            .ok_or_else(|| TryFromProtoError::missing(Self::TRANSACTION_FIELD.name))
    }

    pub fn signatures(&self) -> Result<&super::signatures::UserSignatures, TryFromProtoError> {
        self.signatures
            .as_ref()
            .ok_or_else(|| TryFromProtoError::missing(Self::SIGNATURES_FIELD.name))
    }

    pub fn effects(&self) -> Result<&super::transaction::TransactionEffects, TryFromProtoError> {
        self.effects
            .as_ref()
            .ok_or_else(|| TryFromProtoError::missing(Self::EFFECTS_FIELD.name))
    }

    pub fn events(&self) -> Result<&super::transaction::TransactionEvents, TryFromProtoError> {
        self.events
            .as_ref()
            .ok_or_else(|| TryFromProtoError::missing(Self::EVENTS_FIELD.name))
    }

    fn events_opt(&self) -> Result<Option<iota_sdk_types::TransactionEvents>, TryFromProtoError> {
        self.events
            .as_ref()
            .map(TransactionEvents::events)
            .transpose()
    }

    /// Get checkpoint sequence number.
    pub fn checkpoint_sequence_number(
        &self,
    ) -> Result<iota_sdk_types::CheckpointSequenceNumber, TryFromProtoError> {
        self.checkpoint
            .ok_or_else(|| TryFromProtoError::missing(Self::CHECKPOINT_FIELD.name))
    }

    /// Get timestamp in milliseconds.
    pub fn timestamp_ms(&self) -> Result<iota_sdk_types::CheckpointTimestamp, TryFromProtoError> {
        let ts = self
            .timestamp
            .ok_or_else(|| TryFromProtoError::missing(Self::TIMESTAMP_FIELD.name))?;
        crate::proto::proto_to_timestamp_ms(ts).map_err(|e| e.nested(Self::TIMESTAMP_FIELD.name))
    }

    /// Get input objects.
    pub fn input_objects(&self) -> Result<&super::object::Objects, TryFromProtoError> {
        self.input_objects
            .as_ref()
            .ok_or_else(|| TryFromProtoError::missing(Self::INPUT_OBJECTS_FIELD.name))
    }

    /// Get output objects.
    pub fn output_objects(&self) -> Result<&super::object::Objects, TryFromProtoError> {
        self.output_objects
            .as_ref()
            .ok_or_else(|| TryFromProtoError::missing(Self::OUTPUT_OBJECTS_FIELD.name))
    }
}

// TryFrom implementations for CheckpointTransaction
impl TryFrom<&ExecutedTransaction> for iota_sdk_types::CheckpointTransaction {
    type Error = TryFromProtoError;

    fn try_from(value: &ExecutedTransaction) -> Result<Self, Self::Error> {
        let input_objects: Result<Vec<_>, _> = value
            .input_objects()?
            .objects
            .iter()
            .map(|obj| obj.object())
            .collect();

        let output_objects: Result<Vec<_>, _> = value
            .output_objects()?
            .objects
            .iter()
            .map(|obj| obj.object())
            .collect();

        Ok(Self {
            transaction: iota_sdk_types::SignedTransaction {
                transaction: value.transaction()?.transaction()?,
                signatures: value
                    .signatures()?
                    .signatures
                    .iter()
                    .map(|s| s.signature())
                    .collect::<Result<Vec<_>, _>>()?,
            },
            effects: value.effects()?.effects()?,
            events: value.events_opt()?,
            input_objects: input_objects?,
            output_objects: output_objects?,
        })
    }
}

// TryFrom implementations for Transaction
impl TryFrom<&Transaction> for iota_sdk_types::Transaction {
    type Error = TryFromProtoError;

    fn try_from(value: &Transaction) -> Result<Self, Self::Error> {
        let bcs = value
            .bcs
            .as_ref()
            .ok_or_else(|| TryFromProtoError::missing(Transaction::BCS_FIELD.name))?;

        bcs.deserialize()
            .map_err(|e| TryFromProtoError::invalid(Transaction::BCS_FIELD.name, e))
    }
}

impl TryFrom<&Transaction> for iota_sdk_types::Digest {
    type Error = TryFromProtoError;

    fn try_from(value: &Transaction) -> Result<Self, Self::Error> {
        let digest_proto = value
            .digest
            .as_ref()
            .ok_or_else(|| TryFromProtoError::missing(Transaction::DIGEST_FIELD.name))?;

        iota_sdk_types::Digest::from_bytes(&digest_proto.digest)
            .map_err(|e| TryFromProtoError::invalid(Transaction::DIGEST_FIELD.name, e))
    }
}

// Convenience methods for Transaction (delegate to TryFrom)
impl Transaction {
    /// Get the transaction digest.
    pub fn digest(&self) -> Result<iota_sdk_types::Digest, TryFromProtoError> {
        self.try_into()
    }

    /// Deserialize the transaction from BCS.
    pub fn transaction(&self) -> Result<iota_sdk_types::Transaction, TryFromProtoError> {
        self.try_into()
    }
}
