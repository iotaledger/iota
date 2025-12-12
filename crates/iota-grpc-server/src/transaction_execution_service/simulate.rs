// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::sync::Arc;

use iota_grpc_types::{
    field::FieldMaskTree,
    google::rpc::bad_request::FieldViolation,
    merge::Merge,
    v0::{
        command::CommandResults,
        error_reason::ErrorReason,
        transaction::ExecutedTransaction,
        transaction_execution_service::{
            SimulateTransactionRequest, SimulateTransactionResponse,
            simulate_transaction_request::TransactionCheckModes,
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

use super::{CommandResultsReadSource, TransactionReadSource};
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
    config: &iota_config::node::GrpcApiConfig,
    request: SimulateTransactionRequest,
) -> Result<SimulateTransactionResponse, RpcError> {
    // Parse read mask
    let read_mask = request
        .read_mask
        .map(|mask| FieldMaskTree::from_field_mask(&mask))
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

    // Perform budget estimation if requested and if VmChecks are enabled
    // (it makes no sense to do gas estimation if checks are disabled because such a
    // transaction can't ever be committed to the chain).
    if request.estimate_gas_budget.unwrap_or(false) && vm_checks.enabled() {
        let (reference_gas_price, protocol_config) = {
            let system_state = reader.get_system_state_summary()?;
            let protocol_config = ProtocolConfig::get_for_version_if_supported(
                system_state.protocol_version(),
                reader.get_chain_identifier()?.chain(),
            )
            .ok_or_else(|| {
                RpcError::new(
                    tonic::Code::Internal,
                    "failed to get protocol config for gas estimation".to_string(),
                )
            })?;

            (system_state.reference_gas_price(), protocol_config)
        };

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
            reference_gas_price,
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
    if let Some(tx_mask) = read_mask.subtree(SimulateTransactionResponse::TRANSACTION_FIELD.name) {
        // Create a source for the merge
        let source = TransactionReadSource {
            reader: reader.clone(),
            config,
            transaction_data,
            signatures: None,
            effects: Some(effects),
            events,
            checkpoint: None,
            timestamp_ms: None,
            input_objects: Some(input_objects.into_values().collect()),
            output_objects: Some(output_objects.into_values().collect()),
        };

        response.transaction = Some(ExecutedTransaction::merge_from(&source, &tx_mask).map_err(
            |e| {
                RpcError::new(
                    tonic::Code::Internal,
                    format!("failed to build executed transaction in simulation response: {e}"),
                )
            },
        )?);
    }

    // Only include command results if requested
    if let Some(cmd_mask) =
        read_mask.subtree(SimulateTransactionResponse::COMMAND_RESULTS_FIELD.name)
    {
        match execution_result {
            Ok(execution_results) => {
                // Only build command results if the execution was successful
                // Create a source for the merge
                let source = CommandResultsReadSource {
                    reader: reader.clone(),
                    config,
                    execution_results,
                };

                response.command_results =
                    Some(CommandResults::merge_from(&source, &cmd_mask).map_err(|e| {
                        RpcError::new(
                            tonic::Code::Internal,
                            format!("failed to build command results in simulation response: {e}"),
                        )
                    })?);
            }
            Err(_) => {
                // If execution failed, return empty results
                response.command_results = Some(CommandResults::default());
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
            match gas_loading_cost_units.checked_next_multiple_of(step) {
                Some(rounded) => rounded,
                None => u64::MAX,
            }
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
