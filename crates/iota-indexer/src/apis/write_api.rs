// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::sync::Arc;

use async_trait::async_trait;
use fastcrypto::encoding::Base64;
use futures::{FutureExt, TryFutureExt};
use iota_grpc_client::{GrpcClient, read_mask_fields::SimulateField};
use iota_json::IotaJsonValue;
use iota_json_rpc::IotaRpcModule;
use iota_json_rpc_api::WriteApiServer;
use iota_json_rpc_types::{
    BalanceChange, DevInspectArgs, DevInspectResults, DryRunTransactionBlockResponse,
    ExecuteTransactionRequestType, IotaExecutionResult, IotaMoveViewCallResults,
    IotaTransactionBlock, IotaTransactionBlockEffects, IotaTransactionBlockResponse,
    IotaTransactionBlockResponseOptions, IotaTypeTag, MoveFunctionName, ObjectChange,
};
use iota_open_rpc::Module;
use iota_package_resolver::{PackageStore, Resolver};
use iota_sdk_types::{
    Address, GasPayment, SenderSignedTransaction, Transaction, TransactionEffects,
    TransactionEvents, TransactionExpiration, TransactionKind, TransactionV1, UserSignature,
};
use iota_transaction_builder::TransactionBuilder;
use iota_types::{
    effects::TransactionEffectsAPI, error::ExecutionError, iota_serde::BigInt, object::Object,
};
use jsonrpsee::{RpcModule, core::RpcResult};

use crate::{
    errors::{IndexerError, IndexerResult},
    models::transactions::{StoredTransaction, tx_events_to_iota_tx_events},
    optimistic_indexing::{IngestionPath, OptimisticTransactionExecutor},
    read::IndexerReader,
    store::package_resolver::{IndexerStorePackageResolver, SimulationPackageStore},
    types::grpc_conversion,
};

// The fields every simulation reads back, whatever the caller. Anything beyond
// this is requested through `SimulationFields`; `simulate_transaction` builds
// the full read mask from both.
const SIMULATE_CORE_READ_MASK: &[SimulateField] = &[
    SimulateField::EXECUTED_TRANSACTION_EFFECTS_BCS,
    SimulateField::EXECUTED_TRANSACTION_EVENTS_EVENTS_BCS,
    // Needed to resolve types against a package the simulated transaction published.
    SimulateField::EXECUTED_TRANSACTION_OUTPUT_OBJECTS_BCS,
    SimulateField::EXECUTION_RESULT_EXECUTION_ERROR_SOURCE,
];

/// The fields to read back from a dry-run simulation beyond the core ones
/// (which are always returned). Each flag adds the fields it needs to the read
/// mask and populates the matching field on [`SimulationOutput`]; a flag
/// left `false` leaves that field empty.
#[derive(Default)]
pub struct SimulationFields {
    /// The transaction the simulation ran, with any gas the caller left unset
    /// filled in by the node.
    pub transaction: bool,
    /// The balance changes the transaction caused.
    pub balance_changes: bool,
    /// The gas price the node suggests for the transaction.
    pub suggested_gas_price: bool,
    /// The simulated transaction's signatures.
    pub signatures: bool,
    /// The node's object changes
    pub object_changes: bool,
    /// The simulation's input objects
    pub input_objects: bool,
    /// The per-command results (mutated references and return values).
    pub command_results: bool,
    /// The full execution error (its kind, command index and source); the
    /// source alone is a core field and always returned.
    pub execution_error: bool,
}

/// The result of a dry-run simulation, in the node's native types. The core
/// fields are always present; the rest are populated only when the matching
/// [`SimulationFields`] flag was set.
pub struct SimulationOutput {
    pub transaction: Option<Transaction>,
    pub effects: TransactionEffects,
    pub events: TransactionEvents,
    pub output_objects: Vec<Object>,
    pub balance_changes: Vec<BalanceChange>,
    pub suggested_gas_price: Option<u64>,
    pub execution_error_source: Option<String>,
    pub signatures: Vec<UserSignature>,
    pub object_changes: Vec<ObjectChange>,
    pub input_objects: Vec<Object>,
    pub command_results: Option<Vec<IotaExecutionResult>>,
    pub execution_error: Option<String>,
}

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

    /// Run a dry-run simulation and return its result in the node's native
    /// types. The read mask is built here from the core fields plus
    /// whatever `fields` asks for, so callers never deal with it directly.
    /// Both dry-run endpoints go through this, so the simulation and its
    /// conversions live in one place.
    pub async fn simulate_transaction(
        &self,
        tx: Transaction,
        skip_checks: bool,
        fields: SimulationFields,
    ) -> IndexerResult<SimulationOutput> {
        let mut read_mask = SIMULATE_CORE_READ_MASK.to_vec();
        if fields.transaction {
            read_mask.push(SimulateField::EXECUTED_TRANSACTION_TRANSACTION_BCS);
        }
        if fields.balance_changes {
            read_mask.push(SimulateField::EXECUTED_TRANSACTION_BALANCE_CHANGES);
        }
        if fields.suggested_gas_price {
            read_mask.push(SimulateField::SUGGESTED_GAS_PRICE);
        }
        if fields.signatures {
            read_mask.push(SimulateField::EXECUTED_TRANSACTION_SIGNATURES_BCS);
        }
        if fields.object_changes {
            read_mask.push(SimulateField::EXECUTED_TRANSACTION_OBJECT_CHANGES);
        }
        if fields.input_objects {
            read_mask.push(SimulateField::EXECUTED_TRANSACTION_INPUT_OBJECTS_BCS);
        }
        if fields.command_results {
            read_mask.push(SimulateField::EXECUTION_RESULT_COMMAND_RESULTS_MUTATED_BY_REF);
            read_mask.push(SimulateField::EXECUTION_RESULT_COMMAND_RESULTS_RETURN_VALUES);
        }
        if fields.execution_error {
            read_mask.push(SimulateField::EXECUTION_RESULT_EXECUTION_ERROR_BCS_KIND);
            read_mask.push(SimulateField::EXECUTION_RESULT_EXECUTION_ERROR_COMMAND_INDEX);
        }

        let response = self
            .fullnode_grpc_client
            .simulate_transaction(tx, skip_checks, read_mask)
            .await?
            .into_inner();
        let executed_transaction = response.executed_transaction()?;

        let signatures = if fields.signatures {
            executed_transaction
                .signatures()?
                .signatures
                .iter()
                .map(|s| -> IndexerResult<_> { Ok(s.signature()?) })
                .collect::<IndexerResult<Vec<UserSignature>>>()?
        } else {
            vec![]
        };
        let object_changes = if fields.object_changes {
            grpc_conversion::object_changes(executed_transaction.object_changes()?)?
        } else {
            vec![]
        };
        let input_objects = if fields.input_objects {
            grpc_conversion::objects(executed_transaction.input_objects()?)?
        } else {
            vec![]
        };
        let command_results = if fields.command_results {
            response
                .command_results()
                .map(|command_results| grpc_conversion::command_results(command_results.clone()))
                .transpose()?
        } else {
            None
        };
        let execution_error = if fields.execution_error {
            response
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
                .transpose()?
        } else {
            None
        };

        let transaction = if fields.transaction {
            // Report the transaction the simulation ran, not the one that was sent: the
            // node fills in the gas the caller left unset and reports the values it
            // used, which is how a caller reads back an estimate.
            Some(executed_transaction.transaction()?.transaction()?)
        } else {
            None
        };
        let balance_changes = if fields.balance_changes {
            grpc_conversion::balance_changes(executed_transaction.balance_changes()?)?
        } else {
            vec![]
        };
        let suggested_gas_price = if fields.suggested_gas_price {
            response.suggested_gas_price
        } else {
            None
        };

        Ok(SimulationOutput {
            transaction,
            effects: executed_transaction.effects()?.effects()?,
            events: executed_transaction.events()?.events()?,
            output_objects: grpc_conversion::objects(executed_transaction.output_objects()?)?,
            balance_changes,
            suggested_gas_price,
            execution_error_source: response.execution_error().and_then(|e| e.source.clone()),
            signatures,
            object_changes,
            input_objects,
            command_results,
            execution_error,
        })
    }

    async fn dry_run_transaction_block_impl(
        &self,
        tx_bytes: Base64,
        package_resolver: &Arc<Resolver<impl PackageStore>>,
    ) -> IndexerResult<DryRunTransactionBlockResponse> {
        let tx = bcs::from_bytes::<Transaction>(&tx_bytes.to_vec()?)?;
        let sim = self
            .simulate_transaction(
                tx,
                false,
                SimulationFields {
                    transaction: true,
                    balance_changes: true,
                    suggested_gas_price: true,
                    signatures: true,
                    object_changes: true,
                    ..Default::default()
                },
            )
            .await?;

        // The digest of what actually ran, which is the one the effects and the events
        // are keyed by. It differs from the digest of the transaction as sent whenever
        // the simulation filled gas in — a mock gas coin changes the transaction it is
        // taken over.
        let tx_digest = *sim.effects.transaction_digest();
        let transaction = sim
            .transaction
            .expect("dry run always requests the transaction");
        let sender_signed_tx = SenderSignedTransaction::new(transaction, sim.signatures);

        // Resolve types against the packages the simulation published before falling
        // back to the database, so that a transaction publishing a package can decode
        // the types it introduces — an event from its `init`, for one.
        let package_resolver = Arc::new(Resolver::new(SimulationPackageStore::new(
            &sim.output_objects,
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
        let fut2 = tx_events_to_iota_tx_events(sim.events, &package_resolver, tx_digest, None);

        let fut3 = IotaTransactionBlockEffects::from_native_with_clever_error(
            sim.effects,
            &package_resolver,
        )
        .map(Ok);

        let (transaction_block, events, effects) =
            futures::future::try_join3(fut1, fut2, fut3).await?;

        Ok(DryRunTransactionBlockResponse {
            effects,
            events,
            object_changes: sim.object_changes,
            balance_changes: sim.balance_changes,
            input: transaction_block.data,
            suggested_gas_price: sim.suggested_gas_price,
            execution_error_source: sim.execution_error_source,
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

        let simulation = self
            .simulate_transaction(
                tx,
                skip_checks,
                SimulationFields {
                    transaction: show_raw_txn_data_and_effects,
                    command_results: true,
                    execution_error: true,
                    ..Default::default()
                },
            )
            .await?;

        let tx_digest = *simulation.effects.transaction_digest();

        // Report the transaction the simulation ran, not the one that was sent: the
        // node fills in the gas the caller left unset.
        let raw_txn_data = simulation
            .transaction
            .as_ref()
            .map(bcs::to_bytes)
            .transpose()?
            .unwrap_or_default();
        let raw_effects = show_raw_txn_data_and_effects
            .then(|| bcs::to_bytes(&simulation.effects))
            .transpose()?
            .unwrap_or_default();

        // Resolve types against the packages the simulation published before falling
        // back to the database, so that a transaction publishing a package can decode
        // the types it introduces — an event from its `init`, for one.
        let package_resolver = Arc::new(Resolver::new(SimulationPackageStore::new(
            &simulation.output_objects,
            package_resolver.clone(),
        )));
        // timestamp is None because it represent a checkpoint one, on a dev inspect
        // operation we don't have this information.
        let events =
            tx_events_to_iota_tx_events(simulation.events, &package_resolver, tx_digest, None)
                .await?;

        Ok(DevInspectResults {
            effects: simulation.effects.try_into()?,
            events,
            results: simulation.command_results,
            error: simulation.execution_error,
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

    pub async fn simulate_transaction(
        &self,
        tx: Transaction,
        skip_checks: bool,
        fields: SimulationFields,
    ) -> IndexerResult<SimulationOutput> {
        self.write_api
            .simulate_transaction(tx, skip_checks, fields)
            .await
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
