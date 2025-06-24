// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
use std::{collections::BTreeMap, time::Duration};

use diesel::{OptionalExtension, RunQueryDsl, sql_query, sql_types};
use downcast::Any;
use fastcrypto::{encoding::Base64, error::FastCryptoError, traits::ToFromBytes};
use iota_json_rpc_types::{IotaTransactionBlockResponse, IotaTransactionBlockResponseOptions};
use iota_rest_api::{ExecuteTransactionQueryParameters, client::TransactionExecutionResponse};
use iota_types::{
    base_types::TransactionDigest,
    effects::TransactionEffectsAPI,
    full_checkpoint_content::CheckpointTransaction,
    signature::GenericSignature,
    transaction::{Transaction, TransactionData},
};

use crate::{
    errors::IndexerError,
    handlers::{
        TransactionObjectChangesToCommit,
        checkpoint_handler::{CheckpointHandler, try_extract_df_kind},
    },
    indexer_reader::IndexerReader,
    metrics::IndexerMetrics,
    models::{
        display::StoredDisplay,
        event_indices::OptimisticEventIndices,
        events::{OptimisticEvent, StoredEvent},
        transactions::{OptimisticTransaction, StoredTransaction, TxGlobalOrder},
        tx_indices::OptimisticTxIndices,
    },
    store::{IndexerStore, PgIndexerStore},
    transactional_blocking_with_retry,
    types::{
        EventIndex, IndexedDeletedObject, IndexedEvent, IndexedObject, IndexedTransaction,
        IndexerResult, IotaTransactionBlockResponseWithOptions, TxIndex,
    },
};

type TransactionDataToCommit = (
    OptimisticTransaction,
    OptimisticTxIndices,
    Vec<OptimisticEvent>,
    OptimisticEventIndices,
    BTreeMap<String, StoredDisplay>,
    TransactionObjectChangesToCommit,
);

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

        let TransactionExecutionResponse {
            effects,
            events,
            input_objects,
            output_objects,
            ..
        } = response;
        let tx_digest = *effects.transaction_digest();

        match (input_objects, output_objects) {
            (Some(input_objects), Some(output_objects))
                if !input_objects.is_empty() && !output_objects.is_empty() =>
            {
                let full_tx_data = CheckpointTransaction {
                    transaction,
                    effects,
                    events,
                    input_objects,
                    output_objects,
                };
                self.index_transaction(&full_tx_data).await?;
            }
            _ => {
                tracing::warn!(
                    "Cannot optimistically index because of missing in/out objs for tx: {tx_digest}"
                );
            }
        }

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

    async fn index_transaction(
        &self,
        full_tx_data: &CheckpointTransaction,
    ) -> Result<(), IndexerError> {
        let assigned_global_order = self
            .assign_optimistic_tx_global_order(full_tx_data.transaction.digest())
            .await?;

        let Some(assigned_global_order) = assigned_global_order else {
            // Global order was assigned earlier by other indexing process, we avoid double
            // or concurrent indexing and return
            return Ok(());
        };

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

        let tx_data_to_commit = extractor.to_transaction_data_to_commit().await?;

        self.persist_optimistic_tx(tx_data_to_commit).await
    }

    async fn assign_optimistic_tx_global_order(
        &self,
        tx_digest: &TransactionDigest,
    ) -> Result<Option<TxGlobalOrder>, IndexerError> {
        let tx_digest_bytes = tx_digest.inner().to_vec();

        let pool = self.indexer_reader.get_pool();

        transactional_blocking_with_retry!(
            &pool,
            |conn| {
                sql_query(
                    r#"
                        INSERT INTO tx_global_order (tx_digest, global_sequence_number)
                        SELECT $1, MAX(tx_sequence_number) FROM tx_digests
                        ON CONFLICT (tx_digest) DO NOTHING
                        RETURNING *;
                    "#,
                )
                .bind::<sql_types::Bytea, _>(&tx_digest_bytes)
                .get_result::<TxGlobalOrder>(conn)
                .optional()
            },
            Duration::from_secs(30)
        )
    }

    async fn persist_optimistic_tx(
        &self,
        tx_data_to_commit: TransactionDataToCommit,
    ) -> Result<(), IndexerError> {
        let (
            optimistic_tx,
            optimistic_tx_indices,
            optimistic_events,
            optimistic_event_indices,
            indexed_displays,
            object_changes,
        ) = tx_data_to_commit;

        self.store.persist_objects(vec![object_changes]).await?;
        self.store.persist_displays(indexed_displays).await?;

        self.store
            .persist_optimistic_transaction(optimistic_tx)
            .await?;
        self.store
            .persist_optimistic_events(optimistic_events)
            .await?;
        self.store
            .persist_optimistic_event_indices(optimistic_event_indices)
            .await?;
        self.store
            .persist_optimistic_tx_indices(optimistic_tx_indices)
            .await?;

        Ok(())
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

    async fn get_indexed_transactions_events_and_displays(
        &self,
    ) -> IndexerResult<(
        IndexedTransaction,
        TxIndex,
        Vec<IndexedEvent>,
        Vec<EventIndex>,
        BTreeMap<String, StoredDisplay>,
    )> {
        CheckpointHandler::index_transaction(
            self.full_tx_data,
            self.optimistic_sequence_number,
            0, // checkpoint sequence number - unknown
            0, // checkpoint timestamp - unknown
            self.metrics,
        )
        .await
    }

    async fn to_transaction_data_to_commit(&self) -> IndexerResult<TransactionDataToCommit> {
        let object_changes = self.get_object_changes()?;
        let (indexed_tx, tx_indices, indexed_events, events_indices, indexed_displays) =
            self.get_indexed_transactions_events_and_displays().await?;

        let optimistic_tx = StoredTransaction::from(&indexed_tx).into();
        let optimistic_tx_indices = Self::optimistic_tx_indices(tx_indices);
        let optimistic_events = indexed_events
            .into_iter()
            .map(StoredEvent::from)
            .map(Into::into)
            .collect();
        let optimistic_event_indices = Self::optimistic_event_indices(events_indices);

        Ok((
            optimistic_tx,
            optimistic_tx_indices,
            optimistic_events,
            optimistic_event_indices,
            indexed_displays,
            object_changes,
        ))
    }

    fn optimistic_event_indices(event_indices: Vec<EventIndex>) -> OptimisticEventIndices {
        let splits: Vec<_> = event_indices.into_iter().map(|i| i.split()).collect();

        OptimisticEventIndices {
            optimistic_event_emit_packages: splits.iter().map(|t| t.0.clone().into()).collect(),
            optimistic_event_emit_modules: splits.iter().map(|t| t.1.clone().into()).collect(),
            optimistic_event_senders: splits.iter().map(|t| t.2.clone().into()).collect(),
            optimistic_event_struct_packages: splits.iter().map(|t| t.3.clone().into()).collect(),
            optimistic_event_struct_modules: splits.iter().map(|t| t.4.clone().into()).collect(),
            optimistic_event_struct_names: splits.iter().map(|t| t.5.clone().into()).collect(),
            optimistic_event_struct_instantiations: splits
                .iter()
                .map(|t| t.6.clone().into())
                .collect(),
        }
    }

    fn optimistic_tx_indices(tx_index: TxIndex) -> OptimisticTxIndices {
        let (senders, recipients, input_objects, changed_objects, pkgs, mods, funs, _, kinds) =
            tx_index.split();

        OptimisticTxIndices {
            optimistic_tx_senders: senders.into_iter().map(Into::into).collect(),
            optimistic_tx_recipients: recipients.into_iter().map(Into::into).collect(),
            optimistic_tx_input_objects: input_objects.into_iter().map(Into::into).collect(),
            optimistic_tx_changed_objects: changed_objects.into_iter().map(Into::into).collect(),
            optimistic_tx_pkgs: pkgs.into_iter().map(Into::into).collect(),
            optimistic_tx_mods: mods.into_iter().map(Into::into).collect(),
            optimistic_tx_funs: funs.into_iter().map(Into::into).collect(),
            optimistic_tx_kinds: kinds.into_iter().map(Into::into).collect(),
        }
    }
}
