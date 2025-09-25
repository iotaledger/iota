// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
use std::{
    collections::{BTreeMap, HashSet},
    time::Duration,
};

use diesel::{PgConnection, RunQueryDsl, result::DatabaseErrorKind, sql_query, sql_types};
use downcast::Any;
use fastcrypto::{encoding::Base64, error::FastCryptoError, traits::ToFromBytes};
use iota_json_rpc_types::{IotaTransactionBlockResponse, IotaTransactionBlockResponseOptions};
use iota_rest_api::{ExecuteTransactionQueryParameters, client::TransactionExecutionResponse};
use iota_types::{
    base_types::TransactionDigest,
    effects::{TransactionEffects, TransactionEffectsAPI},
    full_checkpoint_content::CheckpointTransaction,
    signature::GenericSignature,
    transaction::{Transaction, TransactionData},
};

use crate::{
    errors::IndexerError,
    handlers::{
        TransactionObjectChangesToCommit,
        checkpoint_handler::{
            CheckpointHandler, IndexedTransactionComponentsV2, try_extract_df_kind,
        },
    },
    indexer_reader::IndexerReader,
    metrics::IndexerMetrics,
    models::{
        display::StoredDisplay,
        transactions::{OptimisticTransaction, TxGlobalOrder},
    },
    store::{IndexerStore, PgIndexerStore},
    transactional_blocking_with_retry_with_conditional_abort,
    types::{
        IndexedDeletedObject, IndexedObject, IndexerResult, IotaTransactionBlockResponseWithOptions,
    },
};

const WAIT_FOR_DEPS_MAX_ELAPSED_TIME: Duration = Duration::from_secs(3);

type TransactionDataToCommit = (
    OptimisticTransaction,
    BTreeMap<String, StoredDisplay>,
    TransactionObjectChangesToCommit,
);

#[derive(Clone)]
pub(crate) struct OptimisticTransactionExecutor {
    rpc_client: iota_rest_api::Client,
    indexer_reader: IndexerReader,
    store: PgIndexerStore,
    metrics: IndexerMetrics,
}

impl OptimisticTransactionExecutor {
    pub(crate) fn new(
        rpc_client_url: &str,
        indexer_reader: IndexerReader,
        store: PgIndexerStore,
        metrics: IndexerMetrics,
    ) -> Self {
        let rpc_client = iota_rest_api::Client::new(rpc_client_url);
        Self {
            rpc_client,
            indexer_reader,
            store,
            metrics,
        }
    }

    /// Wait until all dependencies are indexed through the `tx_global_order`
    /// table.
    ///
    /// It uses exponential backoff to retry the check.
    ///
    /// This does not cover old transactions that do not have
    /// entries in `tx_global_order`.
    pub(crate) async fn wait_for_tx_dependencies(
        &self,
        effects: &TransactionEffects,
    ) -> Result<(), IndexerError> {
        let expected_dependencies = effects
            .dependencies()
            .iter()
            .copied()
            .collect::<HashSet<_>>();
        let backoff = backoff::ExponentialBackoff {
            max_elapsed_time: Some(WAIT_FOR_DEPS_MAX_ELAPSED_TIME),
            ..Default::default()
        };

        backoff::future::retry(backoff, async || {
            let count = self
                .indexer_reader
                .count_indexed_tx_global_orders_in_blocking_task(expected_dependencies.clone())
                .await?;
            if count as usize != expected_dependencies.len() {
                return Err(IndexerError::TransactionDependenciesNotIndexed)?;
            }
            Ok(())
        })
        .await
        .or(Err(IndexerError::TransactionDependenciesNotIndexed))
    }

    /// Index the executed transaction under the following conditions:
    ///
    /// * If the transaction has input and output objects, and
    /// * If the transaction dependencies are already indexed.
    ///
    /// The latter is essential in avoiding race conditions while
    /// indexing checkpointed transactions.
    pub(crate) async fn maybe_index_executed_transaction(
        &self,
        transaction: Transaction,
        execution_response: TransactionExecutionResponse,
    ) -> Result<(), IndexerError> {
        let TransactionExecutionResponse {
            effects,
            events,
            input_objects,
            output_objects,
            ..
        } = execution_response;
        let tx_digest = transaction.digest();
        let (Some(input_objects), Some(output_objects)) = (input_objects, output_objects) else {
            tracing::warn!(
                "Cannot optimistically index because of missing in/out objs for tx: {tx_digest}"
            );
            return Ok(());
        };

        if input_objects.is_empty() || output_objects.is_empty() {
            tracing::warn!(
                "Cannot optimistically index because of missing in/out objs for tx: {tx_digest}"
            );
            return Ok(());
        }
        tokio::select! {
            Ok(_) = self.wait_for_tx_dependencies(&effects) => (),
            Ok(true) = self.deep_check_all_dependencies_are_indexed(&effects) => (),
            else => {
                tracing::warn!(
                    "Transaction {tx_digest} dependencies are not indexed, skipping optimistic indexing",
                );
                return Ok(());
            }
        };
        let full_tx_data = CheckpointTransaction {
            transaction,
            effects,
            events,
            input_objects,
            output_objects,
        };
        self.index_transaction_in_blocking_task(&full_tx_data).await
    }

    /// Expensive operation that checks if all transactions
    /// are indexed.
    ///
    /// This queries both `tx_global_order` which represents
    /// the index status for newer transactions, and the `checkpoints`
    /// table for older transactions that do not have entries
    /// in `tx_global_order`.
    pub(crate) async fn deep_check_all_dependencies_are_indexed(
        &self,
        effects: &TransactionEffects,
    ) -> Result<bool, IndexerError> {
        self.indexer_reader
            .deep_check_all_transactions_are_indexed_in_blocking_task(
                effects.dependencies().to_vec(),
            )
            .await
    }

    pub(crate) async fn execute_and_index_transaction(
        &self,
        tx_bytes: Base64,
        signatures: Vec<Base64>,
        options: Option<IotaTransactionBlockResponseOptions>,
    ) -> Result<IotaTransactionBlockResponse, IndexerError> {
        let tx_data: TransactionData = bcs::from_bytes(&tx_bytes.to_vec()?)?;
        let sigs = signatures
            .into_iter()
            .map(|sig| GenericSignature::from_bytes(&sig.to_vec()?))
            .collect::<Result<Vec<_>, FastCryptoError>>()?;

        let transaction = Transaction::from_generic_sig_data(tx_data, sigs);
        let response = self
            .rpc_client
            .execute_transaction(
                &ExecuteTransactionQueryParameters {
                    events: true,
                    balance_changes: false,
                    input_objects: true,
                    output_objects: true,
                },
                &transaction,
            )
            .await
            .map_err(|e| IndexerError::Generic(e.to_string()))?;

        let tx_digest = *response.effects.transaction_digest();
        self.maybe_index_executed_transaction(transaction, response)
            .await?;
        let tx_block_response = self
            .wait_for_local_indexing(tx_digest, options.clone())
            .await?;

        Ok(IotaTransactionBlockResponseWithOptions {
            response: tx_block_response,
            options: options.unwrap_or_default(),
        }
        .into())
    }

    async fn wait_for_local_indexing(
        &self,
        tx_digest: TransactionDigest,
        options: Option<IotaTransactionBlockResponseOptions>,
    ) -> Result<IotaTransactionBlockResponse, IndexerError> {
        let backoff = backoff::ExponentialBackoff {
            max_elapsed_time: Some(Duration::from_secs(30)),
            ..Default::default()
        };

        backoff::future::retry(backoff, async || {
            let tx_block_response = self
                .indexer_reader
                .multi_get_transaction_block_response_in_blocking_task(
                    vec![tx_digest],
                    options.clone().unwrap_or_default(),
                )
                .await
                .map_err(|e| backoff::Error::Transient {
                    err: e,
                    retry_after: None,
                })?
                .pop();

            match tx_block_response {
                Some(tx_block_response) => Ok(tx_block_response),
                None => Err(backoff::Error::Transient {
                    err: IndexerError::PostgresRead("Transaction not present in DB".to_string()),
                    retry_after: None,
                }),
            }
        })
        .await
    }

    async fn index_transaction_in_blocking_task(
        &self,
        full_tx_data: &CheckpointTransaction,
    ) -> Result<(), IndexerError> {
        match tokio::task::spawn_blocking({
            let this: OptimisticTransactionExecutor = self.clone();
            let full_tx_data = full_tx_data.clone();
            move || this.index_transaction(&full_tx_data)
        })
        .await
        .map_err(|e| {
            tracing::error!("Failed to join optimistic index_transaction: {e}");
            IndexerError::from(e)
        })? {
            // The unique violation error means that checkpoint indexing was faster than the
            // optimistic indexing. Let's just return and let checkpoint indexing handle
            // the transaction.
            Ok(_) | Err(IndexerError::PostgresUniqueTxGlobalOrderViolation(_)) => Ok(()),
            Err(e) => Err(IndexerError::PostgresWrite(format!(
                "Failed to persist optimistic tx: {e:?}",
            ))),
        }
    }

    fn index_transaction(&self, full_tx_data: &CheckpointTransaction) -> Result<(), IndexerError> {
        let pool = self.store.blocking_cp();
        transactional_blocking_with_retry_with_conditional_abort!(
            &pool,
            move |conn| {
                let assigned_global_order =
                    OptimisticTransactionExecutor::assign_optimistic_tx_global_order(
                        conn,
                        full_tx_data.transaction.digest(),
                    )?;

                let extractor = TransactionExtractor::new(
                    full_tx_data,
                    assigned_global_order
                        .optimistic_sequence_number
                        .expect("Optimistic sequence number is always set for data read from DB")
                        .try_into()
                        .map_err(|e| {
                            IndexerError::PersistentStorageDataCorruption(format!(
                                "Failed to convert optimistic sequence number: {e}"
                            ))
                        })?,
                    &self.metrics,
                );

                let tx_data_to_commit = extractor
                    .to_transaction_data_to_commit(assigned_global_order.global_sequence_number)?;

                self.persist_optimistic_tx(conn, tx_data_to_commit)
            },
            |e: &IndexerError| matches!(*e, IndexerError::PostgresUniqueTxGlobalOrderViolation(_)),
            Duration::from_secs(3600)
        )
    }

    fn assign_optimistic_tx_global_order(
        conn: &mut PgConnection,
        tx_digest: &TransactionDigest,
    ) -> Result<TxGlobalOrder, IndexerError> {
        let tx_digest_bytes = tx_digest.inner().to_vec();

        sql_query(
            r#"
                INSERT INTO tx_global_order (tx_digest, global_sequence_number, chk_tx_sequence_number)
                SELECT $1, MAX(tx_sequence_number), NULL FROM tx_digests
                RETURNING *;
            "#,
        )
        .bind::<sql_types::Bytea, _>(&tx_digest_bytes)
        .get_result::<TxGlobalOrder>(conn)
        .map_err(|e| match e {
            diesel::result::Error::DatabaseError(DatabaseErrorKind::UniqueViolation, _) => {
                IndexerError::PostgresUniqueTxGlobalOrderViolation(e.to_string())
            }
            _ => IndexerError::PostgresWrite(format!("Failed to assign global order: {e}")),
        })
    }

    fn persist_optimistic_tx(
        &self,
        conn: &mut PgConnection,
        tx_data_to_commit: TransactionDataToCommit,
    ) -> Result<(), IndexerError> {
        let (optimistic_tx, indexed_displays, object_changes) = tx_data_to_commit;

        self.store
            .persist_objects_in_existing_transaction(conn, vec![object_changes.clone()])?;
        self.store.persist_displays_in_existing_transaction(
            conn,
            indexed_displays.values().collect::<Vec<_>>(),
        )?;

        self.store
            .persist_optimistic_transaction_in_existing_transaction(conn, optimistic_tx.clone())
    }
}

struct TransactionExtractor<'a> {
    full_tx_data: &'a CheckpointTransaction,
    optimistic_sequence_number: u64,
    metrics: &'a IndexerMetrics,
}

impl<'a> TransactionExtractor<'a> {
    fn new(
        full_tx_data: &'a CheckpointTransaction,
        optimistic_sequence_number: u64,
        metrics: &'a IndexerMetrics,
    ) -> Self {
        Self {
            full_tx_data,
            optimistic_sequence_number,
            metrics,
        }
    }

    fn get_object_changes(&self) -> IndexerResult<TransactionObjectChangesToCommit> {
        let indexed_eventually_removed_objects = self
            .full_tx_data
            .removed_object_refs_post_version()
            .map(|obj_ref| IndexedDeletedObject {
                object_id: obj_ref.0,
                object_version: obj_ref.1.into(),
                checkpoint_sequence_number: 0,
            })
            .collect::<Vec<_>>();

        let changed_objects = self
            .full_tx_data
            .output_objects
            .iter()
            .map(|o| {
                try_extract_df_kind(o).map(|df_kind| {
                    IndexedObject::from_object(
                        0, // checkpoint sequence number, ignored in further processing
                        o.clone(),
                        df_kind,
                    )
                })
            })
            .collect::<Result<Vec<_>, _>>()?;

        Ok(TransactionObjectChangesToCommit {
            changed_objects,
            deleted_objects: indexed_eventually_removed_objects,
        })
    }

    fn get_indexed_transactions_events_and_displays(
        &self,
    ) -> IndexerResult<IndexedTransactionComponentsV2> {
        let handle = tokio::runtime::Handle::current();
        handle.block_on(async move {
            CheckpointHandler::index_transaction_v2(
                self.full_tx_data,
                self.optimistic_sequence_number,
                0, // checkpoint sequence number - unknown
                0, // checkpoint timestamp - unknown
                self.metrics,
            )
            .await
        })
    }

    fn to_transaction_data_to_commit(
        &self,
        global_sequence_number: i64,
    ) -> IndexerResult<TransactionDataToCommit> {
        let object_changes = self.get_object_changes()?;
        let (indexed_tx, _, _, _, indexed_displays) =
            self.get_indexed_transactions_events_and_displays()?;

        let optimistic_tx =
            OptimisticTransaction::from_stored(global_sequence_number, (&indexed_tx).into());

        Ok((optimistic_tx, indexed_displays, object_changes))
    }
}
