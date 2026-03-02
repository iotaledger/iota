// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! High-level API for transaction queries.

use iota_grpc_types::v0::{
    ledger_service::{GetTransactionsRequest, TransactionRequest, TransactionRequests},
    transaction::ExecutedTransaction,
};
use iota_sdk_types::Digest;

use crate::{
    Client,
    api::{Error, GET_TRANSACTIONS_READ_MASK, ProtoResult, Result, field_mask_with_default},
};

impl Client {
    /// Get transactions by their digests.
    ///
    /// Returns proto `ExecutedTransaction` for each transaction. Use the lazy
    /// conversion methods to extract data:
    /// - `tx.digest()` - Get transaction digest
    /// - `tx.transaction()` - Deserialize transaction
    /// - `tx.signatures()` - Deserialize signatures
    /// - `tx.effects()` - Deserialize effects
    /// - `tx.events()` - Deserialize events (if available)
    /// - `tx.checkpoint_sequence_number()` - Get checkpoint number
    /// - `tx.timestamp_ms()` - Get timestamp
    ///
    /// Results are returned in the same order as the input digests.
    /// If a transaction is not found, an error is returned.
    ///
    /// # Available Read Mask Fields
    ///
    /// The optional `read_mask` parameter controls which fields the server
    /// returns. If `None`, uses [`GET_TRANSACTIONS_READ_MASK`].
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
    ///
    /// ## Event Fields
    /// - `events` - includes all event fields
    ///   - `events.digest` - the events digest
    ///   - `events.events` - includes all event fields
    ///     - `events.events.bcs` - the full BCS-encoded event
    ///     - `events.events.package_id` - the ID of the package that emitted
    ///       the event
    ///     - `events.events.module` - the module that emitted the event
    ///     - `events.events.sender` - the sender that triggered the event
    ///     - `events.events.event_type` - the type of the event
    ///     - `events.events.bcs_contents` - the full BCS-encoded contents of
    ///       the event
    ///     - `events.events.json_contents` - the JSON-encoded contents of the
    ///       event
    ///
    /// ## Timing Fields
    /// - `checkpoint` - the checkpoint that included the transaction
    /// - `timestamp` - the timestamp of the checkpoint that included the
    ///   transaction
    ///
    /// ## Object Fields
    /// - `input_objects` - includes all input object fields
    ///   - `input_objects.reference` - includes all reference fields
    ///     - `input_objects.reference.object_id` - the ID of the input object
    ///     - `input_objects.reference.version` - the version of the input
    ///       object, which can be used to fetch a specific historical version
    ///       or the latest version if not provided
    ///     - `input_objects.reference.digest` - the digest of the input object
    ///       contents, which can be used for integrity verification
    ///   - `input_objects.bcs` - the full BCS-encoded object
    /// - `output_objects` - includes all output object fields
    ///   - `output_objects.reference` - includes all reference fields
    ///     - `output_objects.reference.object_id` - the ID of the output object
    ///     - `output_objects.reference.version` - the version of the output
    ///       object, which can be used to fetch a specific historical version
    ///       or the latest version if not provided
    ///     - `output_objects.reference.digest` - the digest of the output
    ///       object contents, which can be used for integrity verification
    ///   - `output_objects.bcs` - the full BCS-encoded object
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use iota_grpc_client::Client;
    /// # use iota_sdk_types::Digest;
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let client = Client::connect("http://localhost:9000").await?;
    /// let digest: Digest = todo!();
    ///
    /// // Get transactions - returns proto types
    /// let txs = client.get_transactions(&[digest], None).await?;
    ///
    /// for tx in txs {
    ///     // Lazy conversion - only deserialize what you need
    ///     let effects = tx.effects()?.effects()?;
    ///     println!("Status: {:?}", effects.status());
    ///
    ///     // Access checkpoint number
    ///     let checkpoint = tx.checkpoint_sequence_number()?;
    ///     println!("Checkpoint: {}", checkpoint);
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub async fn get_transactions(
        &self,
        digests: &[Digest],
        read_mask: Option<&str>,
    ) -> Result<Vec<ExecutedTransaction>> {
        if digests.is_empty() {
            return Ok(vec![]);
        }

        let requests = TransactionRequests::default().with_requests(
            digests
                .iter()
                .map(|d| TransactionRequest::default().with_digest(*d))
                .collect(),
        );

        let mut request = GetTransactionsRequest::default()
            .with_requests(requests)
            .with_read_mask(field_mask_with_default(
                read_mask,
                GET_TRANSACTIONS_READ_MASK,
            ));

        if let Some(max_size) = self.max_decoding_message_size() {
            request = request.with_max_message_size_bytes(max_size as u32);
        }

        let mut client = self.ledger_service_client();

        let mut stream = client.get_transactions(request).await?.into_inner();

        // Server guarantees results are returned in request order
        let mut results = Vec::with_capacity(digests.len());
        let mut has_next = false;

        while let Some(response) = stream.message().await? {
            has_next = response.has_next;
            for result in response.transaction_results {
                results.push(result.into_result()?);
            }
        }

        if has_next {
            return Err(Error::UnexpectedEndOfStream);
        }

        Ok(results)
    }
}
