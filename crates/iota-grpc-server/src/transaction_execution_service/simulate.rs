// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::sync::Arc;

use iota_grpc_types::{
    field::FieldMaskTree,
    google::rpc::bad_request::FieldViolation,
    read_masks::SIMULATE_TRANSACTION_READ_MASK,
    v0::{
        bcs::{self as grpc_bcs},
        command::CommandResults,
        error_reason::ErrorReason,
        transaction::ExecutedTransaction,
        transaction_execution_service::{
            ExecutionError, SimulateTransactionRequest, SimulateTransactionResponse,
            simulate_transaction_request::TransactionCheckModes,
            simulate_transaction_response::ExecutionResult,
        },
    },
};
use iota_protocol_config::ProtocolConfig;
use iota_types::{
    effects::TransactionEffectsAPI,
    gas::GasCostSummary,
    transaction::TransactionDataAPI,
    transaction_executor::{SimulateTransactionResult, TransactionExecutor, VmChecks},
};

use super::TransactionReadSource;
use crate::{
    error::RpcError, merge::Merge, transaction_execution_service::CommandResultsReadSource,
    types::GrpcReader,
};

/// Available Read Mask Fields
///
/// The `simulate_transaction` function supports the following `read_mask`
/// fields to control which data is included in the response:
///
/// ## Transaction Fields
/// - `executed_transaction` - includes all executed transaction fields
///   - `executed_transaction.transaction` - includes all transaction fields
///     - `executed_transaction.transaction.digest` - the transaction digest
///     - `executed_transaction.transaction.bcs` - the full BCS-encoded
///       transaction
///   - `executed_transaction.signatures` - includes all signature fields
///     - `executed_transaction.signatures.bcs` - the full BCS-encoded signature
///   - `executed_transaction.effects` - includes all effects fields
///     - `executed_transaction.effects.digest` - the effects digest
///     - `executed_transaction.effects.bcs` - the full BCS-encoded effects
///   - `executed_transaction.events` - includes all event fields
///     - `executed_transaction.events.digest` - the events digest
///     - `executed_transaction.events.events` - includes all event fields (all
///       events of the transaction)
///       - `executed_transaction.events.events.bcs` - the full BCS-encoded
///         event
///       - `executed_transaction.events.events.package_id` - the ID of the
///         package that emitted the event
///       - `executed_transaction.events.events.module` - the module that
///         emitted the event
///       - `executed_transaction.events.events.sender` - the sender that
///         triggered the event
///       - `executed_transaction.events.events.event_type` - the type of the
///         event
///       - `executed_transaction.events.events.bcs_contents` - the full
///         BCS-encoded contents of the event
///       - `executed_transaction.events.events.json_contents` - the
///         JSON-encoded contents of the event
///   - `executed_transaction.checkpoint` - the checkpoint that included the
///     transaction (not available for just-executed transactions)
///   - `executed_transaction.timestamp` - the timestamp of the checkpoint (not
///     available for just-executed transactions)
///   - `executed_transaction.input_objects` - includes all input object fields
///     - `executed_transaction.input_objects.reference` - includes all
///       reference fields
///       - `executed_transaction.input_objects.reference.object_id` - the ID of
///         the input object
///       - `executed_transaction.input_objects.reference.version` - the version
///         of the input object
///       - `executed_transaction.input_objects.reference.digest` - the digest
///         of the input object contents
///     - `executed_transaction.input_objects.bcs` - the full BCS-encoded object
///   - `executed_transaction.output_objects` - includes all output object
///     fields
///     - `executed_transaction.output_objects.reference` - includes all
///       reference fields
///       - `executed_transaction.output_objects.reference.object_id` - the ID
///         of the output object
///       - `executed_transaction.output_objects.reference.version` - the
///         version of the output object
///       - `executed_transaction.output_objects.reference.digest` - the digest
///         of the output object contents
///     - `executed_transaction.output_objects.bcs` - the full BCS-encoded
///       object
///
/// ## Gas Fields
/// - `suggested_gas_price` - the suggested gas price for the transaction,
///   denominated in NANOS
///
/// ## Execution Result Fields
/// - `execution_result` - the execution result (oneof: command_results on
///   success, execution_error on failure)
///   - `execution_result.command_results` - includes all fields of per-command
///     results if execution succeeded
///     - `execution_result.command_results.mutated_by_ref` - includes all
///       fields of objects mutated by reference
///       - `execution_result.command_results.mutated_by_ref.argument` - the
///         argument reference
///       - `execution_result.command_results.mutated_by_ref.type_tag` - the
///         Move type tag
///       - `execution_result.command_results.mutated_by_ref.bcs` - the
///         BCS-encoded value
///       - `execution_result.command_results.mutated_by_ref.json` - the
///         JSON-encoded value
///     - `execution_result.command_results.return_values` - includes all fields
///       of return values returned by the command
///       - `execution_result.command_results.return_values.argument` - the
///         argument reference
///       - `execution_result.command_results.return_values.type_tag` - the Move
///         type tag
///       - `execution_result.command_results.return_values.bcs` - the
///         BCS-encoded value
///       - `execution_result.command_results.return_values.json` - the
///         JSON-encoded value
///   - `execution_result.execution_error` - includes all fields of the
///     execution error if execution failed
///     - `execution_result.execution_error.bcs_kind` - the BCS-encoded error
///       kind
///     - `execution_result.execution_error.source` - the error source
///       description
///     - `execution_result.execution_error.command_index` - the index of the
///       command that failed
pub async fn simulate_transaction(
    reader: &Arc<GrpcReader>,
    executor: &Arc<dyn TransactionExecutor>,
    config: &iota_config::node::GrpcApiConfig,
    request: SimulateTransactionRequest,
) -> Result<SimulateTransactionResponse, RpcError> {
    // Parse read mask
    let read_mask = request
        .read_mask
        .map(|mask| FieldMaskTree::from_field_mask(&mask))
        .unwrap_or_else(|| {
            SIMULATE_TRANSACTION_READ_MASK
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

    let sdk_transaction: iota_sdk_types::transaction::Transaction =
        bcs::from_bytes(&transaction_bcs.data).map_err(|e| {
            FieldViolation::new("transaction.bcs")
                .with_description(format!("invalid transaction BCS: {e}"))
                .with_reason(ErrorReason::FieldInvalid)
        })?;

    // Validate the digest if provided
    if let Some(provided_digest) = &transaction_proto.digest {
        let computed_digest = sdk_transaction.digest();
        let provided_digest_bytes: [u8; 32] =
            provided_digest.digest.as_ref().try_into().map_err(|_| {
                FieldViolation::new("transaction.digest")
                    .with_description("digest must be exactly 32 bytes")
                    .with_reason(ErrorReason::FieldInvalid)
            })?;

        if computed_digest.inner() != &provided_digest_bytes {
            let provided_digest_typed = iota_sdk_types::Digest::new(provided_digest_bytes);
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

    let mut transaction_data = iota_types::transaction::TransactionData::try_from(sdk_transaction)
        .map_err(|e| {
            RpcError::new(
                tonic::Code::InvalidArgument,
                format!("failed to convert transaction to internal type: {e}"),
            )
        })?;

    let system_state = if read_mask
        .contains(SimulateTransactionResponse::SUGGESTED_GAS_PRICE_FIELD.name)
        || (request.estimate_gas_budget.unwrap_or(false) && vm_checks.enabled())
    {
        Some(reader.get_system_state_summary().map_err(|e| {
            RpcError::new(
                tonic::Code::Internal,
                format!("failed to get system state for suggested gas price or gas price estimation: {e}"),
            )
        })?)
    } else {
        None
    };

    // Perform budget estimation if requested and if VmChecks are enabled
    // (it makes no sense to do gas estimation if checks are disabled because such a
    // transaction can't ever be committed to the chain).
    if request.estimate_gas_budget.unwrap_or(false) && vm_checks.enabled() {
        let protocol_config = ProtocolConfig::get_for_version_if_supported(
            system_state
                .as_ref()
                .expect("system state should be available")
                .protocol_version(),
            reader.get_chain_identifier()?.chain(),
        )
        .ok_or_else(|| {
            RpcError::new(
                tonic::Code::Internal,
                "failed to get protocol config for gas estimation".to_string(),
            )
        })?;

        let mut estimation_transaction = transaction_data.clone();
        estimation_transaction.gas_data_mut().payment = Vec::new();
        estimation_transaction.gas_data_mut().budget = protocol_config.max_tx_gas();

        let simulation_result = executor
            .simulate_transaction(estimation_transaction, VmChecks::Enabled)
            .map_err(|e| {
                RpcError::new(
                    tonic::Code::Internal,
                    format!("Transaction simulation for gas estimation failed: {e}"),
                )
            })?;

        if !simulation_result.effects.status().is_ok() {
            return Err(RpcError::new(
                tonic::Code::InvalidArgument,
                format!(
                    "Budget estimation failed with status: {:?}.",
                    simulation_result.effects.status()
                ),
            ));
        }

        let estimate = estimate_gas_budget_from_gas_cost(
            simulation_result.effects.gas_cost_summary(),
            system_state
                .as_ref()
                .expect("system state should be available")
                .reference_gas_price(),
            transaction_data.gas_data().payment.len(),
            &protocol_config,
        );

        // We don't want to return a resolved transaction where the gas payment can't
        // satisfy the budget, so validate that balance can actually cover the
        // estimated budget.
        let gas_balance = transaction_data.gas_data().budget;
        if gas_balance < estimate {
            return Err(RpcError::new(
                tonic::Code::InvalidArgument,
                format!(
                    "Insufficient gas balance to cover estimated transaction cost. \
                    Available gas balance: {gas_balance} NANOS. Estimated gas budget required: {estimate} NANOS"
                ),
            ));
        }

        // Update transaction with estimated budget
        transaction_data.gas_data_mut().budget = estimate;
    }

    // Simulate the transaction
    let SimulateTransactionResult {
        effects,
        events,
        input_objects,
        output_objects,
        execution_result,
        mock_gas_id: _,
        suggested_gas_price,
    } = executor
        .simulate_transaction(transaction_data.clone(), vm_checks)
        .map_err(|e| {
            RpcError::new(
                tonic::Code::Internal,
                format!("transaction simulation failed: {e}"),
            )
        })?;

    // Build the response
    let mut response = SimulateTransactionResponse::default();

    // Only include transaction if requested
    if let Some(tx_mask) =
        read_mask.subtree(SimulateTransactionResponse::EXECUTED_TRANSACTION_FIELD.name)
    {
        let transaction: iota_sdk_types::Transaction = transaction_data.try_into()?;

        // Create a source for the merge
        let source = TransactionReadSource {
            reader: reader.clone(),
            config,
            transaction: Some(transaction),
            signatures: None,
            effects: Some(effects),
            events,
            checkpoint: None,
            timestamp_ms: None,
            input_objects: Some(input_objects.into_values().collect()),
            output_objects: Some(output_objects.into_values().collect()),
        };

        response.executed_transaction = Some(
            ExecutedTransaction::merge_from(&source, &tx_mask)
                .map_err(|e| e.with_context("failed to merge executed transaction"))?,
        );
    }

    // Only include suggested gas price if requested
    if read_mask.contains(SimulateTransactionResponse::SUGGESTED_GAS_PRICE_FIELD.name) {
        response.suggested_gas_price = Some(suggested_gas_price.unwrap_or_else(|| {
            system_state
                .as_ref()
                .expect("system state should be available")
                .reference_gas_price()
        }));
    }

    // Only include the result if requested
    if let Some(result_mask) =
        read_mask.subtree(SimulateTransactionResponse::EXECUTION_RESULT_ONEOF)
    {
        match execution_result {
            Ok(ref execution_results) => {
                if let Some(command_results_mask) =
                    result_mask.subtree(SimulateTransactionResponse::COMMAND_RESULTS_FIELD.name)
                {
                    // Only build command results if the execution was successful
                    let cmd_source = CommandResultsReadSource {
                        reader: reader.clone(),
                        config,
                        execution_results: execution_results.clone(),
                    };

                    let command_results =
                        CommandResults::merge_from(&cmd_source, &command_results_mask)
                            .map_err(|e| e.with_context("failed to merge command results"))?;

                    response.execution_result =
                        Some(ExecutionResult::CommandResults(command_results));
                }
            }
            Err(ref execution_error) => {
                if let Some(error_mask) =
                    result_mask.subtree(SimulateTransactionResponse::EXECUTION_ERROR_FIELD.name)
                {
                    let mut exec_error = ExecutionError::default();

                    // Serialize the execution error kind as BCS
                    if error_mask.contains(ExecutionError::BCS_KIND_FIELD.name) {
                        exec_error.bcs_kind = Some(
                            grpc_bcs::BcsData::serialize(execution_error.kind()).map_err(|e| {
                                RpcError::new(
                                    tonic::Code::Internal,
                                    format!("failed to serialize execution error kind: {e}"),
                                )
                            })?,
                        );
                    }

                    if error_mask.contains(ExecutionError::SOURCE_FIELD.name) {
                        exec_error.source = execution_error
                            .source()
                            .as_ref()
                            .map(|source| source.to_string());
                    }

                    // Set the command index if available
                    if error_mask.contains(ExecutionError::COMMAND_INDEX_FIELD.name) {
                        if let Some(command_idx) = execution_error.command() {
                            exec_error.command_index = Some(command_idx as u64);
                        }
                    }

                    response.execution_result = Some(ExecutionResult::ExecutionError(exec_error));
                }
            }
        }
    }

    Ok(response)
}

// An amount of gas (in gas units) that is added to transactions as an overhead
// to ensure transactions do not fail.
const GAS_SAFE_OVERHEAD: u64 = 1000;
const GAS_COIN_BCS_BYTES_SIZE: u64 = 40;

/// Estimate the gas budget for a transaction based on simulation results.
///
/// The estimation includes:
/// 1. Base cost from gas_cost_summary (computation + storage costs)
/// 2. Cost of loading gas payment objects (which weren't loaded during
///    simulation)
/// 3. Rounding up to the protocol gas rounding step (typically 1000 NANOS)
/// 4. Adding safe overhead buffer (1000 * reference_gas_price)
/// 5. Clamping to max_tx_gas protocol limit
pub fn estimate_gas_budget_from_gas_cost(
    gas_cost_summary: &GasCostSummary,
    reference_gas_price: u64,
    num_payment_objects_on_request: usize,
    protocol_config: &iota_protocol_config::ProtocolConfig,
) -> u64 {
    // Calculate base estimate from gas cost summary (in NANOS)
    let gas_usage = gas_cost_summary.net_gas_usage();
    let base_estimate_nanos =
        gas_cost_summary
            .computation_cost
            .max(if gas_usage < 0 { 0 } else { gas_usage as u64 });

    // Calculate cost of loading gas payment objects.
    // Subtract 1 because the simulation already loaded one ephemeral gas coin.
    let num_payment_objects_for_estimation = {
        let total = if num_payment_objects_on_request == 0 {
            protocol_config.max_gas_payment_objects() as u64
        } else {
            num_payment_objects_on_request as u64
        };
        total.saturating_sub(1)
    };

    // Calculate gas loading cost in gas units
    let gas_loading_cost_units = num_payment_objects_for_estimation
        .saturating_mul(GAS_COIN_BCS_BYTES_SIZE)
        .saturating_mul(protocol_config.obj_access_cost_read_per_byte());

    // Round up to the nearest gas rounding step (in gas units)
    let rounded_gas_loading_cost_units =
        if let Some(step) = protocol_config.gas_rounding_step_as_option() {
            gas_loading_cost_units
                .checked_next_multiple_of(step)
                .unwrap_or(u64::MAX)
        } else {
            gas_loading_cost_units
        };

    // Convert gas loading cost to NANOS
    let gas_loading_cost_nanos = rounded_gas_loading_cost_units.saturating_mul(reference_gas_price);

    // Calculate safe overhead buffer in NANOS
    let safe_overhead_nanos = GAS_SAFE_OVERHEAD.saturating_mul(reference_gas_price);

    // Add all together: base (NANOS) + loading (NANOS) + overhead (NANOS)
    let estimate_nanos = base_estimate_nanos
        .saturating_add(gas_loading_cost_nanos)
        .saturating_add(safe_overhead_nanos);

    // Clamp to max_tx_gas to ensure we don't exceed the protocol limit
    estimate_nanos.min(protocol_config.max_tx_gas())
}
