// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! High-level API for transaction queries.

use iota_grpc_types::{
    proto::proto_to_timestamp_ms,
    v0::ledger_service::{GetTransactionsRequest, TransactionRequest, TransactionRequests},
};
use iota_sdk_types::{Digest, Transaction, UserSignature};

use crate::{
    Client,
    api::{
        Error, ProtoResult, Result, TRANSACTIONS_READ_MASK, TransactionResponse, TryFromProtoError,
        extract_effects_and_events, field_mask_with_default,
    },
};

impl Client {
    /// Get transactions by their digests.
    ///
    /// Returns `TransactionResponse` for each transaction, containing the
    /// transaction data, signatures, effects, and optional events.
    ///
    /// Results are returned in the same order as the input digests.
    /// If a transaction is not found, an error is returned.
    ///
    /// # Field Mask
    ///
    /// The optional `read_mask` parameter controls which fields the server
    /// returns. If `None`, uses [`TRANSACTIONS_READ_MASK`] which includes all
    /// fields needed for `TransactionResponse`.
    ///
    /// **Required fields** (must be included in custom masks):
    /// - `transaction.bcs` - Transaction data
    /// - `signatures.bcs` - User signatures
    /// - `effects.bcs` - Transaction effects
    ///
    /// **Optional fields:**
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
    /// // Default: get all fields
    /// let txs = client.get_transactions(&[digest], None).await?;
    ///
    /// // Custom: only required fields (smaller response)
    /// let txs = client
    ///     .get_transactions(
    ///         &[digest],
    ///         Some("transaction.bcs,signatures.bcs,effects.bcs"),
    ///     )
    ///     .await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn get_transactions(
        &self,
        digests: &[Digest],
        read_mask: Option<&str>,
    ) -> Result<Vec<TransactionResponse>> {
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
                let proto_tx = result.into_result()?;
                results.push(convert_to_response(*proto_tx)?);
            }
        }

        Ok(results)
    }
}

/// Convert a proto ExecutedTransaction to TransactionResponse.
fn convert_to_response(
    proto: iota_grpc_types::v0::transaction::ExecutedTransaction,
) -> Result<TransactionResponse> {
    // Extract and deserialize transaction BCS (required)
    // Note: The BCS contains Transaction (not SignedTransaction).
    // Signatures are stored separately in proto.signatures.
    let tx_bcs = proto
        .transaction
        .as_ref()
        .and_then(|t| t.bcs.as_ref())
        .ok_or(TryFromProtoError::missing("transaction.bcs"))?;

    let transaction: Transaction = tx_bcs
        .deserialize()
        .map_err(|e| TryFromProtoError::invalid("transaction.bcs", e))?;

    // Extract signatures from proto.signatures
    let signatures = extract_signatures(&proto)?;

    // Extract checkpoint and timestamp from proto (these are per-transaction
    // historical data, not available in response headers)
    let checkpoint = proto.checkpoint;
    let timestamp_ms = proto.timestamp.map(proto_to_timestamp_ms).transpose()?;

    let (effects, events) = extract_effects_and_events(&proto)?;

    Ok(TransactionResponse {
        digest: transaction.digest(),
        transaction,
        signatures,
        effects,
        events,
        checkpoint,
        timestamp_ms,
    })
}

/// Extract user signatures from proto ExecutedTransaction.
fn extract_signatures(
    proto: &iota_grpc_types::v0::transaction::ExecutedTransaction,
) -> Result<Vec<UserSignature>> {
    proto
        .signatures
        .as_ref()
        .map(|sigs| {
            sigs.signatures
                .iter()
                .map(|sig| {
                    UserSignature::try_from(sig).map_err(|e| Error::Signature(e.to_string()))
                })
                .collect::<Result<Vec<_>>>()
        })
        .unwrap_or_else(|| Ok(vec![]))
}
