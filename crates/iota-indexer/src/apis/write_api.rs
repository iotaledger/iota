// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{sync::Arc, time::Duration};

use async_trait::async_trait;
use fastcrypto::encoding::Base64;
use futures::{FutureExt, TryFutureExt};
use iota_grpc_client::{
    Client as GrpcClient, ReadMask,
    read_mask_fields::{EpochField, SimulateExecutedTransactionField, SimulateField},
};
use iota_json::IotaJsonValue;
use iota_json_rpc::{
    IotaRpcModule, ObjectProvider, get_balance_changes_from_effect, get_object_changes,
};
use iota_json_rpc_api::WriteApiServer;
use iota_json_rpc_types::{
    DevInspectArgs, DevInspectResults, DryRunTransactionBlockResponse,
    ExecuteTransactionRequestType, IotaMoveViewCallResults, IotaTransactionBlock,
    IotaTransactionBlockEffects, IotaTransactionBlockResponse, IotaTransactionBlockResponseOptions,
    IotaTypeTag, MoveFunctionName,
};
use iota_open_rpc::Module;
use iota_package_resolver::{PackageStore, Resolver};
use iota_protocol_config::Chain;
use iota_transaction_builder::TransactionBuilder;
use iota_types::{
    base_types::{IotaAddress, ObjectID, SequenceNumber},
    digests::TransactionDigest,
    effects::{
        TransactionEffects, TransactionEffectsAPI, TransactionEffectsExt, TransactionEvents,
    },
    error::ExecutionError,
    iota_serde::BigInt,
    object::{Object, PastObjectRead},
    signature::GenericSignature,
    transaction::{
        GasData, SenderSignedData, TransactionData, TransactionDataAPI, TransactionDataV1,
        TransactionExpiration, TransactionKind,
    },
};
use jsonrpsee::{RpcModule, core::RpcResult};

use crate::{
    apis::error::Error as ApiError,
    errors::{IndexerError, IndexerResult},
    ingestion::primary::prepare::InMemObjectCache,
    models::transactions::{StoredTransaction, tx_events_to_iota_tx_events},
    optimistic_indexing::{IngestionPath, OptimisticTransactionExecutor},
    read::IndexerReader,
    store::package_resolver::IndexerStorePackageResolver,
    types::{IndexedObjectChange, grpc_conversion},
};

// As an optimization, we're trying to request only the fields we actually need.
const DRY_RUN_TRANSACTION_READ_MASK: &[&str] = &[
    SimulateExecutedTransactionField::SIGNATURES_BCS,
    SimulateExecutedTransactionField::EFFECTS_BCS,
    SimulateExecutedTransactionField::EVENTS_EVENTS_BCS,
    SimulateExecutedTransactionField::INPUT_OBJECTS_BCS,
    SimulateExecutedTransactionField::OUTPUT_OBJECTS_BCS,
    SimulateField::SUGGESTED_GAS_PRICE,
    SimulateField::EXECUTION_RESULT_EXECUTION_ERROR_SOURCE,
];
const DEV_INSPECT_TRANSACTION_READ_MASK: &[&str] = &[
    SimulateExecutedTransactionField::EFFECTS_BCS,
    SimulateExecutedTransactionField::EVENTS_EVENTS_BCS,
    SimulateField::EXECUTION_RESULT_EXECUTION_ERROR_BCS_KIND,
    SimulateField::EXECUTION_RESULT_EXECUTION_ERROR_SOURCE,
    SimulateField::EXECUTION_RESULT_EXECUTION_ERROR_COMMAND_INDEX,
    SimulateField::EXECUTION_RESULT_COMMAND_RESULTS_MUTATED_BY_REF,
    SimulateField::EXECUTION_RESULT_COMMAND_RESULTS_RETURN_VALUES,
];

#[derive(Clone)]
pub struct WriteApi {
    fullnode_grpc_client: GrpcClient,
    transaction_builder: TransactionBuilder,
    package_resolver: Arc<Resolver<IndexerStorePackageResolver>>,
    reader: Arc<IndexerReader>,
}

#[derive(Clone)]
pub struct OptimisticWriteApi {
    write_api: WriteApi,
    optimistic_tx_executor: OptimisticTransactionExecutor,
}

impl WriteApi {
    pub fn new(fullnode_grpc_client: GrpcClient, reader: IndexerReader) -> Self {
        let package_resolver = IndexerStorePackageResolver::new(reader.get_pool());
        let data_reader = Arc::new(reader);
        Self {
            reader: data_reader.clone(),
            fullnode_grpc_client,
            transaction_builder: TransactionBuilder::new(data_reader),
            package_resolver: Arc::new(Resolver::new(package_resolver)),
        }
    }

    async fn dry_run_transaction_block_impl(
        &self,
        tx_bytes: Base64,
        package_resolver: &Arc<Resolver<impl PackageStore>>,
    ) -> IndexerResult<DryRunTransactionBlockResponse> {
        let transaction_data = bcs::from_bytes::<TransactionData>(&tx_bytes.to_vec()?)?;
        let tx_digest = transaction_data.digest();

        let simulate_tx_response = self
            .fullnode_grpc_client
            .simulate_transaction(
                transaction_data.clone(),
                false,
                Some(ReadMask::from(DRY_RUN_TRANSACTION_READ_MASK)),
            )
            .await?
            .into_inner();

        let executed_transaction = simulate_tx_response.executed_transaction()?;
        let execution_error_source = simulate_tx_response
            .execution_error()
            .and_then(|e| e.source.clone());
        let suggested_gas_price = simulate_tx_response.suggested_gas_price;

        let input_objects = grpc_conversion::objects(executed_transaction.input_objects()?)?;
        let output_objects = grpc_conversion::objects(executed_transaction.output_objects()?)?;

        let objects = input_objects
            .iter()
            .chain(output_objects.iter())
            .collect::<Vec<_>>();

        let tx_effects: TransactionEffects = executed_transaction.effects()?.effects()?;

        let tx_signatures = executed_transaction
            .signatures()?
            .signatures
            .iter()
            .map(|s| -> IndexerResult<_> { Ok(GenericSignature::try_from(s.signature()?)?) })
            .collect::<IndexerResult<Vec<GenericSignature>>>()?;

        let sender_signed_data = SenderSignedData::new(transaction_data.clone(), tx_signatures);

        let tx_events = TransactionEvents::from(executed_transaction.events()?.events()?);

        let in_mem_tx_changes = TxObjectResolver::new(&objects, self.reader.clone());

        // as a minor optimization we will run concurrently the following four futures
        let fut1 = in_mem_tx_changes
            .get_changes(&transaction_data, &tx_effects, &tx_digest)
            .map_ok(|(balance_changes, object_changes)| {
                (
                    balance_changes,
                    object_changes
                        .into_iter()
                        .map(iota_json_rpc_types::ObjectChange::from)
                        .collect::<Vec<_>>(),
                )
            });

        let fut2 = IotaTransactionBlock::try_from_with_package_resolver(
            sender_signed_data,
            package_resolver,
            tx_digest,
        )
        .map_err(Into::into);

        // timestamp is None because it represent a checkpoint one, on a dry run
        // operation we don't have this information.
        let fut3 = tx_events_to_iota_tx_events(tx_events, package_resolver, tx_digest, None);

        let fut4 = IotaTransactionBlockEffects::from_native_with_clever_error(
            tx_effects.clone(),
            package_resolver,
        )
        .map(Ok);

        let ((balance_changes, object_changes), transaction_block, events, effects) =
            futures::future::try_join4(fut1, fut2, fut3, fut4).await?;

        Ok(DryRunTransactionBlockResponse {
            effects,
            events,
            object_changes,
            balance_changes,
            input: transaction_block.data,
            suggested_gas_price,
            execution_error_source,
        })
    }

    async fn dev_inspect_transaction_block_impl(
        &self,
        sender_address: IotaAddress,
        tx_bytes: Base64,
        gas_price: Option<BigInt<u64>>,
        additional_args: Option<DevInspectArgs>,
        package_resolver: &Arc<Resolver<impl PackageStore>>,
    ) -> IndexerResult<DevInspectResults> {
        let DevInspectArgs {
            gas_sponsor,
            gas_budget,
            gas_objects,
            show_raw_txn_data_and_effects,
            skip_checks,
        } = additional_args.unwrap_or_default();

        let show_raw_txn_data_and_effects = show_raw_txn_data_and_effects.unwrap_or(false);
        let skip_checks = skip_checks.unwrap_or(true);

        let (price, budget) = match (gas_price, gas_budget) {
            (Some(price), Some(budget)) => (price.into_inner(), budget),
            (price, budget) => {
                let (ref_price, max_gas) = self.reference_gas_price_and_max_tx_gas().await?;
                (
                    price.map(BigInt::into_inner).unwrap_or(ref_price),
                    budget.unwrap_or(max_gas),
                )
            }
        };

        let owner = gas_sponsor.unwrap_or(sender_address);
        let payment = gas_objects.unwrap_or_default();

        let kind = bcs::from_bytes::<TransactionKind>(&tx_bytes.to_vec()?)?;

        let transaction_data = TransactionData::V1(TransactionDataV1 {
            kind,
            sender: sender_address,
            gas_payment: GasData {
                objects: payment,
                owner,
                price,
                budget,
            },
            expiration: TransactionExpiration::None,
        });

        let raw_txn_data = show_raw_txn_data_and_effects
            .then(|| bcs::to_bytes(&transaction_data))
            .transpose()?
            .unwrap_or_default();

        let simulate_tx_response = self
            .fullnode_grpc_client
            .simulate_transaction(
                transaction_data,
                skip_checks,
                Some(ReadMask::from(DEV_INSPECT_TRANSACTION_READ_MASK)),
            )
            .await?
            .into_inner();

        let executed_transaction = simulate_tx_response.executed_transaction()?;

        let tx_effects: TransactionEffects = executed_transaction.effects()?.effects()?;

        let raw_effects = show_raw_txn_data_and_effects
            .then(|| bcs::to_bytes(&tx_effects))
            .transpose()?
            .unwrap_or_default();

        let tx_events = TransactionEvents::from(executed_transaction.events()?.events()?);

        let tx_digest = *tx_effects.transaction_digest();
        // timestamp is None because it represent a checkpoint one, on a dev inspect
        // operation we don't have this information.
        let events =
            tx_events_to_iota_tx_events(tx_events, package_resolver, tx_digest, None).await?;

        let execution_error = simulate_tx_response
            .execution_error()
            .map(|execution_error| -> IndexerResult<_> {
                let exec_err = execution_error.error_kind()?;
                let source = execution_error
                    .source
                    .clone()
                    .map(|s| -> Box<dyn std::error::Error + Send + Sync> { s.into() });

                let mut error = ExecutionError::new(exec_err, source);
                if let Some(command_index) = execution_error.command_index {
                    error = error.with_command_index(command_index);
                }
                Ok(error.to_string())
            })
            .transpose()?;

        let results = simulate_tx_response
            .command_results()
            .map(|command_results| grpc_conversion::command_results(command_results.clone()))
            .transpose()?;

        Ok(DevInspectResults {
            effects: tx_effects.try_into()?,
            events,
            results,
            error: execution_error,
            raw_txn_data,
            raw_effects,
        })
    }

    /// Gets the reference gas price and max transaction gas from the gRPC API.
    async fn reference_gas_price_and_max_tx_gas(&self) -> IndexerResult<(u64, u64)> {
        let epoch = self
            .fullnode_grpc_client
            .get_epoch(
                None, // we're requesting the information for the current epoch.
                {
                    let max_tx_gas = EpochField::attribute("max_tx_gas");
                    Some(ReadMask::from(&[
                        EpochField::REFERENCE_GAS_PRICE,
                        &max_tx_gas,
                    ]))
                },
            )
            .await?
            .into_inner();

        let max_tx_gas = epoch
            .protocol_config()?
            .attributes()?
            .get("max_tx_gas")
            .ok_or_else(|| {
                IndexerError::Grpc("protocol_config's `max_tx_gas` should be available".into())
            })?
            .parse::<u64>()
            .map_err(|e| IndexerError::Grpc(e.to_string()))?;

        Ok((epoch.reference_gas_price(), max_tx_gas))
    }
}

impl OptimisticWriteApi {
    pub fn new(write_api: WriteApi, optimistic_tx_executor: OptimisticTransactionExecutor) -> Self {
        Self {
            write_api,
            optimistic_tx_executor,
        }
    }

    async fn build_response(
        &self,
        ingestion_path: IngestionPath,
        options: IotaTransactionBlockResponseOptions,
    ) -> Result<IotaTransactionBlockResponse, IndexerError> {
        let package_resolver = self.write_api.package_resolver.clone();
        let stored_transaction = StoredTransaction::from(ingestion_path);
        stored_transaction
            .try_into_iota_transaction_block_response(options, &package_resolver)
            .await
    }

    pub fn executor(&self) -> &OptimisticTransactionExecutor {
        &self.optimistic_tx_executor
    }
}

#[async_trait]
impl WriteApiServer for WriteApi {
    /// This method will always return an error. The user shall use the
    /// [`OptimisticWriteApi`] to execute transactions.
    async fn execute_transaction_block(
        &self,
        _tx_bytes: Base64,
        _signatures: Vec<Base64>,
        _options: Option<IotaTransactionBlockResponseOptions>,
        _request_type: Option<ExecuteTransactionRequestType>,
    ) -> RpcResult<IotaTransactionBlockResponse> {
        Err(IndexerError::Generic(
            "execute_transaction_block should be called from OptimisticWriteApi".into(),
        )
        .into())
    }

    async fn dev_inspect_transaction_block(
        &self,
        sender_address: IotaAddress,
        tx_bytes: Base64,
        gas_price: Option<BigInt<u64>>,
        _epoch: Option<BigInt<u64>>,
        additional_args: Option<DevInspectArgs>,
    ) -> RpcResult<DevInspectResults> {
        self.dev_inspect_transaction_block_impl(
            sender_address,
            tx_bytes,
            gas_price,
            additional_args,
            &self.package_resolver,
        )
        .await
        .map_err(Into::into)
    }

    async fn dry_run_transaction_block(
        &self,
        tx_bytes: Base64,
    ) -> RpcResult<DryRunTransactionBlockResponse> {
        self.dry_run_transaction_block_impl(tx_bytes, &self.package_resolver)
            .await
            .map_err(Into::into)
    }

    async fn view_function_call(
        &self,
        function_name: String,
        type_args: Option<Vec<IotaTypeTag>>,
        arguments: Vec<IotaJsonValue>,
    ) -> RpcResult<IotaMoveViewCallResults> {
        let MoveFunctionName {
            package,
            module,
            function,
        } = function_name.as_str().parse().map_err(IndexerError::from)?;
        let sender = IotaAddress::ZERO;
        let tx_kind = self
            .transaction_builder
            .move_view_call_tx_kind(
                package,
                &module,
                &function,
                type_args.unwrap_or_default(),
                arguments,
            )
            .await
            .map_err(IndexerError::from)?;
        let tx_bytes = Base64::from_bytes(&bcs::to_bytes(&tx_kind).map_err(IndexerError::from)?);
        let dev_inspect_results = self
            .dev_inspect_transaction_block(sender, tx_bytes, None, None, None)
            .await?;
        Ok(IotaMoveViewCallResults::from_dev_inspect_results(
            self.package_resolver.package_store().clone(),
            dev_inspect_results,
        )
        .await
        .map_err(IndexerError::from)?)
    }
}

#[async_trait]
impl WriteApiServer for OptimisticWriteApi {
    async fn execute_transaction_block(
        &self,
        tx_bytes: Base64,
        signatures: Vec<Base64>,
        options: Option<IotaTransactionBlockResponseOptions>,
        _request_type: Option<ExecuteTransactionRequestType>,
    ) -> RpcResult<IotaTransactionBlockResponse> {
        let ingestion_path = self
            .optimistic_tx_executor
            .execute_and_index_transaction(tx_bytes, signatures)
            .await?;
        Ok(self
            .build_response(ingestion_path, options.unwrap_or_default())
            .await?)
    }

    async fn dev_inspect_transaction_block(
        &self,
        sender_address: IotaAddress,
        tx_bytes: Base64,
        gas_price: Option<BigInt<u64>>,
        epoch: Option<BigInt<u64>>,
        additional_args: Option<DevInspectArgs>,
    ) -> RpcResult<DevInspectResults> {
        self.write_api
            .dev_inspect_transaction_block(
                sender_address,
                tx_bytes,
                gas_price,
                epoch,
                additional_args,
            )
            .await
    }

    async fn dry_run_transaction_block(
        &self,
        tx_bytes: Base64,
    ) -> RpcResult<DryRunTransactionBlockResponse> {
        self.write_api.dry_run_transaction_block(tx_bytes).await
    }

    async fn view_function_call(
        &self,
        function_name: String,
        type_args: Option<Vec<IotaTypeTag>>,
        arguments: Vec<IotaJsonValue>,
    ) -> RpcResult<IotaMoveViewCallResults> {
        let chain = self
            .optimistic_tx_executor
            .read
            .get_chain_identifier_in_blocking_task()
            .await?
            .chain();
        if !matches!(chain, Chain::Unknown) {
            return Err(ApiError::UnsupportedFeature(format!(
                "View calls are not yet supported on {}",
                chain.as_str()
            ))
            .into());
        }

        self.write_api
            .view_function_call(function_name, type_args, arguments)
            .await
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

impl IotaRpcModule for OptimisticWriteApi {
    fn rpc(self) -> RpcModule<Self> {
        self.into_rpc()
    }

    fn rpc_doc_module() -> Module {
        iota_json_rpc_api::WriteApiOpenRpc::module_doc()
    }
}

/// Resolves balance and object changes in dry_run.
///
/// Checks the in-memory cache (from the simulate
/// response) first, then falls back to the indexer's `objects` table for
/// dynamically loaded objects not included in the response.
pub struct TxObjectResolver {
    object_cache: InMemObjectCache,
    reader: Arc<IndexerReader>,
}

impl TxObjectResolver {
    pub fn new(objects: &[&Object], reader: Arc<IndexerReader>) -> Self {
        let mut object_cache = InMemObjectCache::new();
        for obj in objects {
            object_cache.insert_object(<&Object>::clone(obj).clone());
        }
        Self {
            object_cache,
            reader,
        }
    }

    async fn get_past_object_read_with_retry(
        &self,
        id: ObjectID,
        version: SequenceNumber,
    ) -> IndexerResult<PastObjectRead> {
        let backoff = backoff::ExponentialBackoff {
            initial_interval: Duration::from_millis(100),
            max_elapsed_time: Some(Duration::from_secs(3)),
            multiplier: 2.0,
            ..Default::default()
        };

        backoff::future::retry(backoff, || async {
            self.reader
                .get_past_object_read_with_fallback(id, version, false)
                .await
                .map_err(backoff::Error::transient)
        })
        .await
    }

    pub(crate) async fn get_changes(
        &self,
        tx: &TransactionData,
        effects: &TransactionEffects,
        tx_digest: &TransactionDigest,
    ) -> IndexerResult<(
        Vec<iota_json_rpc_types::BalanceChange>,
        Vec<IndexedObjectChange>,
    )> {
        let object_changes: Vec<_> = get_object_changes(
            self,
            tx.sender(),
            effects.modified_at_versions(),
            effects.all_changed_objects(),
            effects.all_removed_objects(),
        )
        .await?
        .into_iter()
        .map(IndexedObjectChange::from)
        .collect();
        let balance_changes = get_balance_changes_from_effect(
            self,
            effects,
            tx.input_objects().unwrap_or_else(|e| {
                panic!("checkpointed tx {tx_digest} has invalid input objects: {e}")
            }),
            None,
        )
        .await?;
        Ok((balance_changes, object_changes))
    }
}

#[async_trait]
impl ObjectProvider for TxObjectResolver {
    type Error = IndexerError;

    async fn get_object(
        &self,
        id: &ObjectID,
        version: &SequenceNumber,
    ) -> Result<Object, Self::Error> {
        // try in-memory cache first
        if let Some(o) = self.object_cache.get(id, Some(version)) {
            return Ok(o.clone());
        }

        let past_read = self.get_past_object_read_with_retry(*id, *version).await?;

        past_read.into_object().map_err(|e| {
            IndexerError::Generic(format!(
                "object {id} at version {version} not found in cache or indexer DB: {e}"
            ))
        })
    }

    async fn find_object_lt_or_eq_version(
        &self,
        id: &ObjectID,
        version: &SequenceNumber,
    ) -> Result<Option<Object>, Self::Error> {
        // try exact version in cache
        if let Some(o) = self.object_cache.get(id, Some(version)) {
            return Ok(Some(o.clone()));
        }

        // try latest in cache
        if let Some(o) = self.object_cache.get(id, None) {
            if o.version() <= *version {
                return Ok(Some(o.clone()));
            }
        }

        self.get_past_object_read_with_retry(*id, *version)
            .await
            .map(|past_read| past_read.into_object().ok())
    }
}
