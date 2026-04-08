// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::sync::Arc;

use iota_grpc_types::{
    field::FieldMaskTree,
    proto::timestamp_ms_to_proto,
    v1::{
        bcs::{self as grpc_bcs, BcsData},
        command::{CommandOutput, CommandOutputs, CommandResult, CommandResults},
        event as grpc_event, object as grpc_obj, signatures as grpc_sig, transaction as grpc_tx,
    },
};
use iota_types::iota_sdk_types_conversions::type_tag_core_to_sdk;

use crate::{GrpcReader, error::RpcError, merge::Merge, utils::render_json};

/// Source for building ExecutedTransaction using the Merge trait
pub struct TransactionReadSource<'a> {
    pub reader: Arc<GrpcReader>,
    pub config: &'a iota_config::node::GrpcApiConfig,
    pub transaction: Option<iota_sdk_types::transaction::Transaction>,
    pub signatures: Option<Vec<iota_sdk_types::UserSignature>>,
    pub effects: Option<iota_types::effects::TransactionEffects>,
    pub events: Option<iota_types::effects::TransactionEvents>,
    pub checkpoint: Option<u64>,
    pub timestamp_ms: Option<u64>,
    pub input_objects: Option<Vec<iota_types::object::Object>>,
    pub output_objects: Option<Vec<iota_types::object::Object>>,
}

impl Merge<&TransactionReadSource<'_>> for grpc_tx::ExecutedTransaction {
    type Error = RpcError;

    fn merge(
        &mut self,
        source: &TransactionReadSource<'_>,
        mask: &FieldMaskTree,
    ) -> Result<(), Self::Error> {
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
    type Error = RpcError;

    fn merge(
        &mut self,
        source: &TransactionReadSource<'_>,
        mask: &FieldMaskTree,
    ) -> Result<(), Self::Error> {
        if !mask.contains(Self::DIGEST_FIELD.name) && !mask.contains(Self::BCS_FIELD.name) {
            // No need to convert if no field is requested
            return Ok(());
        }

        let transaction = source
            .transaction
            .as_ref()
            .ok_or_else(|| RpcError::internal().with_context("missing transaction"))?;

        // Set digest if requested
        if mask.contains(Self::DIGEST_FIELD.name) {
            self.digest = Some(transaction.digest().into());
        }

        // Set BCS if requested
        if mask.contains(Self::BCS_FIELD.name) {
            self.bcs = Some(grpc_bcs::BcsData::serialize(transaction)?);
        }

        Ok(())
    }
}

impl Merge<&TransactionReadSource<'_>> for grpc_tx::TransactionEffects {
    type Error = RpcError;

    fn merge(
        &mut self,
        source: &TransactionReadSource<'_>,
        mask: &FieldMaskTree,
    ) -> Result<(), Self::Error> {
        let Some(effects) = source.effects.as_ref() else {
            return Ok(());
        };

        Merge::merge(self, effects.clone(), mask)
    }
}

impl Merge<&TransactionReadSource<'_>> for grpc_tx::TransactionEvents {
    type Error = RpcError;

    fn merge(
        &mut self,
        source: &TransactionReadSource<'_>,
        mask: &FieldMaskTree,
    ) -> Result<(), Self::Error> {
        // Use unwrap_or_default so that when no events were emitted we still
        // compute a real digest (hash of the empty list) and populate an empty
        // events vec — to distinguish between "no events" and "events
        // not requested in the mask".
        let events = source.events.clone().unwrap_or_default();

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
                        );
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
    type Error = RpcError;

    fn merge(
        &mut self,
        source: &TransactionReadSource<'_>,
        mask: &FieldMaskTree,
    ) -> Result<(), Self::Error> {
        // Use mask directly — UserSignatures is a transparent wrapper
        if let Some(signatures) = source.signatures.as_ref() {
            self.signatures = signatures
                .iter()
                .map(|sig| grpc_sig::UserSignature::merge_from(sig.clone(), mask))
                .collect::<Result<Vec<_>, _>>()?;
        }

        Ok(())
    }
}

/// Source for building CommandResults using the Merge trait
pub struct CommandResultsReadSource<'a> {
    pub reader: Arc<GrpcReader>,
    pub config: &'a iota_config::node::GrpcApiConfig,
    pub execution_results: Vec<iota_types::execution::ExecutionResult>,
}

impl Merge<&CommandResultsReadSource<'_>> for CommandResults {
    type Error = RpcError;

    fn merge(
        &mut self,
        source: &CommandResultsReadSource<'_>,
        mask: &FieldMaskTree,
    ) -> Result<(), Self::Error> {
        // Use mask directly — CommandResults is a transparent wrapper
        self.results = source
            .execution_results
            .iter()
            .map(|(mutable_reference_outputs, return_values)| {
                let result_source = CommandResultReadSource {
                    reader: &source.reader,
                    config: source.config,
                    mutable_reference_outputs,
                    return_values,
                };
                CommandResult::merge_from(&result_source, mask)
            })
            .collect::<Result<Vec<_>, _>>()?;

        Ok(())
    }
}

/// Source for building a single CommandResult
struct CommandResultReadSource<'a> {
    reader: &'a Arc<GrpcReader>,
    config: &'a iota_config::node::GrpcApiConfig,
    mutable_reference_outputs: &'a [(
        iota_types::transaction::Argument,
        Vec<u8>,
        iota_types::TypeTag,
    )],
    return_values: &'a [(Vec<u8>, iota_types::TypeTag)],
}

impl Merge<&CommandResultReadSource<'_>> for CommandResult {
    type Error = RpcError;

    fn merge(
        &mut self,
        source: &CommandResultReadSource<'_>,
        mask: &FieldMaskTree,
    ) -> Result<(), Self::Error> {
        if let Some(mutated_mask) = mask.subtree(Self::MUTATED_BY_REF_FIELD.name) {
            let outputs_source = CommandOutputsReadSource {
                reader: source.reader,
                config: source.config,
                outputs: source
                    .mutable_reference_outputs
                    .iter()
                    .map(|(arg, bcs_bytes, ty)| (Some(*arg), bcs_bytes.as_slice(), ty))
                    .collect(),
            };
            self.mutated_by_ref = Some(CommandOutputs::merge_from(&outputs_source, &mutated_mask)?);
        }
        if let Some(return_values_mask) = mask.subtree(Self::RETURN_VALUES_FIELD.name) {
            let outputs_source = CommandOutputsReadSource {
                reader: source.reader,
                config: source.config,
                outputs: source
                    .return_values
                    .iter()
                    .map(|(bcs_bytes, ty)| (None, bcs_bytes.as_slice(), ty))
                    .collect(),
            };
            self.return_values = Some(CommandOutputs::merge_from(
                &outputs_source,
                &return_values_mask,
            )?);
        }

        Ok(())
    }
}

/// Source for building CommandOutputs
struct CommandOutputsReadSource<'a> {
    reader: &'a Arc<GrpcReader>,
    config: &'a iota_config::node::GrpcApiConfig,
    outputs: Vec<(
        Option<iota_types::transaction::Argument>,
        &'a [u8],
        &'a iota_types::TypeTag,
    )>,
}

impl Merge<&CommandOutputsReadSource<'_>> for CommandOutputs {
    type Error = RpcError;

    fn merge(
        &mut self,
        source: &CommandOutputsReadSource<'_>,
        mask: &FieldMaskTree,
    ) -> Result<(), Self::Error> {
        // Use mask directly — CommandOutputs is a transparent wrapper
        self.outputs = source
            .outputs
            .iter()
            .map(|(arg, bcs_bytes, ty)| {
                let output_source = CommandOutputReadSource {
                    reader: source.reader,
                    config: source.config,
                    arg: *arg,
                    bcs_bytes,
                    ty,
                };
                CommandOutput::merge_from(&output_source, mask)
            })
            .collect::<Result<Vec<_>, _>>()?;

        Ok(())
    }
}

/// Source for building a single CommandOutput
struct CommandOutputReadSource<'a> {
    reader: &'a Arc<GrpcReader>,
    config: &'a iota_config::node::GrpcApiConfig,
    arg: Option<iota_types::transaction::Argument>,
    bcs_bytes: &'a [u8],
    ty: &'a iota_types::TypeTag,
}

impl Merge<&CommandOutputReadSource<'_>> for CommandOutput {
    type Error = RpcError;

    fn merge(
        &mut self,
        source: &CommandOutputReadSource<'_>,
        mask: &FieldMaskTree,
    ) -> Result<(), Self::Error> {
        if mask.contains(Self::ARGUMENT_FIELD.name) {
            self.argument = source
                .arg
                .map(|arg| -> Result<_, RpcError> {
                    let sdk_arg: iota_sdk_types::Argument = arg.into();
                    sdk_arg
                        .try_into()
                        .map_err(|e| RpcError::from(e).with_context("failed to merge argument"))
                })
                .transpose()?;
        }

        if mask.contains(Self::TYPE_TAG_FIELD.name) {
            self.type_tag = Some({
                let sdk_type_tag = type_tag_core_to_sdk(source.ty.clone())
                    .map_err(|e| RpcError::from(e).with_context("failed to convert type tag"))?;
                (&sdk_type_tag).into()
            });
        }

        if mask.contains(Self::BCS_FIELD.name) {
            self.bcs = Some(BcsData::default().with_data(source.bcs_bytes.to_vec()));
        }

        if mask.contains(Self::JSON_FIELD.name) {
            self.json = render_json(
                source.reader.clone(),
                source.config.max_json_move_value_size,
                source.ty,
                source.bcs_bytes,
            );
        }

        Ok(())
    }
}
