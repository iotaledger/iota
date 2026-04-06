// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

include!("../../../generated/iota.grpc.v1.transaction.rs");
include!("../../../generated/iota.grpc.v1.transaction.field_info.rs");
include!("../../../generated/iota.grpc.v1.transaction.accessors.rs");

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
    ///
    /// **Read mask:** `effects.digest` relative to the parent
    /// `ExecutedTransaction` (see [`TRANSACTION_EFFECTS_DIGEST`]).
    ///
    /// [`TRANSACTION_EFFECTS_DIGEST`]: crate::read_masks::TRANSACTION_EFFECTS_DIGEST
    pub fn digest(&self) -> Result<iota_sdk_types::Digest, TryFromProtoError> {
        self.try_into()
    }

    /// Deserialize effects from BCS.
    ///
    /// **Read mask:** `effects.bcs` relative to the parent
    /// `ExecutedTransaction` (see [`TRANSACTION_EFFECTS_BCS`]).
    ///
    /// [`TRANSACTION_EFFECTS_BCS`]: crate::read_masks::TRANSACTION_EFFECTS_BCS
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
                bcs.deserialize::<crate::v1::versioned::VersionedEvent>()
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
    ///
    /// **Read mask:** `events.digest` relative to the parent
    /// `ExecutedTransaction` (see [`TRANSACTION_EVENTS_DIGEST`]).
    ///
    /// [`TRANSACTION_EVENTS_DIGEST`]: crate::read_masks::TRANSACTION_EVENTS_DIGEST
    pub fn digest(&self) -> Result<iota_sdk_types::Digest, TryFromProtoError> {
        self.try_into()
    }

    /// Deserialize all events from BCS.
    ///
    /// **Read mask:** `events.events.bcs` relative to the parent
    /// `ExecutedTransaction` (see [`TRANSACTION_EVENTS_BCS`]).
    ///
    /// [`TRANSACTION_EVENTS_BCS`]: crate::read_masks::TRANSACTION_EVENTS_BCS
    pub fn events(&self) -> Result<iota_sdk_types::TransactionEvents, TryFromProtoError> {
        self.try_into()
    }
}

// ExecutedTransaction
//

// Lazy conversion methods for ExecutedTransaction
impl ExecutedTransaction {
    /// Get the transaction.
    ///
    /// Returns the proto [`Transaction`] which provides:
    /// - [`Transaction::digest()`] — the transaction digest
    /// - [`Transaction::transaction()`] — the deserialized SDK `Transaction`
    ///
    /// **Read mask:** `"transaction"` (see
    /// [`EXECUTED_TRANSACTION_TRANSACTION`]). For checkpoint context use
    /// `"transactions.transaction"`.
    ///
    /// [`EXECUTED_TRANSACTION_TRANSACTION`]: crate::read_masks::EXECUTED_TRANSACTION_TRANSACTION
    pub fn transaction(&self) -> Result<&super::transaction::Transaction, TryFromProtoError> {
        self.transaction
            .as_ref()
            .ok_or_else(|| TryFromProtoError::missing(Self::TRANSACTION_FIELD.name))
    }

    /// Get the user signatures.
    ///
    /// **Read mask:** `"signatures"` (see [`EXECUTED_TRANSACTION_SIGNATURES`]).
    /// For checkpoint context use `"transactions.signatures"`.
    ///
    /// [`EXECUTED_TRANSACTION_SIGNATURES`]: crate::read_masks::EXECUTED_TRANSACTION_SIGNATURES
    pub fn signatures(&self) -> Result<&super::signatures::UserSignatures, TryFromProtoError> {
        self.signatures
            .as_ref()
            .ok_or_else(|| TryFromProtoError::missing(Self::SIGNATURES_FIELD.name))
    }

    /// Get the transaction effects.
    ///
    /// Returns the proto [`TransactionEffects`] which provides:
    /// - [`TransactionEffects::digest()`] — the effects digest
    /// - [`TransactionEffects::effects()`] — the deserialized SDK
    ///   `TransactionEffects`
    ///
    /// **Read mask:** `"effects"` (see [`EXECUTED_TRANSACTION_EFFECTS`]).
    /// For checkpoint context use `"transactions.effects"`.
    ///
    /// [`EXECUTED_TRANSACTION_EFFECTS`]: crate::read_masks::EXECUTED_TRANSACTION_EFFECTS
    pub fn effects(&self) -> Result<&super::transaction::TransactionEffects, TryFromProtoError> {
        self.effects
            .as_ref()
            .ok_or_else(|| TryFromProtoError::missing(Self::EFFECTS_FIELD.name))
    }

    /// Get the transaction events.
    ///
    /// Returns the proto [`TransactionEvents`] which provides:
    /// - [`TransactionEvents::digest()`] — the events digest
    /// - [`TransactionEvents::events()`] — the deserialized SDK
    ///   `TransactionEvents`
    ///
    /// **Read mask:** `"events"` (see [`EXECUTED_TRANSACTION_EVENTS`]).
    /// For checkpoint context use `"transactions.events"`.
    ///
    /// [`EXECUTED_TRANSACTION_EVENTS`]: crate::read_masks::EXECUTED_TRANSACTION_EVENTS
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
    ///
    /// **Read mask:** `"checkpoint"` (see [`EXECUTED_TRANSACTION_CHECKPOINT`]).
    /// For checkpoint context use `"transactions.checkpoint"`.
    ///
    /// [`EXECUTED_TRANSACTION_CHECKPOINT`]: crate::read_masks::EXECUTED_TRANSACTION_CHECKPOINT
    pub fn checkpoint_sequence_number(
        &self,
    ) -> Result<iota_sdk_types::CheckpointSequenceNumber, TryFromProtoError> {
        self.checkpoint
            .ok_or_else(|| TryFromProtoError::missing(Self::CHECKPOINT_FIELD.name))
    }

    /// Get timestamp in milliseconds.
    ///
    /// **Read mask:** `"timestamp"` (see [`EXECUTED_TRANSACTION_TIMESTAMP`]).
    /// For checkpoint context use `"transactions.timestamp"`.
    ///
    /// [`EXECUTED_TRANSACTION_TIMESTAMP`]: crate::read_masks::EXECUTED_TRANSACTION_TIMESTAMP
    pub fn timestamp_ms(&self) -> Result<iota_sdk_types::CheckpointTimestamp, TryFromProtoError> {
        let ts = self
            .timestamp
            .ok_or_else(|| TryFromProtoError::missing(Self::TIMESTAMP_FIELD.name))?;
        crate::proto::proto_to_timestamp_ms(ts).map_err(|e| e.nested(Self::TIMESTAMP_FIELD.name))
    }

    /// Get input objects.
    ///
    /// Returns proto [`Objects`](super::object::Objects) containing the
    /// transaction's input objects. Call `.object()` on each to deserialize.
    ///
    /// **Read mask:** `"input_objects"` (see
    /// [`EXECUTED_TRANSACTION_INPUT_OBJECTS`]).
    /// For checkpoint context use `"transactions.input_objects"`.
    ///
    /// [`EXECUTED_TRANSACTION_INPUT_OBJECTS`]: crate::read_masks::EXECUTED_TRANSACTION_INPUT_OBJECTS
    pub fn input_objects(&self) -> Result<&super::object::Objects, TryFromProtoError> {
        self.input_objects
            .as_ref()
            .ok_or_else(|| TryFromProtoError::missing(Self::INPUT_OBJECTS_FIELD.name))
    }

    /// Get output objects.
    ///
    /// Returns proto [`Objects`](super::object::Objects) containing the
    /// transaction's output objects. Call `.object()` on each to deserialize.
    ///
    /// **Read mask:** `"output_objects"` (see
    /// [`EXECUTED_TRANSACTION_OUTPUT_OBJECTS`]).
    /// For checkpoint context use `"transactions.output_objects"`.
    ///
    /// [`EXECUTED_TRANSACTION_OUTPUT_OBJECTS`]: crate::read_masks::EXECUTED_TRANSACTION_OUTPUT_OBJECTS
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
    ///
    /// **Read mask:** `transaction.digest` relative to the parent
    /// `ExecutedTransaction` (see [`TRANSACTION_DIGEST`]).
    ///
    /// [`TRANSACTION_DIGEST`]: crate::read_masks::TRANSACTION_DIGEST
    pub fn digest(&self) -> Result<iota_sdk_types::Digest, TryFromProtoError> {
        self.try_into()
    }

    /// Deserialize the transaction from BCS.
    ///
    /// **Read mask:** `transaction.bcs` relative to the parent
    /// `ExecutedTransaction` (see [`TRANSACTION_BCS`]).
    ///
    /// [`TRANSACTION_BCS`]: crate::read_masks::TRANSACTION_BCS
    pub fn transaction(&self) -> Result<iota_sdk_types::Transaction, TryFromProtoError> {
        self.try_into()
    }
}
