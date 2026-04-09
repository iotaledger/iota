// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! High-level API for transaction execution.

use iota_grpc_types::v1::{
    signatures::{UserSignature as ProtoUserSignature, UserSignatures},
    transaction::ExecutedTransaction,
    transaction_execution_service::{ExecuteTransactionItem, ExecuteTransactionsRequest},
};
use iota_sdk_types::SignedTransaction;

use crate::{
    Client,
    api::{
        EXECUTE_TRANSACTIONS_READ_MASK, Error, MetadataEnvelope, ProtoResult, Result,
        build_proto_transaction, field_mask_with_default,
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
    /// returns. If `None`, uses [`EXECUTE_TRANSACTIONS_READ_MASK`] which
    /// includes effects, events, and input/output objects.
    ///
    /// ## Transaction Fields
    /// - `transaction` - includes all transaction fields
    ///   - `transaction.digest` - the transaction digest
    ///   - `transaction.bcs` - the full BCS-encoded transaction
    /// - `signatures` - includes all signature fields
    ///   - `signatures.bcs` - the full BCS-encoded signature
    /// - `effects` - includes all effects fields
    ///   - `effects.digest` - the effects digest
    ///   - `effects.bcs` - the full BCS-encoded effects
    /// - `checkpoint` - the checkpoint that included the transaction. Requires
    ///   `checkpoint_inclusion_timeout_ms` to be set.
    /// - `timestamp` - the timestamp of the checkpoint. Requires
    ///   `checkpoint_inclusion_timeout_ms` to be set.
    ///
    /// ## Event Fields
    /// - `events` - includes all event fields (all events of the transaction)
    ///   - `events.digest` - the events digest
    ///   - `events.events.bcs` - the full BCS-encoded event
    ///   - `events.events.package_id` - the ID of the package that emitted the
    ///     event
    ///   - `events.events.module` - the module that emitted the event
    ///   - `events.events.sender` - the sender that triggered the event
    ///   - `events.events.event_type` - the type of the event
    ///   - `events.events.bcs_contents` - the full BCS-encoded contents of the
    ///     event
    ///   - `events.events.json_contents` - the JSON-encoded contents of the
    ///     event
    ///
    /// ## Object Fields
    /// - `input_objects` - includes all input object fields
    ///   - `input_objects.reference` - includes all reference fields
    ///     - `input_objects.reference.object_id` - the ID of the input object
    ///     - `input_objects.reference.version` - the version of the input
    ///       object
    ///     - `input_objects.reference.digest` - the digest of the input object
    ///       contents
    ///   - `input_objects.bcs` - the full BCS-encoded object
    /// - `output_objects` - includes all output object fields
    ///   - `output_objects.reference` - includes all reference fields
    ///     - `output_objects.reference.object_id` - the ID of the output object
    ///     - `output_objects.reference.version` - the version of the output
    ///       object
    ///     - `output_objects.reference.digest` - the digest of the output
    ///       object contents
    ///   - `output_objects.bcs` - the full BCS-encoded object
    ///
    /// # Checkpoint Inclusion
    ///
    /// If `checkpoint_inclusion_timeout_ms` is set, the server will wait up to
    /// the specified duration (in milliseconds) for the transaction to be
    /// included in a checkpoint before returning. When set, include
    /// `checkpoint` and `timestamp` in the `read_mask` to receive the data.
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
    /// let result = client.execute_transaction(signed_tx, None, None).await?;
    ///
    /// // Lazy conversion - only deserialize what you need
    /// let effects = result.body().effects()?.effects()?;
    /// println!("Status: {:?}", effects.status());
    ///
    /// let events = result.body().events()?.events()?;
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
        checkpoint_inclusion_timeout_ms: Option<u64>,
    ) -> Result<MetadataEnvelope<ExecutedTransaction>> {
        self.execute_transactions(
            vec![signed_transaction],
            read_mask,
            checkpoint_inclusion_timeout_ms,
        )
        .await?
        .try_map(|results| {
            results
                .into_iter()
                .next()
                .ok_or_else(|| Error::Protocol("empty transaction_results".into()))?
        })
    }

    /// Execute a batch of signed transactions.
    ///
    /// Transactions are executed sequentially on the server. Each transaction
    /// is independent — failure of one does not abort the rest.
    ///
    /// Returns a `Vec<Result<ExecutedTransaction>>` in the same order as the
    /// input. Each element is either the successfully executed transaction or
    /// the per-item error returned by the server.
    ///
    /// # Available Read Mask Fields
    ///
    /// The optional `read_mask` parameter controls which fields the server
    /// returns for each `ExecutedTransaction`. If `None`, uses
    /// [`EXECUTE_TRANSACTIONS_READ_MASK`] which includes effects, events, and
    /// input/output objects.
    ///
    /// See [`execute_transaction`](Self::execute_transaction) for the full list
    /// of supported read mask fields.
    ///
    /// # Checkpoint Inclusion
    ///
    /// If `checkpoint_inclusion_timeout_ms` is set, the server will wait up to
    /// the specified duration (in milliseconds) for all executed transactions
    /// to be included in a checkpoint before returning. When set, include
    /// `checkpoint` and `timestamp` in the `read_mask` to receive the data.
    ///
    /// # Errors
    ///
    /// Returns [`Error::EmptyRequest`] if `transactions` is empty.
    /// Returns a transport-level [`Error::Grpc`] if the entire RPC fails
    /// (e.g. batch size exceeded).
    pub async fn execute_transactions(
        &self,
        transactions: Vec<SignedTransaction>,
        read_mask: Option<&str>,
        checkpoint_inclusion_timeout_ms: Option<u64>,
    ) -> Result<MetadataEnvelope<Vec<Result<ExecutedTransaction>>>> {
        if transactions.is_empty() {
            return Err(Error::EmptyRequest);
        }

        let items = transactions
            .into_iter()
            .map(build_execute_item)
            .collect::<Result<Vec<_>>>()?;

        let mut request = ExecuteTransactionsRequest::default()
            .with_transactions(items)
            .with_read_mask(field_mask_with_default(
                read_mask,
                EXECUTE_TRANSACTIONS_READ_MASK,
            ));

        if let Some(timeout_ms) = checkpoint_inclusion_timeout_ms {
            request = request.with_checkpoint_inclusion_timeout_ms(timeout_ms);
        }

        let response = self
            .execution_service_client()
            .execute_transactions(request)
            .await?;

        MetadataEnvelope::from(response).try_map(|r| {
            Ok(r.transaction_results
                .into_iter()
                .map(ProtoResult::into_result)
                .collect())
        })
    }
}

/// Convert a `SignedTransaction` into a proto `ExecuteTransactionItem`.
fn build_execute_item(signed_transaction: SignedTransaction) -> Result<ExecuteTransactionItem> {
    let tx_digest = signed_transaction.transaction.digest();
    let proto_transaction = build_proto_transaction(&signed_transaction.transaction, tx_digest)?;

    let proto_signatures = UserSignatures::default().with_signatures(
        signed_transaction
            .signatures
            .into_iter()
            .map(|sig| {
                ProtoUserSignature::try_from(sig).map_err(|e| Error::Signature(e.to_string()))
            })
            .collect::<Result<Vec<_>>>()?,
    );

    Ok(ExecuteTransactionItem::default()
        .with_transaction(proto_transaction)
        .with_signatures(proto_signatures))
}
