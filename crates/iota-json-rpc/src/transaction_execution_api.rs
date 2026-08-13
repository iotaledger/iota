// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{sync::Arc, time::Duration};

use async_trait::async_trait;
use fastcrypto::encoding::Base64;
use iota_core::{
    authority::AuthorityState, authority_client::NetworkAuthorityClient,
    transaction_orchestrator::TransactionOrchestrator,
};
use iota_json::IotaJsonValue;
use iota_json_rpc_api::{JsonRpcMetrics, WriteApiOpenRpc, WriteApiServer};
use iota_json_rpc_types::{
    DevInspectArgs, DevInspectResults, DryRunTransactionBlockResponse,
    ExecuteTransactionRequestType as ExecuteTransactionRequestTypeSchema, IotaExecutionStatus,
    IotaMoveViewCallResults, IotaTransactionBlock, IotaTransactionBlockData,
    IotaTransactionBlockEffects, IotaTransactionBlockEffectsAPI, IotaTransactionBlockEvents,
    IotaTransactionBlockResponse, IotaTransactionBlockResponseOptions, IotaTypeTag,
    MoveFunctionName,
};
use iota_metrics::spawn_monitored_task;
use iota_open_rpc::Module;
use iota_package_resolver::{
    Package, PackageStore, Resolver, error::Error as PackageResolverError,
};
use iota_sdk_types::{
    Address, GasPayment, ObjectId, Transaction, TransactionDigest, TransactionExpiration,
    TransactionKind, TransactionV1, UserSignature,
};
use iota_transaction_builder::TransactionBuilder;
use iota_types::{
    effects::{TransactionEffectsAPI, TransactionEffectsExt},
    error::IotaError,
    execution_config_utils::to_binary_config,
    inner_temporary_store::{
        ObjectMapPackageStore, PackageStoreWithFallback, TemporaryModuleResolver,
    },
    iota_serde::BigInt,
    quorum_driver_types::{
        ExecuteTransactionRequestType, ExecuteTransactionRequestV1, ExecuteTransactionResponseV1,
    },
    storage::PostExecutionPackageResolver,
    transaction::{InputObjectKind, TransactionAPI, TransactionEnvelope},
    transaction_executor::{SimulateTransactionResult, VmChecks},
};
use jsonrpsee::{RpcModule, core::RpcResult};
use tracing::{Instrument, instrument};

use crate::{
    IotaRpcModule, ObjectProviderCache,
    authority_state::StateRead,
    error::{Error, IotaRpcInputError},
    get_balance_changes_from_effect, get_object_changes,
    logger::FutureWithTracing,
    transaction_builder_api::AuthorityStateDataReader,
};

#[derive(Clone)]
pub struct TransactionExecutionApi {
    state: Arc<dyn StateRead>,
    transaction_orchestrator: Arc<TransactionOrchestrator<NetworkAuthorityClient>>,
    metrics: Arc<JsonRpcMetrics>,
    transaction_builder: TransactionBuilder,
}

impl TransactionExecutionApi {
    pub fn new(
        state: Arc<AuthorityState>,
        transaction_orchestrator: Arc<TransactionOrchestrator<NetworkAuthorityClient>>,
        metrics: Arc<JsonRpcMetrics>,
    ) -> Self {
        let reader = Arc::new(AuthorityStateDataReader::new(state.clone()));
        Self {
            state,
            transaction_orchestrator,
            metrics,
            transaction_builder: TransactionBuilder::new(reader),
        }
    }

    pub fn convert_bytes<T: serde::de::DeserializeOwned>(
        &self,
        tx_bytes: Base64,
    ) -> Result<T, IotaRpcInputError> {
        let data: T = bcs::from_bytes(&tx_bytes.to_vec()?)?;
        Ok(data)
    }

    #[expect(clippy::type_complexity)]
    fn prepare_execute_transaction_block(
        &self,
        tx_bytes: Base64,
        signatures: Vec<Base64>,
        opts: Option<IotaTransactionBlockResponseOptions>,
    ) -> Result<
        (
            ExecuteTransactionRequestV1,
            IotaTransactionBlockResponseOptions,
            Address,
            Vec<InputObjectKind>,
            TransactionEnvelope,
            Option<IotaTransactionBlock>,
            Vec<u8>,
        ),
        IotaRpcInputError,
    > {
        let opts = opts.unwrap_or_default();
        let tx: Transaction = self.convert_bytes(tx_bytes)?;
        let sender = tx.sender();
        let input_objs = tx.input_objects().unwrap_or_default();

        let mut sigs = Vec::new();
        for sig in signatures {
            sigs.push(
                UserSignature::from_base64(&sig.encoded())
                    .map_err(|e| IotaRpcInputError::GenericInvalid(e.to_string()))?,
            );
        }
        let txn = TransactionEnvelope::from_user_sig_data(tx, sigs);
        let raw_transaction = if opts.show_raw_input {
            bcs::to_bytes(txn.data())?
        } else {
            vec![]
        };
        let transaction = if opts.show_input {
            let epoch_store = self.state.load_epoch_store_one_call_per_task();

            Some(IotaTransactionBlock::try_from(
                txn.data().clone(),
                epoch_store.module_cache(),
                *txn.digest(),
            )?)
        } else {
            None
        };

        let request = ExecuteTransactionRequestV1 {
            transaction: txn.clone(),
            include_events: opts.show_events,
            include_input_objects: opts.show_balance_changes || opts.show_object_changes,
            include_output_objects: opts.show_balance_changes
                || opts.show_object_changes
                // In order to resolve events, we may need access to the newly published packages.
                || opts.show_events,
            include_auxiliary_data: false,
        };

        Ok((
            request,
            opts,
            sender,
            input_objs,
            txn,
            transaction,
            raw_transaction,
        ))
    }

    #[instrument("json_rpc_api_execute_transaction_block", level = "trace", skip_all)]
    async fn execute_transaction_block(
        &self,
        tx_bytes: Base64,
        signatures: Vec<Base64>,
        opts: Option<IotaTransactionBlockResponseOptions>,
        request_type: Option<ExecuteTransactionRequestType>,
    ) -> Result<IotaTransactionBlockResponse, Error> {
        let request_type =
            request_type.unwrap_or(ExecuteTransactionRequestType::WaitForEffectsCert);
        let (request, opts, sender, input_objs, txn, transaction, raw_transaction) =
            self.prepare_execute_transaction_block(tx_bytes, signatures, opts)?;
        let digest = *txn.digest();

        let transaction_orchestrator = self.transaction_orchestrator.clone();
        let orch_timer = self.metrics.orchestrator_latency_ms.start_timer();

        tracing::trace!(
            "Spawning transaction orchestrator task for transaction: {}",
            digest
        );
        let (response, is_executed_locally) = spawn_monitored_task!(
            transaction_orchestrator.execute_transaction_block(request, request_type, None)
        )
        .await?
        .map_err(Error::from)?;
        drop(orch_timer);

        self.handle_post_orchestration(
            response,
            is_executed_locally,
            opts,
            digest,
            input_objs,
            transaction,
            raw_transaction,
            sender,
        )
        .await
    }

    #[instrument(level = "trace", skip_all)]
    async fn handle_post_orchestration(
        &self,
        response: ExecuteTransactionResponseV1,
        is_executed_locally: bool,
        opts: IotaTransactionBlockResponseOptions,
        digest: TransactionDigest,
        input_objs: Vec<InputObjectKind>,
        transaction: Option<IotaTransactionBlock>,
        raw_transaction: Vec<u8>,
        sender: Address,
    ) -> Result<IotaTransactionBlockResponse, Error> {
        let _post_orch_timer = self.metrics.post_orchestrator_latency_ms.start_timer();

        let events = if opts.show_events {
            tracing::trace!("Resolving events");
            let epoch_store = self.state.load_epoch_store_one_call_per_task();
            let backing_package_store = PostExecutionPackageResolver::new(
                self.state.get_backing_package_store().clone(),
                &response.output_objects,
            );
            let mut layout_resolver = epoch_store
                .executor()
                .type_layout_resolver(Box::new(backing_package_store));
            Some(IotaTransactionBlockEvents::try_from(
                response.events.unwrap_or_default(),
                digest,
                None,
                layout_resolver.as_mut(),
            )?)
        } else {
            None
        };

        // Skip cache (and downstream balance/object_changes) when the validator
        // returned no input/output objects — e.g. the already-executed early-return.
        // Without this guard, cache misses fall through to a provider lookup that
        // races with local state and returns "version higher than latest".
        let object_cache = if (opts.show_balance_changes || opts.show_object_changes)
            && (response.input_objects.is_some() || response.output_objects.is_some())
        {
            let mut object_cache = ObjectProviderCache::new(self.state.clone());
            if let Some(input_objects) = response.input_objects {
                object_cache.insert_objects_into_cache(input_objects);
            }
            if let Some(output_objects) = response.output_objects {
                object_cache.insert_objects_into_cache(output_objects);
            }
            Some(object_cache)
        } else {
            None
        };

        let balance_changes = match &object_cache {
            Some(object_cache) if opts.show_balance_changes => Some(
                get_balance_changes_from_effect(
                    object_cache,
                    &response.effects.effects,
                    input_objs,
                    None,
                )
                .instrument(tracing::trace_span!("resolving balance changes"))
                .await?,
            ),
            _ => None,
        };

        let object_changes = match &object_cache {
            Some(object_cache) if opts.show_object_changes => Some(
                get_object_changes(
                    object_cache,
                    sender,
                    response.effects.effects.modified_at_versions(),
                    response.effects.effects.all_changed_objects(),
                    response.effects.effects.all_removed_objects(),
                )
                .instrument(tracing::trace_span!("resolving object changes"))
                .await?,
            ),
            _ => None,
        };

        let raw_effects = if opts.show_raw_effects {
            bcs::to_bytes(&response.effects.effects)?
        } else {
            vec![]
        };
        let resolver = Resolver::new(self.clone());

        let effects = if opts.show_effects {
            Some(
                IotaTransactionBlockEffects::from_native_with_clever_error(
                    response.effects.effects,
                    &resolver,
                )
                .await,
            )
        } else {
            None
        };

        let errors = match effects.as_ref().map(|e| e.status()) {
            Some(IotaExecutionStatus::Failure { error }) => vec![error.clone()],
            _ => vec![],
        };

        Ok(IotaTransactionBlockResponse {
            digest,
            transaction,
            raw_transaction,
            effects,
            events,
            object_changes,
            balance_changes,
            timestamp_ms: None,
            confirmed_local_execution: Some(is_executed_locally),
            checkpoint: None,
            errors,
            raw_effects,
        })
    }

    pub fn prepare_dry_run_transaction_block(
        &self,
        tx_bytes: Base64,
    ) -> Result<(Transaction, Vec<InputObjectKind>), IotaRpcInputError> {
        let tx: Transaction = self.convert_bytes(tx_bytes)?;
        let input_objs = tx.input_objects()?;
        Ok((tx, input_objs))
    }

    /// Report the gas the simulation ran with, in place of whatever the caller
    /// left unset. Same rule as gRPC `simulate_transactions`, which shares the
    /// helper.
    fn report_simulation_gas(
        transaction: &mut Transaction,
        simulation: &SimulateTransactionResult,
    ) {
        iota_types::gas::report_simulation_gas(
            transaction.gas_data_mut(),
            &simulation.gas_data,
            simulation.effects.gas_cost_summary().gas_used(),
        );
    }

    /// The synchronous part of
    /// [`dry_run_transaction_block`](Self::dry_run_transaction_block): the
    /// simulation, and the resolution of the response's input and events over
    /// the objects it wrote. Meant to run on a blocking thread; the async
    /// object- and balance-change queries stay with the caller.
    fn dry_run_transaction_block_impl(
        &self,
        mut tx: Transaction,
    ) -> Result<
        (
            SimulateTransactionResult,
            IotaTransactionBlockData,
            IotaTransactionBlockEvents,
        ),
        Error,
    > {
        // Hold on to one epoch store for the whole operation, so that the simulation
        // and the type resolution below observe the same epoch. A full `Arc` rather
        // than the arc-swap guard: a guard occupies one of arc-swap's scarce
        // per-thread borrow slots, meant for short borrows, not a whole simulation.
        let epoch_store = Arc::clone(&self.state.load_epoch_store_one_call_per_task());

        let mut simulation = self.state.simulate_transaction_in_epoch(
            &epoch_store,
            tx.clone(),
            VmChecks::Enabled,
        )?;

        Self::report_simulation_gas(&mut tx, &simulation);

        let tx_digest = *simulation.effects.transaction_digest();
        // Resolve types against the objects the simulation wrote before falling back to
        // the store, so that packages published by the transaction itself are visible.
        let (input, events) = {
            let output_objects = simulation.output_objects.clone();
            let mut layout_resolver = epoch_store.executor().type_layout_resolver(Box::new(
                PackageStoreWithFallback::new(
                    ObjectMapPackageStore(&output_objects),
                    self.state.get_backing_package_store(),
                ),
            ));
            let module_cache = TemporaryModuleResolver::new(
                &output_objects,
                to_binary_config(epoch_store.protocol_config()),
                epoch_store.module_cache().clone(),
            );

            let input =
                IotaTransactionBlockData::try_from_with_module_cache(tx, &module_cache, tx_digest)
                    .map_err(|e| IotaError::TransactionSerialization {
                        error: format!(
                            "Failed to convert transaction to IotaTransactionBlockData: {e}"
                        ),
                    })?;
            let events = IotaTransactionBlockEvents::try_from(
                simulation.events.take().unwrap_or_default(),
                tx_digest,
                None,
                layout_resolver.as_mut(),
            )?;

            (input, events)
        };

        Ok((simulation, input, events))
    }

    async fn dry_run_transaction_block(
        &self,
        tx_bytes: Base64,
    ) -> Result<DryRunTransactionBlockResponse, Error> {
        let (txn_data, input_objs) = self.prepare_dry_run_transaction_block(tx_bytes)?;
        let sender = txn_data.sender();

        // Use spawn_blocking since simulating a transaction and resolving types
        // over its output are long-running synchronous operations
        let (simulation, input, events) = {
            let this = self.clone();
            tokio::task::spawn_blocking(move || this.dry_run_transaction_block_impl(txn_data))
                .await
                .map_err(Error::from)??
        };

        let execution_error_source = simulation
            .execution_result
            .as_ref()
            .err()
            .and_then(|e| e.source().as_ref().map(|e| e.to_string()));

        let object_cache =
            ObjectProviderCache::new_with_cache(self.state.clone(), &simulation.output_objects);
        let balance_changes = get_balance_changes_from_effect(
            &object_cache,
            &simulation.effects,
            input_objs,
            simulation.mock_gas_id,
        )
        .await?;
        let object_changes = get_object_changes(
            &object_cache,
            sender,
            simulation.effects.modified_at_versions(),
            simulation.effects.all_changed_objects(),
            simulation.effects.all_removed_objects(),
        )
        .await?;

        let resolver = Resolver::new(self.clone());
        let effects = IotaTransactionBlockEffects::from_native_with_clever_error(
            simulation.effects,
            &resolver,
        )
        .await;

        Ok(DryRunTransactionBlockResponse {
            effects,
            events,
            object_changes,
            balance_changes,
            input,
            suggested_gas_price: simulation.suggested_gas_price,
            execution_error_source,
        })
    }

    fn dev_inspect_transaction_impl(
        &self,
        sender: Address,
        transaction_kind: TransactionKind,
        gas_price: Option<u64>,
        args: DevInspectArgs,
    ) -> Result<DevInspectResults, Error> {
        let DevInspectArgs {
            gas_sponsor,
            gas_budget,
            gas_objects,
            show_raw_txn_data_and_effects,
            skip_checks,
        } = args;
        let show_raw_txn_data_and_effects = show_raw_txn_data_and_effects.unwrap_or(false);
        let skip_checks = skip_checks.unwrap_or(true);

        // Hold on to one epoch store for the whole operation, so that the simulation
        // and the type resolution below observe the same epoch. A full `Arc` rather
        // than the arc-swap guard: a guard occupies one of arc-swap's scarce
        // per-thread borrow slots, meant for short borrows, not a whole simulation.
        let epoch_store = Arc::clone(&self.state.load_epoch_store_one_call_per_task());

        let transaction = Transaction::V1(TransactionV1 {
            kind: transaction_kind,
            sender,
            gas_payment: GasPayment {
                // Any of these the caller leaves out is filled in by the simulation,
                // whether or not the checks are skipped: an empty payment gets a mock
                // gas coin, a zero price gets the epoch's reference gas price, and a
                // zero budget as much as the gas coins can back, up to the protocol
                // maximum.
                objects: gas_objects.unwrap_or_default(),
                owner: gas_sponsor.unwrap_or(sender),
                price: gas_price.unwrap_or_default(),
                budget: gas_budget.unwrap_or_default(),
            },
            expiration: TransactionExpiration::None,
        });

        let checks = if skip_checks {
            VmChecks::Disabled
        } else {
            VmChecks::Enabled
        };
        // Kept back from the simulation, which consumes the transaction, so that the
        // reported gas can be filled in from what the simulation charged.
        let mut reported_transaction = show_raw_txn_data_and_effects.then(|| transaction.clone());
        let simulation =
            self.state
                .simulate_transaction_in_epoch(&epoch_store, transaction, checks)?;

        let raw_txn_data = match reported_transaction.as_mut() {
            Some(transaction) => {
                Self::report_simulation_gas(transaction, &simulation);
                bcs::to_bytes(transaction).map_err(|_| IotaError::TransactionSerialization {
                    error: "Failed to serialize transaction during dev inspect".to_string(),
                })?
            }
            None => vec![],
        };

        let raw_effects = if show_raw_txn_data_and_effects {
            bcs::to_bytes(&simulation.effects).map_err(|_| IotaError::TransactionSerialization {
                error: "Failed to serialize transaction effects during dev inspect".to_string(),
            })?
        } else {
            vec![]
        };

        // Resolve types against the objects the simulation wrote before falling back to
        // the store, so that packages published by the transaction itself are visible.
        let output_objects = &simulation.output_objects;
        let mut layout_resolver =
            epoch_store
                .executor()
                .type_layout_resolver(Box::new(PackageStoreWithFallback::new(
                    ObjectMapPackageStore(output_objects),
                    self.state.get_backing_package_store(),
                )));

        Ok(DevInspectResults::new(
            simulation.effects,
            simulation.events.unwrap_or_default(),
            simulation.execution_result,
            raw_txn_data,
            raw_effects,
            layout_resolver.as_mut(),
        )?)
    }

    async fn dev_inspect_transaction(
        &self,
        sender: Address,
        transaction_kind: TransactionKind,
        gas_price: Option<u64>,
        args: DevInspectArgs,
    ) -> Result<DevInspectResults, Error> {
        // Use spawn_blocking since simulating a transaction is a long-running
        // synchronous operation
        let this = self.clone();
        tokio::task::spawn_blocking(move || {
            this.dev_inspect_transaction_impl(sender, transaction_kind, gas_price, args)
        })
        .await
        .map_err(Error::from)?
    }
}

#[async_trait]
impl WriteApiServer for TransactionExecutionApi {
    #[instrument(skip(self, tx_bytes, signatures))]
    async fn execute_transaction_block(
        &self,
        tx_bytes: Base64,
        signatures: Vec<Base64>,
        opts: Option<IotaTransactionBlockResponseOptions>,
        request_type: Option<ExecuteTransactionRequestTypeSchema>,
    ) -> RpcResult<IotaTransactionBlockResponse> {
        self.execute_transaction_block(tx_bytes, signatures, opts, request_type.map(Into::into))
            .trace_timeout(Duration::from_secs(10))
            .await
    }

    /// Calls a move view function.
    #[instrument(skip(self, arguments))]
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
        } = function_name.as_str().parse().map_err(Error::from)?;
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
            .map_err(Error::from)?;
        let dev_inspect_results = self
            .dev_inspect_transaction(sender, tx_kind, None, DevInspectArgs::default())
            .await?;
        Ok(
            IotaMoveViewCallResults::from_dev_inspect_results(self.clone(), dev_inspect_results)
                .await
                .map_err(Error::from)?,
        )
    }

    #[instrument(
        skip(self, sender_address, tx_bytes, additional_args),
        fields(sender_address = %sender_address)
    )]
    async fn dev_inspect_transaction_block(
        &self,
        sender_address: Address,
        tx_bytes: Base64,
        gas_price: Option<BigInt<u64>>,
        _epoch: Option<BigInt<u64>>,
        additional_args: Option<DevInspectArgs>,
    ) -> RpcResult<DevInspectResults> {
        async move {
            let tx_kind: TransactionKind = self.convert_bytes(tx_bytes)?;
            self.dev_inspect_transaction(
                sender_address,
                tx_kind,
                gas_price.map(|i| *i),
                additional_args.unwrap_or_default(),
            )
            .await
        }
        .trace()
        .await
    }

    #[instrument(skip(self, tx_bytes))]
    async fn dry_run_transaction_block(
        &self,
        tx_bytes: Base64,
    ) -> RpcResult<DryRunTransactionBlockResponse> {
        self.dry_run_transaction_block(tx_bytes).trace().await
    }
}

impl IotaRpcModule for TransactionExecutionApi {
    fn rpc(self) -> RpcModule<Self> {
        self.into_rpc()
    }

    fn rpc_doc_module() -> Module {
        WriteApiOpenRpc::module_doc()
    }
}

#[async_trait]
impl PackageStore for TransactionExecutionApi {
    async fn fetch(&self, id: Address) -> Result<Arc<Package>, PackageResolverError> {
        let backing_store = self.state.get_backing_package_store();
        match backing_store.get_package_object(&ObjectId::new(id.into_bytes())) {
            Ok(Some(pkg)) => Ok(Arc::new(Package::read_from_package(pkg.move_package())?)),
            Ok(None) => Err(PackageResolverError::PackageNotFound(id)),
            Err(e) => Err(PackageResolverError::Store {
                store: "Node",
                source: Arc::new(e),
            }),
        }
    }
}
