// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{cell::RefCell, collections::HashSet, path::PathBuf, rc::Rc, sync::Arc};

use iota_adapter_vm2::{
    adapter::{new_move_runtime, run_metered_move_bytecode_verifier},
    compat::ExecutionOrEarlyError,
    execution_engine::{
        ExecutionOutput, authenticate_then_execute_transaction_to_effects,
        authenticate_transaction, execute_genesis_state_update, execute_transaction_to_effects,
    },
    execution_mode,
    type_layout_resolver::TypeLayoutResolver,
};
use iota_move_natives_vm2::all_natives;
use iota_protocol_config::ProtocolConfig;
use iota_sdk_types::{
    Address, GasPayment, MoveAuthenticator, ProgrammableTransaction, TransactionDigest,
    TransactionEffects, TransactionKind,
};
use iota_types::{
    account_abstraction::authenticator_function::{
        AuthenticatorFunctionRef, MoveAuthenticatorsForExecution,
    },
    auth_context::AuthContextData,
    base_types::TxContext,
    committee::EpochId,
    error::{ExecutionError, ExecutionErrorKind, IotaError, IotaResult},
    execution::{
        AuthenticateThenExecuteOutput, ExecutionResult, ExecutionTiming, TypeLayoutStore,
        is_certificate_denied,
    },
    gas::IotaGasStatus,
    inner_temporary_store::InnerTemporaryStore,
    layout_resolver::LayoutResolver,
    metrics::{BytecodeVerifierMetrics, LimitsMetrics},
    move_package::ProtocolBuildConfig,
    storage::BackingStore,
    transaction::CheckedInputObjects,
};
use iota_verifier_vm2::meter::IotaVerifierMeter;
use move_binary_format::CompiledModule;
use move_bytecode_verifier_meter::Meter;
use move_trace_format::format::MoveTraceBuilder;
use move_vm_config::verifier::{MeterConfig, VerifierConfig};
use move_vm_runtime_vm2::runtime::MoveRuntime;

use crate::{executor, verifier};

pub(crate) struct Executor {
    runtime: Arc<MoveRuntime>,
    protocol_config: ProtocolConfig,
}

pub(crate) struct Verifier<'m> {
    config: VerifierConfig,
    metrics: &'m Arc<BytecodeVerifierMetrics>,
}

impl Executor {
    /// The vm2 runtime has no gas profiler, so `enable_profiler` is accepted for
    /// interface compatibility only.
    pub(crate) fn new(
        protocol_config: &ProtocolConfig,
        silent: bool,
        _enable_profiler: Option<PathBuf>,
    ) -> Result<Self, IotaError> {
        Ok(Executor {
            runtime: Arc::new(new_move_runtime(
                all_natives(silent, protocol_config),
                protocol_config,
            )?),
            protocol_config: protocol_config.clone(),
        })
    }
}

impl<'m> Verifier<'m> {
    pub(crate) fn new(config: VerifierConfig, metrics: &'m Arc<BytecodeVerifierMetrics>) -> Self {
        Verifier { config, metrics }
    }
}

/// Fails the transaction up front when its certificate is denied, either by the
/// node's deny set or by the hard-coded list in `iota-types`.
fn execution_params(
    certificate_deny_set: &HashSet<TransactionDigest>,
    transaction_digest: &TransactionDigest,
) -> ExecutionOrEarlyError {
    if is_certificate_denied(transaction_digest, certificate_deny_set) {
        ExecutionOrEarlyError::early_error(ExecutionErrorKind::CertificateDenied)
    } else {
        ExecutionOrEarlyError::ok()
    }
}

/// Move account authentication is not ported to this cut yet, so every
/// transaction carrying Move authenticators fails up front with this error.
type EffectsOutput = (
    InnerTemporaryStore,
    IotaGasStatus,
    TransactionEffects,
    Vec<ExecutionTiming>,
    Result<(), ExecutionError>,
);

fn into_tuple(output: ExecutionOutput<execution_mode::Normal<ExecutionError>>) -> EffectsOutput {
    let ExecutionOutput {
        inner_store,
        gas_status,
        effects,
        timings,
        execution_result,
    } = output;
    (inner_store, gas_status, effects, timings, execution_result)
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
        gas_data: GasPayment,
        gas_status: IotaGasStatus,
        transaction_kind: TransactionKind,
        transaction_signer: Address,
        transaction_digest: TransactionDigest,
        trace_builder_opt: &mut Option<MoveTraceBuilder>,
    ) -> EffectsOutput {
        let params = execution_params(certificate_deny_set, &transaction_digest);
        into_tuple(execute_transaction_to_effects::<
            execution_mode::Normal<ExecutionError>,
        >(
            store,
            input_objects,
            gas_data,
            gas_status,
            transaction_kind,
            transaction_signer,
            transaction_digest,
            &self.runtime,
            epoch_id,
            epoch_timestamp_ms,
            protocol_config,
            metrics,
            enable_expensive_checks,
            params,
            trace_builder_opt,
        ))
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
        gas_data: GasPayment,
        gas_status: IotaGasStatus,
        transaction_kind: TransactionKind,
        transaction_signer: Address,
        transaction_digest: TransactionDigest,
        skip_all_checks: bool,
    ) -> (
        InnerTemporaryStore,
        IotaGasStatus,
        TransactionEffects,
        Result<Vec<ExecutionResult>, ExecutionError>,
    ) {
        let params = execution_params(certificate_deny_set, &transaction_digest);
        fn without_timings<const SKIP_ALL_CHECKS: bool>(
            output: ExecutionOutput<execution_mode::DevInspect<SKIP_ALL_CHECKS>>,
        ) -> (
            InnerTemporaryStore,
            IotaGasStatus,
            TransactionEffects,
            Result<Vec<ExecutionResult>, ExecutionError>,
        ) {
            let ExecutionOutput {
                inner_store,
                gas_status,
                effects,
                timings: _,
                execution_result,
            } = output;
            (inner_store, gas_status, effects, execution_result)
        }
        if skip_all_checks {
            without_timings(execute_transaction_to_effects::<
                execution_mode::DevInspect<true>,
            >(
                store,
                input_objects,
                gas_data,
                gas_status,
                transaction_kind,
                transaction_signer,
                transaction_digest,
                &self.runtime,
                epoch_id,
                epoch_timestamp_ms,
                protocol_config,
                metrics,
                enable_expensive_checks,
                params,
                &mut None,
            ))
        } else {
            without_timings(execute_transaction_to_effects::<
                execution_mode::DevInspect<false>,
            >(
                store,
                input_objects,
                gas_data,
                gas_status,
                transaction_kind,
                transaction_signer,
                transaction_digest,
                &self.runtime,
                epoch_id,
                epoch_timestamp_ms,
                protocol_config,
                metrics,
                enable_expensive_checks,
                params,
                &mut None,
            ))
        }
    }

    fn authenticate_then_execute_transaction_to_effects(
        &self,
        store: &dyn BackingStore,
        protocol_config: &ProtocolConfig,
        metrics: Arc<LimitsMetrics>,
        enable_expensive_checks: bool,
        certificate_deny_set: &HashSet<TransactionDigest>,
        epoch_id: &EpochId,
        epoch_timestamp_ms: u64,
        gas_data: GasPayment,
        gas_status: IotaGasStatus,
        authenticators: MoveAuthenticatorsForExecution,
        authenticator_and_transaction_input_objects: CheckedInputObjects,
        gas_owner_pays: Option<&dyn Fn(&ExecutionErrorKind) -> bool>,
        transaction_kind: TransactionKind,
        transaction_signer: Address,
        transaction_digest: TransactionDigest,
        auth_context_data: AuthContextData,
        trace_builder_opt: &mut Option<MoveTraceBuilder>,
    ) -> AuthenticateThenExecuteOutput<()> {
        let params = execution_params(certificate_deny_set, &transaction_digest);
        let (output, authentication_error) = authenticate_then_execute_transaction_to_effects::<
            execution_mode::Normal<ExecutionError>,
        >(
            store,
            protocol_config,
            metrics,
            enable_expensive_checks,
            params,
            epoch_id,
            epoch_timestamp_ms,
            gas_data,
            gas_status,
            authenticators,
            authenticator_and_transaction_input_objects,
            gas_owner_pays,
            transaction_kind,
            transaction_signer,
            transaction_digest,
            auth_context_data,
            trace_builder_opt,
            &self.runtime,
        );
        let (inner_store, gas_status, effects, timings, execution_result) = into_tuple(output);
        (
            inner_store,
            gas_status,
            effects,
            timings,
            execution_result,
            authentication_error,
        )
    }

    fn authenticate_transaction(
        &self,
        store: &dyn BackingStore,
        protocol_config: &ProtocolConfig,
        metrics: Arc<LimitsMetrics>,
        epoch_id: &EpochId,
        epoch_timestamp_ms: u64,
        gas_data: GasPayment,
        gas_status: IotaGasStatus,
        move_authenticators: Vec<(
            MoveAuthenticator,
            AuthenticatorFunctionRef,
            CheckedInputObjects,
        )>,
        aggregated_authenticator_input_objects: CheckedInputObjects,
        authenticated_transaction_kind: TransactionKind,
        authenticated_transaction_signer: Address,
        authenticated_transaction_digest: TransactionDigest,
        auth_context_data: AuthContextData,
        trace_builder_opt: &mut Option<MoveTraceBuilder>,
    ) -> (IotaGasStatus, Result<(), ExecutionError>) {
        authenticate_transaction(
            store,
            protocol_config,
            metrics,
            epoch_id,
            epoch_timestamp_ms,
            gas_data,
            gas_status,
            move_authenticators,
            aggregated_authenticator_input_objects,
            authenticated_transaction_kind,
            authenticated_transaction_signer,
            authenticated_transaction_digest,
            auth_context_data,
            trace_builder_opt,
            &self.runtime,
        )
    }

    fn update_genesis_state(
        &self,
        store: &dyn BackingStore,
        protocol_config: &ProtocolConfig,
        metrics: Arc<LimitsMetrics>,
        tx_context: Rc<RefCell<TxContext>>,
        input_objects: CheckedInputObjects,
        pt: ProgrammableTransaction,
    ) -> Result<InnerTemporaryStore, ExecutionError> {
        execute_genesis_state_update(
            store,
            protocol_config,
            metrics,
            &self.runtime,
            tx_context,
            input_objects,
            pt,
        )
    }

    fn type_layout_resolver<'r, 'vm: 'r, 'store: 'r>(
        &'vm self,
        store: Box<dyn TypeLayoutStore + 'store>,
    ) -> Box<dyn LayoutResolver + 'r> {
        Box::new(TypeLayoutResolver::new(
            &self.runtime,
            &self.protocol_config,
            store,
        ))
    }
}

impl verifier::Verifier for Verifier<'_> {
    fn meter(&self, config: MeterConfig) -> Box<dyn Meter> {
        Box::new(IotaVerifierMeter::new(config))
    }

    fn deprecate_global_storage_ops(&self) -> bool {
        self.config.deprecate_global_storage_ops
    }

    fn meter_compiled_modules(
        &mut self,
        protocol_config: &ProtocolConfig,
        modules: &[CompiledModule],
        meter: &mut dyn Meter,
    ) -> IotaResult<()> {
        run_metered_move_bytecode_verifier(
            modules,
            &self.config,
            meter,
            self.metrics,
            &ProtocolBuildConfig::from(protocol_config),
        )
    }
}
