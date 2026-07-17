// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! The [`LocalVm`] executor and its public API.

use std::sync::{Arc, OnceLock};

use iota_execution::Executor;
use iota_protocol_config::ProtocolConfig;
use iota_sdk_types::Address;
use iota_types::{
    effects::{TransactionEffectsAPI, TransactionEvents},
    gas::IotaGasStatus,
    metrics::{BytecodeVerifierMetrics, LimitsMetrics},
    move_authenticator::MoveAuthenticator,
    signature::VerifyParams,
    signature_verification::verify_sender_signed_data_message_signatures,
    transaction::{SenderSignedData, TransactionData, TransactionDataAPI},
    transaction_executor::SimulateTransactionResult,
};
use move_bytecode_utils::{layout::TypeLayoutBuilder, module_cache::GetModule};
use move_core_types::language_storage::ModuleId;
use move_trace_format::format::MoveTraceBuilder;

use crate::{
    debug::{DebugArtifacts, DebugConfig},
    error::{ExecutionError, ValidationError, VmSdkError},
    executor::{
        env::{ExecutionEnv, build_executor, new_bytecode_verifier_metrics, new_limits_metrics},
        prepare::{
            authenticate_only, build_auth_context_data, decode_one_event, execute_prepared,
            execute_with_move_authenticators, prepare_authenticators, prepare_transaction,
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
            limits_metrics: Arc::new(new_limits_metrics()),
            bytecode_verifier_metrics: Arc::new(new_bytecode_verifier_metrics()),
            store: Box::new(store),
            cached_executor: OnceLock::new(),
        })
    }

    /// A shared reference to the underlying store, for read-only lookups.
    pub fn store(&self) -> &dyn Store {
        self.store.as_ref()
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
            prepare_transaction(
                &env,
                &backend,
                tx,
                opts.mode,
                &opts.deny_config,
                &[],
                &[],
                opts.check_coin_deny_list,
            )?
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
        self.verify_standard_signatures(&signed)?;

        let move_authenticators: Vec<_> =
            signed.move_authenticators().into_iter().cloned().collect();
        // The deny checks inspect the signatures (e.g. `move_authenticator_disabled`,
        // deprecated zkLogin), so they must survive `signed` being consumed.
        let tx_signatures = signed.tx_signatures().to_vec();
        // The auth digests must be computed from the signed data before it is
        // consumed; the `MoveAuthenticator` execution path needs them in its
        // `AuthContextData`.
        let auth_digests = signed
            .compute_auth_digests()
            .map_err(VmSdkError::SignatureVerification)?;
        let transaction = signed.into_inner().intent_message.value;

        // A `MoveAuthenticator` on a protocol version that predates Move
        // authentication cannot be run; reject it with a typed error rather
        // than reaching the engine.
        ensure_move_authentication_supported(
            &self.protocol_config,
            !move_authenticators.is_empty(),
        )?;

        let prepared = {
            let backend = StoreBackend::new(self.store.as_ref());
            prepare_transaction(
                &env,
                &backend,
                transaction,
                opts.mode,
                &opts.deny_config,
                &tx_signatures,
                &move_authenticators,
                opts.check_coin_deny_list,
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
                let (sim, authenticator_outcome) = execute_with_move_authenticators(
                    &env,
                    &backend,
                    prepared,
                    move_authenticators,
                    auth_digests,
                    opts.check_coin_deny_list,
                    &mut trace_builder,
                )?;
                (
                    sim,
                    SignatureStatus::from_authentication(authenticator_outcome),
                    trace_builder,
                )
            }
        };
        let artifacts = env.collect_artifacts(trace_builder)?;

        self.finish(sim, opts.mode, signature_status, artifacts)
    }

    /// Check whether the transaction's `MoveAuthenticator`(s) would be admitted
    /// at signing: runs the pre-consensus authenticator set alone — all
    /// authenticators, or only the sponsor's for a sponsored transaction when
    /// the protocol enables `pre_consensus_sponsor_only_move_authentication` —
    /// via
    /// [`authenticate_transaction`](iota_execution::Executor::authenticate_transaction),
    /// metered at the signing gas cap (`max_auth_gas`); commits nothing.
    ///
    /// Complements [`execute_signed`](Self::execute_signed), which models the
    /// post-consensus path and is never capped at `max_auth_gas`; a transaction
    /// is accepted on-chain only if it passes both. See
    /// `docs/execution-model.md` for the full phase/budget mapping.
    ///
    /// Standard-scheme signatures are verified cryptographically first, as at
    /// signing; a transaction with no `MoveAuthenticator` is admitted once they
    /// verify. Deny-list and input policies are not checked.
    ///
    /// # Errors
    ///
    /// Returns [`VmSdkError::SignatureVerification`] for an invalid
    /// standard-scheme signature, [`VmSdkError::UnsupportedProtocolVersion`]
    /// when the protocol version predates Move authentication, or another
    /// [`VmSdkError`] on a VM fault. An authenticator that rejects or exceeds
    /// `max_auth_gas` is reported via the returned
    /// [`SignatureStatus::Failed`], not as an error.
    pub fn check_signing_authentication(
        &self,
        signed: SenderSignedData,
    ) -> Result<SignatureStatus, VmSdkError> {
        self.verify_standard_signatures(&signed)?;

        let move_authenticators: Vec<_> =
            signed.move_authenticators().into_iter().cloned().collect();
        // Without a `MoveAuthenticator` nothing is gas-capped at signing; the
        // standard-scheme signatures verified above are all a validator checks.
        if move_authenticators.is_empty() {
            return Ok(SignatureStatus::Verified);
        }
        ensure_move_authentication_supported(&self.protocol_config, true)?;

        // The subset the node runs before consensus. Each authenticator
        // authorizes a distinct address, so the subset is identified by address.
        let pre_consensus_addresses =
            pre_consensus_authenticator_addresses(&signed, &self.protocol_config);
        let auth_digests = signed
            .compute_auth_digests()
            .map_err(VmSdkError::SignatureVerification)?;
        let transaction = signed.into_inner().intent_message.value;

        let env = ExecutionEnv::new(self, &DebugConfig::default())?;
        let backend = StoreBackend::new(self.store.as_ref());

        // Resolve all authenticators so the auth context carries both the
        // sender's and the sponsor's function refs, matching the node, then run
        // only the pre-consensus subset.
        let prepared_auths = prepare_authenticators(&backend, move_authenticators)?;
        let auth_context_data =
            build_auth_context_data(&transaction, &prepared_auths, auth_digests)?;
        let to_run: Vec<_> = prepared_auths
            .into_iter()
            .filter(|(a, _, _)| pre_consensus_addresses.contains(&a.address()))
            .collect();

        let gas_status = IotaGasStatus::new(
            self.protocol_config.max_auth_gas(),
            transaction.gas_price(),
            self.reference_gas_price,
            &self.protocol_config,
        )
        .map_err(|e| ValidationError::new("signing gas status", e))?;

        let authenticator_outcome = authenticate_only(
            &env,
            &backend,
            &transaction,
            &to_run,
            gas_status,
            auth_context_data,
        )?;
        Ok(SignatureStatus::from_authentication(authenticator_outcome))
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
        // `BoundedVisitor` bounds the deserialized value's depth and
        // allocation, like every node-side decoder of externally-sourced
        // bytes.
        iota_types::object::bounded_visitor::BoundedVisitor::deserialize_value(bytes, &layout)
            .map_err(|e| {
                ExecutionError::new(format!("decode value of type {type_tag}: {e}")).into()
            })
    }

    /// Verify the standard-scheme signatures on `signed`.
    fn verify_standard_signatures(&self, signed: &SenderSignedData) -> Result<(), VmSdkError> {
        // Match the node's verifier, which derives these from the protocol
        // config (see `AuthorityPerEpochStore`); `VerifyParams::default()` would
        // hardcode both off and diverge for passkey-in-multisig / additional
        // multisig checks.
        let verify_params = VerifyParams::new(
            self.protocol_config.accept_passkey_in_multisig(),
            self.protocol_config.additional_multisig_checks(),
        );
        verify_sender_signed_data_message_signatures(signed, &verify_params)
            .map_err(VmSdkError::SignatureVerification)
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

        let committed = matches!(mode, ExecutionMode::Execute) && status.is_success();
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

/// Addresses whose `MoveAuthenticator` the node runs before consensus: all
/// authenticators, or only the sponsor's for a sponsored transaction when the
/// protocol enables `pre_consensus_sponsor_only_move_authentication`. Mirrors
/// the node's `pre_consensus_move_authenticators`.
fn pre_consensus_authenticator_addresses(
    signed: &SenderSignedData,
    protocol_config: &ProtocolConfig,
) -> Vec<Address> {
    let selected: Vec<&MoveAuthenticator> = if protocol_config
        .pre_consensus_sponsor_only_move_authentication()
        && signed.transaction_data().is_sponsored_tx()
    {
        signed.sponsor_move_authenticator().into_iter().collect()
    } else {
        signed.move_authenticators()
    };
    selected.iter().map(|a| a.address()).collect()
}

/// Check that the protocol version supports Move authentication when the
/// transaction carries `MoveAuthenticator`s.
///
/// Support is signalled by `max_auth_gas` being set; it is unset on versions
/// predating Move authentication, where the panicking getter would crash, so a
/// typed [`VmSdkError::UnsupportedProtocolVersion`] is returned instead. When
/// no authenticators are present nothing is read, so any version is accepted.
fn ensure_move_authentication_supported(
    protocol_config: &ProtocolConfig,
    has_authenticators: bool,
) -> Result<(), VmSdkError> {
    if has_authenticators && protocol_config.max_auth_gas_as_option().is_none() {
        return Err(VmSdkError::UnsupportedProtocolVersion {
            version: protocol_config.version,
            feature: Some("MoveAuthenticator signatures"),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use iota_protocol_config::{Chain, MAX_PROTOCOL_VERSION, ProtocolConfig, ProtocolVersion};

    use super::ensure_move_authentication_supported;
    use crate::error::VmSdkError;

    #[test]
    fn move_authentication_support_ignored_without_authenticators() {
        // No authenticators: `max_auth_gas` is never read, so even a version
        // that predates Move authentication is accepted.
        let old = ProtocolConfig::get_for_version(ProtocolVersion::new(1), Chain::Unknown);
        assert!(ensure_move_authentication_supported(&old, false).is_ok());
    }

    #[test]
    fn move_authentication_support_errors_before_move_authentication() {
        // Protocol v1 predates Move authentication, so `max_auth_gas` is unset:
        // an authenticator transaction must surface a typed error, not panic.
        let old = ProtocolConfig::get_for_version(ProtocolVersion::new(1), Chain::Unknown);
        assert!(old.max_auth_gas_as_option().is_none());
        assert!(matches!(
            ensure_move_authentication_supported(&old, true),
            Err(VmSdkError::UnsupportedProtocolVersion {
                feature: Some(_),
                ..
            })
        ));
    }

    #[test]
    fn move_authentication_support_ok_when_configured() {
        // The latest protocol version has Move authentication configured, so an
        // authenticator transaction is accepted.
        let new = ProtocolConfig::get_for_version(
            ProtocolVersion::new(MAX_PROTOCOL_VERSION),
            Chain::Unknown,
        );
        assert!(new.max_auth_gas_as_option().is_some());
        assert!(ensure_move_authentication_supported(&new, true).is_ok());
    }
}
