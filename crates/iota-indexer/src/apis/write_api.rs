// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::sync::Arc;

use async_trait::async_trait;
use fastcrypto::encoding::Base64;
use futures::{FutureExt, TryFutureExt};
use iota_grpc_client::{Client as GrpcClient, read_mask_fields::SimulateField};
use iota_json::IotaJsonValue;
use iota_json_rpc::IotaRpcModule;
use iota_json_rpc_api::WriteApiServer;
use iota_json_rpc_types::{
    DevInspectArgs, DevInspectResults, DryRunTransactionBlockResponse,
    ExecuteTransactionRequestType, IotaMoveViewCallResults, IotaTransactionBlock,
    IotaTransactionBlockEffects, IotaTransactionBlockResponse, IotaTransactionBlockResponseOptions,
    IotaTypeTag, MoveFunctionName,
};
use iota_open_rpc::Module;
use iota_package_resolver::{PackageStore, Resolver};
use iota_sdk_types::{
    Address, GasPayment, SenderSignedTransaction, Transaction, TransactionEffects,
    TransactionExpiration, TransactionKind, TransactionV1, UserSignature,
};
use iota_transaction_builder::TransactionBuilder;
use iota_types::{effects::TransactionEffectsAPI, error::ExecutionError, iota_serde::BigInt};
use jsonrpsee::{RpcModule, core::RpcResult};

use crate::{
    errors::{IndexerError, IndexerResult},
    models::transactions::{StoredTransaction, tx_events_to_iota_tx_events},
    optimistic_indexing::{IngestionPath, OptimisticTransactionExecutor},
    read::IndexerReader,
    store::package_resolver::{IndexerStorePackageResolver, SimulationPackageStore},
    types::grpc_conversion,
};

// As an optimization, we're trying to request only the fields we actually need.
const DRY_RUN_TRANSACTION_READ_MASK: &[SimulateField] = &[
    // The transaction the simulation ran, rather than the one that was sent: the node fills
    // in gas the caller left unset, and that is what the response reports back.
    SimulateField::EXECUTED_TRANSACTION_TRANSACTION_BCS,
    SimulateField::EXECUTED_TRANSACTION_SIGNATURES_BCS,
    SimulateField::EXECUTED_TRANSACTION_EFFECTS_BCS,
    // Needed to resolve types against a package the simulated transaction published.
    SimulateField::EXECUTED_TRANSACTION_OUTPUT_OBJECTS_BCS,
    SimulateField::EXECUTED_TRANSACTION_EVENTS_EVENTS_BCS,
    SimulateField::EXECUTED_TRANSACTION_BALANCE_CHANGES,
    SimulateField::EXECUTED_TRANSACTION_OBJECT_CHANGES,
    SimulateField::SUGGESTED_GAS_PRICE,
    SimulateField::EXECUTION_RESULT_EXECUTION_ERROR_SOURCE,
];
const DEV_INSPECT_TRANSACTION_READ_MASK: &[SimulateField] = &[
    SimulateField::EXECUTED_TRANSACTION_EFFECTS_BCS,
    // Needed to resolve types against a package the simulated transaction published.
    SimulateField::EXECUTED_TRANSACTION_OUTPUT_OBJECTS_BCS,
    SimulateField::EXECUTED_TRANSACTION_EVENTS_EVENTS_BCS,
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
}

#[derive(Clone)]
pub struct OptimisticWriteApi {
    write_api: WriteApi,
    optimistic_tx_executor: OptimisticTransactionExecutor,
}

impl WriteApi {
    pub fn new(fullnode_grpc_client: GrpcClient, reader: IndexerReader) -> Self {
        let package_resolver = IndexerStorePackageResolver::new(reader.get_pool());
        Self {
            fullnode_grpc_client,
            transaction_builder: TransactionBuilder::new(Arc::new(reader)),
            package_resolver: Arc::new(Resolver::new(package_resolver)),
        }
    }

    async fn dry_run_transaction_block_impl(
        &self,
        tx_bytes: Base64,
        package_resolver: &Arc<Resolver<impl PackageStore>>,
    ) -> IndexerResult<DryRunTransactionBlockResponse> {
        let tx = bcs::from_bytes::<Transaction>(&tx_bytes.to_vec()?)?;

        let simulate_tx_response = self
            .fullnode_grpc_client
            .simulate_transaction(tx.clone(), false, DRY_RUN_TRANSACTION_READ_MASK)
            .await?
            .into_inner();

        let executed_transaction = simulate_tx_response.executed_transaction()?;
        let execution_error_source = simulate_tx_response
            .execution_error()
            .and_then(|e| e.source.clone());
        let suggested_gas_price = simulate_tx_response.suggested_gas_price;

        let output_objects = grpc_conversion::objects(executed_transaction.output_objects()?)?;
        let balance_changes =
            grpc_conversion::balance_changes(executed_transaction.balance_changes()?)?;
        let object_changes =
            grpc_conversion::object_changes(executed_transaction.object_changes()?)?;

        let tx_effects: TransactionEffects = executed_transaction.effects()?.effects()?;

        // The digest of what actually ran, which is the one the effects and the events
        // are keyed by. It differs from the digest of the transaction as sent whenever
        // the simulation filled gas in — a mock gas coin changes the transaction it is
        // taken over.
        let tx_digest = *tx_effects.transaction_digest();

        let tx_signatures = executed_transaction
            .signatures()?
            .signatures
            .iter()
            .map(|s| -> IndexerResult<_> { Ok(s.signature()?) })
            .collect::<IndexerResult<Vec<UserSignature>>>()?;

        // Report the transaction the simulation ran, not the one that was sent: the
        // node fills in the gas the caller left unset and reports what it charged in
        // its place, which is how a caller reads back an estimate.
        let simulated_transaction = executed_transaction.transaction()?.transaction()?;
        let sender_signed_tx = SenderSignedTransaction::new(simulated_transaction, tx_signatures);

        let tx_events = executed_transaction.events()?.events()?;

        // Resolve types against the packages the simulation published before falling
        // back to the database, so that a transaction publishing a package can decode
        // the types it introduces — an event from its `init`, for one.
        let package_resolver = Arc::new(Resolver::new(SimulationPackageStore::new(
            &output_objects,
            package_resolver.clone(),
        )));

        let fut1 = IotaTransactionBlock::try_from_with_package_resolver(
            sender_signed_tx,
            &package_resolver,
            tx_digest,
        )
        .map_err(Into::into);

        // timestamp is None because it represent a checkpoint one, on a dry run
        // operation we don't have this information.
        let fut2 = tx_events_to_iota_tx_events(tx_events, &package_resolver, tx_digest, None);

        let fut3 = IotaTransactionBlockEffects::from_native_with_clever_error(
            tx_effects,
            &package_resolver,
        )
        .map(Ok);

        let (transaction_block, events, effects) =
            futures::future::try_join3(fut1, fut2, fut3).await?;

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
        sender_address: Address,
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

        let kind = bcs::from_bytes::<TransactionKind>(&tx_bytes.to_vec()?)?;

        let tx = Transaction::V1(TransactionV1 {
            kind,
            sender: sender_address,
            gas_payment: GasPayment {
                // Any of these the caller leaves out is filled in by the simulation on
                // the node: an empty payment gets a mock gas coin, a zero price gets the
                // epoch's reference gas price, and a zero budget gets the protocol
                // maximum.
                objects: gas_objects.unwrap_or_default(),
                owner: gas_sponsor.unwrap_or(sender_address),
                price: gas_price.map(BigInt::into_inner).unwrap_or_default(),
                budget: gas_budget.unwrap_or_default(),
            },
            expiration: TransactionExpiration::None,
        });

        // The transaction is only read back when it is going to be reported, since it
        // costs bytes on the wire.
        let mut read_mask = DEV_INSPECT_TRANSACTION_READ_MASK.to_vec();
        if show_raw_txn_data_and_effects {
            read_mask.push(SimulateField::EXECUTED_TRANSACTION_TRANSACTION_BCS);
        }

        let simulate_tx_response = self
            .fullnode_grpc_client
            .simulate_transaction(tx, skip_checks, read_mask)
            .await?
            .into_inner();

        let executed_transaction = simulate_tx_response.executed_transaction()?;

        let tx_effects: TransactionEffects = executed_transaction.effects()?.effects()?;

        // Report the transaction the simulation ran, not the one that was sent: the
        // node fills in the gas the caller left unset and reports what it charged in
        // its place, which is how a caller reads back an estimate.
        let raw_txn_data = show_raw_txn_data_and_effects
            .then(|| -> IndexerResult<_> {
                Ok(bcs::to_bytes(
                    &executed_transaction.transaction()?.transaction()?,
                )?)
            })
            .transpose()?
            .unwrap_or_default();

        let raw_effects = show_raw_txn_data_and_effects
            .then(|| bcs::to_bytes(&tx_effects))
            .transpose()?
            .unwrap_or_default();

        let tx_events = executed_transaction.events()?.events()?;

        // Resolve types against the packages the simulation published before falling
        // back to the database, so that a transaction publishing a package can decode
        // the types it introduces — an event from its `init`, for one.
        let output_objects = grpc_conversion::objects(executed_transaction.output_objects()?)?;
        let package_resolver = Arc::new(Resolver::new(SimulationPackageStore::new(
            &output_objects,
            package_resolver.clone(),
        )));

        let tx_digest = *tx_effects.transaction_digest();
        // timestamp is None because it represent a checkpoint one, on a dev inspect
        // operation we don't have this information.
        let events =
            tx_events_to_iota_tx_events(tx_events, &package_resolver, tx_digest, None).await?;

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
        sender_address: Address,
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
        let sender = Address::ZERO;
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
        let tx_bytes = Base64::from_bytes(&tx_kind.to_bcs());
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
        sender_address: Address,
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
