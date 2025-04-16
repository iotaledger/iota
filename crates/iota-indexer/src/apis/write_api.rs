// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::time::Duration;

use async_trait::async_trait;
use diesel::{ExpressionMethods, QueryDsl, RunQueryDsl, SelectableHelper};
use fastcrypto::{encoding::Base64, traits::ToFromBytes};
use iota_json_rpc::IotaRpcModule;
use iota_json_rpc_api::{WriteApiClient, WriteApiServer, error_object_from_rpc};
use iota_json_rpc_types::{
    DevInspectArgs, DevInspectResults, DryRunTransactionBlockResponse, EpochInfo,
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
        event_indices::OptimisticEventIndices,
        events::StoredEvent,
        transactions::{StoredTransaction, TxInsertionOrder},
        tx_indices::OptimisticTxIndices,
    },
    run_query_async,
    schema::tx_insertion_order,
    spawn_read_only_blocking,
    store::{IndexerStore, PgIndexerStore},
    types::{
        EventIndex, IndexedDeletedObject, IndexedObject, IotaTransactionBlockResponseWithOptions,
        TxIndex,
    },
};

pub(crate) struct WriteApi {
    fullnode: HttpClient,
    fullnode_rest_client: iota_rest_api::Client,
    inner: IndexerReader,
    store: PgIndexerStore,
    metrics: IndexerMetrics,
}

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

    async fn execute_and_optimistically_index_tx_effects(
        &self,
        tx_bytes: Base64,
        signatures: Vec<Base64>,
        options: Option<IotaTransactionBlockResponseOptions>,
    ) -> RpcResult<IotaTransactionBlockResponse> {
        let tx_data: TransactionData =
            bcs::from_bytes(&tx_bytes.to_vec().map_err(IndexerError::FastCrypto)?)
                .map_err(IndexerError::Bcs)?;
        let mut sigs = Vec::new();
        for sig in signatures {
            sigs.push(
                GenericSignature::from_bytes(&sig.to_vec().map_err(IndexerError::FastCrypto)?)
                    .map_err(IndexerError::FastCrypto)?,
            );
        }
        let transaction = Transaction::from_generic_sig_data(tx_data, sigs);

        // TODO: shouldn't return type below be from rust-sdk types? Is this type
        // correct?
        let result = self
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

        let latest_epoch = EpochInfo::try_from(
            self.inner
                .spawn_blocking(|this| this.get_latest_epoch_info_from_db())
                .await?,
        )?;

        let TransactionExecutionResponse {
            effects,
            finality: _,
            events,
            balance_changes: _,
            input_objects,
            output_objects,
        } = result;
        let tx_digest = *effects.transaction_digest();
        println!("INDEXER: got node response for: {}", tx_digest);

        if effects.executed_epoch() < latest_epoch.epoch {
            // Transaction is from previous epoch, we cannot optimistically
            // index it safely since we may have already pruned optimistic
            // tables from that epoch. It's very likely that it's already
            // indexed anyway, let's just wait for it.
            println!(
                "Epoch change from {} to {}",
                effects.executed_epoch(),
                latest_epoch.epoch
            );
            // TODO: check if this case need to be supported
        } else if !self
            .check_if_tx_dependencies_are_satisfied(effects.dependencies())
            .await?
        {
            println!("Unsatisfied tx dependencies, not indexing. Waiting for checkpoint execution");
        } else if let (Some(input_objects), Some(output_objects)) = (input_objects, output_objects)
        {
            // We have all needed data, let's optimistically index the tx.
            let full_tx_data = CheckpointTransaction {
                transaction,
                effects,
                events,
                input_objects,
                output_objects,
            };
            println!("INDEXER: optimistically indexing: {}", tx_digest);
            self.optimistically_index_transaction(&full_tx_data).await?;
        } else {
            // TODO: input/output objects are missing, let's create some metric
            // for this
            println!("Missing in/out objs");
        }

        println!("INDEXER: waiting for indexing of: {}", tx_digest);
        let tx_block_response = self
            .wait_for_and_return_tx_block_response(tx_digest, options.clone())
            .await?;

        println!("INDEXER: returning response for: {}", tx_digest);
        Ok(IotaTransactionBlockResponseWithOptions {
            response: tx_block_response,
            options: options.unwrap_or_default(),
        }
        .into())
    }

    async fn wait_for_and_return_tx_block_response(
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

    async fn optimistically_index_transaction(
        &self,
        full_tx_data: &CheckpointTransaction,
    ) -> Result<(), IndexerError> {
        let assigned_insertion_order: TxInsertionOrder = {
            let tx_digest_bytes = full_tx_data.transaction.digest().inner().to_vec();

            self.store
                .persist_tx_insertion_order(vec![TxInsertionOrder {
                    insertion_order: -1, // ignored value
                    tx_digest: tx_digest_bytes.clone(),
                }])
                .await?;

            let pool = self.inner.get_pool();
            run_query_async!(&pool, |conn| {
                tx_insertion_order::table
                    .select(TxInsertionOrder::as_select())
                    .filter(tx_insertion_order::tx_digest.eq(tx_digest_bytes))
                    .first::<TxInsertionOrder>(conn)
            })?
        };

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
                assigned_insertion_order.insertion_order.try_into().unwrap(),
                0,
                0,
                &self.metrics,
            )
            .await?;

        let conn_with_locked_tx = self
            .store
            .hold_execution_lock_for_transactions(&[indexed_tx.tx_digest.inner().to_vec()])
            .await?;
        let tx_status = self
            .store
            .get_execution_status_of_transactions(&[indexed_tx.tx_digest.inner().to_vec()])
            .await?
            .pop()
            .expect("Execution status should always be present since it was just added");

        // Index only if such transaction was not yet indexed
        // TODO: check tx deps
        // TODO: what if tx was executed before status table was added?
        if !tx_status.indexing_completed {
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

            self.store
                .mark_transactions_as_indexed(
                    &[indexed_tx.tx_digest.inner().to_vec()],
                    conn_with_locked_tx,
                )
                .await?;
        }

        Ok(())
    }

    async fn check_if_tx_dependencies_are_satisfied(
        &self,
        dependencies: &[TransactionDigest],
    ) -> Result<bool, IndexerError> {
        let digests = dependencies
            .iter()
            .map(|digest| digest.into_inner().to_vec())
            .collect::<Vec<_>>();

        let dependencies_indexing_statuses = self
            .store
            .get_execution_status_of_transactions(&digests)
            .await?;

        // Some transactions will not have any status and will be missing from the
        // response. This is fine since those will be old transactions from
        // before this feature was introduced, or from previous epoch, in both cases the
        // tx will already be indexed, so we don't have to check it.
        Ok(dependencies_indexing_statuses
            .iter()
            .all(|status| status.indexing_completed))
    }

    fn optimistic_event_indices_from_indexed_event_indices(
        event_indices: Vec<EventIndex>,
    ) -> OptimisticEventIndices {
        let (
            event_emit_packages,
            event_emit_modules,
            event_senders,
            event_struct_packages,
            event_struct_modules,
            event_struct_names,
            event_struct_instantiations,
        ) = event_indices.into_iter().map(|i| i.split()).fold(
            (
                Vec::new(),
                Vec::new(),
                Vec::new(),
                Vec::new(),
                Vec::new(),
                Vec::new(),
                Vec::new(),
            ),
            |(
                mut event_emit_packages,
                mut event_emit_modules,
                mut event_senders,
                mut event_struct_packages,
                mut event_struct_modules,
                mut event_struct_names,
                mut event_struct_instantiations,
            ),
             index| {
                event_emit_packages.push(index.0);
                event_emit_modules.push(index.1);
                event_senders.push(index.2);
                event_struct_packages.push(index.3);
                event_struct_modules.push(index.4);
                event_struct_names.push(index.5);
                event_struct_instantiations.push(index.6);
                (
                    event_emit_packages,
                    event_emit_modules,
                    event_senders,
                    event_struct_packages,
                    event_struct_modules,
                    event_struct_names,
                    event_struct_instantiations,
                )
            },
        );

        OptimisticEventIndices {
            optimistic_event_emit_packages: event_emit_packages
                .into_iter()
                .map(Into::into)
                .collect(),
            optimistic_event_emit_modules: event_emit_modules.into_iter().map(Into::into).collect(),
            optimistic_event_senders: event_senders.into_iter().map(Into::into).collect(),
            optimistic_event_struct_packages: event_struct_packages
                .into_iter()
                .map(Into::into)
                .collect(),
            optimistic_event_struct_modules: event_struct_modules
                .into_iter()
                .map(Into::into)
                .collect(),
            optimistic_event_struct_names: event_struct_names.into_iter().map(Into::into).collect(),
            optimistic_event_struct_instantiations: event_struct_instantiations
                .into_iter()
                .map(Into::into)
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
        _request_type: Option<ExecuteTransactionRequestType>,
    ) -> RpcResult<IotaTransactionBlockResponse> {
        let request_type = Some(ExecuteTransactionRequestType::WaitForLocalExecution); // force local execution, for testing purposes
        let iota_transaction_response = match request_type {
            None | Some(ExecuteTransactionRequestType::WaitForEffectsCert) => self
                .fullnode
                .execute_transaction_block(tx_bytes, signatures, options.clone(), request_type)
                .await
                .map_err(error_object_from_rpc)?, // should this be the default option? What if
            // fullnode returns that the tx is locally
            // executed?
            Some(ExecuteTransactionRequestType::WaitForLocalExecution) => {
                self.execute_and_optimistically_index_tx_effects(
                    tx_bytes,
                    signatures,
                    options.clone(),
                )
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
