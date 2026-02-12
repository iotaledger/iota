// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::error::Error;

use iota_grpc_types::{
    field::FieldMaskTree,
    v0::{
        bcs::BcsData,
        checkpoint::{Checkpoint, CheckpointContents, CheckpointSummary},
        epoch::{Epoch, ProtocolConfig},
        event::{Event, Events},
        object::{Object, Objects},
        signatures::{UserSignature, UserSignatures},
        transaction::{ExecutedTransaction, Transaction, TransactionEffects, TransactionEvents},
        types::{Address, ObjectReference},
    },
};
use iota_types::iota_sdk_types_conversions::SdkTypeConversionError;

pub trait Merge<T> {
    fn merge(&mut self, source: T, mask: &FieldMaskTree) -> Result<(), Box<dyn Error>>;

    fn merge_from(source: T, mask: &FieldMaskTree) -> Result<Self, Box<dyn Error>>
    where
        Self: std::default::Default,
    {
        let mut message = Self::default();
        message.merge(source, mask)?;
        Ok(message)
    }
}

// Epoch implementations
impl Merge<&Epoch> for Epoch {
    fn merge(
        &mut self,
        source: &Epoch,
        mask: &FieldMaskTree,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let Epoch {
            epoch,
            committee,
            bcs_system_state,
            first_checkpoint,
            last_checkpoint,
            start,
            end,
            reference_gas_price,
            protocol_config,
            ..
        } = source;

        if mask.contains(Self::EPOCH_FIELD.name) {
            self.epoch = *epoch;
        }

        if mask.contains(Self::COMMITTEE_FIELD.name) {
            self.committee = committee.to_owned();
        }

        if mask.contains(Self::BCS_SYSTEM_STATE_FIELD.name) {
            self.bcs_system_state = bcs_system_state.to_owned();
        }

        if mask.contains(Self::FIRST_CHECKPOINT_FIELD.name) {
            self.first_checkpoint = first_checkpoint.to_owned();
        }

        if mask.contains(Self::LAST_CHECKPOINT_FIELD.name) {
            self.last_checkpoint = last_checkpoint.to_owned();
        }

        if mask.contains(Self::START_FIELD.name) {
            self.start = start.to_owned();
        }

        if mask.contains(Self::END_FIELD.name) {
            self.end = end.to_owned();
        }

        if mask.contains(Self::REFERENCE_GAS_PRICE_FIELD.name) {
            self.reference_gas_price = reference_gas_price.to_owned();
        }

        if let Some(submask) = mask.subtree(Self::PROTOCOL_CONFIG_FIELD.name) {
            self.protocol_config = protocol_config
                .as_ref()
                .map(|config| ProtocolConfig::merge_from(config, &submask))
                .transpose()?;
        }

        Ok(())
    }
}

impl Merge<&ProtocolConfig> for ProtocolConfig {
    fn merge(
        &mut self,
        source: &ProtocolConfig,
        mask: &FieldMaskTree,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let ProtocolConfig {
            protocol_version,
            feature_flags,
            attributes,
            ..
        } = source;

        if mask.contains(Self::PROTOCOL_VERSION_FIELD.name) {
            self.protocol_version = *protocol_version;
        }

        if mask.contains(Self::FEATURE_FLAGS_FIELD.name) {
            self.feature_flags = feature_flags.to_owned();
        }

        if mask.contains(Self::ATTRIBUTES_FIELD.name) {
            self.attributes = attributes.to_owned();
        }

        Ok(())
    }
}

impl Merge<ProtocolConfig> for ProtocolConfig {
    fn merge(
        &mut self,
        source: ProtocolConfig,
        mask: &FieldMaskTree,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let ProtocolConfig {
            protocol_version,
            feature_flags,
            attributes,
            ..
        } = source;

        if mask.contains(Self::PROTOCOL_VERSION_FIELD.name) {
            self.protocol_version = protocol_version;
        }

        if mask.contains(Self::FEATURE_FLAGS_FIELD.name) {
            self.feature_flags = feature_flags;
        }

        if mask.contains(Self::ATTRIBUTES_FIELD.name) {
            self.attributes = attributes;
        }

        Ok(())
    }
}

// Signature implementations
impl Merge<iota_types::signature::GenericSignature> for UserSignature {
    fn merge(
        &mut self,
        source: iota_types::signature::GenericSignature,
        mask: &FieldMaskTree,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if !mask.contains(Self::BCS_FIELD.name) {
            // No need to convert if no field is requested
            return Ok(());
        }

        let sdk_signature: iota_sdk_types::UserSignature = source
            .try_into()
            .map_err(|e| format!("Failed to convert signature: {}", e))?;

        Merge::merge(self, sdk_signature, mask)
    }
}

impl Merge<iota_sdk_types::UserSignature> for UserSignature {
    fn merge(
        &mut self,
        source: iota_sdk_types::UserSignature,
        mask: &FieldMaskTree,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if mask.contains(Self::BCS_FIELD.name) {
            self.bcs = Some(BcsData::serialize(&source)?);
        }

        Ok(())
    }
}

impl Merge<&UserSignature> for UserSignature {
    fn merge(
        &mut self,
        source: &UserSignature,
        mask: &FieldMaskTree,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let UserSignature { bcs, .. } = source;

        if mask.contains(Self::BCS_FIELD.name) {
            self.bcs = bcs.clone();
        }

        Ok(())
    }
}

impl Merge<iota_types::transaction::Transaction> for UserSignatures {
    fn merge(
        &mut self,
        source: iota_types::transaction::Transaction,
        mask: &FieldMaskTree,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // Get signatures directly from transaction without converting the whole
        // transaction
        let tx_signatures = source.tx_signatures();

        self.signatures = tx_signatures
            .iter()
            .map(|sig| {
                // Convert iota_types signature to SDK signature, then merge
                let sdk_sig: iota_sdk_types::UserSignature = sig
                    .clone()
                    .try_into()
                    .map_err(|e| format!("Failed to convert signature: {e}"))?;
                UserSignature::merge_from(sdk_sig, mask)
            })
            .collect::<Result<Vec<_>, _>>()?;

        Ok(())
    }
}

impl Merge<&iota_sdk_types::SignedTransaction> for UserSignatures {
    fn merge(
        &mut self,
        source: &iota_sdk_types::SignedTransaction,
        mask: &FieldMaskTree,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(signatures_mask) = mask.subtree(Self::SIGNATURES_FIELD.name) {
            self.signatures = source
                .signatures
                .iter()
                .map(|sig| UserSignature::merge_from(sig.clone(), &signatures_mask))
                .collect::<Result<Vec<_>, _>>()?;
        }

        Ok(())
    }
}

impl Merge<&UserSignatures> for UserSignatures {
    fn merge(
        &mut self,
        source: &UserSignatures,
        mask: &FieldMaskTree,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(signatures_mask) = mask.subtree(Self::SIGNATURES_FIELD.name) {
            self.signatures = source
                .signatures
                .iter()
                .map(|sig| UserSignature::merge_from(sig, &signatures_mask))
                .collect::<Result<Vec<_>, _>>()?;
        }

        Ok(())
    }
}

// Event implementations
impl Merge<&iota_sdk_types::TransactionEvents> for Events {
    fn merge(
        &mut self,
        source: &iota_sdk_types::TransactionEvents,
        mask: &FieldMaskTree,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(events_mask) = mask.subtree(Self::EVENTS_FIELD.name) {
            // TransactionEvents is a tuple struct with Vec<Event> at index 0
            self.events = source
                .0
                .iter()
                .map(|event| -> Result<_, Box<dyn std::error::Error>> {
                    Merge::merge_from(event, &events_mask)
                })
                .collect::<Result<Vec<_>, _>>()?;
        }

        Ok(())
    }
}

impl Merge<&iota_sdk_types::Event> for Event {
    fn merge(
        &mut self,
        source: &iota_sdk_types::Event,
        mask: &FieldMaskTree,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if mask.contains(Self::BCS_FIELD.name) {
            self.bcs = Some(BcsData::serialize(&source)?);
        }

        if mask.contains(Self::PACKAGE_ID_FIELD.name) {
            self.package_id =
                Some(Address::default().with_address(source.package_id.as_bytes().to_vec()));
        }

        if mask.contains(Self::MODULE_FIELD.name) {
            self.module = Some(source.module.to_string());
        }

        if mask.contains(Self::SENDER_FIELD.name) {
            self.sender = Some(Address::default().with_address(source.sender.as_bytes().to_vec()));
        }

        if mask.contains(Self::EVENT_TYPE_FIELD.name) {
            self.event_type = Some(source.type_.to_string());
        }

        if mask.contains(Self::BCS_CONTENTS_FIELD.name) {
            self.bcs_contents = Some(BcsData::default().with_data(source.contents.clone()));
        }

        Ok(())

        // json_contents is not populated here by default - it requires Move
        // type layout information which is not available at this level.
        // The caller should use `populate_json_contents_with_layout` if
        // json_contents is needed.
    }
}

// Object implementations
impl Merge<iota_types::object::Object> for Object {
    fn merge(
        &mut self,
        source: iota_types::object::Object,
        mask: &FieldMaskTree,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if !mask.contains(Self::REFERENCE_FIELD.name) && !mask.contains(Self::BCS_FIELD.name) {
            // No need to convert if no field is requested
            return Ok(());
        }

        // TODO: wrap Object into a type with a version
        let sdk_object: iota_sdk_types::object::Object = source
            .try_into()
            .map_err(|e: SdkTypeConversionError| format!("Failed to convert SDK object: {}", e))?;

        Merge::merge(self, &sdk_object, mask)
    }
}

// TODO: wrap Object into a type with a version
impl Merge<&iota_sdk_types::object::Object> for Object {
    fn merge(
        &mut self,
        source: &iota_sdk_types::object::Object,
        mask: &FieldMaskTree,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if mask.contains(Self::BCS_FIELD.name) {
            self.bcs = Some(BcsData::serialize(source)?);
        }

        if mask.contains(Self::REFERENCE_FIELD.name) {
            let reference = if let Some(reference_mask) = mask.subtree(Self::REFERENCE_FIELD.name) {
                // Check for nested fields within reference
                let mut ref_builder = ObjectReference::default();

                if reference_mask.contains(ObjectReference::OBJECT_ID_FIELD.name) {
                    ref_builder = ref_builder.with_object_id(source.object_id().to_string());
                }

                if reference_mask.contains(ObjectReference::VERSION_FIELD.name) {
                    ref_builder = ref_builder.with_version(source.version());
                }

                if reference_mask.contains(ObjectReference::DIGEST_FIELD.name) {
                    ref_builder = ref_builder.with_digest(source.digest());
                }

                ref_builder
            } else {
                // If no subtree, include all reference fields
                ObjectReference::default()
                    .with_object_id(source.object_id().to_string())
                    .with_version(source.version())
                    .with_digest(source.digest())
            };

            self.reference = Some(reference);
        }

        Ok(())
    }
}

impl Merge<&Object> for Object {
    fn merge(
        &mut self,
        source: &Object,
        mask: &FieldMaskTree,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if mask.contains(Self::REFERENCE_FIELD.name) {
            self.reference = source.reference.clone();
        }

        if mask.contains(Self::BCS_FIELD.name) {
            self.bcs = source.bcs.clone();
        }

        Ok(())
    }
}

impl Merge<Option<Vec<iota_types::object::Object>>> for Objects {
    fn merge(
        &mut self,
        source: Option<Vec<iota_types::object::Object>>,
        mask: &FieldMaskTree,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // Objects is a wrapper message containing a repeated field `objects`.
        // When a user requests the wrapper (e.g., "input_objects"), the mask becomes
        // a wildcard since it's a leaf node. Calling subtree("objects") on a wildcard
        // returns Some(wildcard), which populates the objects array.
        // When a user requests specific fields (e.g., "input_objects.objects.bcs"),
        // subtree("objects") returns the sub-mask with the requested fields.
        if let Some(objects_mask) = mask.subtree(Self::OBJECTS_FIELD.name) {
            if let Some(objects) = source {
                // Merge each object in the source list with the appropriate field mask
                self.objects = objects
                    .into_iter()
                    .map(|obj| Object::merge_from(obj, &objects_mask))
                    .collect::<Result<Vec<_>, _>>()?;
            }
        }

        Ok(())
    }
}

impl Merge<&Objects> for Objects {
    fn merge(
        &mut self,
        source: &Objects,
        mask: &FieldMaskTree,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(objects_mask) = mask.subtree(Self::OBJECTS_FIELD.name) {
            self.objects = source
                .objects
                .iter()
                .map(|obj| Object::merge_from(obj, &objects_mask))
                .collect::<Result<Vec<_>, _>>()?;
        }

        Ok(())
    }
}

// Checkpoint implementations
impl Merge<iota_sdk_types::CheckpointSummary> for CheckpointSummary {
    fn merge(
        &mut self,
        source: iota_sdk_types::CheckpointSummary,
        mask: &FieldMaskTree,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if mask.contains(Self::BCS_FIELD.name) {
            self.bcs = Some(BcsData::serialize(&source)?);
        }

        if mask.contains(Self::DIGEST_FIELD.name) {
            self.digest = Some(source.digest().into());
        }

        Ok(())
    }
}

impl Merge<&CheckpointSummary> for CheckpointSummary {
    fn merge(
        &mut self,
        source: &CheckpointSummary,
        mask: &FieldMaskTree,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let CheckpointSummary { bcs, digest, .. } = source;

        if mask.contains(Self::DIGEST_FIELD.name) {
            self.digest = digest.clone();
        }

        if mask.contains(Self::BCS_FIELD.name) {
            self.bcs = bcs.clone();
        }

        Ok(())
    }
}

impl Merge<iota_sdk_types::CheckpointContents> for CheckpointContents {
    fn merge(
        &mut self,
        source: iota_sdk_types::CheckpointContents,
        mask: &FieldMaskTree,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if mask.contains(Self::BCS_FIELD.name) {
            // TODO: add version
            self.bcs = Some(BcsData::serialize(&source)?);
        }

        if mask.contains(Self::DIGEST_FIELD.name) {
            self.digest = Some(source.digest().into());
        }

        Ok(())
    }
}

impl Merge<&CheckpointContents> for CheckpointContents {
    fn merge(
        &mut self,
        source: &CheckpointContents,
        mask: &FieldMaskTree,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let CheckpointContents { bcs, digest, .. } = source;

        if mask.contains(Self::BCS_FIELD.name) {
            self.bcs = bcs.clone();
        }

        if mask.contains(Self::DIGEST_FIELD.name) {
            self.digest = digest.clone();
        }

        Ok(())
    }
}

impl Merge<&iota_sdk_types::CheckpointSummary> for Checkpoint {
    fn merge(
        &mut self,
        source: &iota_sdk_types::CheckpointSummary,
        mask: &FieldMaskTree,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(submask) = mask.subtree(Self::SUMMARY_FIELD.name) {
            self.summary = Some(CheckpointSummary::merge_from(source.clone(), &submask)?);
        }

        Ok(())
    }
}

impl Merge<iota_sdk_types::ValidatorAggregatedSignature> for Checkpoint {
    fn merge(
        &mut self,
        source: iota_sdk_types::ValidatorAggregatedSignature,
        mask: &FieldMaskTree,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if mask.contains(Self::SIGNATURE_FIELD.name) {
            self.signature = Some(source.into());
        }

        Ok(())
    }
}

impl Merge<iota_sdk_types::CheckpointContents> for Checkpoint {
    fn merge(
        &mut self,
        source: iota_sdk_types::CheckpointContents,
        mask: &FieldMaskTree,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(submask) = mask.subtree(Self::CONTENTS_FIELD.name) {
            self.contents = Some(CheckpointContents::merge_from(source, &submask)?);
        }

        Ok(())
    }
}

impl Merge<&Checkpoint> for Checkpoint {
    fn merge(
        &mut self,
        source: &Checkpoint,
        mask: &FieldMaskTree,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let Checkpoint {
            sequence_number,
            summary,
            signature,
            contents,
            ..
        } = source;

        if mask.contains(Self::SEQUENCE_NUMBER_FIELD.name) {
            self.sequence_number = *sequence_number;
        }

        if let Some(submask) = mask.subtree(Self::SUMMARY_FIELD.name) {
            self.summary = summary
                .as_ref()
                .map(|summary| CheckpointSummary::merge_from(summary, &submask))
                .transpose()?;
        }

        if mask.contains(Self::SIGNATURE_FIELD.name) {
            self.signature = signature.clone();
        }

        if let Some(submask) = mask.subtree(Self::CONTENTS_FIELD.name) {
            self.contents = contents
                .as_ref()
                .map(|contents| CheckpointContents::merge_from(contents, &submask))
                .transpose()?;
        }

        Ok(())
    }
}

// Transaction implementations
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
            self.bcs = Some(BcsData::serialize(source)?);
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
            self.events = Some(Events::merge_from(source, &events_mask)?);
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
            ..
        } = source;

        if let Some(submask) = mask.subtree(Self::TRANSACTION_FIELD.name) {
            if let Some(tx) = transaction {
                self.transaction = Some(Transaction::merge_from(tx, &submask)?);
            }
        }

        if let Some(submask) = mask.subtree(Self::SIGNATURES_FIELD.name) {
            if let Some(sigs) = signatures {
                self.signatures = Some(UserSignatures::merge_from(sigs, &submask)?);
            }
        }

        if let Some(submask) = mask.subtree(Self::EFFECTS_FIELD.name) {
            if let Some(fx) = effects {
                self.effects = Some(TransactionEffects::merge_from(fx, &submask)?);
            }
        }

        if let Some(submask) = mask.subtree(Self::EVENTS_FIELD.name) {
            if let Some(ev) = events {
                self.events = Some(TransactionEvents::merge_from(ev, &submask)?);
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
                self.input_objects = Some(Objects::merge_from(objs, &submask)?);
            }
        }

        if let Some(submask) = mask.subtree(Self::OUTPUT_OBJECTS_FIELD.name) {
            if let Some(objs) = output_objects {
                self.output_objects = Some(Objects::merge_from(objs, &submask)?);
            }
        }

        Ok(())
    }
}

impl Merge<iota_types::transaction::Transaction> for Transaction {
    fn merge(
        &mut self,
        source: iota_types::transaction::Transaction,
        mask: &FieldMaskTree,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if !mask.contains(Self::DIGEST_FIELD.name) && !mask.contains(Self::BCS_FIELD.name) {
            // No need to convert if no field is requested
            return Ok(());
        }

        let sdk_transaction: iota_sdk_types::Transaction = source
            .transaction_data()
            .clone()
            .try_into()
            .map_err(|e| format!("failed to convert transaction to SDK type: {e}"))?;

        Merge::merge(self, &sdk_transaction, mask)
    }
}

impl Merge<&iota_sdk_types::Transaction> for Transaction {
    fn merge(
        &mut self,
        source: &iota_sdk_types::Transaction,
        mask: &FieldMaskTree,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if mask.contains(Self::DIGEST_FIELD.name) {
            self.digest = Some((source.digest()).into());
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
        let Transaction { bcs, digest, .. } = source;

        if mask.contains(Self::DIGEST_FIELD.name) {
            self.digest = digest.clone();
        }

        if mask.contains(Self::BCS_FIELD.name) {
            self.bcs = bcs.clone();
        }

        Ok(())
    }
}
