// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::sync::Arc;

use iota_grpc_types::{
    field::FieldMaskTree,
    read_masks::SIMULATE_TRANSACTIONS_READ_MASK,
    v1::{
        bcs::{self as grpc_bcs},
        command::CommandResults,
        transaction::ExecutedTransaction,
        transaction_execution_service::{
            ExecutionError, SimulateTransactionItem, SimulateTransactionResult,
            SimulateTransactionsRequest, SimulateTransactionsResponse, SimulatedTransaction,
            simulate_transaction_item::TransactionCheckModes,
        },
    },
};
use iota_protocol_config::ProtocolConfig;
use iota_types::{
    effects::TransactionEffectsAPI,
    transaction::TransactionDataAPI,
    transaction_executor::{
        SimulateTransactionResult as InternalSimulateResult, TransactionExecutor, VmChecks,
    },
};

use super::TransactionReadSource;
use crate::{
    error::RpcError, merge::Merge, transaction_execution_service::CommandResultsReadSource,
    types::GrpcReader,
};

/// Simulate a batch of transactions sequentially.
///
/// Each transaction is simulated independently — failure of one does not abort
/// the rest. Results are returned in the same order as the input.
///
/// ## Available Read Mask Fields
///
/// The `simulate_transactions` function supports the following `read_mask`
/// fields to control which data is included in each simulated transaction
/// result:
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
#[tracing::instrument(skip(reader, executor))]
pub async fn simulate_transactions(
    reader: &Arc<GrpcReader>,
    executor: &Arc<dyn TransactionExecutor>,
    config: &iota_config::node::GrpcApiConfig,
    request: SimulateTransactionsRequest,
) -> Result<SimulateTransactionsResponse, RpcError> {
    super::validate_batch_size(
        request.transactions.len(),
        config.max_simulate_transaction_batch_size as usize,
    )?;
    let read_mask = super::parse_read_mask::<SimulatedTransaction>(
        request.read_mask,
        SIMULATE_TRANSACTIONS_READ_MASK,
    )?;

    // Simulate each transaction sequentially, collecting per-item results
    let mut transaction_results = Vec::with_capacity(request.transactions.len());
    for item in &request.transactions {
        let result =
            match simulate_single_transaction(reader, executor, config, item, &read_mask).await {
                Ok(tx) => SimulateTransactionResult::default().with_simulated_transaction(tx),
                Err(error) => {
                    SimulateTransactionResult::default().with_error(error.into_status_proto())
                }
            };
        transaction_results.push(result);
    }

    Ok(SimulateTransactionsResponse::default().with_transaction_results(transaction_results))
}

/// Simulate a single transaction item.
async fn simulate_single_transaction(
    reader: &Arc<GrpcReader>,
    executor: &Arc<dyn TransactionExecutor>,
    config: &iota_config::node::GrpcApiConfig,
    item: &SimulateTransactionItem,
    read_mask: &FieldMaskTree,
) -> Result<SimulatedTransaction, RpcError> {
    let sdk_transaction = super::parse_transaction_proto(item.transaction.as_ref())?;

    // Determine VM checks from request
    let vm_checks = if item
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

    // If the transaction has a zero gas budget and VM checks are disabled, we'll
    // set the gas budget in the result to the actual cost from the simulation.
    // This allows clients to estimate the gas cost by simulating with a zero
    // budget.
    let set_gas_budget = vm_checks.disabled() && transaction_data.gas_data().budget == 0;

    let system_state = if read_mask.contains(SimulatedTransaction::SUGGESTED_GAS_PRICE_FIELD.name)
        || set_gas_budget
    {
        Some(reader.get_system_state_summary().map_err(|e| {
            RpcError::new(
                tonic::Code::Internal,
                format!("failed to get system state: {e}"),
            )
        })?)
    } else {
        None
    };

    if set_gas_budget {
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
                "failed to get protocol config for gas budget validation".to_string(),
            )
        })?;

        // A zero budget signals "use maximum" — run the dry run with
        // max_tx_gas so the actual cost shows up in the gas status.
        transaction_data.gas_data_mut().budget = protocol_config.max_tx_gas();
    }

    // Simulate the transaction
    let InternalSimulateResult {
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
    let mut response = SimulatedTransaction::default();

    // Only include transaction if requested
    if let Some(tx_mask) = read_mask.subtree(SimulatedTransaction::EXECUTED_TRANSACTION_FIELD.name)
    {
        if set_gas_budget {
            transaction_data.gas_data_mut().budget = effects.gas_cost_summary().gas_used();
        }

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
    if read_mask.contains(SimulatedTransaction::SUGGESTED_GAS_PRICE_FIELD.name) {
        response.suggested_gas_price = Some(suggested_gas_price.unwrap_or_else(|| {
            system_state
                .as_ref()
                .expect("system state should be available")
                .reference_gas_price()
        }));
    }

    // Only include the result if requested
    if let Some(result_mask) = read_mask.subtree(SimulatedTransaction::EXECUTION_RESULT_ONEOF) {
        match execution_result {
            Ok(ref execution_results) => {
                if let Some(command_results_mask) =
                    result_mask.subtree(SimulatedTransaction::COMMAND_RESULTS_FIELD.name)
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

                    response.execution_result = Some(
                        iota_grpc_types::v1::transaction_execution_service::simulated_transaction::ExecutionResult::CommandResults(command_results),
                    );
                }
            }
            Err(ref execution_error) => {
                if let Some(error_mask) =
                    result_mask.subtree(SimulatedTransaction::EXECUTION_ERROR_FIELD.name)
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

                    response.execution_result = Some(
                        iota_grpc_types::v1::transaction_execution_service::simulated_transaction::ExecutionResult::ExecutionError(exec_error),
                    );
                }
            }
        }
    }

    Ok(response)
}
