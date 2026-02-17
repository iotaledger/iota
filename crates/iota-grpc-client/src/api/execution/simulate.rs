// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! High-level API for transaction simulation.

use iota_grpc_types::v0::transaction_execution_service::{
    SimulateTransactionRequest, SimulateTransactionResponse,
    simulate_transaction_request::TransactionCheckModes,
};
use iota_sdk_types::Transaction;

use crate::{
    Client,
    api::{EXECUTION_READ_MASK, Result, build_proto_transaction, field_mask_with_default},
};

impl Client {
    /// Simulate a transaction without executing it.
    ///
    /// This allows you to preview the effects of a transaction before
    /// actually submitting it to the network.
    ///
    /// # Parameters
    ///
    /// - `transaction`: The transaction to simulate
    /// - `dev_inspect`: Set to true for relaxed Move VM checks (useful for
    ///   debugging and development)
    /// - `estimate_gas_budget`: Set to true to estimate the gas budget required
    /// - `read_mask`: Optional field mask to control which fields are returned
    ///
    /// Returns [`SimulateTransactionResponse`] which contains:
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
    /// # Field Mask
    ///
    /// The optional `read_mask` parameter controls which fields the server
    /// returns. If `None`, uses [`EXECUTION_READ_MASK`] which includes effects,
    /// events, and input/output objects.
    ///
    /// **Optional fields:**
    /// - `transaction.effects` - Transaction effects
    /// - `transaction.events` - Transaction events
    /// - `transaction.input_objects` - Input objects used
    /// - `transaction.output_objects` - Output objects created/modified
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
    /// let result = client.simulate_transaction(tx, false, false, None).await?;
    ///
    /// // Lazy conversion - only deserialize what you need
    /// let executed_tx = result.executed_transaction()?;
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
        dev_inspect: bool,
        estimate_gas_budget: bool,
        read_mask: Option<&str>,
    ) -> Result<SimulateTransactionResponse> {
        // Build proto transaction directly from SDK types
        let proto_transaction = build_proto_transaction(&transaction, transaction.digest())?;

        let tx_checks = if dev_inspect {
            vec![TransactionCheckModes::DisableVmChecks as i32]
        } else {
            vec![]
        };

        let request = SimulateTransactionRequest::default()
            .with_transaction(proto_transaction)
            .with_tx_checks(tx_checks)
            .with_estimate_gas_budget(estimate_gas_budget)
            .with_read_mask(field_mask_with_default(read_mask, EXECUTION_READ_MASK));

        Ok(self
            .execution_service_client()
            .simulate_transaction(request)
            .await?
            .into_inner())
    }
}
