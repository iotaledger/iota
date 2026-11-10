// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

pub use checked::*;
#[iota_macros::with_checked_arithmetic]
mod checked {
    use std::{cell::RefCell, collections::BTreeMap, rc::Rc, sync::Arc};

    use anyhow::Result;
    use iota_common::debug_fatal;
    use iota_move_natives::{
        NativesCostTable, authentication_context::AuthenticationContext, object_runtime,
        object_runtime::ObjectRuntime, transaction_context::TransactionContext,
    };
    use iota_protocol_config::ProtocolConfig;
    use iota_sdk_types::ObjectId;
    use iota_types::{
        auth_context::AuthContext,
        base_types::*,
        error::{ExecutionError, IotaError},
        execution_config_utils::to_binary_config,
        metrics::{BytecodeVerifierMetrics, LimitsMetrics},
        move_package::ProtocolBuildConfig,
        storage::ChildObjectResolver,
    };
    use iota_verifier::{
        check_for_verifier_timeout, verifier::iota_verify_module_metered_check_timeout_only,
    };
    use move_binary_format::file_format::CompiledModule;
    use move_bytecode_verifier::verify_module_with_config_metered;
    use move_bytecode_verifier_meter::{Meter, Scope};
    use move_core_types::account_address::AccountAddress;
    use move_vm_config::{
        runtime::{VMConfig, VMRuntimeLimitsConfig},
        verifier::VerifierConfig,
    };
    use move_vm_runtime::{
        natives::{
            extensions::NativeExtensions,
            functions::{NativeFunctionTable, NativeFunctions},
        },
        runtime::MoveRuntime,
    };
    use tracing::instrument;

    use crate::compat::{
        IotaErrorKind, PACKAGE_ARENA_SIZE_IN_BYTES, execution_status::ExecutionErrorKind,
    };

    pub fn new_move_runtime(
        natives: NativeFunctionTable,
        protocol_config: &ProtocolConfig,
    ) -> Result<MoveRuntime, IotaError> {
        let native_functions = NativeFunctions::new(natives)
            .map_err(|_| IotaErrorKind::ExecutionInvariantViolation)?;
        Ok(MoveRuntime::new(
            native_functions,
            vm_config(protocol_config),
        ))
    }

    pub fn vm_config(protocol_config: &ProtocolConfig) -> VMConfig {
        VMConfig {
            verifier: protocol_config.verifier_config(/* signing_limits */ None),
            max_binary_format_version: protocol_config.move_binary_format_version(),
            runtime_limits_config: VMRuntimeLimitsConfig {
                vector_len_max: protocol_config.max_move_vector_len(),
                max_value_nest_depth: protocol_config.max_move_value_depth_as_option(),
                hardened_otw_check: protocol_config.hardened_otw_check(),
                package_arena_size: Some(PACKAGE_ARENA_SIZE_IN_BYTES),
            },
            enable_invariant_violation_check_in_swap_loc: !protocol_config
                .disable_invariant_violation_check_in_swap_loc(),
            check_no_extraneous_bytes_during_deserialization: protocol_config
                .no_extraneous_module_bytes(),
            // Don't augment errors with execution state on-chain
            error_execution_state: false,
            binary_config: to_binary_config(protocol_config, None),
            rethrow_serialization_type_layout_errors: protocol_config
                .rethrow_serialization_type_layout_errors(),
            max_type_to_layout_nodes: protocol_config.max_type_to_layout_nodes_as_option(),
            variant_nodes: protocol_config.variant_nodes(),
            deprecate_global_storage_ops_during_deserialization: protocol_config
                .deprecate_global_storage_ops(),
            normalize_depth_formula: true,
            charge_ld_const_abstract_size: false,
            profiler_config: None,
        }
    }

    /// Creates a new set of `NativeContextExtensions`.
    ///
    /// Configuring extensions such as `ObjectRuntime` and
    /// `NativesCostTable`. These extensions manage object resolution, input
    /// objects, metering, protocol configuration, and metrics tracking.
    /// They are available and mainly used in native function implementations
    /// via `NativeContext` instance.
    pub fn new_native_extensions<'r>(
        child_resolver: &'r dyn ChildObjectResolver,
        input_objects: BTreeMap<ObjectId, object_runtime::InputObject>,
        is_metered: bool,
        protocol_config: &'r ProtocolConfig,
        metrics: Arc<LimitsMetrics>,
        tx_context: Rc<RefCell<TxContext>>,
        auth_context: Option<Rc<RefCell<AuthContext>>>,
    ) -> Result<NativeExtensions<'r>, ExecutionError> {
        let current_epoch_id: EpochId = tx_context.borrow().epoch();
        let extensions = NativeExtensions::default();
        let mut exts = extensions.try_borrow_mut().map_err(|_| {
            make_invariant_violation!(
                "Failed to mutably borrow native extensions to populate them right after creating them"
            )
        })?;
        exts.add(ObjectRuntime::new(
            child_resolver,
            input_objects,
            is_metered,
            protocol_config,
            metrics,
            current_epoch_id,
        ));
        exts.add(NativesCostTable::from_protocol_config(protocol_config));
        exts.add(TransactionContext::new(tx_context));
        if let Some(auth_context) = auth_context {
            exts.add(AuthenticationContext::new(auth_context));
        }
        drop(exts);
        Ok(extensions)
    }

    /// Given a list of `modules` and an `object_id`, mutate each module's self
    /// ID (which must be 0x0) to be `object_id`.
    pub fn substitute_package_id(
        modules: &mut [CompiledModule],
        object_id: ObjectId,
    ) -> Result<(), ExecutionError> {
        let new_address = AccountAddress::new(object_id.into_bytes());

        for module in modules.iter_mut() {
            let self_handle = module.self_handle().clone();
            let self_address_idx = self_handle.address;

            let addrs = &mut module.address_identifiers;
            let Some(address_mut) = addrs.get_mut(self_address_idx.0 as usize) else {
                let name = module.identifier_at(self_handle.name);
                return Err(ExecutionError::new_with_source(
                    ExecutionErrorKind::PublishErrorNonZeroAddress,
                    format!("Publishing module {name} with invalid address index"),
                ));
            };

            if *address_mut != AccountAddress::ZERO {
                let name = module.identifier_at(self_handle.name);
                return Err(ExecutionError::new_with_source(
                    ExecutionErrorKind::PublishErrorNonZeroAddress,
                    format!("Publishing module {name} with non-zero address is not allowed"),
                ));
            };

            *address_mut = new_address;
        }

        Ok(())
    }

    /// Run the bytecode verifier with a meter limit
    ///
    /// This function only fails if the verification does not complete within
    /// the limit.  If the modules fail to verify but verification completes
    /// within the meter limit, the function succeeds.
    #[instrument(level = "trace", skip_all)]
    pub fn run_metered_move_bytecode_verifier(
        modules: &[CompiledModule],
        verifier_config: &VerifierConfig,
        meter: &mut (impl Meter + ?Sized),
        metrics: &Arc<BytecodeVerifierMetrics>,
        protocol_build_config: &ProtocolBuildConfig,
    ) -> Result<(), IotaError> {
        // run the Move verifier
        for module in modules.iter() {
            let per_module_meter_verifier_timer = metrics
                .verifier_runtime_per_module_success_latency
                .start_timer();

            if let Err(e) =
                verify_module_timeout_only(module, verifier_config, meter, protocol_build_config)
            {
                // We only checked that the failure was due to timeout
                // Discard success timer, but record timeout/failure timer
                metrics
                    .verifier_runtime_per_module_timeout_latency
                    .observe(per_module_meter_verifier_timer.stop_and_discard());
                metrics
                    .verifier_timeout_metrics
                    .with_label_values(&[
                        BytecodeVerifierMetrics::OVERALL_TAG,
                        BytecodeVerifierMetrics::TIMEOUT_TAG,
                    ])
                    .inc();

                return Err(e);
            };

            // Save the success timer
            per_module_meter_verifier_timer.stop_and_record();
            metrics
                .verifier_timeout_metrics
                .with_label_values(&[
                    BytecodeVerifierMetrics::OVERALL_TAG,
                    BytecodeVerifierMetrics::SUCCESS_TAG,
                ])
                .inc();
        }

        Ok(())
    }

    /// Run both the Move verifier and the IOTA verifier, checking just for timeouts. Returns Ok(())
    /// if the verifier completes within the module meter limit and the ticks are successfully
    /// transferred to the package limit (regardless of whether verification succeeds or not).
    fn verify_module_timeout_only(
        module: &CompiledModule,
        verifier_config: &VerifierConfig,
        meter: &mut (impl Meter + ?Sized),
        protocol_build_config: &ProtocolBuildConfig,
    ) -> Result<(), IotaError> {
        meter.enter_scope(module.self_id().name().as_str(), Scope::Module);

        if let Err(e) = verify_module_with_config_metered(verifier_config, module, meter) {
            // Check that the status indicates metering timeout. Under the old vector borrow rule
            // this status is left to execution.
            if check_for_verifier_timeout(&e.major_status())
                && (verifier_config.deprecate_global_storage_ops
                    || e.major_status()
                        != move_core_types::vm_status::StatusCode::VEC_BORROW_ELEMENT_EXISTS_MUTABLE_BORROW_ERROR)
            {
                if e.major_status()
                    == move_core_types::vm_status::StatusCode::REFERENCE_SAFETY_INCONSISTENT
                {
                    let mut bytes = vec![];
                    let _ = module.serialize_with_version(
                        move_binary_format::file_format_common::VERSION_MAX,
                        &mut bytes,
                    );
                    debug_fatal!(
                        "Reference safety inconsistency detected in module: {:?}",
                        bytes
                    );
                }
                return Err(IotaErrorKind::ModuleVerificationFailure {
                    error: format!("Verification timed out: {e}"),
                });
            }
        } else if let Err(err) = iota_verify_module_metered_check_timeout_only(
            module,
            &BTreeMap::new(),
            meter,
            protocol_build_config,
        ) {
            return Err(err.into());
        }

        if meter.transfer(Scope::Module, Scope::Package, 1.0).is_err() {
            return Err(IotaErrorKind::ModuleVerificationFailure {
                error: "Verification timed out".to_string(),
            });
        }

        Ok(())
    }
}
