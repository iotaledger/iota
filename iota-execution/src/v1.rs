// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{collections::HashSet, path::PathBuf, sync::Arc};

use move_binary_format::CompiledModule;
use move_bytecode_verifier_v1::meter::Scope;
use move_vm_config::verifier::VerifierConfig;
use move_vm_runtime_v1::move_vm::MoveVM;
use iota_adapter_v1::{
    adapter::{default_verifier_config, new_move_vm, run_metered_move_bytecode_verifier},
    execution_engine::{execute_genesis_state_update, execute_transaction_to_effects},
    type_layout_resolver::TypeLayoutResolver,
};
use iota_move_natives_v1::all_natives;
use iota_protocol_config::ProtocolConfig;
use iota_types::{
    base_types::{ObjectRef, IotaAddress, TxContext},
    committee::EpochId,
    digests::TransactionDigest,
    effects::TransactionEffects,
    error::{ExecutionError, IotaError, IotaResult},
    execution::TypeLayoutStore,
    execution_mode::{self, ExecutionResult},
    gas::IotaGasStatus,
    inner_temporary_store::InnerTemporaryStore,
    metrics::{BytecodeVerifierMetrics, LimitsMetrics},
    storage::BackingStore,
    transaction::{CheckedInputObjects, ProgrammableTransaction, TransactionKind},
    type_resolver::LayoutResolver,
};
use iota_verifier_v1::meter::IotaVerifierMeter;

use crate::{
    executor, verifier,
    verifier::{VerifierMeteredValues, VerifierOverrides},
};

pub(crate) struct Executor(Arc<MoveVM>);

pub(crate) struct Verifier<'m> {
    config: VerifierConfig,
    metrics: &'m Arc<BytecodeVerifierMetrics>,
    meter: IotaVerifierMeter,
}

impl Executor {
    pub(crate) fn new(
        protocol_config: &ProtocolConfig,
        silent: bool,
        enable_profiler: Option<PathBuf>,
    ) -> Result<Self, IotaError> {
        Ok(Executor(Arc::new(new_move_vm(
            all_natives(silent),
            protocol_config,
            enable_profiler,
        )?)))
    }
}

impl<'m> Verifier<'m> {
    pub(crate) fn new(
        protocol_config: &ProtocolConfig,
        is_metered: bool,
        metrics: &'m Arc<BytecodeVerifierMetrics>,
    ) -> Self {
        let config = default_verifier_config(protocol_config, is_metered);
        let meter = IotaVerifierMeter::new(&config);
        Verifier {
            config,
            metrics,
            meter,
        }
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
            )
        }
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

impl<'m> verifier::Verifier for Verifier<'m> {
    fn meter_compiled_modules(
        &mut self,
        _protocol_config: &ProtocolConfig,
        modules: &[CompiledModule],
    ) -> IotaResult<()> {
        run_metered_move_bytecode_verifier(modules, &self.config, &mut self.meter, self.metrics)
    }

    fn meter_compiled_modules_with_overrides(
        &mut self,
        modules: &[CompiledModule],
        _protocol_config: &ProtocolConfig,
        config_overrides: &VerifierOverrides,
    ) -> IotaResult<VerifierMeteredValues> {
        let mut config = self.config.clone();
        let max_per_fun_meter_current = config.max_per_fun_meter_units;
        let max_per_mod_meter_current = config.max_per_mod_meter_units;
        config.max_per_fun_meter_units = config_overrides.max_per_fun_meter_units;
        config.max_per_mod_meter_units = config_overrides.max_per_mod_meter_units;
        run_metered_move_bytecode_verifier(modules, &config, &mut self.meter, self.metrics)?;
        let fun_meter_units_result = self.meter.get_usage(Scope::Function);
        let mod_meter_units_result = self.meter.get_usage(Scope::Module);
        Ok(VerifierMeteredValues::new(
            max_per_fun_meter_current,
            max_per_mod_meter_current,
            fun_meter_units_result,
            mod_meter_units_result,
        ))
    }
}
