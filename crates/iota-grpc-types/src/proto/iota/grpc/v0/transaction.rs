// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

include!("../../../generated/iota.grpc.v0.transaction.rs");
include!("../../../generated/iota.grpc.v0.transaction.field_info.rs");
include!("../../../generated/iota.grpc.v0.transaction.accessors.rs");

use crate::{field::FieldMaskTree, merge::Merge, v0::bcs::BcsData};

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

// ExecutedTransaction
//

impl TryFrom<iota_types::full_checkpoint_content::CheckpointTransaction> for ExecutedTransaction {
    type Error = Box<dyn std::error::Error>;

    fn try_from(
        transaction: iota_types::full_checkpoint_content::CheckpointTransaction,
    ) -> Result<Self, Self::Error> {
        Self::merge_from(transaction, &FieldMaskTree::new_wildcard())
    }
}

impl Merge<iota_types::full_checkpoint_content::CheckpointTransaction> for ExecutedTransaction {
    fn merge(
        &mut self,
        source: iota_types::full_checkpoint_content::CheckpointTransaction,
        mask: &FieldMaskTree,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(submask) = mask.subtree(Self::TRANSACTION_FIELD.name) {
            self.transaction = Some(crate::v0::transaction::Transaction::merge_from(
                source.transaction.clone(),
                &submask,
            )?);
        }

        if let Some(submask) = mask.subtree(Self::EFFECTS_FIELD.name) {
            self.effects = Some(crate::v0::transaction::TransactionEffects::merge_from(
                source.effects.clone(),
                &submask,
            )?);
        }

        if let Some(submask) = mask.subtree(Self::EVENTS_FIELD.name) {
            if let Some(events) = source.events {
                self.events = Some(crate::v0::transaction::TransactionEvents::merge_from(
                    events, &submask,
                )?);
            }
        }

        if let Some(submask) = mask.subtree(Self::INPUT_OBJECTS_FIELD.name) {
            self.input_objects = Some(crate::v0::object::Objects::merge_from(
                Some(source.input_objects),
                &submask,
            )?);
        }

        if let Some(submask) = mask.subtree(Self::OUTPUT_OBJECTS_FIELD.name) {
            self.output_objects = Some(crate::v0::object::Objects::merge_from(
                Some(source.output_objects),
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
