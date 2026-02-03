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
    api::{ProtoResult, Result, TRANSACTIONS_READ_MASK, field_mask_with_default},
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
    /// # Field Mask
    ///
    /// The optional `read_mask` parameter controls which fields the server
    /// returns. If `None`, uses [`TRANSACTIONS_READ_MASK`].
    ///
    /// **Optional fields:**
    /// - `transaction.bcs` - Transaction data
    /// - `transaction.digest` - Transaction digest
    /// - `signatures` - User signatures
    /// - `effects.bcs` - Transaction effects
    /// - `effects.digest` - Effects digest
    /// - `events` - Transaction events
    /// - `checkpoint` - Checkpoint sequence number
    /// - `timestamp` - Execution timestamp
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
    ///     let effects = tx.effects()?;
    ///     println!("Status: {:?}", effects.status());
    ///
    ///     // Access raw proto fields without deserialization
    ///     if let Some(checkpoint) = tx.checkpoint_sequence_number() {
    ///         println!("Checkpoint: {}", checkpoint);
    ///     }
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

        let requests = TransactionRequests {
            requests: digests
                .iter()
                .map(|d| TransactionRequest {
                    digest: Some((*d).into()),
                })
                .collect(),
        };

        let request = GetTransactionsRequest {
            requests: Some(requests),
            read_mask: Some(field_mask_with_default(read_mask, TRANSACTIONS_READ_MASK)),
            max_message_size_bytes: self.max_decoding_message_size().map(|s| s as u32),
        };

        let mut client = self.ledger_service_client();

        let mut stream = client.get_transactions(request).await?.into_inner();

        // Server guarantees results are returned in request order
        let mut results = Vec::with_capacity(digests.len());

        while let Some(response) = stream.message().await? {
            for result in response.transactions {
                results.push(result.into_result()?);
            }
        }

        Ok(results)
    }
}
