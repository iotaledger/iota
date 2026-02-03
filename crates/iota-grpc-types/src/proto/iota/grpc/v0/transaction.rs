// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

include!("../../../generated/iota.grpc.v0.transaction.rs");
include!("../../../generated/iota.grpc.v0.transaction.field_info.rs");

use crate::{field::FieldMaskTree, merge::Merge, proto::timestamp_ms_to_proto, v0::bcs::BcsData};

impl Merge<iota_types::effects::TransactionEffects> for TransactionEffects {
    fn merge(
        &mut self,
        source: iota_types::effects::TransactionEffects,
        mask: &FieldMaskTree,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if !mask.contains(Self::DIGEST_FIELD.name) && !mask.contains(Self::BCS_FIELD.name) {
            // No need to convert if no field is requested
            return Ok(());
        }

        // Convert iota_types to iota_sdk_types types for external compatibility
        let sdk_effects: iota_sdk_types::TransactionEffects = source
            .try_into()
            .map_err(|e| format!("failed to convert effects to SDK type: {e}"))?;

        Merge::merge(self, &sdk_effects, mask)
    }
}

impl Merge<&iota_sdk_types::TransactionEffects> for TransactionEffects {
    fn merge(
        &mut self,
        source: &iota_sdk_types::TransactionEffects,
        mask: &FieldMaskTree,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // Set digest if requested
        if mask.contains(Self::DIGEST_FIELD.name) {
            self.digest = Some(source.digest().into());
        }

        // Set BCS if requested
        if mask.contains(Self::BCS_FIELD.name) {
            self.bcs = BcsData::serialize(source).ok();
        }

        Ok(())
    }
}

impl Merge<&TransactionEffects> for TransactionEffects {
    fn merge(
        &mut self,
        source: &TransactionEffects,
        mask: &FieldMaskTree,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if mask.contains(Self::DIGEST_FIELD.name) {
            self.digest = source.digest.clone();
        }

        if mask.contains(Self::BCS_FIELD.name) {
            self.bcs = source.bcs.clone();
        }

        Ok(())
    }
}

// TryFrom implementations for TransactionEffects
impl TryFrom<&TransactionEffects> for iota_sdk_types::TransactionEffects {
    type Error = crate::proto::TryFromProtoError;

    fn try_from(value: &TransactionEffects) -> Result<Self, Self::Error> {
        let bcs = value.bcs.as_ref().ok_or_else(|| {
            crate::proto::TryFromProtoError::missing(TransactionEffects::BCS_FIELD.name)
        })?;

        bcs.deserialize().map_err(|e| {
            crate::proto::TryFromProtoError::invalid(TransactionEffects::BCS_FIELD.name, e)
        })
    }
}

impl TryFrom<&TransactionEffects> for iota_sdk_types::Digest {
    type Error = crate::proto::TryFromProtoError;

    fn try_from(value: &TransactionEffects) -> Result<Self, Self::Error> {
        let digest_proto = value.digest.as_ref().ok_or_else(|| {
            crate::proto::TryFromProtoError::missing(TransactionEffects::DIGEST_FIELD.name)
        })?;

        iota_sdk_types::Digest::from_bytes(&digest_proto.digest).map_err(|e| {
            crate::proto::TryFromProtoError::invalid(TransactionEffects::DIGEST_FIELD.name, e)
        })
    }
}

// Convenience methods for TransactionEffects (delegate to TryFrom)
impl TransactionEffects {
    /// Get the effects digest.
    pub fn digest(&self) -> Result<iota_sdk_types::Digest, crate::proto::TryFromProtoError> {
        self.try_into()
    }

    /// Deserialize effects from BCS.
    pub fn effects(
        &self,
    ) -> Result<iota_sdk_types::TransactionEffects, crate::proto::TryFromProtoError> {
        self.try_into()
    }
}

impl Merge<iota_types::effects::TransactionEvents> for TransactionEvents {
    fn merge(
        &mut self,
        source: iota_types::effects::TransactionEvents,
        mask: &FieldMaskTree,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if !mask.contains(Self::DIGEST_FIELD.name) && !mask.contains(Self::EVENTS_FIELD.name) {
            // No need to convert if no field is requested
            return Ok(());
        }

        let sdk_events: iota_sdk_types::TransactionEvents = source
            .try_into()
            .map_err(|e| format!("failed to convert events to SDK type: {e}"))?;

        Merge::merge(self, &sdk_events, mask)
    }
}

// TODO: Wrap TransactionEvents into a type with a version
impl Merge<&iota_sdk_types::TransactionEvents> for TransactionEvents {
    fn merge(
        &mut self,
        source: &iota_sdk_types::TransactionEvents,
        mask: &FieldMaskTree,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // Set digest if requested
        if mask.contains(Self::DIGEST_FIELD.name) {
            self.digest = Some(source.digest().into());
        }

        if let Some(events_mask) = mask.subtree(Self::EVENTS_FIELD.name) {
            self.events = Some(super::event::Events::merge_from(source, &events_mask)?);
        }

        Ok(())
    }
}

impl Merge<&TransactionEvents> for TransactionEvents {
    fn merge(
        &mut self,
        source: &TransactionEvents,
        mask: &FieldMaskTree,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if mask.contains(Self::DIGEST_FIELD.name) {
            self.digest = source.digest.clone();
        }

        if mask.contains(Self::EVENTS_FIELD.name) {
            self.events = source.events.clone();
        }

        Ok(())
    }
}

// TryFrom implementations for TransactionEvents
impl TryFrom<&TransactionEvents> for iota_sdk_types::TransactionEvents {
    type Error = crate::proto::TryFromProtoError;

    fn try_from(value: &TransactionEvents) -> Result<Self, Self::Error> {
        let events = value.events.as_ref().ok_or_else(|| {
            crate::proto::TryFromProtoError::missing(TransactionEvents::EVENTS_FIELD.name)
        })?;

        let sdk_events: Vec<iota_sdk_types::Event> = events
            .events
            .iter()
            .enumerate()
            .map(|(i, e)| {
                let bcs = e.bcs.as_ref().ok_or_else(|| {
                    crate::proto::TryFromProtoError::missing("event.bcs")
                        .nested_at(TransactionEvents::EVENTS_FIELD.name, i)
                })?;
                bcs.deserialize().map_err(|err| {
                    crate::proto::TryFromProtoError::invalid("event.bcs", err)
                        .nested_at(TransactionEvents::EVENTS_FIELD.name, i)
                })
            })
            .collect::<Result<Vec<_>, _>>()?;

        Ok(iota_sdk_types::TransactionEvents(sdk_events))
    }
}

impl TryFrom<&TransactionEvents> for iota_sdk_types::Digest {
    type Error = crate::proto::TryFromProtoError;

    fn try_from(value: &TransactionEvents) -> Result<Self, Self::Error> {
        let digest_proto = value.digest.as_ref().ok_or_else(|| {
            crate::proto::TryFromProtoError::missing(TransactionEvents::DIGEST_FIELD.name)
        })?;

        iota_sdk_types::Digest::from_bytes(&digest_proto.digest).map_err(|e| {
            crate::proto::TryFromProtoError::invalid(TransactionEvents::DIGEST_FIELD.name, e)
        })
    }
}

// Convenience methods for TransactionEvents (delegate to TryFrom)
impl TransactionEvents {
    /// Get the events digest.
    pub fn digest(&self) -> Result<iota_sdk_types::Digest, crate::proto::TryFromProtoError> {
        self.try_into()
    }

    /// Deserialize all events from BCS.
    pub fn events(
        &self,
    ) -> Result<iota_sdk_types::TransactionEvents, crate::proto::TryFromProtoError> {
        self.try_into()
    }
}

// ExecutedTransaction
//

/// Wrapper type that includes checkpoint context for a CheckpointTransaction.
#[derive(Debug, Clone)]
pub struct CheckpointTransactionWithContext {
    pub transaction: iota_types::full_checkpoint_content::CheckpointTransaction,
    pub checkpoint_sequence_number: Option<u64>,
    pub checkpoint_timestamp_ms: Option<u64>,
}

impl CheckpointTransactionWithContext {
    pub fn new(
        transaction: iota_types::full_checkpoint_content::CheckpointTransaction,
        checkpoint_sequence_number: Option<u64>,
        checkpoint_timestamp_ms: Option<u64>,
    ) -> Self {
        Self {
            transaction,
            checkpoint_sequence_number,
            checkpoint_timestamp_ms,
        }
    }
}

impl TryFrom<CheckpointTransactionWithContext> for ExecutedTransaction {
    type Error = Box<dyn std::error::Error>;

    fn try_from(transaction_ctx: CheckpointTransactionWithContext) -> Result<Self, Self::Error> {
        Self::merge_from(transaction_ctx, &FieldMaskTree::new_wildcard())
    }
}

impl Merge<CheckpointTransactionWithContext> for ExecutedTransaction {
    fn merge(
        &mut self,
        source: CheckpointTransactionWithContext,
        mask: &FieldMaskTree,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(submask) = mask.subtree(Self::TRANSACTION_FIELD.name) {
            self.transaction = Some(crate::v0::transaction::Transaction::merge_from(
                source.transaction.transaction.clone(),
                &submask,
            )?);
        }

        if let Some(submask) = mask.subtree(Self::SIGNATURES_FIELD.name) {
            self.signatures = Some(crate::v0::signatures::UserSignatures::merge_from(
                source.transaction.transaction.clone(),
                &submask,
            )?);
        }

        if let Some(submask) = mask.subtree(Self::EFFECTS_FIELD.name) {
            self.effects = Some(crate::v0::transaction::TransactionEffects::merge_from(
                source.transaction.effects.clone(),
                &submask,
            )?);
        }

        if let Some(submask) = mask.subtree(Self::EVENTS_FIELD.name) {
            if let Some(events) = source.transaction.events {
                self.events = Some(crate::v0::transaction::TransactionEvents::merge_from(
                    events, &submask,
                )?);
            }
        }

        // Set checkpoint sequence number if requested
        if mask.contains(Self::CHECKPOINT_FIELD.name) {
            self.checkpoint = source.checkpoint_sequence_number;
        }

        // Set checkpoint timestamp if requested
        if mask.contains(Self::TIMESTAMP_FIELD.name) {
            self.timestamp = source.checkpoint_timestamp_ms.map(timestamp_ms_to_proto);
        }

        if let Some(submask) = mask.subtree(Self::INPUT_OBJECTS_FIELD.name) {
            self.input_objects = Some(crate::v0::object::Objects::merge_from(
                Some(source.transaction.input_objects),
                &submask,
            )?);
        }

        if let Some(submask) = mask.subtree(Self::OUTPUT_OBJECTS_FIELD.name) {
            self.output_objects = Some(crate::v0::object::Objects::merge_from(
                Some(source.transaction.output_objects),
                &submask,
            )?);
        }

        Ok(())
    }
}

impl Merge<&ExecutedTransaction> for ExecutedTransaction {
    fn merge(
        &mut self,
        source: &ExecutedTransaction,
        mask: &FieldMaskTree,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let ExecutedTransaction {
            transaction,
            signatures,
            effects,
            events,
            checkpoint,
            timestamp,
            input_objects,
            output_objects,
        } = source;

        if let Some(submask) = mask.subtree(Self::TRANSACTION_FIELD.name) {
            if let Some(tx) = transaction {
                self.transaction = Some(crate::v0::transaction::Transaction::merge_from(
                    tx, &submask,
                )?);
            }
        }

        if let Some(submask) = mask.subtree(Self::SIGNATURES_FIELD.name) {
            if let Some(sigs) = signatures {
                self.signatures = Some(crate::v0::signatures::UserSignatures::merge_from(
                    sigs, &submask,
                )?);
            }
        }

        if let Some(submask) = mask.subtree(Self::EFFECTS_FIELD.name) {
            if let Some(fx) = effects {
                self.effects = Some(crate::v0::transaction::TransactionEffects::merge_from(
                    fx, &submask,
                )?);
            }
        }

        if let Some(submask) = mask.subtree(Self::EVENTS_FIELD.name) {
            if let Some(ev) = events {
                self.events = Some(crate::v0::transaction::TransactionEvents::merge_from(
                    ev, &submask,
                )?);
            }
        }

        if mask.contains(Self::CHECKPOINT_FIELD.name) {
            self.checkpoint = *checkpoint;
        }

        if mask.contains(Self::TIMESTAMP_FIELD.name) {
            self.timestamp = *timestamp;
        }

        if let Some(submask) = mask.subtree(Self::INPUT_OBJECTS_FIELD.name) {
            if let Some(objs) = input_objects {
                self.input_objects = Some(crate::v0::object::Objects::merge_from(objs, &submask)?);
            }
        }

        if let Some(submask) = mask.subtree(Self::OUTPUT_OBJECTS_FIELD.name) {
            if let Some(objs) = output_objects {
                self.output_objects = Some(crate::v0::object::Objects::merge_from(objs, &submask)?);
            }
        }

        Ok(())
    }
}

// Lazy conversion methods for ExecutedTransaction
impl ExecutedTransaction {
    /// Get the transaction digest.
    pub fn digest(&self) -> Result<iota_sdk_types::Digest, crate::proto::TryFromProtoError> {
        self.transaction
            .as_ref()
            .ok_or_else(|| crate::proto::TryFromProtoError::missing(Self::TRANSACTION_FIELD.name))?
            .digest()
            .map_err(|e| e.nested(Self::TRANSACTION_FIELD.name))
    }

    /// Deserialize the transaction from BCS.
    pub fn transaction(
        &self,
    ) -> Result<iota_sdk_types::Transaction, crate::proto::TryFromProtoError> {
        self.transaction
            .as_ref()
            .ok_or_else(|| crate::proto::TryFromProtoError::missing(Self::TRANSACTION_FIELD.name))?
            .transaction()
            .map_err(|e| e.nested(Self::TRANSACTION_FIELD.name))
    }

    /// Deserialize user signatures.
    pub fn signatures(
        &self,
    ) -> Result<Vec<iota_sdk_types::UserSignature>, crate::proto::TryFromProtoError> {
        let signatures_proto = self
            .signatures
            .as_ref()
            .ok_or_else(|| crate::proto::TryFromProtoError::missing(Self::SIGNATURES_FIELD.name))?;

        signatures_proto
            .signatures
            .iter()
            .enumerate()
            .map(|(i, sig)| {
                <&super::signatures::UserSignature as TryInto<iota_sdk_types::UserSignature>>::try_into(sig)
                    .map_err(|e: crate::proto::TryFromProtoError| e.nested_at(Self::SIGNATURES_FIELD.name, i))
            })
            .collect()
    }

    /// Deserialize transaction effects from BCS.
    pub fn effects(
        &self,
    ) -> Result<iota_sdk_types::TransactionEffects, crate::proto::TryFromProtoError> {
        self.effects
            .as_ref()
            .ok_or_else(|| crate::proto::TryFromProtoError::missing(Self::EFFECTS_FIELD.name))?
            .effects()
            .map_err(|e| e.nested(Self::EFFECTS_FIELD.name))
    }

    /// Deserialize transaction events. Returns Ok(None) if not present.
    pub fn events(
        &self,
    ) -> Result<Option<iota_sdk_types::TransactionEvents>, crate::proto::TryFromProtoError> {
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
    pub fn timestamp_ms(&self) -> Result<Option<u64>, crate::proto::TryFromProtoError> {
        self.timestamp
            .as_ref()
            .map(|ts| {
                crate::proto::proto_to_timestamp_ms(*ts)
                    .map_err(|e| e.nested(Self::TIMESTAMP_FIELD.name))
            })
            .transpose()
    }

    /// Deserialize input objects. Returns Ok(None) if not present.
    pub fn input_objects(
        &self,
    ) -> Result<Option<Vec<iota_sdk_types::Object>>, crate::proto::TryFromProtoError> {
        self.input_objects
            .as_ref()
            .map(|objs| {
                objs.objects()
                    .map_err(|e| e.nested(Self::INPUT_OBJECTS_FIELD.name))
            })
            .transpose()
    }

    /// Deserialize output objects. Returns Ok(None) if not present.
    pub fn output_objects(
        &self,
    ) -> Result<Option<Vec<iota_sdk_types::Object>>, crate::proto::TryFromProtoError> {
        self.output_objects
            .as_ref()
            .map(|objs| {
                objs.objects()
                    .map_err(|e| e.nested(Self::OUTPUT_OBJECTS_FIELD.name))
            })
            .transpose()
    }
}

// Transaction (proto) <- iota_types::transaction::Transaction
//

impl TryFrom<iota_types::transaction::Transaction> for Transaction {
    type Error = Box<dyn std::error::Error>;

    fn try_from(tx: iota_types::transaction::Transaction) -> Result<Self, Self::Error> {
        Self::merge_from(tx, &FieldMaskTree::new_wildcard())
    }
}

impl Merge<iota_types::transaction::Transaction> for Transaction {
    fn merge(
        &mut self,
        source: iota_types::transaction::Transaction,
        mask: &FieldMaskTree,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if mask.contains(Self::DIGEST_FIELD.name) {
            self.digest = Some((*source.digest()).into());
        }

        if mask.contains(Self::BCS_FIELD.name) {
            self.bcs = Some(BcsData::serialize(&source)?);
        }

        Ok(())
    }
}

impl Merge<&Transaction> for Transaction {
    fn merge(
        &mut self,
        source: &Transaction,
        mask: &FieldMaskTree,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let Transaction { bcs, digest } = source;

        if mask.contains(Self::DIGEST_FIELD.name) {
            self.digest = digest.clone();
        }

        if mask.contains(Self::BCS_FIELD.name) {
            self.bcs = bcs.clone();
        }

        Ok(())
    }
}

// TryFrom implementations for Transaction
impl TryFrom<&Transaction> for iota_sdk_types::Transaction {
    type Error = crate::proto::TryFromProtoError;

    fn try_from(value: &Transaction) -> Result<Self, Self::Error> {
        let bcs = value
            .bcs
            .as_ref()
            .ok_or_else(|| crate::proto::TryFromProtoError::missing(Transaction::BCS_FIELD.name))?;

        bcs.deserialize()
            .map_err(|e| crate::proto::TryFromProtoError::invalid(Transaction::BCS_FIELD.name, e))
    }
}

impl TryFrom<&Transaction> for iota_sdk_types::Digest {
    type Error = crate::proto::TryFromProtoError;

    fn try_from(value: &Transaction) -> Result<Self, Self::Error> {
        let digest_proto = value.digest.as_ref().ok_or_else(|| {
            crate::proto::TryFromProtoError::missing(Transaction::DIGEST_FIELD.name)
        })?;

        iota_sdk_types::Digest::from_bytes(&digest_proto.digest).map_err(|e| {
            crate::proto::TryFromProtoError::invalid(Transaction::DIGEST_FIELD.name, e)
        })
    }
}

// Convenience methods for Transaction (delegate to TryFrom)
impl Transaction {
    /// Get the transaction digest.
    pub fn digest(&self) -> Result<iota_sdk_types::Digest, crate::proto::TryFromProtoError> {
        self.try_into()
    }

    /// Deserialize the transaction from BCS.
    pub fn transaction(
        &self,
    ) -> Result<iota_sdk_types::Transaction, crate::proto::TryFromProtoError> {
        self.try_into()
    }
}
