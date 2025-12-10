// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::sync::Arc;

use iota_grpc_types::{
    field::FieldMaskTree,
    merge::Merge,
    proto::timestamp_ms_to_proto,
    v0::{
        bcs as grpc_bcs, event as grpc_event, object as grpc_obj, signatures as grpc_sig,
        transaction as grpc_tx,
    },
};

use crate::GrpcReader;

/// Source data bundle for populating gRPC transaction response messages.
///
/// Different gRPC endpoints return different message types based on client
/// needs:
/// - `GetTransaction`: `Transaction` (digest + BCS)
/// - `ExecuteTransaction`: `ExecutedTransaction` (effects, events, signatures)
/// - Other endpoints may return specific subsets like `UserSignatures`
///
/// This struct bundles all transaction-related data in one place, allowing the
/// `Merge` implementation to populate any response type from a common source.
/// Each response type's `Merge` impl extracts only the fields it needs.
//
/// # Note
/// The digest is stored separately even though it's derivable from
/// `transaction.data` because `iota_sdk_types::SignedTransaction` doesn't
/// expose a `digest()` method, and the digest is computed externally from
/// `iota_types::TransactionData`.
pub struct TransactionReadSource<'a> {
    pub reader: Arc<GrpcReader>,
    pub config: &'a iota_config::node::GrpcApiConfig,
    pub transaction_data: iota_types::transaction::TransactionData,
    pub signatures: Option<Vec<iota_types::signature::GenericSignature>>,
    pub effects: Option<iota_types::effects::TransactionEffects>,
    pub events: Option<iota_types::effects::TransactionEvents>,
    pub checkpoint: Option<u64>,
    pub timestamp_ms: Option<u64>,
    pub input_objects: Option<Vec<iota_types::object::Object>>,
    pub output_objects: Option<Vec<iota_types::object::Object>>,
}

impl Merge<&TransactionReadSource<'_>> for grpc_tx::ExecutedTransaction {
    fn merge(
        &mut self,
        source: &TransactionReadSource<'_>,
        mask: &FieldMaskTree,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // Set transaction if requested
        if let Some(tx_mask) = mask.subtree(Self::TRANSACTION_FIELD.name) {
            self.transaction = Some(grpc_tx::Transaction::merge_from(source, &tx_mask)?);
        }

        // Set signatures if requested
        if let Some(signatures_mask) = mask.subtree(Self::SIGNATURES_FIELD.name) {
            self.signatures = Some(grpc_sig::UserSignatures::merge_from(
                source,
                &signatures_mask,
            )?);
        }

        // Set effects if requested
        if let Some(effects_mask) = mask.subtree(Self::EFFECTS_FIELD.name) {
            self.effects = Some(grpc_tx::TransactionEffects::merge_from(
                source,
                &effects_mask,
            )?);
        }

        if let Some(events_mask) = mask.subtree(Self::EVENTS_FIELD.name) {
            self.events = Some(grpc_tx::TransactionEvents::merge_from(
                source,
                &events_mask,
            )?);
        }

        // Set checkpoint if requested
        if mask.contains(Self::CHECKPOINT_FIELD.name) {
            self.checkpoint = source.checkpoint;
        }

        // Set timestamp if requested
        if mask.contains(Self::TIMESTAMP_FIELD.name) {
            self.timestamp = source.timestamp_ms.map(timestamp_ms_to_proto);
        }

        // Handle input_objects if requested
        if let Some(input_objects_mask) = mask.subtree(Self::INPUT_OBJECTS_FIELD.name) {
            self.input_objects = Some(grpc_obj::Objects::merge_from(
                source.input_objects.clone(),
                &input_objects_mask,
            )?);
        }

        // Handle output_objects if requested
        if let Some(output_objects_mask) = mask.subtree(Self::OUTPUT_OBJECTS_FIELD.name) {
            self.output_objects = Some(grpc_obj::Objects::merge_from(
                source.output_objects.clone(),
                &output_objects_mask,
            )?);
        }

        Ok(())
    }
}

impl Merge<&TransactionReadSource<'_>> for grpc_tx::Transaction {
    fn merge(
        &mut self,
        source: &TransactionReadSource<'_>,
        mask: &FieldMaskTree,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if !mask.contains(Self::DIGEST_FIELD.name) && !mask.contains(Self::BCS_FIELD.name) {
            // No need to convert if no field is requested
            return Ok(());
        }

        let sdk_transaction: iota_sdk_types::Transaction = source
            .transaction_data
            .clone()
            .try_into()
            .map_err(|e| format!("failed to convert transaction to SDK type: {e}"))?;

        // Set digest if requested
        if mask.contains(Self::DIGEST_FIELD.name) {
            self.digest = Some(sdk_transaction.digest().into());
        }

        // Set BCS if requested
        if mask.contains(Self::BCS_FIELD.name) {
            self.bcs = grpc_bcs::BcsData::serialize(&sdk_transaction).ok();
        }

        Ok(())
    }
}

impl Merge<&TransactionReadSource<'_>> for grpc_tx::TransactionEffects {
    fn merge(
        &mut self,
        source: &TransactionReadSource<'_>,
        mask: &FieldMaskTree,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let Some(effects) = source.effects.as_ref() else {
            return Ok(());
        };

        Merge::merge(self, effects.clone(), mask)
    }
}

impl Merge<&TransactionReadSource<'_>> for grpc_tx::TransactionEvents {
    fn merge(
        &mut self,
        source: &TransactionReadSource<'_>,
        mask: &FieldMaskTree,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let Some(events) = source.events.as_ref() else {
            return Ok(());
        };

        Self::merge(self, events.clone(), mask)?;

        if mask
            .subtree(Self::EVENTS_FIELD.name)
            .is_some_and(|event_mask| {
                event_mask.contains(grpc_event::Event::JSON_CONTENTS_FIELD.name)
            })
        {
            match self.events.as_mut() {
                None => return Ok(()),
                Some(proto_events) => {
                    for (message, event) in proto_events.events.iter_mut().zip(&events.data) {
                        // Populate json_contents if we have a valid datatype layout
                        message.json_contents = crate::utils::render_json(
                            source.reader.clone(),
                            source.config.max_json_move_value_size,
                            &iota_types::TypeTag::Struct(Box::new(event.type_.clone())),
                            &event.contents,
                        )
                        .map(Box::new);
                    }
                }
            }
        }

        Ok(())
    }
}

// UserSignatures
//
impl Merge<&TransactionReadSource<'_>> for grpc_sig::UserSignatures {
    fn merge(
        &mut self,
        source: &TransactionReadSource<'_>,
        mask: &FieldMaskTree,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(signatures_mask) = mask.subtree(Self::SIGNATURES_FIELD.name) {
            if let Some(signatures) = source.signatures.as_ref() {
                self.signatures = signatures
                    .iter()
                    .map(|sig| grpc_sig::UserSignature::merge_from(sig.clone(), &signatures_mask))
                    .collect::<Result<Vec<_>, _>>()?;
            }
        }

        Ok(())
    }
}
