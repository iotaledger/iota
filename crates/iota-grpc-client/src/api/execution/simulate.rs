// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! High-level API for transaction simulation.

use iota_grpc_types::v1::transaction_execution_service::{
    SimulateTransactionItem, SimulateTransactionsRequest, SimulatedTransaction,
    simulate_transaction_item::TransactionCheckModes,
};
use iota_sdk_types::Transaction;

use crate::{
    Client,
    api::{
        Error, MetadataEnvelope, ProtoResult, Result, SIMULATE_TRANSACTIONS_READ_MASK,
        build_proto_transaction, field_mask_with_default,
    },
};

/// A single transaction with simulation options for use in batch simulation.
pub struct SimulateTransactionInput {
    /// The transaction to simulate.
    pub transaction: Transaction,
    /// Set to true for relaxed Move VM checks (useful for debugging and
    /// development).
    pub skip_checks: bool,
}

impl Client {
    /// Simulate a transaction without executing it.
    ///
    /// This allows you to preview the effects of a transaction before
    /// actually submitting it to the network.
    ///
    /// # Parameters
    ///
    /// - `transaction`: The transaction to simulate
    /// - `skip_checks`: Set to true for relaxed Move VM checks (useful for
    ///   debugging and development)
    /// - `read_mask`: Optional field mask to control which fields are returned
    ///
    /// Returns [`SimulatedTransaction`] which contains:
    /// - `executed_transaction()` - Access to the simulated ExecutedTransaction
    /// - `command_results()` - Access to intermediate command execution results
    ///
    /// Use lazy conversion methods on the executed transaction to extract data:
    /// - `result.executed_transaction()?.effects()` - Get simulated effects
    /// - `result.executed_transaction()?.events()` - Get simulated events (if
    ///   available)
    /// - `result.executed_transaction()?.input_objects()` - Get input objects
    ///   (if requested)
    /// - `result.executed_transaction()?.output_objects()` - Get output objects
    ///   (if requested)
    ///
    /// # Available Read Mask Fields
    ///
    /// The optional `read_mask` parameter controls which fields the server
    /// returns. If `None`, uses [`SIMULATE_TRANSACTIONS_READ_MASK`] which
    /// includes effects, events, and input/output objects.
    ///
    /// ## Transaction Fields
    /// - `executed_transaction` - includes all executed transaction fields
    ///   - `executed_transaction.transaction` - includes all transaction fields
    ///     - `executed_transaction.transaction.digest` - the transaction digest
    ///     - `executed_transaction.transaction.bcs` - the full BCS-encoded
    ///       transaction
    ///   - `executed_transaction.signatures` - includes all signature fields
    ///     - `executed_transaction.signatures.bcs` - the full BCS-encoded
    ///       signature
    ///   - `executed_transaction.effects` - includes all effects fields
    ///     - `executed_transaction.effects.digest` - the effects digest
    ///     - `executed_transaction.effects.bcs` - the full BCS-encoded effects
    ///   - `executed_transaction.events` - includes all event fields
    ///     - `executed_transaction.events.digest` - the events digest
    ///     - `executed_transaction.events.events` - includes all event fields
    ///       (all events of the transaction)
    ///       - `executed_transaction.events.events.bcs` - the full BCS-encoded
    ///         event
    ///       - `executed_transaction.events.events.package_id` - the ID of the
    ///         package that emitted the event
    ///       - `executed_transaction.events.events.module` - the module that
    ///         emitted the event
    ///       - `executed_transaction.events.events.sender` - the sender that
    ///         triggered the event
    ///       - `executed_transaction.events.events.event_type` - the type of
    ///         the event
    ///       - `executed_transaction.events.events.bcs_contents` - the full
    ///         BCS-encoded contents of the event
    ///       - `executed_transaction.events.events.json_contents` - the
    ///         JSON-encoded contents of the event
    ///   - `executed_transaction.checkpoint` - the checkpoint that included the
    ///     transaction (not available for just-executed transactions)
    ///   - `executed_transaction.timestamp` - the timestamp of the checkpoint
    ///     (not available for just-executed transactions)
    ///   - `executed_transaction.input_objects` - includes all input object
    ///     fields
    ///     - `executed_transaction.input_objects.reference` - includes all
    ///       reference fields
    ///       - `executed_transaction.input_objects.reference.object_id` - the
    ///         ID of the input object
    ///       - `executed_transaction.input_objects.reference.version` - the
    ///         version of the input object
    ///       - `executed_transaction.input_objects.reference.digest` - the
    ///         digest of the input object contents
    ///     - `executed_transaction.input_objects.bcs` - the full BCS-encoded
    ///       object
    ///   - `executed_transaction.output_objects` - includes all output object
    ///     fields
    ///     - `executed_transaction.output_objects.reference` - includes all
    ///       reference fields
    ///       - `executed_transaction.output_objects.reference.object_id` - the
    ///         ID of the output object
    ///       - `executed_transaction.output_objects.reference.version` - the
    ///         version of the output object
    ///       - `executed_transaction.output_objects.reference.digest` - the
    ///         digest of the output object contents
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
    ///   - `execution_result.command_results` - includes all fields of
    ///     per-command results if execution succeeded
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
    ///     - `execution_result.command_results.return_values` - includes all
    ///       fields of return values returned by the command
    ///       - `execution_result.command_results.return_values.argument` - the
    ///         argument reference
    ///       - `execution_result.command_results.return_values.type_tag` - the
    ///         Move type tag
    ///       - `execution_result.command_results.return_values.bcs` - the
    ///         BCS-encoded value
    ///       - `execution_result.command_results.return_values.json` - the
    ///         JSON-encoded value
    ///   - `execution_result.execution_error` - includes all fields of the
    ///     execution error if execution failed
    ///     - `execution_result.execution_error.bcs_kind` - the BCS-encoded
    ///       error kind
    ///     - `execution_result.execution_error.source` - the error source
    ///       description
    ///     - `execution_result.execution_error.command_index` - the index of
    ///       the command that failed
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use iota_grpc_client::Client;
    /// # use iota_sdk_types::Transaction;
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let client = Client::connect("http://localhost:9000").await?;
    ///
    /// let tx: Transaction = todo!();
    ///
    /// // Simulate transaction - returns proto type
    /// let result = client.simulate_transaction(tx, false, None).await?;
    ///
    /// // Lazy conversion - only deserialize what you need
    /// let executed_tx = result.body().executed_transaction()?;
    /// let effects = executed_tx.effects()?.effects()?;
    /// println!("Simulation status: {:?}", effects.status());
    ///
    /// let output_objs = executed_tx.output_objects()?;
    /// println!("Would create {} objects", output_objs.objects.len());
    /// # Ok(())
    /// # }
    /// ```
    pub async fn simulate_transaction(
        &self,
        transaction: Transaction,
        skip_checks: bool,
        read_mask: Option<&str>,
    ) -> Result<MetadataEnvelope<SimulatedTransaction>> {
        self.simulate_transactions(
            vec![SimulateTransactionInput {
                transaction,
                skip_checks,
            }],
            read_mask,
        )
        .await?
        .try_map(|results| {
            results
                .into_iter()
                .next()
                .ok_or_else(|| Error::Protocol("empty transaction_results".into()))?
        })
    }

    /// Simulate a batch of transactions without executing them.
    ///
    /// Transactions are simulated sequentially on the server. Each transaction
    /// is independent — failure of one does not abort the rest.
    ///
    /// Returns a `Vec<Result<SimulatedTransaction>>` in the same order as the
    /// input. Each element is either the successfully simulated transaction or
    /// the per-item error returned by the server.
    ///
    /// # Errors
    ///
    /// Returns [`Error::EmptyRequest`] if `transactions` is empty.
    /// Returns a transport-level [`Error::Grpc`] if the entire RPC fails
    /// (e.g. batch size exceeded).
    pub async fn simulate_transactions(
        &self,
        transactions: Vec<SimulateTransactionInput>,
        read_mask: Option<&str>,
    ) -> Result<MetadataEnvelope<Vec<Result<SimulatedTransaction>>>> {
        if transactions.is_empty() {
            return Err(Error::EmptyRequest);
        }

        let items = transactions
            .into_iter()
            .map(|input| build_simulate_item(input.transaction, input.skip_checks))
            .collect::<Result<Vec<_>>>()?;

        let request = SimulateTransactionsRequest::default()
            .with_transactions(items)
            .with_read_mask(field_mask_with_default(
                read_mask,
                SIMULATE_TRANSACTIONS_READ_MASK,
            ));

        let response = self
            .execution_service_client()
            .simulate_transactions(request)
            .await?;

        MetadataEnvelope::from(response).try_map(|r| {
            Ok(r.transaction_results
                .into_iter()
                .map(ProtoResult::into_result)
                .collect())
        })
    }
}

/// Convert a transaction and options into a proto `SimulateTransactionItem`.
fn build_simulate_item(
    transaction: Transaction,
    skip_checks: bool,
) -> Result<SimulateTransactionItem> {
    let proto_transaction = build_proto_transaction(&transaction, transaction.digest())?;

    let tx_checks = if skip_checks {
        vec![TransactionCheckModes::DisableVmChecks as i32]
    } else {
        vec![]
    };

    Ok(SimulateTransactionItem::default()
        .with_transaction(proto_transaction)
        .with_tx_checks(tx_checks))
}
