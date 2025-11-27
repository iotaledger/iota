// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{str::FromStr, sync::Arc};

use iota_grpc_types::{
    field::FieldMaskTree,
    google::rpc::bad_request::FieldViolation,
    merge::Merge,
    v0::{
        bcs::BcsData,
        command::{
            Argument as ProtoArgument, CommandOutput, CommandOutputs, CommandResult,
            CommandResults, argument,
        },
        error_reason::ErrorReason,
        event::Event,
        transaction::{
            ExecutedTransaction, TransactionEvents as ProtoTransactionEvents, TransactionReadSource,
        },
        transaction_execution_service::{
            SimulateTransactionRequest, SimulateTransactionResponse,
            simulate_transaction_request::TransactionCheckModes,
        },
        types::{TypeTag as ProtoTypeTag, TypeTagStruct, type_tag},
    },
};
use iota_package_resolver::{PackageStoreWithLruCache, Resolver};
use iota_types::{
    effects::TransactionEffectsAPI,
    execution::ExecutionResult,
    gas::GasCostSummary,
    transaction::TransactionDataAPI,
    transaction_executor::{TransactionExecutor, VmChecks},
};
use move_core_types::{annotated_value::MoveDatatypeLayout, language_storage::StructTag};

use crate::{error::RpcError, types::GrpcReader};

pub const SIMULATE_TRANSACTION_READ_MASK_DEFAULT: &str = crate::field_mask!(
    "transaction.digest",
    "transaction.transaction",
    "transaction.effects",
    "command_results"
);

pub async fn simulate_transaction(
    reader: &Arc<GrpcReader>,
    executor: &Arc<dyn TransactionExecutor>,
    request: SimulateTransactionRequest,
) -> Result<SimulateTransactionResponse, RpcError> {
    // Parse read mask
    let read_mask = request
        .read_mask
        .as_ref()
        .map(FieldMaskTree::from_field_mask)
        .unwrap_or_else(|| {
            SIMULATE_TRANSACTION_READ_MASK_DEFAULT
                .parse::<FieldMaskTree>()
                .unwrap()
        });

    // Extract and validate transaction
    let transaction_proto = request
        .transaction
        .as_ref()
        .ok_or_else(|| FieldViolation::new("transaction").with_reason(ErrorReason::FieldMissing))?;

    let transaction_bcs = transaction_proto.bcs.as_ref().ok_or_else(|| {
        FieldViolation::new("transaction.bcs")
            .with_description("transaction BCS is required for simulation")
            .with_reason(ErrorReason::FieldMissing)
    })?;

    let mut transaction_data: iota_types::transaction::TransactionData =
        bcs::from_bytes(&transaction_bcs.data).map_err(|e| {
            FieldViolation::new("transaction.bcs")
                .with_description(format!("invalid transaction BCS: {e}"))
                .with_reason(ErrorReason::FieldInvalid)
        })?;

    // Validate the digest if provided
    if let Some(provided_digest) = &transaction_proto.digest {
        let computed_digest = transaction_data.digest();
        let provided_digest_bytes: [u8; 32] =
            provided_digest.digest.as_ref().try_into().map_err(|_| {
                FieldViolation::new("transaction.digest")
                    .with_description("digest must be exactly 32 bytes")
                    .with_reason(ErrorReason::FieldInvalid)
            })?;

        if computed_digest.inner() != &provided_digest_bytes {
            let provided_digest_typed =
                iota_types::digests::TransactionDigest::new(provided_digest_bytes);
            return Err(FieldViolation::new("transaction.digest")
                .with_description(format!(
                    "provided digest does not match computed digest: provided={provided_digest_typed}, computed={computed_digest}"
                ))
                .with_reason(ErrorReason::FieldInvalid)
                .into());
        }
    }

    // Determine VM checks from request
    let vm_checks = if request
        .tx_checks
        .contains(&(TransactionCheckModes::DisableVmChecks as i32))
    {
        VmChecks::Disabled
    } else {
        VmChecks::Enabled
    };

    // Perform gas budget estimation if requested
    if request.estimate_gas_budget.unwrap_or(false) {
        // Run simulation to get gas cost
        let estimation_result = executor
            .simulate_transaction(transaction_data.clone(), VmChecks::Enabled)
            .map_err(|e| {
                RpcError::new(
                    tonic::Code::Internal,
                    format!("transaction simulation for gas estimation failed: {e}"),
                )
            })?;

        let reference_gas_price = transaction_data.gas_data().price;
        let estimate = estimate_gas_budget_from_gas_cost(
            estimation_result.effects.gas_cost_summary(),
            reference_gas_price,
        );

        // Update transaction with estimated budget
        transaction_data.gas_data_mut().budget = estimate;
    }

    // Simulate the transaction
    let simulation_result = executor
        .simulate_transaction(transaction_data.clone(), vm_checks)
        .map_err(|e| {
            RpcError::new(
                tonic::Code::Internal,
                format!("transaction simulation failed: {e}"),
            )
        })?;

    let effects = simulation_result.effects;
    let events = simulation_result.events;
    let execution_result = simulation_result.execution_result;
    let input_objects = simulation_result.input_objects;
    let output_objects = simulation_result.output_objects;

    // Convert iota_types to iota_sdk_types types for external compatibility
    // TODO: Remove this conversion when we migrate iota-types to iota_sdk_types
    // types
    let sdk_effects: iota_sdk_types::TransactionEffects =
        effects.clone().try_into().map_err(|e| {
            RpcError::new(
                tonic::Code::Internal,
                format!("failed to convert effects to SDK type: {e}"),
            )
        })?;

    let sdk_events: Option<iota_sdk_types::TransactionEvents> = events
        .as_ref()
        .map(|e| e.clone().try_into())
        .transpose()
        .map_err(|e| {
            RpcError::new(
                tonic::Code::Internal,
                format!("failed to convert events to SDK type: {e}"),
            )
        })?;

    // Convert input objects to SDK2 types
    let sdk_input_objects: Option<Vec<iota_sdk_types::object::Object>> =
        if !input_objects.is_empty() {
            Some(
                input_objects
                    .into_values()
                    .map(|obj| obj.try_into())
                    .collect::<Result<Vec<_>, _>>()
                    .map_err(|e| {
                        RpcError::new(
                            tonic::Code::Internal,
                            format!("failed to convert input objects to SDK type: {e}"),
                        )
                    })?,
            )
        } else {
            None
        };

    // Convert output objects to SDK2 types
    let sdk_output_objects: Option<Vec<iota_sdk_types::object::Object>> =
        if !output_objects.is_empty() {
            Some(
                output_objects
                    .into_values()
                    .map(|obj| obj.try_into())
                    .collect::<Result<Vec<_>, _>>()
                    .map_err(|e| {
                        RpcError::new(
                            tonic::Code::Internal,
                            format!("failed to convert output objects to SDK type: {e}"),
                        )
                    })?,
            )
        } else {
            None
        };

    // Build the response
    let mut response = SimulateTransactionResponse::default();

    // Build executed transaction if requested
    if let Some(tx_mask) = read_mask.subtree(SimulateTransactionResponse::TRANSACTION_FIELD.name) {
        // Convert transaction_data to sdk2 types for merge
        // This includes the updated gas budget if estimation was requested
        let sdk_transaction: iota_sdk_types::SignedTransaction =
            iota_types::transaction::Transaction::from_data(transaction_data.clone(), vec![])
                .try_into()
                .map_err(|e| {
                    RpcError::new(
                        tonic::Code::Internal,
                        format!("failed to convert transaction to SDK type: {e}"),
                    )
                })?;

        let digest = transaction_data.digest();
        let sdk_digest: iota_sdk_types::Digest = digest.into();

        // Create a source for the merge
        let source = TransactionReadSource {
            digest: sdk_digest,
            transaction: &sdk_transaction,
            effects: &sdk_effects,
            events: sdk_events.as_ref(),
            checkpoint: None,
            timestamp_ms: None,
        };

        let mut executed_transaction = ExecutedTransaction::default();
        executed_transaction.merge(&source, &tx_mask);

        // Handle events separately since they need special rendering for json_contents
        if let Some(events_mask) = tx_mask.subtree(ExecutedTransaction::EVENTS_FIELD.name) {
            let mut proto_events = ProtoTransactionEvents::default();
            if let Some(sdk_events) = &sdk_events {
                proto_events.merge(sdk_events, &events_mask);

                // Populate json_contents for events if requested in the mask
                if events_mask
                    .subtree(ProtoTransactionEvents::EVENTS_FIELD.name)
                    .is_some_and(|mask| mask.contains(Event::JSON_CONTENTS_FIELD.name))
                {
                    // Create a package resolver with LRU cache for better performance
                    let package_store = PackageStoreWithLruCache::new(reader.as_ref().clone());
                    let resolver = Resolver::new(package_store);

                    // proto_events.events is Option<Events>, and Events.events is Vec<Event>
                    if let Some(ref mut events) = proto_events.events {
                        for (proto_event, sdk_event) in events.events.iter_mut().zip(&sdk_events.0)
                        {
                            // Convert sdk2 StructTag to move_core_types StructTag via string
                            // representation
                            let type_str = sdk_event.type_.to_string();
                            if let Ok(struct_tag) = StructTag::from_str(&type_str) {
                                // Get the type layout for this event's type
                                if let Ok(layout) = resolver.type_layout(struct_tag.into()).await {
                                    // Extract the datatype layout from the type layout
                                    let datatype_layout = match layout {
                                        move_core_types::annotated_value::MoveTypeLayout::Struct(s) => {
                                            Some(MoveDatatypeLayout::Struct(s))
                                        },
                                        move_core_types::annotated_value::MoveTypeLayout::Enum(e) => {
                                            Some(MoveDatatypeLayout::Enum(e))
                                        },
                                        _ => None, // Primitives are not datatypes
                                    };

                                    // Populate json_contents if we have a valid datatype layout
                                    if let Some(dt_layout) = datatype_layout {
                                        proto_event.populate_json_contents_with_layout(
                                            sdk_event, &dt_layout,
                                        );
                                    }
                                }
                            }
                        }
                    }
                }
            }
            // Always set events if requested in mask, even if empty
            executed_transaction.events = Some(proto_events);
        }

        // Handle input_objects if requested
        if let Some(input_objects_mask) =
            tx_mask.subtree(ExecutedTransaction::INPUT_OBJECTS_FIELD.name)
        {
            let mut proto_objects = iota_grpc_types::v0::object::Objects::default();
            if let Some(sdk_input_objects) = &sdk_input_objects {
                proto_objects.merge(sdk_input_objects.as_slice(), &input_objects_mask);
            }
            executed_transaction.input_objects = Some(proto_objects);
        }

        // Handle output_objects if requested
        if let Some(output_objects_mask) =
            tx_mask.subtree(ExecutedTransaction::OUTPUT_OBJECTS_FIELD.name)
        {
            let mut proto_objects = iota_grpc_types::v0::object::Objects::default();
            if let Some(sdk_output_objects) = &sdk_output_objects {
                proto_objects.merge(sdk_output_objects.as_slice(), &output_objects_mask);
            }
            executed_transaction.output_objects = Some(proto_objects);
        }

        response.transaction = Some(executed_transaction);
    }

    // Build command results if requested
    if read_mask.contains(SimulateTransactionResponse::COMMAND_RESULTS_FIELD.name) {
        let command_results = build_command_results(reader, execution_result)?;
        response.command_results = Some(command_results);
    }

    Ok(response)
}

fn build_command_results(
    _reader: &Arc<GrpcReader>, // reader is unused for now but may be needed in the future
    execution_result: std::result::Result<Vec<ExecutionResult>, iota_types::error::ExecutionError>,
) -> Result<CommandResults, RpcError> {
    let mut results = CommandResults::default();

    match execution_result {
        Ok(execution_results) => {
            results.results = execution_results
                .into_iter()
                .map(|(mutable_reference_outputs, return_values)| {
                    let mut command_result = CommandResult::default();

                    // Process return values
                    let return_outputs: Vec<CommandOutput> = return_values
                        .into_iter()
                        .map(|(bcs_bytes, tt)| CommandOutput {
                            argument: None,
                            type_tag: Some(ProtoTypeTag {
                                type_tag: Some(type_tag::TypeTag::StructTag(TypeTagStruct {
                                    struct_tag: tt.to_canonical_string(true),
                                })),
                            }),
                            bcs: Some(BcsData {
                                data: bcs_bytes.into(),
                            }),
                        })
                        .collect();
                    command_result.return_values = Some(CommandOutputs {
                        outputs: return_outputs,
                    });

                    // Process mutable reference outputs
                    let mutated_outputs: Vec<CommandOutput> = mutable_reference_outputs
                        .into_iter()
                        .map(|(arg, bcs_bytes, tt)| CommandOutput {
                            argument: Some(convert_argument(arg)),
                            type_tag: Some(ProtoTypeTag {
                                type_tag: Some(type_tag::TypeTag::StructTag(TypeTagStruct {
                                    struct_tag: tt.to_canonical_string(true),
                                })),
                            }),
                            bcs: Some(BcsData {
                                data: bcs_bytes.into(),
                            }),
                        })
                        .collect();
                    command_result.mutated_by_ref = Some(CommandOutputs {
                        outputs: mutated_outputs,
                    });

                    command_result
                })
                .collect();
        }
        Err(e) => {
            // If execution failed, return empty results with error info
            // The error is captured in the effects status
            tracing::debug!("Simulation execution failed: {e}");
        }
    }

    Ok(results)
}

fn convert_argument(arg: iota_types::transaction::Argument) -> ProtoArgument {
    match arg {
        iota_types::transaction::Argument::GasCoin => ProtoArgument {
            kind: Some(argument::Kind::GasCoin(argument::GasCoin {})),
        },
        iota_types::transaction::Argument::Input(idx) => ProtoArgument {
            kind: Some(argument::Kind::Input(argument::Input {
                index: Some(idx as u32),
            })),
        },
        iota_types::transaction::Argument::Result(idx) => ProtoArgument {
            kind: Some(argument::Kind::Result(argument::Result {
                index: Some(idx as u32),
                nested_result_index: None,
            })),
        },
        iota_types::transaction::Argument::NestedResult(idx, nested_idx) => ProtoArgument {
            kind: Some(argument::Kind::Result(argument::Result {
                index: Some(idx as u32),
                nested_result_index: Some(nested_idx as u32),
            })),
        },
    }
}

// An amount of gas (in gas units) that is added to transactions as an overhead
// to ensure transactions do not fail.
pub const GAS_SAFE_OVERHEAD: u64 = 1000;

/// Estimate the gas budget using the gas_cost_summary from a previous DryRun
///
/// The estimated gas budget is computed as following:
/// * the maximum between A and B, where: A = computation cost +
///   GAS_SAFE_OVERHEAD * reference gas price B = computation cost + storage
///   cost - storage rebate + GAS_SAFE_OVERHEAD * reference gas price overhead
///
/// This gas estimate is computed similarly as in the TypeScript SDK
pub fn estimate_gas_budget_from_gas_cost(
    gas_cost_summary: &GasCostSummary,
    reference_gas_price: u64,
) -> u64 {
    let safe_overhead = GAS_SAFE_OVERHEAD * reference_gas_price;
    let computation_cost_with_overhead = gas_cost_summary.computation_cost + safe_overhead;

    let gas_usage = gas_cost_summary.net_gas_usage() + safe_overhead as i64;
    computation_cost_with_overhead.max(if gas_usage < 0 { 0 } else { gas_usage as u64 })
}
