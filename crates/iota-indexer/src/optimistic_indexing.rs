// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
use std::{
    collections::{BTreeMap, HashSet},
    time::Duration,
};

use diesel::{PgConnection, RunQueryDsl, result::DatabaseErrorKind, sql_query, sql_types};
use downcast::Any;
use fastcrypto::{encoding::Base64, error::FastCryptoError, traits::ToFromBytes};
use iota_grpc_client::Client as GrpcClient;
use iota_grpc_types::{
    field::{FieldMask, FieldMaskUtil},
    v1::transaction::ExecutedTransaction,
};
use iota_json_rpc_types::{IotaTransactionBlockResponse, IotaTransactionBlockResponseOptions};
use iota_types::{
    base_types::TransactionDigest,
    effects::{TransactionEffects, TransactionEffectsAPI, TransactionEvents},
    full_checkpoint_content::CheckpointTransaction,
    signature::GenericSignature,
    transaction::{Transaction, TransactionData},
};

use crate::{
    errors::IndexerError,
    ingestion::{
        common::prepare::extract_df_kind,
        primary::{
            persist::TransactionObjectChangesToCommit,
            prepare::{IndexedTransactionComponents, PrimaryWorker},
        },
    },
    metrics::IndexerMetrics,
    models::{
        display::StoredDisplay,
        transactions::{OptimisticTransaction, TxGlobalOrder},
    },
    read::IndexerReader,
    store::{IndexerStore, PgIndexerStore},
    transactional_blocking_with_retry_with_conditional_abort,
    types::{
        IndexedDeletedObject, IndexedObject, IndexerResult,
        IotaTransactionBlockResponseWithOptions, grpc_conversion,
    },
};

const WAIT_FOR_DEPS_MAX_ELAPSED_TIME: Duration = Duration::from_secs(3);

// As an optimization, we're trying to request only the fields we actually need.
const EXECUTE_TRANSACTION_READ_MASK: &[&str] = &[
    "effects.bcs",
    "events.events.bcs",
    "input_objects.bcs",
    "output_objects.bcs",
];

type TransactionDataToCommit = (
    OptimisticTransaction,
    BTreeMap<String, StoredDisplay>,
    TransactionObjectChangesToCommit,
);

#[derive(Clone)]
pub struct OptimisticTransactionExecutor {
    rpc_client: GrpcClient,
    pub(crate) read: IndexerReader,
    store: PgIndexerStore,
    metrics: IndexerMetrics,
}

impl OptimisticTransactionExecutor {
    pub async fn new(
        fullnode_grpc_client: GrpcClient,
        read: IndexerReader,
        store: PgIndexerStore,
        metrics: IndexerMetrics,
    ) -> IndexerResult<Self> {
        Ok(Self {
            rpc_client: fullnode_grpc_client,
            read,
            store,
            metrics,
        })
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
            let digests: Vec<Vec<u8>> = expected_dependencies
                .iter()
                .map(|d| d.inner().to_vec())
                .collect();
            let count = self
                .read
                .count_indexed_tx_global_orders_in_blocking_task(digests.into_iter())
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
        executed_transaction: ExecutedTransaction,
    ) -> Result<(), IndexerError> {
        // The methods check for fields being Some. Based on the provided read mask,
        // all fields should be Some, the only exception should be `checkpoint` &
        // `timestamp` fields which are always None.
        let effects = executed_transaction.effects()?.effects()?.try_into()?;
        let events = TransactionEvents::try_from(executed_transaction.events()?.events()?)?;
        let input_objects = grpc_conversion::objects(executed_transaction.input_objects()?)?;
        let output_objects = grpc_conversion::objects(executed_transaction.output_objects()?)?;

        let tx_digest = transaction.digest();

        if input_objects.is_empty() || output_objects.is_empty() {
            tracing::warn!(
                "cannot optimistically index because of missing in/out objs for tx: {tx_digest}"
            );
            self.metrics.optimistic_tx_with_missing_objects_counts.inc();
            return Ok(());
        }
        let deps_timer = self
            .metrics
            .optimistic_tx_dependencies_wait_time
            .start_timer();
        tokio::select! {
            Ok(_) = self.wait_for_tx_dependencies(&effects) => {
                deps_timer.stop_and_record();
            },
            Ok(true) = self.deep_check_all_dependencies_are_indexed(&effects) => {
                deps_timer.stop_and_record();
            },
            else => {
                deps_timer.stop_and_discard();
                tracing::warn!(
                    "transaction {tx_digest} dependencies are not indexed, skipping optimistic indexing",
                );
                self.metrics.optimistic_tx_with_missing_dependencies_count.inc();
                return Ok(());
            }
        };
        let full_tx_data = CheckpointTransaction {
            transaction,
            effects,
            events: Some(events),
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
        self.read
            .deep_check_all_transactions_are_indexed_in_blocking_task(effects.dependencies())
            .await
    }

    pub async fn execute_and_index_transaction(
        &self,
        tx_bytes: Base64,
        signatures: Vec<Base64>,
        options: Option<IotaTransactionBlockResponseOptions>,
    ) -> Result<IotaTransactionBlockResponse, IndexerError> {
        let _total_execution_time = self
            .metrics
            .optimistic_tx_total_execution_and_indexing_time
            .start_timer();
        self.metrics.optimistic_tx_count.inc();
        let tx_data: TransactionData = bcs::from_bytes(&tx_bytes.to_vec()?)?;
        let sigs = signatures
            .into_iter()
            .map(|sig| GenericSignature::from_bytes(&sig.to_vec()?))
            .collect::<Result<Vec<_>, FastCryptoError>>()?;

        let transaction = Transaction::from_generic_sig_data(tx_data, sigs);

        let node_timer = self
            .metrics
            .optimistic_tx_node_response_wait_time
            .start_timer();

        let readmask = FieldMask::from_paths(EXECUTE_TRANSACTION_READ_MASK)
            .display()
            .to_string();

        let response = self
            .rpc_client
            .execute_transaction(
                transaction.clone().try_into()?,
                Some(readmask.as_str()),
                None,
            )
            .await;

        let executed_transaction = match response {
            Ok(response) => {
                node_timer.stop_and_record();
                response.into_inner()
            }
            Err(e) => {
                node_timer.stop_and_discard();
                self.metrics.optimistic_tx_failed_node_requests_count.inc();
                return Err(IndexerError::from(e));
            }
        };

        let tx_digest = *TransactionEffects::try_from(executed_transaction.effects()?.effects()?)?
            .transaction_digest();

        self.maybe_index_executed_transaction(transaction, executed_transaction)
            .await?;

        let db_read_timer = self
            .metrics
            .optimistic_tx_db_wait_and_read_time
            .start_timer();
        let tx_block_response = self
            .wait_for_local_indexing(tx_digest, options.clone())
            .await?;
        db_read_timer.stop_and_record();

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
                .read
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
        let db_write_timer = self.metrics.optimistic_tx_db_write_time.start_timer();
        match tokio::task::spawn_blocking({
            let this: OptimisticTransactionExecutor = self.clone();
            let full_tx_data = full_tx_data.clone();
            move || this.index_transaction(&full_tx_data)
        })
        .await
        .map_err(|e| {
            tracing::error!("failed to join optimistic index_transaction: {e}");
            IndexerError::from(e)
        })? {
            Ok(_) => {
                db_write_timer.stop_and_record();
                self.metrics.optimistic_tx_successful_db_writes_count.inc();
                Ok(())
            }
            // The unique violation error means that checkpoint indexing was faster than the
            // optimistic indexing. Let's just return and let checkpoint indexing handle
            // the transaction.
            Err(IndexerError::PostgresUniqueTxGlobalOrderViolation(_)) => {
                db_write_timer.stop_and_discard();
                self.metrics
                    .optimistic_tx_unique_global_order_violations_count
                    .inc();
                Ok(())
            }
            Err(e) => {
                db_write_timer.stop_and_discard();
                self.metrics.optimistic_tx_failed_db_writes_count.inc();
                Err(IndexerError::PostgresWrite(format!(
                    "Failed to persist optimistic tx: {e:?}",
                )))
            }
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
                        .expect("optimistic sequence number is always set for data read from DB")
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
            .persist_objects_in_existing_transaction(conn, vec![object_changes])?;
        self.store.persist_displays_in_existing_transaction(
            conn,
            indexed_displays.values().collect::<Vec<_>>(),
        )?;

        self.store
            .persist_optimistic_transaction_in_existing_transaction(conn, optimistic_tx)
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
                let df_kind = extract_df_kind(o);
                IndexedObject::from_object(
                    0, // checkpoint sequence number, ignored in further processing
                    o.clone(),
                    df_kind,
                )
            })
            .collect::<Vec<_>>();

        Ok(TransactionObjectChangesToCommit {
            changed_objects,
            deleted_objects: indexed_eventually_removed_objects,
        })
    }

    fn get_indexed_transactions_events_and_displays(
        &self,
    ) -> IndexerResult<IndexedTransactionComponents> {
        let handle = tokio::runtime::Handle::current();
        handle.block_on(async move {
            PrimaryWorker::index_transaction_components(
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
