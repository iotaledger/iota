// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
use async_graphql::*;
use fastcrypto::encoding::Base64;
use iota_indexer::optimistic_indexing::OptimisticTransactionExecutor;
use iota_json_rpc_types::IotaTransactionBlockResponseOptions;
use iota_types::{
    effects::TransactionEffects as NativeTransactionEffects, event::Event as NativeEvent,
    transaction::SenderSignedData,
};

use crate::{
    error::Error,
    types::{
        execution_result::ExecutionResult,
        transaction_block_effects::{TransactionBlockEffects, TransactionBlockEffectsKind},
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
    /// There may be a delay between transaction finality and when GraphQL
    /// requests (including the request that issued the transaction) reflect
    /// its effects. As a result, queries that depend on indexing the state
    /// of the chain (e.g. contents of output objects, address-level balance
    /// information at the time of the transaction), must wait for indexing to
    /// catch up by polling for the transaction digest using
    /// `Query.transactionBlock`.
    async fn execute_transaction_block(
        &self,
        ctx: &Context<'_>,
        tx_bytes: String,
        signatures: Vec<String>,
    ) -> Result<ExecutionResult> {
        let optimistic_tx_executor: &Option<OptimisticTransactionExecutor> = ctx
            .data()
            .map_err(|_| {
                Error::Internal("Unable to fetch OptimisticTransactionExecutor".to_string())
            })
            .extend()?;
        let optimistic_tx_executor = optimistic_tx_executor
            .as_ref()
            .ok_or_else(|| {
                Error::Internal("OptimisticTransactionExecutor not initialized".to_string())
            })
            .extend()?;
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
        let options = IotaTransactionBlockResponseOptions::new()
            .with_events()
            .with_raw_input()
            .with_raw_effects();

        let result = optimistic_tx_executor
            .execute_and_index_transaction(tx_data, sigs, Some(options))
            .await
            .map_err(|e| Error::Internal(format!("Unable to execute transaction: {e}")))
            .extend()?;

        let native: NativeTransactionEffects = bcs::from_bytes(&result.raw_effects)
            .map_err(|e| Error::Internal(format!("Unable to deserialize transaction effects: {e}")))
            .extend()?;
        let tx_data: SenderSignedData = bcs::from_bytes(&result.raw_transaction)
            .map_err(|e| Error::Internal(format!("Unable to deserialize transaction data: {e}")))
            .extend()?;

        let events = result
            .events
            .ok_or_else(|| {
                Error::Internal("No events are returned from transaction execution".to_string())
            })?
            .data
            .into_iter()
            .map(|e| NativeEvent {
                package_id: e.package_id,
                transaction_module: e.transaction_module,
                sender: e.sender,
                type_: e.type_,
                contents: e.bcs.into_bytes(),
            })
            .collect();

        Ok(ExecutionResult {
            errors: if result.errors.is_empty() {
                None
            } else {
                Some(result.errors)
            },
            effects: TransactionBlockEffects {
                kind: TransactionBlockEffectsKind::Executed {
                    tx_data,
                    native,
                    events,
                },
                // set to u64::MAX, as the executed transaction has not been indexed yet
                checkpoint_viewed_at: u64::MAX,
            },
        })
    }
}
