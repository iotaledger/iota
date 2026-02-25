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
    /// # Available Read Mask Fields
    ///
    /// The optional `read_mask` parameter controls which fields the server
    /// returns. If `None`, uses [`EXECUTION_READ_MASK`] which includes effects,
    /// events, and input/output objects.
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
    ///   - `executed_transaction.checkpoint` - the checkpoint that included the
    ///     transaction (not available for just-executed transactions)
    ///   - `executed_transaction.timestamp` - the timestamp of the checkpoint
    ///     (not available for just-executed transactions)
    ///
    /// ## Event Fields
    /// - `executed_transaction.events` - includes all event fields (all events
    ///   of the transaction)
    ///   - `executed_transaction.events.digest` - the events digest
    ///   - `executed_transaction.events.events` - includes all event fields
    ///     - `executed_transaction.events.events.bcs` - the full BCS-encoded
    ///       event
    ///     - `executed_transaction.events.events.package_id` - the ID of the
    ///       package that emitted the event
    ///     - `executed_transaction.events.events.module` - the module that
    ///       emitted the event
    ///     - `executed_transaction.events.events.sender` - the sender that
    ///       triggered the event
    ///     - `executed_transaction.events.events.event_type` - the type of the
    ///       event
    ///     - `executed_transaction.events.events.bcs_contents` - the full
    ///       BCS-encoded contents of the event
    ///     - `executed_transaction.events.events.json_contents` - the
    ///       JSON-encoded contents of the event
    ///
    /// ## Object Fields
    /// - `executed_transaction.input_objects` - includes all input object
    ///   fields
    ///   - `executed_transaction.input_objects.reference` - includes all
    ///     reference fields
    ///     - `executed_transaction.input_objects.reference.object_id` - the ID
    ///       of the input object
    ///     - `executed_transaction.input_objects.reference.version` - the
    ///       version of the input object
    ///     - `executed_transaction.input_objects.reference.digest` - the digest
    ///       of the input object contents
    ///   - `executed_transaction.input_objects.bcs` - the full BCS-encoded
    ///     object
    /// - `executed_transaction.output_objects` - includes all output object
    ///   fields
    ///   - `executed_transaction.output_objects.reference` - includes all
    ///     reference fields
    ///     - `executed_transaction.output_objects.reference.object_id` - the ID
    ///       of the output object
    ///     - `executed_transaction.output_objects.reference.version` - the
    ///       version of the output object
    ///     - `executed_transaction.output_objects.reference.digest` - the
    ///       digest of the output object contents
    ///   - `executed_transaction.output_objects.bcs` - the full BCS-encoded
    ///     object
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
    /// let effects = result.effects()?.effects()?;
    /// println!("Status: {:?}", effects.status());
    ///
    /// let events = result.events()?.events()?;
    /// if !events.0.is_empty() {
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

        // Convert signatures to proto format
        let proto_signatures = UserSignatures::default().with_signatures(
            signed_transaction
                .signatures
                .into_iter()
                .map(|sig| {
                    ProtoUserSignature::try_from(sig).map_err(|e| Error::Signature(e.to_string()))
                })
                .collect::<Result<Vec<_>>>()?,
        );

        let request = ExecuteTransactionRequest::default()
            .with_transaction(proto_transaction)
            .with_signatures(proto_signatures)
            .with_read_mask(field_mask_with_default(read_mask, EXECUTION_READ_MASK));

        let response = self
            .execution_service_client()
            .execute_transaction(request)
            .await?
            .into_inner();

        response
            .executed_transaction
            .ok_or_else(|| TryFromProtoError::missing("executed_transaction").into())
    }
}
