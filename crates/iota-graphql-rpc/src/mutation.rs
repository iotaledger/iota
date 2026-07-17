// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
use async_graphql::*;
use fastcrypto::encoding::Base64;

use crate::{
    error::Error,
    server::builder::get_write_api,
    types::{
        execution_result::ExecutionResult, transaction_block_effects::TransactionBlockEffects,
    },
};
pub struct Mutation;

/// Mutations are used to write to the IOTA network.
#[Object]
impl Mutation {
    /// Execute a transaction, committing its effects on chain.
    ///
    /// - `txBytes` is a `TransactionData` struct that has been BCS-encoded and
    ///   then Base64-encoded.
    /// - `signatures` are a list of `flag || signature || pubkey` bytes,
    ///   Base64-encoded.
    ///
    /// Waits until the transaction has reached finality on chain to return its
    /// transaction digest, or returns the error that prevented finality if
    /// that was not possible. A transaction is final when its effects are
    /// guaranteed on chain (it cannot be revoked).
    ///
    /// Transaction effects are now available immediately after execution
    /// through `Query.transactionBlock`. However, other queries that depend
    /// on the chain’s indexed state (e.g., address-level balance updates)
    /// may still lag until the transaction has been checkpointed.
    /// To confirm that a transaction has been included in a checkpoint, query
    /// `Query.transactionBlock` and check whether the `effects.checkpoint`
    /// field is set (or `null` if not yet checkpointed).
    async fn execute_transaction_block(
        &self,
        ctx: &Context<'_>,
        tx_bytes: String,
        signatures: Vec<String>,
    ) -> Result<ExecutionResult> {
        let write_api = get_write_api(ctx).extend()?;
        let tx_data = Base64::try_from(tx_bytes)
            .map_err(|e| {
                Error::Client(format!(
                    "Unable to deserialize transaction bytes from Base64: {e}"
                ))
            })
            .extend()?;

        let mut sigs = Vec::new();
        for sig in signatures {
            sigs.push(
                Base64::try_from(sig.clone())
                    .map_err(|e| {
                        Error::Client(format!(
                            "Unable to deserialize signature bytes {sig} from Base64: {e}"
                        ))
                    })
                    .extend()?,
            );
        }
        let ingestion_path = write_api
            .executor()
            .execute_and_index_transaction(tx_data, sigs)
            .await
            .map_err(|e| Error::Internal(format!("Unable to execute transaction: {e}")))
            .extend()?;

        let effects = TransactionBlockEffects::try_from(ingestion_path).extend()?;

        Ok(ExecutionResult {
            errors: effects.errors(ctx).await?.map(|e| vec![e]),
            effects,
        })
    }
}
