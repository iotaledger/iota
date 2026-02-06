// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

include!("../../../generated/iota.grpc.v0.transaction.rs");
include!("../../../generated/iota.grpc.v0.transaction.field_info.rs");

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
                bcs.deserialize().map_err(|err| {
                    TryFromProtoError::invalid("event.bcs", err)
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
    /// Get the transaction digest.
    pub fn digest(&self) -> Result<iota_sdk_types::Digest, TryFromProtoError> {
        self.transaction
            .as_ref()
            .ok_or_else(|| TryFromProtoError::missing(Self::TRANSACTION_FIELD.name))?
            .digest()
            .map_err(|e| e.nested(Self::TRANSACTION_FIELD.name))
    }

    /// Deserialize the transaction from BCS.
    pub fn transaction(&self) -> Result<iota_sdk_types::Transaction, TryFromProtoError> {
        self.transaction
            .as_ref()
            .ok_or_else(|| TryFromProtoError::missing(Self::TRANSACTION_FIELD.name))?
            .transaction()
            .map_err(|e| e.nested(Self::TRANSACTION_FIELD.name))
    }

    /// Deserialize user signatures.
    pub fn signatures(&self) -> Result<Vec<iota_sdk_types::UserSignature>, TryFromProtoError> {
        let signatures_proto = self
            .signatures
            .as_ref()
            .ok_or_else(|| TryFromProtoError::missing(Self::SIGNATURES_FIELD.name))?;

        signatures_proto
            .signatures
            .iter()
            .enumerate()
            .map(|(i, sig)| {
                <&super::signatures::UserSignature as TryInto<iota_sdk_types::UserSignature>>::try_into(sig)
                    .map_err(|e: TryFromProtoError| e.nested_at(Self::SIGNATURES_FIELD.name, i))
            })
            .collect()
    }

    /// Deserialize transaction effects from BCS.
    pub fn effects(&self) -> Result<iota_sdk_types::TransactionEffects, TryFromProtoError> {
        self.effects
            .as_ref()
            .ok_or_else(|| TryFromProtoError::missing(Self::EFFECTS_FIELD.name))?
            .effects()
            .map_err(|e| e.nested(Self::EFFECTS_FIELD.name))
    }

    /// Deserialize transaction events. Returns Ok(None) if not present.
    pub fn events(&self) -> Result<Option<iota_sdk_types::TransactionEvents>, TryFromProtoError> {
        self.events
            .as_ref()
            .map(|ev| ev.events().map_err(|e| e.nested(Self::EVENTS_FIELD.name)))
            .transpose()
    }

    /// Get checkpoint sequence number (no deserialization needed).
    pub fn checkpoint_sequence_number(&self) -> Option<u64> {
        self.checkpoint
    }

    /// Get timestamp in milliseconds.
    pub fn timestamp_ms(&self) -> Result<Option<u64>, TryFromProtoError> {
        self.timestamp
            .as_ref()
            .map(|ts| {
                crate::proto::proto_to_timestamp_ms(*ts)
                    .map_err(|e| e.nested(Self::TIMESTAMP_FIELD.name))
            })
            .transpose()
    }

    /// Deserialize input objects. Returns Ok(None) if not present.
    pub fn input_objects(&self) -> Result<Option<Vec<iota_sdk_types::Object>>, TryFromProtoError> {
        self.input_objects
            .as_ref()
            .map(|objs| {
                objs.objects()
                    .map_err(|e| e.nested(Self::INPUT_OBJECTS_FIELD.name))
            })
            .transpose()
    }

    /// Deserialize output objects. Returns Ok(None) if not present.
    pub fn output_objects(&self) -> Result<Option<Vec<iota_sdk_types::Object>>, TryFromProtoError> {
        self.output_objects
            .as_ref()
            .map(|objs| {
                objs.objects()
                    .map_err(|e| e.nested(Self::OUTPUT_OBJECTS_FIELD.name))
            })
            .transpose()
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
