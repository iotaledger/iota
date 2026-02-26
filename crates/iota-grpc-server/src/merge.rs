// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::error::Error;

use iota_grpc_types::{
    field::FieldMaskTree,
    v0::{
        bcs::BcsData,
        checkpoint::{Checkpoint, CheckpointContents, CheckpointSummary},
        epoch::{ProtocolAttributes, ProtocolConfig, ProtocolFeatureFlags},
        event::{Event, Events},
        object::{Object, Objects},
        signatures::{UserSignature, UserSignatures},
        transaction::{ExecutedTransaction, Transaction, TransactionEffects, TransactionEvents},
        types::{Address, ObjectReference},
        versioned::{VersionedCheckpointSummary, VersionedEvent, VersionedObject},
    },
};
use iota_protocol_config::{ProtocolConfig as IotaProtocolConfig, ProtocolConfigValue};
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

impl Merge<&IotaProtocolConfig> for ProtocolConfig {
    fn merge(
        &mut self,
        source: &IotaProtocolConfig,
        mask: &FieldMaskTree,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if mask.contains(Self::PROTOCOL_VERSION_FIELD.name) {
            self.protocol_version = Some(source.version.as_u64());
        }

        // `map_field_filter` handles the wildcard-vs-explicit distinction:
        // - None: field not explicitly requested (skip)
        // - Some(None): field requested without specific keys (include all entries)
        // - Some(Some(keys)): only these keys were requested
        if let Some(filter) = mask.map_field_filter(Self::FEATURE_FLAGS_FIELD.name) {
            let flags = match filter {
                None => source.feature_map().into_iter().collect(),
                Some(keys) => source
                    .feature_map()
                    .into_iter()
                    .filter(|(k, _)| keys.contains(k))
                    .collect(),
            };
            self.feature_flags = Some(ProtocolFeatureFlags::default().with_flags(flags));
        }

        // `map_field_filter` handles the wildcard-vs-explicit distinction:
        // - None: field not explicitly requested (skip)
        // - Some(None): field requested without specific keys (include all entries)
        // - Some(Some(keys)): only these keys were requested
        if let Some(filter) = mask.map_field_filter(Self::ATTRIBUTES_FIELD.name) {
            let attrs = match filter {
                None => source
                    .attr_map()
                    .into_iter()
                    .filter_map(|(k, v)| v.map(|v| (k, protocol_config_value_to_string(v))))
                    .collect(),
                Some(keys) => source
                    .attr_map()
                    .into_iter()
                    .filter(|(k, _)| keys.contains(k))
                    .filter_map(|(k, v)| v.map(|v| (k, protocol_config_value_to_string(v))))
                    .collect(),
            };
            self.attributes = Some(ProtocolAttributes::default().with_attributes(attrs));
        }

        Ok(())
    }
}

fn protocol_config_value_to_string(v: ProtocolConfigValue) -> String {
    match v {
        ProtocolConfigValue::u16(x) => x.to_string(),
        ProtocolConfigValue::u32(x) => x.to_string(),
        ProtocolConfigValue::u64(x) => x.to_string(),
        ProtocolConfigValue::bool(x) => x.to_string(),
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
        // Use mask directly — UserSignatures is a transparent wrapper
        self.signatures = source
            .signatures
            .iter()
            .map(|sig| UserSignature::merge_from(sig.clone(), mask))
            .collect::<Result<Vec<_>, _>>()?;

        Ok(())
    }
}

impl Merge<&UserSignatures> for UserSignatures {
    fn merge(
        &mut self,
        source: &UserSignatures,
        mask: &FieldMaskTree,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // Use mask directly — UserSignatures is a transparent wrapper
        self.signatures = source
            .signatures
            .iter()
            .map(|sig| UserSignature::merge_from(sig, mask))
            .collect::<Result<Vec<_>, _>>()?;

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
        // Use mask directly — Events is a transparent wrapper
        // TransactionEvents is a tuple struct with Vec<Event> at index 0
        self.events = source
            .0
            .iter()
            .map(|event| -> Result<_, Box<dyn std::error::Error>> {
                Merge::merge_from(event, mask)
            })
            .collect::<Result<Vec<_>, _>>()?;

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
            self.bcs = Some(BcsData::serialize(&VersionedEvent::V1(source.clone()))?);
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

        let sdk_object: iota_sdk_types::object::Object = source
            .try_into()
            .map_err(|e: SdkTypeConversionError| format!("Failed to convert SDK object: {}", e))?;

        Merge::merge(self, &sdk_object, mask)
    }
}

impl Merge<&iota_sdk_types::object::Object> for Object {
    fn merge(
        &mut self,
        source: &iota_sdk_types::object::Object,
        mask: &FieldMaskTree,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if mask.contains(Self::BCS_FIELD.name) {
            self.bcs = Some(BcsData::serialize(&VersionedObject::V1(source.clone()))?);
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
        // Use mask directly — Objects is a transparent wrapper
        if let Some(objects) = source {
            self.objects = objects
                .into_iter()
                .map(|obj| Object::merge_from(obj, mask))
                .collect::<Result<Vec<_>, _>>()?;
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
        // Use mask directly — Objects is a transparent wrapper
        self.objects = source
            .objects
            .iter()
            .map(|obj| Object::merge_from(obj, mask))
            .collect::<Result<Vec<_>, _>>()?;

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
            self.bcs = Some(BcsData::serialize(&VersionedCheckpointSummary::V1(
                source.clone(),
            ))?);
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
            // CheckpointContents has a custom Serialize impl that embeds
            // a BCS enum discriminant byte, so no versioned wrapper needed.
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

#[cfg(test)]
mod tests {
    use iota_grpc_types::{field::FieldMaskUtil, v0::epoch::ProtocolConfig};
    use iota_protocol_config::{Chain, ProtocolConfig as IotaProtocolConfig};
    use prost_types::FieldMask;

    use super::*;

    fn make_iota_protocol_config() -> IotaProtocolConfig {
        IotaProtocolConfig::get_for_version(1.into(), Chain::Testnet)
    }

    // ── attributes ──────────────────────────────────────────────────────────

    #[test]
    fn test_protocol_config_merge_attributes_returns_all() {
        // "attributes" → all non-None entries from attr_map()
        let source = make_iota_protocol_config();
        let expected_count = source
            .attr_map()
            .into_values()
            .filter(Option::is_some)
            .count();
        let mask = FieldMaskTree::from_field_mask(&FieldMask::from_paths(["attributes"]));
        let result = ProtocolConfig::merge_from(&source, &mask).unwrap();
        assert_eq!(result.attributes.unwrap().attributes.len(), expected_count);
    }

    #[test]
    fn test_protocol_config_merge_explicit_attribute_key() {
        // "attributes.<key>" → only that one attribute
        let source = make_iota_protocol_config();
        let key = source
            .attr_map()
            .into_iter()
            .find(|(_, v)| v.is_some())
            .map(|(k, _)| k)
            .unwrap();
        let path = format!("attributes.{key}");
        let mask = FieldMaskTree::from_field_mask(&FieldMask::from_paths([&path]));
        let result = ProtocolConfig::merge_from(&source, &mask).unwrap();
        let attrs = result.attributes.unwrap().attributes;
        assert_eq!(attrs.len(), 1);
        assert!(attrs.contains_key(&key));
    }

    #[test]
    fn test_protocol_config_merge_multiple_attribute_keys() {
        // Multiple "attributes.<key>" → only those keys
        let source = make_iota_protocol_config();
        let keys: Vec<String> = source
            .attr_map()
            .into_iter()
            .filter(|(_, v)| v.is_some())
            .map(|(k, _)| k)
            .take(2)
            .collect();
        assert_eq!(keys.len(), 2, "expected at least 2 non-None attributes");
        let paths: Vec<String> = keys.iter().map(|k| format!("attributes.{k}")).collect();
        let mask = FieldMaskTree::from_field_mask(&FieldMask::from_paths(
            paths.iter().map(String::as_str),
        ));
        let result = ProtocolConfig::merge_from(&source, &mask).unwrap();
        let attrs = result.attributes.unwrap().attributes;
        assert_eq!(attrs.len(), 2);
        assert!(attrs.contains_key(&keys[0]));
        assert!(attrs.contains_key(&keys[1]));
    }

    // ── feature_flags ────────────────────────────────────────────────────────

    #[test]
    fn test_protocol_config_merge_flags_returns_all() {
        // "feature_flags" → all entries from feature_map()
        let source = make_iota_protocol_config();
        let expected_count = source.feature_map().len();
        let mask = FieldMaskTree::from_field_mask(&FieldMask::from_paths(["feature_flags"]));
        let result = ProtocolConfig::merge_from(&source, &mask).unwrap();
        assert_eq!(result.feature_flags.unwrap().flags.len(), expected_count);
    }

    #[test]
    fn test_protocol_config_merge_explicit_flag_key() {
        // "feature_flags.<key>" → only that one flag
        let source = make_iota_protocol_config();
        let key = source.feature_map().into_keys().next().unwrap();
        let path = format!("feature_flags.{key}");
        let mask = FieldMaskTree::from_field_mask(&FieldMask::from_paths([&path]));
        let result = ProtocolConfig::merge_from(&source, &mask).unwrap();
        let flags = result.feature_flags.unwrap().flags;
        assert_eq!(flags.len(), 1);
        assert!(flags.contains_key(&key));
    }

    #[test]
    fn test_protocol_config_merge_multiple_flag_keys() {
        // Multiple "feature_flags.<key>" → only those keys
        let source = make_iota_protocol_config();
        let keys: Vec<String> = source.feature_map().into_keys().take(2).collect();
        assert_eq!(keys.len(), 2, "expected at least 2 feature flags");
        let paths: Vec<String> = keys.iter().map(|k| format!("feature_flags.{k}")).collect();
        let mask = FieldMaskTree::from_field_mask(&FieldMask::from_paths(
            paths.iter().map(String::as_str),
        ));
        let result = ProtocolConfig::merge_from(&source, &mask).unwrap();
        let flags = result.feature_flags.unwrap().flags;
        assert_eq!(flags.len(), 2);
        assert!(flags.contains_key(&keys[0]));
        assert!(flags.contains_key(&keys[1]));
    }

    #[test]
    fn test_protocol_config_merge_wildcard_does_not_populate_maps() {
        // Bare "protocol_config" wildcard must NOT populate map fields —
        // they are only populated when explicitly named in the mask.
        let source = make_iota_protocol_config();
        let mask = FieldMaskTree::new_wildcard();
        let result = ProtocolConfig::merge_from(&source, &mask).unwrap();
        assert!(result.feature_flags.is_none());
        assert!(result.attributes.is_none());
    }

    // ── misc ─────────────────────────────────────────────────────────────────

    #[test]
    fn test_protocol_config_merge_version_only() {
        // "protocol_version" → version set, no map fields populated
        let source = make_iota_protocol_config();
        let expected_version = source.version.as_u64();
        let mask = FieldMaskTree::from_field_mask(&FieldMask::from_paths(["protocol_version"]));
        let result = ProtocolConfig::merge_from(&source, &mask).unwrap();
        assert_eq!(result.protocol_version, Some(expected_version));
        assert!(result.feature_flags.is_none());
        assert!(result.attributes.is_none());
    }
}
