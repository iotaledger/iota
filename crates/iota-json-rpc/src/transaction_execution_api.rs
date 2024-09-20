// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{sync::Arc, time::Duration};

use async_trait::async_trait;
use fastcrypto::{encoding::Base64, traits::ToFromBytes};
use iota_core::{
    authority::AuthorityState, authority_client::NetworkAuthorityClient,
    transaction_orchestrator::TransactionOrchestrator,
};
use iota_json_rpc_api::{JsonRpcMetrics, WriteApiOpenRpc, WriteApiServer};
use iota_json_rpc_types::{
    DevInspectArgs, DevInspectResults, DryRunTransactionBlockResponse, IotaTransactionBlock,
    IotaTransactionBlockEvents, IotaTransactionBlockResponse, IotaTransactionBlockResponseOptions,
};
use iota_metrics::spawn_monitored_task;
use iota_open_rpc::Module;
use iota_types::{
    base_types::IotaAddress,
    crypto::default_hash,
    digests::TransactionDigest,
    effects::TransactionEffectsAPI,
    iota_serde::BigInt,
    quorum_driver_types::{
        ExecuteTransactionRequestType, ExecuteTransactionRequestV3, ExecuteTransactionResponseV3,
    },
    signature::GenericSignature,
    storage::PostExecutionPackageResolver,
    transaction::{
        InputObjectKind, Transaction, TransactionData, TransactionDataAPI, TransactionKind,
    },
};
use jsonrpsee::{core::RpcResult, RpcModule};
use shared_crypto::intent::{AppId, Intent, IntentMessage, IntentScope, IntentVersion};
use tracing::instrument;

use crate::{
    authority_state::StateRead,
    error::{Error, IotaRpcInputError},
    get_balance_changes_from_effect, get_object_changes,
    logger::FutureWithTracing,
    IotaRpcModule, ObjectProviderCache,
};

pub struct TransactionExecutionApi {
    state: Arc<dyn StateRead>,
    transaction_orchestrator: Arc<TransactionOrchestrator<NetworkAuthorityClient>>,
    metrics: Arc<JsonRpcMetrics>,
}

impl TransactionExecutionApi {
    pub fn new(
        state: Arc<AuthorityState>,
        transaction_orchestrator: Arc<TransactionOrchestrator<NetworkAuthorityClient>>,
        metrics: Arc<JsonRpcMetrics>,
    ) -> Self {
        Self {
            state,
            transaction_orchestrator,
            metrics,
        }
    }

    pub fn convert_bytes<T: serde::de::DeserializeOwned>(
        &self,
        tx_bytes: Base64,
    ) -> Result<T, IotaRpcInputError> {
        let data: T = bcs::from_bytes(&tx_bytes.to_vec()?)?;
        Ok(data)
    }

    #[allow(clippy::type_complexity)]
    fn prepare_execute_transaction_block(
        &self,
        tx_bytes: Base64,
        signatures: Vec<Base64>,
        opts: Option<IotaTransactionBlockResponseOptions>,
    ) -> Result<
        (
            ExecuteTransactionRequestV3,
            IotaTransactionBlockResponseOptions,
            IotaAddress,
            Vec<InputObjectKind>,
            Transaction,
            Option<IotaTransactionBlock>,
            Vec<u8>,
        ),
        IotaRpcInputError,
    > {
        let opts = opts.unwrap_or_default();
        let tx_data: TransactionData = self.convert_bytes(tx_bytes)?;
        let sender = tx_data.sender();
        let input_objs = tx_data.input_objects().unwrap_or_default();

        let mut sigs = Vec::new();
        for sig in signatures {
            sigs.push(GenericSignature::from_bytes(&sig.to_vec()?)?);
        }
        let txn = Transaction::from_generic_sig_data(tx_data, sigs);
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
            )?)
        } else {
            None
        };

        let request = ExecuteTransactionRequestV3 {
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

    async fn handle_post_orchestration(
        &self,
        response: ExecuteTransactionResponseV3,
        is_executed_locally: bool,
        opts: IotaTransactionBlockResponseOptions,
        digest: TransactionDigest,
        input_objs: Vec<InputObjectKind>,
        transaction: Option<IotaTransactionBlock>,
        raw_transaction: Vec<u8>,
        sender: IotaAddress,
    ) -> Result<IotaTransactionBlockResponse, Error> {
        let _post_orch_timer = self.metrics.post_orchestrator_latency_ms.start_timer();

        let events = if opts.show_events {
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

        let object_cache = response.output_objects.map(|output_objects| {
            ObjectProviderCache::new_with_output_objects(self.state.clone(), output_objects)
        });

        let balance_changes = match &object_cache {
            Some(object_cache) if opts.show_balance_changes => Some(
                get_balance_changes_from_effect(
                    object_cache,
                    &response.effects.effects,
                    input_objs,
                    None,
                )
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
                .await?,
            ),
            _ => None,
        };

        let raw_effects = if opts.show_raw_effects {
            bcs::to_bytes(&response.effects.effects)?
        } else {
            vec![]
        };

        Ok(IotaTransactionBlockResponse {
            digest,
            transaction,
            raw_transaction,
            effects: opts
                .show_effects
                .then_some(response.effects.effects.try_into()?),
            events,
            object_changes,
            balance_changes,
            timestamp_ms: None,
            confirmed_local_execution: Some(is_executed_locally),
            checkpoint: None,
            errors: vec![],
            raw_effects,
        })
    }

    pub fn prepare_dry_run_transaction_block(
        &self,
        tx_bytes: Base64,
    ) -> Result<(TransactionData, TransactionDigest, Vec<InputObjectKind>), IotaRpcInputError> {
        let tx_data: TransactionData = self.convert_bytes(tx_bytes)?;
        let input_objs = tx_data.input_objects()?;
        let intent_msg = IntentMessage::new(
            Intent {
                version: IntentVersion::V0,
                scope: IntentScope::TransactionData,
                app_id: AppId::Iota,
            },
            tx_data,
        );
        let txn_digest = TransactionDigest::new(default_hash(&intent_msg.value));
        Ok((intent_msg.value, txn_digest, input_objs))
    }

    async fn dry_run_transaction_block(
        &self,
        tx_bytes: Base64,
    ) -> Result<DryRunTransactionBlockResponse, Error> {
        let (txn_data, txn_digest, input_objs) =
            self.prepare_dry_run_transaction_block(tx_bytes)?;
        let sender = txn_data.sender();
        let (resp, written_objects, transaction_effects, mock_gas) = self
            .state
            .dry_exec_transaction(txn_data.clone(), txn_digest)
            .await?;
        let object_cache = ObjectProviderCache::new_with_cache(self.state.clone(), written_objects);
        let balance_changes = get_balance_changes_from_effect(
            &object_cache,
            &transaction_effects,
            input_objs,
            mock_gas,
        )
        .await?;
        let object_changes = get_object_changes(
            &object_cache,
            sender,
            transaction_effects.modified_at_versions(),
            transaction_effects.all_changed_objects(),
            transaction_effects.all_removed_objects(),
        )
        .await?;

        Ok(DryRunTransactionBlockResponse {
            effects: resp.effects,
            events: resp.events,
            object_changes,
            balance_changes,
            input: resp.input,
        })
    }
}

#[async_trait]
impl WriteApiServer for TransactionExecutionApi {
    #[instrument(skip(self))]
    async fn execute_transaction_block(
        &self,
        tx_bytes: Base64,
        signatures: Vec<Base64>,
        opts: Option<IotaTransactionBlockResponseOptions>,
        request_type: Option<ExecuteTransactionRequestType>,
    ) -> RpcResult<IotaTransactionBlockResponse> {
        self.execute_transaction_block(tx_bytes, signatures, opts, request_type)
            .trace_timeout(Duration::from_secs(10))
            .await
    }

    #[instrument(skip(self))]
    async fn dev_inspect_transaction_block(
        &self,
        sender_address: IotaAddress,
        tx_bytes: Base64,
        gas_price: Option<BigInt<u64>>,
        _epoch: Option<BigInt<u64>>,
        additional_args: Option<DevInspectArgs>,
    ) -> RpcResult<DevInspectResults> {
        async move {
            let DevInspectArgs {
                gas_sponsor,
                gas_budget,
                gas_objects,
                show_raw_txn_data_and_effects,
                skip_checks,
            } = additional_args.unwrap_or_default();
            let tx_kind: TransactionKind = self.convert_bytes(tx_bytes)?;
            self.state
                .dev_inspect_transaction_block(
                    sender_address,
                    tx_kind,
                    gas_price.map(|i| *i),
                    gas_budget.map(|i| *i),
                    gas_sponsor,
                    gas_objects,
                    show_raw_txn_data_and_effects,
                    skip_checks,
                )
                .await
                .map_err(Error::from)
        }
        .trace()
        .await
    }

    #[instrument(skip(self))]
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
