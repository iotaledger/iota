// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{collections::BTreeMap, time::Duration};

use async_trait::async_trait;
use diesel::{RunQueryDsl, pg::sql_types, sql_query};
use downcast::Any;
use fastcrypto::{encoding::Base64, error::FastCryptoError, traits::ToFromBytes};
use iota_json_rpc::IotaRpcModule;
use iota_json_rpc_api::{WriteApiClient, WriteApiServer, error_object_from_rpc};
use iota_json_rpc_types::{
    DevInspectArgs, DevInspectResults, DryRunTransactionBlockResponse,
    IotaTransactionBlockResponse, IotaTransactionBlockResponseOptions,
};
use iota_open_rpc::Module;
use iota_rest_api::{ExecuteTransactionQueryParameters, client::TransactionExecutionResponse};
use iota_types::{
    base_types::IotaAddress,
    digests::TransactionDigest,
    effects::TransactionEffectsAPI,
    full_checkpoint_content::CheckpointTransaction,
    iota_serde::BigInt,
    quorum_driver_types::ExecuteTransactionRequestType,
    signature::GenericSignature,
    transaction::{Transaction, TransactionData},
};
use jsonrpsee::{RpcModule, core::RpcResult, http_client::HttpClient};

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
        events::StoredEvent,
        transactions::{StoredTransaction, TxGlobalOrder},
        tx_indices::OptimisticTxIndices,
    },
    store::{IndexerStore, PgIndexerStore},
    transactional_blocking_with_retry,
    types::{
        EventIndex, IndexedDeletedObject, IndexedEvent, IndexedObject, IndexedTransaction,
        IndexerResult, IotaTransactionBlockResponseWithOptions, TxIndex,
    },
};

pub(crate) struct WriteApi {
    fullnode: HttpClient,
    fullnode_rest_client: iota_rest_api::Client,
    inner: IndexerReader,
    store: PgIndexerStore,
    metrics: IndexerMetrics,
}

type TransactionDataToCommit = (
    IndexedTransaction,
    TxIndex,
    Vec<IndexedEvent>,
    Vec<EventIndex>,
    BTreeMap<String, StoredDisplay>,
    TransactionObjectChangesToCommit,
);

impl WriteApi {
    pub fn new(
        fullnode_client: HttpClient,
        fullnode_rest_client: iota_rest_api::Client,
        inner: IndexerReader,
        store: PgIndexerStore,
        metrics: IndexerMetrics,
    ) -> Self {
        Self {
            fullnode: fullnode_client,
            fullnode_rest_client,
            inner,
            store,
            metrics,
        }
    }

    async fn execute_and_index_tx_effects(
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

        // TODO: shouldn't return type below be from rust-sdk types?
        // Is this type correct?
        let response = self
            .fullnode_rest_client
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

        if let (Some(input_objects), Some(output_objects)) = (input_objects, output_objects) {
            let full_tx_data = CheckpointTransaction {
                transaction,
                effects,
                events,
                input_objects,
                output_objects,
            };
            self.index_transaction(&full_tx_data).await?;
        } else {
            tracing::warn!(
                "Cannot optimistically index because of missing in/out objs for tx: {tx_digest}"
            );
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
                .inner
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

        let tx_data_to_commit = self
            .full_optimistic_tx_data_to_indexed_data(full_tx_data, &assigned_global_order)
            .await?;

        self.persist_optimistic_tx(tx_data_to_commit).await
    }

    async fn assign_optimistic_tx_global_order(
        &self,
        tx_digest: &TransactionDigest,
    ) -> Result<Option<TxGlobalOrder>, IndexerError> {
        let tx_digest_bytes = tx_digest.inner().to_vec();

        let pool = self.inner.get_pool();

        let mut results: Vec<TxGlobalOrder> = transactional_blocking_with_retry!(
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
                .load::<TxGlobalOrder>(conn)
            },
            Duration::from_secs(30)
        )?;

        Ok(results.pop())
    }

    async fn full_optimistic_tx_data_to_indexed_data(
        &self,
        full_tx_data: &CheckpointTransaction,
        assigned_global_order: &TxGlobalOrder,
    ) -> IndexerResult<TransactionDataToCommit> {
        let object_changes = {
            let indexed_eventually_removed_objects = full_tx_data
                .removed_object_refs_post_version()
                .map(|obj_ref| IndexedDeletedObject {
                    object_id: obj_ref.0,
                    object_version: obj_ref.1.into(),
                    checkpoint_sequence_number: 0,
                })
                .collect::<Vec<_>>();

            let changed_objects = full_tx_data
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

            TransactionObjectChangesToCommit {
                changed_objects,
                deleted_objects: indexed_eventually_removed_objects,
            }
        };

        let (indexed_tx, tx_indices, indexed_events, events_indices, indexed_displays) =
            CheckpointHandler::index_transaction(
                full_tx_data,
                assigned_global_order
                    .optimistic_sequence_number
                    .expect("Optimistic sequence number is always set for data read from DB")
                    .try_into()
                    .unwrap(),
                0, // checkpoint sequence number - unknown
                0, // checkpoint timestamp - unknown
                &self.metrics,
            )
            .await?;

        Ok((
            indexed_tx,
            tx_indices,
            indexed_events,
            events_indices,
            indexed_displays,
            object_changes,
        ))
    }

    async fn persist_optimistic_tx(
        &self,
        tx_data_to_commit: TransactionDataToCommit,
    ) -> Result<(), IndexerError> {
        let (
            indexed_tx,
            tx_indices,
            indexed_events,
            events_indices,
            indexed_displays,
            object_changes,
        ) = tx_data_to_commit;

        self.store.persist_objects(vec![object_changes]).await?;
        self.store.persist_displays(indexed_displays).await?;

        self.store
            .persist_optimistic_transaction(StoredTransaction::from(&indexed_tx).into())
            .await?;
        self.store
            .persist_optimistic_events(
                indexed_events
                    .into_iter()
                    .map(StoredEvent::from)
                    .map(Into::into)
                    .collect(),
            )
            .await?;
        self.store
            .persist_optimistic_event_indices(
                Self::optimistic_event_indices_from_indexed_event_indices(events_indices),
            )
            .await?;
        self.store
            .persist_optimistic_tx_indices(Self::optimistic_tx_indices_from_indexed_tx_indices(
                tx_indices,
            ))
            .await?;

        Ok(())
    }

    fn optimistic_event_indices_from_indexed_event_indices(
        event_indices: Vec<EventIndex>,
    ) -> OptimisticEventIndices {
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

    fn optimistic_tx_indices_from_indexed_tx_indices(tx_index: TxIndex) -> OptimisticTxIndices {
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

#[async_trait]
impl WriteApiServer for WriteApi {
    async fn execute_transaction_block(
        &self,
        tx_bytes: Base64,
        signatures: Vec<Base64>,
        options: Option<IotaTransactionBlockResponseOptions>,
        request_type: Option<ExecuteTransactionRequestType>,
    ) -> RpcResult<IotaTransactionBlockResponse> {
        let iota_transaction_response = match request_type {
            None | Some(ExecuteTransactionRequestType::WaitForEffectsCert) => {
                let mut node_response = self
                    .fullnode
                    .execute_transaction_block(tx_bytes, signatures, options.clone(), request_type)
                    .await
                    .map_err(error_object_from_rpc)?;
                // it's not locally executed in indexer, no matter what is the status in the
                // node
                node_response.confirmed_local_execution = Some(false);
                node_response
            }
            Some(ExecuteTransactionRequestType::WaitForLocalExecution) => {
                self.execute_and_index_tx_effects(tx_bytes, signatures, options.clone())
                    .await?
            }
        };
        Ok(IotaTransactionBlockResponseWithOptions {
            response: iota_transaction_response,
            options: options.unwrap_or_default(),
        }
        .into())
    }

    async fn dev_inspect_transaction_block(
        &self,
        sender_address: IotaAddress,
        tx_bytes: Base64,
        gas_price: Option<BigInt<u64>>,
        epoch: Option<BigInt<u64>>,
        additional_args: Option<DevInspectArgs>,
    ) -> RpcResult<DevInspectResults> {
        self.fullnode
            .dev_inspect_transaction_block(
                sender_address,
                tx_bytes,
                gas_price,
                epoch,
                additional_args,
            )
            .await
            .map_err(error_object_from_rpc)
    }

    async fn dry_run_transaction_block(
        &self,
        tx_bytes: Base64,
    ) -> RpcResult<DryRunTransactionBlockResponse> {
        self.fullnode
            .dry_run_transaction_block(tx_bytes)
            .await
            .map_err(error_object_from_rpc)
    }
}

impl IotaRpcModule for WriteApi {
    fn rpc(self) -> RpcModule<Self> {
        self.into_rpc()
    }

    fn rpc_doc_module() -> Module {
        iota_json_rpc_api::WriteApiOpenRpc::module_doc()
    }
}
