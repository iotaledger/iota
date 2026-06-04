// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! [`LocalVm`] — the public decode -> store -> execute -> introspect executor.
//!
//! `LocalVm` owns a [`Store`] and a Move execution engine configured for the
//! chain described by a [`ChainContext`]. Each [`LocalVm::execute`] /
//! [`LocalVm::execute_signed`] call runs a transaction in one of three
//! [`ExecutionMode`]s; `DevInspect`/`DryRun` leave the store untouched, while
//! `Execute` commits writes/deletions on success.

use std::sync::Arc;

use iota_protocol_config::ProtocolConfig;
use iota_types::{
    effects::{TransactionEffectsAPI, TransactionEvents},
    metrics::{BytecodeVerifierMetrics, LimitsMetrics},
    signature::VerifyParams,
    signature_verification::verify_sender_signed_data_message_signatures,
    transaction::{SenderSignedData, TransactionData},
    transaction_executor::SimulateTransactionResult,
};
use move_trace_format::format::MoveTraceBuilder;

use super::{
    env::{ExecutionEnv, build_executor, new_bytecode_verifier_metrics, new_limits_metrics},
    prepare::{
        decode_one_event, execute_prepared, execute_with_move_authenticator, prepare_transaction,
    },
    types::{
        ChainContext, DecodedEvent, ExecuteOptions, ExecutionMode, ExecutionResult, GasEstimate,
        SignatureStatus,
    },
};
use crate::{
    debug::{DebugArtifacts, DebugConfig},
    error::VmSdkError,
    store::{Store, StoreBackend},
};

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
}

impl LocalVm {
    /// Build a `LocalVm` for the given chain context, taking ownership of the
    /// store.
    pub fn new(ctx: ChainContext, store: impl Store + 'static) -> Result<Self, VmSdkError> {
        let protocol_config = ProtocolConfig::get_for_version(ctx.protocol_version, ctx.chain);
        Ok(Self {
            protocol_config,
            reference_gas_price: ctx.reference_gas_price,
            epoch_id: ctx.epoch_id,
            epoch_timestamp_ms: ctx.epoch_timestamp_ms,
            limits_metrics: Arc::new(new_limits_metrics()),
            bytecode_verifier_metrics: Arc::new(new_bytecode_verifier_metrics()),
            store: Box::new(store),
        })
    }

    /// A mutable reference to the underlying store, for inserting objects
    /// before a run.
    pub fn store_mut(&mut self) -> &mut dyn Store {
        self.store.as_mut()
    }

    /// Run an unsigned transaction.
    pub fn execute(
        &mut self,
        tx: TransactionData,
        opts: ExecuteOptions,
    ) -> Result<ExecutionResult, VmSdkError> {
        let env = ExecutionEnv::new(self, &opts.debug)?;
        let prepared = {
            let backend = StoreBackend::new(self.store.as_ref());
            prepare_transaction(&env, &backend, tx, opts.mode, 0)?
        };
        // The plain dev_inspect path does not accept a trace builder (only the
        // authenticator path does), so no trace is captured here.
        let trace_builder = env.trace_enabled().then(MoveTraceBuilder::new);
        let sim = {
            let backend = StoreBackend::new(self.store.as_ref());
            execute_prepared(&env, &backend, prepared, opts.mode)?
        };
        let artifacts = env.collect_artifacts(trace_builder);
        self.finish(sim, opts.mode, SignatureStatus::NotChecked, artifacts)
    }

    /// Run a signed transaction, verifying signatures first.
    ///
    /// Standard schemes are verified cryptographically before execution; a
    /// [`MoveAuthenticator`](iota_types::move_authenticator::MoveAuthenticator)
    /// is verified by running its function inside the VM during execution.
    pub fn execute_signed(
        &mut self,
        signed: SenderSignedData,
        opts: ExecuteOptions,
    ) -> Result<ExecutionResult, VmSdkError> {
        let env = ExecutionEnv::new(self, &opts.debug)?;

        let verify_params = VerifyParams::default();
        verify_sender_signed_data_message_signatures(&signed, &verify_params)
            .map_err(crate::error::SignatureError::new)?;

        let move_authenticator = signed.sender_move_authenticator().cloned();
        // The auth digests must be computed from the signed data before it is
        // consumed; the `MoveAuthenticator` execution path needs them in its
        // `AuthContextData`.
        let auth_digests = signed
            .compute_auth_digests()
            .map_err(crate::error::SignatureError::new)?;
        let transaction = signed.into_inner().intent_message.value;

        let authenticator_gas_budget = match &move_authenticator {
            Some(_) => self.protocol_config.max_auth_gas(),
            None => 0,
        };

        let prepared = {
            let backend = StoreBackend::new(self.store.as_ref());
            prepare_transaction(
                &env,
                &backend,
                transaction,
                opts.mode,
                authenticator_gas_budget,
            )?
        };
        let mut trace_builder = env.trace_enabled().then(MoveTraceBuilder::new);

        let sim = {
            let backend = StoreBackend::new(self.store.as_ref());
            match move_authenticator {
                Some(authenticator) => execute_with_move_authenticator(
                    &env,
                    &backend,
                    prepared,
                    authenticator,
                    auth_digests,
                    &mut trace_builder,
                )?,
                None => execute_prepared(&env, &backend, prepared, opts.mode)?,
            }
        };
        let artifacts = env.collect_artifacts(trace_builder);

        // Signatures cleared verification above. For a `MoveAuthenticator`, the
        // authenticator function's verdict is the run's overall status, so a
        // successful run means the authenticator accepted.
        let signature_status = if sim.effects.status().is_success() {
            SignatureStatus::Verified
        } else {
            SignatureStatus::Failed(crate::error::SignatureError::new(
                "authenticator function rejected the signature",
            ))
        };
        self.finish(sim, opts.mode, signature_status, artifacts)
    }

    /// Decode a [`TransactionEvents`] payload into fully-annotated
    /// [`DecodedEvent`]s using this VM's type-layout resolver and the store.
    /// One `Result` per event so a single bad event doesn't mask the rest.
    pub fn decode_events(
        &self,
        events: &TransactionEvents,
    ) -> Vec<Result<DecodedEvent, VmSdkError>> {
        // Build a default-config executor purely for its layout resolver.
        let executor = match build_executor(&self.protocol_config, &DebugConfig::default()) {
            Ok(e) => e,
            Err(e) => return vec![Err(e)],
        };
        let backend = StoreBackend::new(self.store.as_ref());
        let mut resolver = executor.type_layout_resolver(Box::new(&backend));
        events
            .0
            .iter()
            .map(|ev| decode_one_event(ev, resolver.as_mut()))
            .collect()
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
        let gas_estimate = GasEstimate::from_summary(&gas_summary);
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
            gas_estimate,
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
