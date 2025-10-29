// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
use async_graphql::*;
use diesel::{BoolExpressionMethods, ExpressionMethods, JoinOnDsl, QueryDsl, SelectableHelper};
use fastcrypto::encoding::Base64;
use iota_indexer::{
    models::transactions::{OptimisticTransaction, StoredTransaction},
    schema::{optimistic_transactions, transactions, tx_digests, tx_global_order},
};
use iota_json_rpc_api::WriteApiServer;
use iota_json_rpc_types::IotaTransactionBlockResponseOptions;

use crate::{
    data::{Db, DbConnection, QueryExecutor},
    error::Error,
    server::builder::get_write_api,
    types::{
        execution_result::ExecutionResult, transaction_block::TransactionBlock,
        transaction_block_effects::TransactionBlockEffects,
    },
};
pub struct Mutation;

/// Query checkpointed transaction by digest from the database
async fn query_checkpointed_transaction_by_digest(
    db: &Db,
    digest_bytes: Vec<u8>,
) -> Result<StoredTransaction, Error> {
    db.execute_repeatable(move |conn| {
        conn.first(move || {
            transactions::table
                .inner_join(
                    tx_digests::table
                        .on(transactions::tx_sequence_number.eq(tx_digests::tx_sequence_number)),
                )
                .filter(tx_digests::tx_digest.eq(digest_bytes.clone()))
                .select(StoredTransaction::as_select())
        })
    })
    .await
    .map_err(|e| Error::Internal(format!("Unable to query checkpointed transaction: {e}")))
}

/// Query optimistic transaction by digest from the database
async fn query_optimistic_transaction_by_digest(
    db: &Db,
    digest_bytes: Vec<u8>,
) -> Result<OptimisticTransaction, Error> {
    db.execute_repeatable(move |conn| {
        conn.first(move || {
            optimistic_transactions::table
                .inner_join(
                    tx_global_order::table.on(optimistic_transactions::global_sequence_number
                        .eq(tx_global_order::global_sequence_number)
                        .and(
                            optimistic_transactions::optimistic_sequence_number
                                .eq(tx_global_order::optimistic_sequence_number),
                        )),
                )
                .filter(tx_global_order::tx_digest.eq(digest_bytes.clone()))
                .select(OptimisticTransaction::as_select())
        })
    })
    .await
    .map_err(|e| Error::Internal(format!("Unable to query optimistic transaction: {e}")))
}

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
        let options = IotaTransactionBlockResponseOptions::new()
            .with_events()
            .with_raw_input()
            .with_raw_effects();

        let result = write_api
            .execute_transaction_block(tx_data, sigs, Some(options), None)
            .await
            .map_err(|e| Error::Internal(format!("Unable to execute transaction: {e}")))
            .extend()?;

        let tx_digest = result.digest;
        let digest_bytes = tx_digest.inner().to_vec();

        let db: &Db = ctx.data_unchecked();
        let query_optimistic_tx = query_optimistic_transaction_by_digest(db, digest_bytes.clone());
        let query_checkpointed_tx = query_checkpointed_transaction_by_digest(db, digest_bytes);
        tokio::pin!(query_optimistic_tx, query_checkpointed_tx);

        let effects: Result<TransactionBlockEffects, _> = tokio::select! {
                checkpointed_tx = &mut query_checkpointed_tx => match checkpointed_tx {
                    Ok(checkpointed_tx) => TransactionBlock::try_from(checkpointed_tx)?.try_into(),
                    _ => query_optimistic_tx.await?.try_into()
                },
                optimistic_tx = &mut query_optimistic_tx => {
                    match optimistic_tx {
                        Ok(optimistic_tx) => optimistic_tx.try_into(),
                        _ => TransactionBlock::try_from(query_checkpointed_tx.await?)?.try_into(),
                    }
                }
        };

        Ok(ExecutionResult {
            errors: if result.errors.is_empty() {
                None
            } else {
                Some(result.errors)
            },
            effects: effects
                .map_err(|_| Error::Internal("Transaction not indexed after execution".into()))
                .extend()?,
        })
    }
}
