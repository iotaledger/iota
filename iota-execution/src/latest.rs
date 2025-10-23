// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{collections::HashSet, path::PathBuf, sync::Arc};

use iota_adapter_latest::{
    adapter::{new_move_vm, run_metered_move_bytecode_verifier},
    execution_engine::{
        authenticate_then_execute_transaction_to_effects, authenticate_transaction,
        execute_genesis_state_update, execute_transaction_to_effects,
    },
    execution_mode,
    type_layout_resolver::TypeLayoutResolver,
};
use iota_move_natives_latest::all_natives;
use iota_protocol_config::ProtocolConfig;
use iota_types::{
    account::AuthenticatorInfoV1,
    base_types::{IotaAddress, ObjectRef, TxContext},
    committee::EpochId,
    digests::TransactionDigest,
    effects::TransactionEffects,
    error::{ExecutionError, IotaError, IotaResult},
    execution::{ExecutionResult, TypeLayoutStore},
    gas::IotaGasStatus,
    inner_temporary_store::InnerTemporaryStore,
    layout_resolver::LayoutResolver,
    metrics::{BytecodeVerifierMetrics, LimitsMetrics},
    move_authenticator::MoveAuthenticator,
    storage::BackingStore,
    transaction::{CheckedInputObjects, ProgrammableTransaction, TransactionKind},
};
use iota_verifier_latest::meter::IotaVerifierMeter;
use move_binary_format::CompiledModule;
use move_bytecode_verifier_meter::Meter;
use move_trace_format::format::MoveTraceBuilder;
use move_vm_config::verifier::{MeterConfig, VerifierConfig};
use move_vm_runtime_latest::move_vm::MoveVM;

use crate::{executor, verifier};

pub(crate) struct Executor(Arc<MoveVM>);

pub(crate) struct Verifier<'m> {
    config: VerifierConfig,
    metrics: &'m Arc<BytecodeVerifierMetrics>,
}

impl Executor {
    pub(crate) fn new(
        protocol_config: &ProtocolConfig,
        silent: bool,
        enable_profiler: Option<PathBuf>,
    ) -> Result<Self, IotaError> {
        Ok(Executor(Arc::new(new_move_vm(
            all_natives(silent, protocol_config),
            protocol_config,
            enable_profiler,
        )?)))
    }
}

impl<'m> Verifier<'m> {
    pub(crate) fn new(config: VerifierConfig, metrics: &'m Arc<BytecodeVerifierMetrics>) -> Self {
        Verifier { config, metrics }
    }
}

impl executor::Executor for Executor {
    fn execute_transaction_to_effects(
        &self,
        store: &dyn BackingStore,
        protocol_config: &ProtocolConfig,
        metrics: Arc<LimitsMetrics>,
        enable_expensive_checks: bool,
        certificate_deny_set: &HashSet<TransactionDigest>,
        epoch_id: &EpochId,
        epoch_timestamp_ms: u64,
        input_objects: CheckedInputObjects,
        gas_coins: Vec<ObjectRef>,
        gas_status: IotaGasStatus,
        transaction_kind: TransactionKind,
        transaction_signer: IotaAddress,
        transaction_digest: TransactionDigest,
        trace_builder_opt: &mut Option<MoveTraceBuilder>,
    ) -> (
        InnerTemporaryStore,
        IotaGasStatus,
        TransactionEffects,
        Result<(), ExecutionError>,
    ) {
        execute_transaction_to_effects::<execution_mode::Normal>(
            store,
            input_objects,
            gas_coins,
            gas_status,
            transaction_kind,
            transaction_signer,
            transaction_digest,
            &self.0,
            epoch_id,
            epoch_timestamp_ms,
            protocol_config,
            metrics,
            enable_expensive_checks,
            certificate_deny_set,
            trace_builder_opt,
        )
    }

    fn dev_inspect_transaction(
        &self,
        store: &dyn BackingStore,
        protocol_config: &ProtocolConfig,
        metrics: Arc<LimitsMetrics>,
        enable_expensive_checks: bool,
        certificate_deny_set: &HashSet<TransactionDigest>,
        epoch_id: &EpochId,
        epoch_timestamp_ms: u64,
        input_objects: CheckedInputObjects,
        gas_coins: Vec<ObjectRef>,
        gas_status: IotaGasStatus,
        transaction_kind: TransactionKind,
        transaction_signer: IotaAddress,
        transaction_digest: TransactionDigest,
        skip_all_checks: bool,
    ) -> (
        InnerTemporaryStore,
        IotaGasStatus,
        TransactionEffects,
        Result<Vec<ExecutionResult>, ExecutionError>,
    ) {
        if skip_all_checks {
            execute_transaction_to_effects::<execution_mode::DevInspect<true>>(
                store,
                input_objects,
                gas_coins,
                gas_status,
                transaction_kind,
                transaction_signer,
                transaction_digest,
                &self.0,
                epoch_id,
                epoch_timestamp_ms,
                protocol_config,
                metrics,
                enable_expensive_checks,
                certificate_deny_set,
                &mut None,
            )
        } else {
            execute_transaction_to_effects::<execution_mode::DevInspect<false>>(
                store,
                input_objects,
                gas_coins,
                gas_status,
                transaction_kind,
                transaction_signer,
                transaction_digest,
                &self.0,
                epoch_id,
                epoch_timestamp_ms,
                protocol_config,
                metrics,
                enable_expensive_checks,
                certificate_deny_set,
                &mut None,
            )
        }
    }

    fn authenticate_then_execute_transaction_to_effects(
        &self,
        store: &dyn BackingStore,
        // Configuration
        protocol_config: &ProtocolConfig,
        metrics: Arc<LimitsMetrics>,
        enable_expensive_checks: bool,
        certificate_deny_set: &HashSet<TransactionDigest>,
        // Epoch
        epoch_id: &EpochId,
        epoch_timestamp_ms: u64,
        // Gas related
        gas_status: IotaGasStatus,
        gas_coins: Vec<ObjectRef>,
        // Authenticator
        authenticator: MoveAuthenticator,
        authenticator_info: AuthenticatorInfoV1,
        authenticator_input_objects: CheckedInputObjects,
        authenticator_and_transaction_input_objects: CheckedInputObjects,
        // Transaction
        transaction_kind: TransactionKind,
        transaction_signer: IotaAddress,
        transaction_digest: TransactionDigest,
        // Tracing
        trace_builder_opt: &mut Option<MoveTraceBuilder>,
    ) -> (
        InnerTemporaryStore,
        IotaGasStatus,
        TransactionEffects,
        Result<(), ExecutionError>,
    ) {
        authenticate_then_execute_transaction_to_effects::<execution_mode::Normal>(
            store,
            protocol_config,
            metrics,
            enable_expensive_checks,
            certificate_deny_set,
            epoch_id,
            epoch_timestamp_ms,
            gas_status,
            gas_coins,
            authenticator,
            authenticator_info,
            authenticator_input_objects,
            authenticator_and_transaction_input_objects,
            transaction_kind,
            transaction_signer,
            transaction_digest,
            trace_builder_opt,
            &self.0,
        )
    }

    fn authenticate_transaction(
        &self,
        store: &dyn BackingStore,
        // Configuration
        protocol_config: &ProtocolConfig,
        metrics: Arc<LimitsMetrics>,
        // Epoch
        epoch_id: &EpochId,
        epoch_timestamp_ms: u64,
        // Gas related
        gas_status: IotaGasStatus,
        // Authenticator
        authenticator: MoveAuthenticator,
        authenticator_info: AuthenticatorInfoV1,
        authenticator_input_objects: CheckedInputObjects,
        // Transaction
        authenticated_transaction_kind: TransactionKind,
        authenticated_transaction_signer: IotaAddress,
        authenticated_transaction_digest: TransactionDigest,
        // Tracing
        trace_builder_opt: &mut Option<MoveTraceBuilder>,
    ) -> Result<(), ExecutionError> {
        authenticate_transaction(
            store,
            protocol_config,
            metrics,
            epoch_id,
            epoch_timestamp_ms,
            gas_status,
            authenticator,
            authenticator_info,
            authenticator_input_objects,
            authenticated_transaction_kind,
            authenticated_transaction_signer,
            authenticated_transaction_digest,
            trace_builder_opt,
            &self.0,
        )
    }

    fn update_genesis_state(
        &self,
        store: &dyn BackingStore,
        protocol_config: &ProtocolConfig,
        metrics: Arc<LimitsMetrics>,
        tx_context: &mut TxContext,
        input_objects: CheckedInputObjects,
        pt: ProgrammableTransaction,
    ) -> Result<InnerTemporaryStore, ExecutionError> {
        execute_genesis_state_update(
            store,
            protocol_config,
            metrics,
            &self.0,
            tx_context,
            input_objects,
            pt,
        )
    }

    fn type_layout_resolver<'r, 'vm: 'r, 'store: 'r>(
        &'vm self,
        store: Box<dyn TypeLayoutStore + 'store>,
    ) -> Box<dyn LayoutResolver + 'r> {
        Box::new(TypeLayoutResolver::new(&self.0, store))
    }
}

impl verifier::Verifier for Verifier<'_> {
    fn meter(&self, config: MeterConfig) -> Box<dyn Meter> {
        Box::new(IotaVerifierMeter::new(config))
    }

    fn meter_compiled_modules(
        &mut self,
        _protocol_config: &ProtocolConfig,
        modules: &[CompiledModule],
        meter: &mut dyn Meter,
    ) -> IotaResult<()> {
        run_metered_move_bytecode_verifier(modules, &self.config, meter, self.metrics)
    }
}
