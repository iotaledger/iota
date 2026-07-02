// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! [`LocalVm`] — the public store -> execute -> inspect executor.
//!
//! `LocalVm` owns a [`Store`] and a Move execution engine configured for the
//! chain described by a [`ChainContext`]. Each [`LocalVm::execute`] /
//! [`LocalVm::execute_signed`] call runs a transaction in one of three
//! [`ExecutionMode`]s; `DevInspect`/`DryRun` leave the store untouched, while
//! `Execute` commits writes/deletions on success.

use std::sync::{Arc, OnceLock};

use iota_execution::Executor;
use iota_protocol_config::ProtocolConfig;
use iota_types::{
    effects::{TransactionEffectsAPI, TransactionEvents},
    metrics::{BytecodeVerifierMetrics, LimitsMetrics},
    signature::VerifyParams,
    signature_verification::verify_sender_signed_data_message_signatures,
    transaction::{SenderSignedData, TransactionData},
    transaction_executor::SimulateTransactionResult,
};
use move_bytecode_utils::{layout::TypeLayoutBuilder, module_cache::GetModule};
use move_core_types::language_storage::ModuleId;
use move_trace_format::format::MoveTraceBuilder;

use crate::{
    debug::DebugArtifacts,
    error::{ExecutionError, VmSdkError},
    executor::{
        env::{ExecutionEnv, build_executor},
        prepare::{
            decode_one_event, execute_prepared, execute_with_move_authenticators,
            prepare_transaction,
        },
        types::{
            ChainContext, DecodedEvent, ExecuteOptions, ExecutionMode, ExecutionResult,
            SignatureStatus,
        },
    },
    store::{Store, StoreBackend},
};

/// Adapts the SDK [`Store`] to the [`GetModule`] interface that
/// [`TypeLayoutBuilder`] needs to resolve struct layouts from packages.
struct StoreModuleResolver<'a>(StoreBackend<'a>);

impl GetModule for StoreModuleResolver<'_> {
    type Error = iota_types::error::IotaError;
    type Item = move_binary_format::CompiledModule;

    fn get_module_by_id(&self, id: &ModuleId) -> Result<Option<Self::Item>, Self::Error> {
        iota_types::storage::get_module_by_id(&self.0, id)
    }
}

/// The local Move VM executor. Owns a [`Store`] and the execution engine for a
/// single [`ChainContext`].
pub struct LocalVm {
    pub(super) protocol_config: ProtocolConfig,
    pub(super) reference_gas_price: u64,
    pub(super) epoch_id: u64,
    pub(super) epoch_timestamp_ms: u64,
    pub(super) limits_metrics: Arc<LimitsMetrics>,
    pub(super) bytecode_verifier_metrics: Arc<BytecodeVerifierMetrics>,
    store: Box<dyn Store>,
    /// Profiler-free executor shared by `decode_events`'s layout resolver and
    /// all non-profiled `execute*` runs, built lazily on first use. It depends
    /// only on the immutable `protocol_config`, so one instance is reused
    /// across calls; a profiled run builds its own executor via
    /// [`ExecutionEnv`].
    cached_executor: OnceLock<Arc<dyn Executor + Send + Sync>>,
}

impl LocalVm {
    /// Build a `LocalVm` for the given chain context, taking ownership of the
    /// store.
    ///
    /// # Errors
    ///
    /// Returns [`VmSdkError::UnsupportedProtocolVersion`] when this build does
    /// not know the requested protocol version (e.g. the chain context was
    /// fetched from a node running a newer protocol).
    pub fn new(ctx: ChainContext, store: impl Store + 'static) -> Result<Self, VmSdkError> {
        let protocol_config =
            ProtocolConfig::get_for_version_if_supported(ctx.protocol_version, ctx.chain).ok_or(
                VmSdkError::UnsupportedProtocolVersion {
                    version: ctx.protocol_version,
                    feature: None,
                },
            )?;
        Ok(Self {
            protocol_config,
            reference_gas_price: ctx.reference_gas_price,
            epoch_id: ctx.epoch_id,
            epoch_timestamp_ms: ctx.epoch_timestamp_ms,
            limits_metrics: Arc::new(LimitsMetrics::new(&prometheus::Registry::new())),
            bytecode_verifier_metrics: Arc::new(BytecodeVerifierMetrics::new(
                &prometheus::Registry::new(),
            )),
            store: Box::new(store),
            cached_executor: OnceLock::new(),
        })
    }

    /// A mutable reference to the underlying store, for inserting objects
    /// before a run.
    pub fn store_mut(&mut self) -> &mut dyn Store {
        self.store.as_mut()
    }

    /// Run an unsigned transaction.
    ///
    /// No signatures are checked: the result reports
    /// [`SignatureStatus::NotChecked`], and with [`ExecutionMode::Execute`] the
    /// effects are committed to the store regardless of whether the transaction
    /// would be authorized on-chain. Use [`LocalVm::execute_signed`] when
    /// signature verification is required.
    ///
    /// # Errors
    ///
    /// Returns a [`VmSdkError`] on preparation or VM faults; a Move-level abort
    /// is reported via [`ExecutionResult::status`], not as an error.
    pub fn execute(
        &mut self,
        tx: TransactionData,
        opts: ExecuteOptions,
    ) -> Result<ExecutionResult, VmSdkError> {
        let env = ExecutionEnv::new(self, &opts.debug)?;
        let prepared = {
            let backend = StoreBackend::new(self.store.as_ref());
            prepare_transaction(&env, &backend, tx, opts.mode, &opts.deny_config, 0)?
        };
        let sim = {
            let backend = StoreBackend::new(self.store.as_ref());
            execute_prepared(&env, &backend, prepared, opts.mode)?
        };
        // The dev-inspect entry point accepts no `MoveTraceBuilder`, so this path
        // never captures a trace; pass `None`. See `DebugConfig::with_tracing`.
        let artifacts = env.collect_artifacts(None)?;
        self.finish(sim, opts.mode, SignatureStatus::NotChecked, artifacts)
    }

    /// Run a signed transaction, verifying signatures first.
    ///
    /// Standard schemes are verified cryptographically first. Every
    /// [`MoveAuthenticator`](iota_types::move_authenticator::MoveAuthenticator)
    /// — the sender's and, for a sponsored tx, the sponsor's — is verified by
    /// running its function in the VM. On failure the authenticators are re-run
    /// alone to tell a rejection from a body abort.
    ///
    /// `opts.mode` governs input-check relaxation
    /// ([`ExecutionMode::DevInspect`]) and commit
    /// ([`ExecutionMode::Execute`]) as for [`execute`](Self::execute),
    /// but the authenticators and transaction body always execute under full
    /// (non-dev-inspect) VM semantics.
    ///
    /// Signatures are verified against the transaction as supplied. Gas and
    /// owned-input references are then resolved against the store's versions,
    /// so the executed bytes (and digest) can differ from the signed bytes when
    /// the store holds other versions; a Move authenticator reading the
    /// transaction bytes from its auth context observes the resolved form.
    ///
    /// # Errors
    ///
    /// Returns [`VmSdkError::SignatureVerification`] for an invalid
    /// standard-scheme signature, or another [`VmSdkError`] on preparation/VM
    /// faults. A rejected `MoveAuthenticator` is reported via
    /// [`ExecutionResult::signature_status`], not as an error.
    pub fn execute_signed(
        &mut self,
        signed: SenderSignedData,
        opts: ExecuteOptions,
    ) -> Result<ExecutionResult, VmSdkError> {
        let env = ExecutionEnv::new(self, &opts.debug)?;

        // Match the node's verifier, which derives these from the protocol
        // config (see `AuthorityPerEpochStore`); `VerifyParams::default()` would
        // hardcode both off and diverge for passkey-in-multisig / additional
        // multisig checks.
        let verify_params = VerifyParams::new(
            self.protocol_config.accept_passkey_in_multisig(),
            self.protocol_config.additional_multisig_checks(),
        );
        verify_sender_signed_data_message_signatures(&signed, &verify_params)
            .map_err(VmSdkError::SignatureVerification)?;

        // All `MoveAuthenticator`s on the transaction — the sender's and, for a
        // sponsored transaction, the sponsor's — must be verified.
        let move_authenticators: Vec<_> =
            signed.move_authenticators().into_iter().cloned().collect();
        // The auth digests must be computed from the signed data before it is
        // consumed; the `MoveAuthenticator` execution path needs them in its
        // `AuthContextData`.
        let auth_digests = signed
            .compute_auth_digests()
            .map_err(VmSdkError::SignatureVerification)?;
        let transaction = signed.into_inner().intent_message.value;

        let authenticator_gas_budget =
            authenticator_gas_budget(&self.protocol_config, !move_authenticators.is_empty())?;

        let prepared = {
            let backend = StoreBackend::new(self.store.as_ref());
            prepare_transaction(
                &env,
                &backend,
                transaction,
                opts.mode,
                &opts.deny_config,
                authenticator_gas_budget,
            )?
        };
        let (sim, signature_status, trace_builder) = {
            let backend = StoreBackend::new(self.store.as_ref());
            if move_authenticators.is_empty() {
                // Standard schemes were verified cryptographically above; the
                // run's outcome cannot retroactively invalidate them. Runs
                // through the dev-inspect entry point, so no trace is captured.
                (
                    execute_prepared(&env, &backend, prepared, opts.mode)?,
                    SignatureStatus::Verified,
                    None,
                )
            } else {
                // Only the authenticator path threads a `MoveTraceBuilder`
                // through the engine, so a trace is built only here.
                let mut trace_builder = env.trace_enabled().then(MoveTraceBuilder::new);
                let (sim, verdict) = execute_with_move_authenticators(
                    &env,
                    &backend,
                    prepared,
                    move_authenticators,
                    auth_digests,
                    authenticator_gas_budget,
                    &mut trace_builder,
                )?;
                let status = match verdict {
                    Ok(()) => SignatureStatus::Verified,
                    // Carry the authenticator's typed rejection cause;
                    // `SignatureError`'s `Display` adds the "signature
                    // verification failed:" prefix.
                    Err(e) => SignatureStatus::Failed(crate::error::SignatureError::new(e)),
                };
                (sim, status, trace_builder)
            }
        };
        let artifacts = env.collect_artifacts(trace_builder)?;

        self.finish(sim, opts.mode, signature_status, artifacts)
    }

    /// The profiler-free executor shared by `decode_events` and non-profiled
    /// runs, built on first use and cached for the lifetime of this `LocalVm`.
    pub(super) fn cached_executor(&self) -> Result<&Arc<dyn Executor + Send + Sync>, VmSdkError> {
        if let Some(executor) = self.cached_executor.get() {
            return Ok(executor);
        }
        // `OnceLock::get_or_try_init` is unstable, so build first and let the
        // first writer win; a redundant build from a racing caller is dropped.
        let executor = build_executor(&self.protocol_config)?;
        Ok(self.cached_executor.get_or_init(|| executor))
    }

    /// Decode a [`TransactionEvents`] payload into fully-annotated
    /// [`DecodedEvent`]s, in event order, using this VM's type-layout resolver
    /// and the store.
    ///
    /// # Errors
    ///
    /// Returns [`VmSdkError`] when the layout resolver cannot be built or an
    /// event fails to decode (the error names the event's type). To decode
    /// events individually, use [`LocalVm::decode_value`] on an event's
    /// contents and type.
    pub fn decode_events(
        &self,
        events: &TransactionEvents,
    ) -> Result<Vec<DecodedEvent>, VmSdkError> {
        let executor = self.cached_executor()?;
        let backend = StoreBackend::new(self.store.as_ref());
        let mut resolver = executor.type_layout_resolver(Box::new(&backend));
        events
            .0
            .iter()
            .map(|ev| decode_one_event(ev, resolver.as_mut()))
            .collect()
    }

    /// Decode a single BCS-encoded value of the given
    /// [`TypeTag`](iota_sdk_types::TypeTag) into an annotated
    /// [`MoveValue`](move_core_types::annotated_value::MoveValue), resolving
    /// any struct layouts from the packages in the store. Turns raw
    /// `(bytes, type)` pairs — dev-inspect return values and mutable
    /// reference outputs — into readable values.
    ///
    /// # Errors
    ///
    /// Returns [`VmSdkError::Execution`] if the layout can't be resolved or
    /// `bytes` don't deserialize against it.
    pub fn decode_value(
        &self,
        bytes: &[u8],
        type_tag: &iota_sdk_types::TypeTag,
    ) -> Result<move_core_types::annotated_value::MoveValue, VmSdkError> {
        let core_tag = iota_types::iota_sdk_types_conversions::type_tag_sdk_to_core(type_tag);
        let resolver = StoreModuleResolver(StoreBackend::new(self.store.as_ref()));
        let layout = TypeLayoutBuilder::build_with_types(&core_tag, &resolver)
            .map_err(|e| ExecutionError::new(format!("build layout for {type_tag}: {e}")))?;
        move_core_types::annotated_value::MoveValue::simple_deserialize(bytes, &layout).map_err(
            |e| ExecutionError::new(format!("decode value of type {type_tag}: {e}")).into(),
        )
    }

    /// Assemble an [`ExecutionResult`] from a raw simulation, committing the
    /// effects to the store when the mode is [`ExecutionMode::Execute`] and the
    /// run succeeded.
    fn finish(
        &mut self,
        sim: SimulateTransactionResult,
        mode: ExecutionMode,
        signature_status: SignatureStatus,
        artifacts: Option<DebugArtifacts>,
    ) -> Result<ExecutionResult, VmSdkError> {
        let gas_summary = sim.effects.gas_cost_summary().clone();
        let status = sim.effects.status().clone();

        let succeeded = sim.effects.status().is_success();
        let committed = matches!(mode, ExecutionMode::Execute) && succeeded;
        if committed {
            self.apply_effects(&sim);
        }

        Ok(ExecutionResult {
            effects: sim.effects,
            events: sim.events,
            command_results: sim.execution_result.unwrap_or_default(),
            input_objects: sim.input_objects.into_values().collect(),
            output_objects: sim.output_objects.into_values().collect(),
            gas_summary,
            mock_gas_id: sim.mock_gas_id,
            status,
            signature_status,
            committed,
            debug: artifacts,
        })
    }

    /// Apply created/mutated/deleted/wrapped changes back into the store so a
    /// subsequent run sees them.
    fn apply_effects(&mut self, sim: &SimulateTransactionResult) {
        // `output_objects` is authoritative for what survives; then drop what
        // was deleted or wrapped. `unwrapped_then_deleted` objects were nested,
        // never standalone store entries, so they need no removal.
        for obj in sim.output_objects.values() {
            self.store.insert(obj.clone());
        }
        for objref in sim.effects.deleted() {
            self.store.remove(&objref.object_id);
        }
        for objref in sim.effects.wrapped() {
            self.store.remove(&objref.object_id);
        }
        // The one-shot mock gas coin is never persisted.
        if let Some(id) = sim.mock_gas_id {
            self.store.remove(&id);
        }
    }
}

/// Gas budget for verifying the transaction's `MoveAuthenticator`s.
///
/// Returns `0` when none are present. Otherwise reads `max_auth_gas` from the
/// protocol config; that value is unset on versions predating Move
/// authentication, where its panicking getter would crash, so a typed
/// [`VmSdkError::UnsupportedProtocolVersion`] is returned instead.
fn authenticator_gas_budget(
    protocol_config: &ProtocolConfig,
    has_authenticators: bool,
) -> Result<u64, VmSdkError> {
    if !has_authenticators {
        return Ok(0);
    }
    protocol_config
        .max_auth_gas_as_option()
        .ok_or(VmSdkError::UnsupportedProtocolVersion {
            version: protocol_config.version,
            feature: Some("MoveAuthenticator signatures"),
        })
}

#[cfg(test)]
mod tests {
    use iota_protocol_config::{Chain, MAX_PROTOCOL_VERSION, ProtocolConfig, ProtocolVersion};

    use super::authenticator_gas_budget;
    use crate::error::VmSdkError;

    #[test]
    fn authenticator_gas_budget_is_zero_without_authenticators() {
        // No authenticators: no budget is needed and `max_auth_gas` is never
        // read, so even a version that predates it is fine.
        let old = ProtocolConfig::get_for_version(ProtocolVersion::new(1), Chain::Unknown);
        assert_eq!(authenticator_gas_budget(&old, false).unwrap(), 0);
    }

    #[test]
    fn authenticator_gas_budget_errors_before_move_authentication() {
        // Protocol v1 predates Move authentication, so `max_auth_gas` is unset:
        // requesting a budget must surface a typed error, not panic.
        let old = ProtocolConfig::get_for_version(ProtocolVersion::new(1), Chain::Unknown);
        assert!(old.max_auth_gas_as_option().is_none());
        assert!(matches!(
            authenticator_gas_budget(&old, true),
            Err(VmSdkError::UnsupportedProtocolVersion {
                feature: Some(_),
                ..
            })
        ));
    }

    #[test]
    fn authenticator_gas_budget_returns_configured_value() {
        // The latest protocol version has Move authentication configured, so
        // the budget is the value from `max_auth_gas`.
        let new = ProtocolConfig::get_for_version(
            ProtocolVersion::new(MAX_PROTOCOL_VERSION),
            Chain::Unknown,
        );
        let expected = new
            .max_auth_gas_as_option()
            .expect("max_auth_gas is set at the latest protocol version");
        assert_eq!(authenticator_gas_budget(&new, true).unwrap(), expected);
    }
}
