// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! High-level API for transaction execution.

use iota_grpc_types::v0::{
    signatures::{UserSignature as ProtoUserSignature, UserSignatures},
    transaction::ExecutedTransaction,
    transaction_execution_service::ExecuteTransactionRequest,
};
use iota_sdk_types::SignedTransaction;

use crate::{
    Client,
    api::{
        EXECUTION_READ_MASK, Error, Result, TryFromProtoError, build_proto_transaction,
        field_mask_with_default,
    },
};

impl Client {
    /// Execute a signed transaction.
    ///
    /// This submits the transaction to the network for execution and waits for
    /// the result. The transaction must be signed with valid signatures.
    ///
    /// Returns proto `ExecutedTransaction`. Use lazy conversion methods to
    /// extract data:
    /// - `result.effects()` - Get transaction effects
    /// - `result.events()` - Get transaction events (if available)
    /// - `result.input_objects()` - Get input objects (if requested)
    /// - `result.output_objects()` - Get output objects (if requested)
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
    /// # use iota_sdk_types::SignedTransaction;
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let client = Client::connect("http://localhost:9000").await?;
    ///
    /// let signed_tx: SignedTransaction = todo!();
    ///
    /// // Execute transaction - returns proto type
    /// let result = client.execute_transaction(signed_tx, None).await?;
    ///
    /// // Lazy conversion - only deserialize what you need
    /// let effects = result.effects()?;
    /// println!("Status: {:?}", effects.status());
    ///
    /// if let Some(events) = result.events()? {
    ///     println!("Events: {}", events.0.len());
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub async fn execute_transaction(
        &self,
        signed_transaction: SignedTransaction,
        read_mask: Option<&str>,
    ) -> Result<ExecutedTransaction> {
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

        response
            .transaction
            .ok_or_else(|| TryFromProtoError::missing("transaction").into())
    }
}
