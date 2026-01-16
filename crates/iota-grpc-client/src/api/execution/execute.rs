// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! High-level API for transaction execution.

use iota_grpc_types::v0::{
    signatures::{UserSignature as ProtoUserSignature, UserSignatures},
    transaction_execution_service::ExecuteTransactionRequest,
};
use iota_sdk_types::SignedTransaction;

use crate::{
    Client,
    api::{
        EXECUTION_READ_MASK, Error, Result, TransactionExecutionResponse, build_proto_transaction,
        extract_execution_response, field_mask_with_default,
    },
};

impl Client {
    /// Execute a signed transaction.
    ///
    /// This submits the transaction to the network for execution and waits for
    /// the result. The transaction must be signed with valid signatures.
    ///
    /// Returns `TransactionExecutionResponse` containing the transaction
    /// effects, events, and optionally input/output objects.
    ///
    /// # Field Mask
    ///
    /// The optional `read_mask` parameter controls which fields the server
    /// returns. If `None`, uses [`EXECUTION_READ_MASK`] which includes effects,
    /// events, and input/output objects.
    ///
    /// **Required fields** (must be included in custom masks):
    /// - `transaction.effects` - Transaction effects
    ///
    /// **Optional fields:**
    /// - `transaction.events` - Transaction events
    /// - `transaction.input_objects` - Input objects used
    /// - `transaction.output_objects` - Output objects created/modified
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use iota_grpc_client::Client;
    /// # use iota_sdk_types::SignedTransaction;
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let client = Client::connect("http://localhost:9000").await?;
    ///
    /// let signed_tx: SignedTransaction = todo!();
    ///
    /// // Default: get effects, events, and objects
    /// let result = client.execute_transaction(signed_tx.clone(), None).await?;
    ///
    /// // Minimal: only effects (smaller response)
    /// let result = client
    ///     .execute_transaction(signed_tx, Some("transaction.effects"))
    ///     .await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn execute_transaction(
        &self,
        signed_transaction: SignedTransaction,
        read_mask: Option<&str>,
    ) -> Result<TransactionExecutionResponse> {
        // Build proto transaction directly from SDK types
        let tx_digest = signed_transaction.transaction.digest();
        let proto_transaction =
            build_proto_transaction(&signed_transaction.transaction, tx_digest)?;

        // Convert signatures to proto format using existing TryFrom
        let proto_signatures = UserSignatures {
            signatures: signed_transaction
                .signatures
                .into_iter()
                .map(|sig| {
                    ProtoUserSignature::try_from(sig).map_err(|e| Error::Signature(e.to_string()))
                })
                .collect::<Result<Vec<_>>>()?,
        };

        let request = ExecuteTransactionRequest {
            transaction: Some(proto_transaction),
            signatures: Some(proto_signatures),
            read_mask: Some(field_mask_with_default(read_mask, EXECUTION_READ_MASK)),
        };

        let response = self
            .execution_service_client()
            .execute_transaction(request)
            .await?
            .into_inner();

        extract_execution_response(response.transaction)
    }
}
