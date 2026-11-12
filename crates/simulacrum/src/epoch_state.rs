// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{collections::HashSet, sync::Arc};

use anyhow::Result;
use iota_config::{
    transaction_deny_config::TransactionDenyConfig, verifier_signing_config::VerifierSigningConfig,
};
use iota_execution::Executor;
use iota_protocol_config::{Chain, ProtocolConfig, ProtocolVersion};
use iota_sdk_types::{Transaction, TransactionEffects};
use iota_transaction_checks::{
    VerifierLimitsSource,
    simulate::{self, SimulationEnv},
};
use iota_types::{
    committee::{Committee, EpochId},
    error::IotaResult,
    gas::IotaGasStatus,
    inner_temporary_store::InnerTemporaryStore,
    iota_system_state::{
        IotaSystemState, IotaSystemStateTrait,
        epoch_start_iota_system_state::{EpochStartSystemState, EpochStartSystemStateTrait},
    },
    metrics::{BytecodeVerifierMetrics, LimitsMetrics},
    transaction::{TransactionAPI, VerifiedTransaction},
    transaction_executor::{InputCheckRules, SimulateTransactionResult, VmChecks},
};

use crate::SimulatorStore;

pub struct EpochState {
    epoch_start_state: EpochStartSystemState,
    committee: Committee,
    protocol_config: ProtocolConfig,
    limits_metrics: Arc<LimitsMetrics>,
    bytecode_verifier_metrics: Arc<BytecodeVerifierMetrics>,
    executor: Arc<dyn Executor + Send + Sync>,
    /// A counter that advances each time we advance the clock in order to
    /// ensure that each update txn has a unique digest. This is reset on
    /// epoch changes
    next_consensus_round: u64,
}

impl EpochState {
    pub fn new(system_state: IotaSystemState) -> Self {
        let epoch_start_state = system_state.into_epoch_start_state();
        let protocol_config =
            ProtocolConfig::get_for_version(epoch_start_state.protocol_version(), Chain::Unknown);
        Self::new_impl(protocol_config, epoch_start_state)
    }

    /// Like [`Self::new`], but reuses the given `protocol_config` instead of
    /// resolving it from the protocol version, keeping any feature flags
    /// customized for testing.
    ///
    /// # Panics
    ///
    /// Panics if the config's version does not match the system state's
    /// protocol version.
    pub fn new_with_config(protocol_config: ProtocolConfig, system_state: IotaSystemState) -> Self {
        let epoch_start_state = system_state.into_epoch_start_state();
        assert_eq!(
            protocol_config.version,
            epoch_start_state.protocol_version()
        );
        Self::new_impl(protocol_config, epoch_start_state)
    }

    fn new_impl(protocol_config: ProtocolConfig, epoch_start_state: EpochStartSystemState) -> Self {
        let committee = epoch_start_state.get_iota_committee();
        let registry = prometheus_filtered::Registry::new();
        let limits_metrics = Arc::new(LimitsMetrics::new(&registry));
        let bytecode_verifier_metrics = Arc::new(BytecodeVerifierMetrics::new(&registry));
        let executor = iota_execution::executor(&protocol_config, true, None).unwrap();

        Self {
            epoch_start_state,
            committee,
            protocol_config,
            limits_metrics,
            bytecode_verifier_metrics,
            executor,
            next_consensus_round: 0,
        }
    }

    pub fn epoch(&self) -> EpochId {
        self.epoch_start_state.epoch()
    }

    pub fn reference_gas_price(&self) -> u64 {
        self.epoch_start_state.reference_gas_price()
    }

    pub fn next_consensus_round(&mut self) -> u64 {
        let round = self.next_consensus_round;
        self.next_consensus_round += 1;
        round
    }

    pub fn committee(&self) -> &Committee {
        &self.committee
    }

    pub fn epoch_start_state(&self) -> EpochStartSystemState {
        self.epoch_start_state.clone()
    }

    pub fn protocol_version(&self) -> ProtocolVersion {
        self.protocol_config().version
    }

    pub fn protocol_config(&self) -> &ProtocolConfig {
        &self.protocol_config
    }

    pub fn execute_transaction(
        &self,
        store: &dyn SimulatorStore,
        deny_config: &TransactionDenyConfig,
        verifier_signing_config: &VerifierSigningConfig,
        transaction: &VerifiedTransaction,
    ) -> Result<(
        InnerTemporaryStore,
        IotaGasStatus,
        TransactionEffects,
        Result<(), iota_types::error::ExecutionError>,
    )> {
        let tx_digest = *transaction.digest();
        let tx = transaction.data().transaction();
        let input_object_kinds = tx.input_objects()?;
        let receiving_object_refs = tx.receiving_objects();

        iota_transaction_checks::deny::check_transaction_for_validation(
            tx,
            transaction.signatures(),
            &input_object_kinds,
            &receiving_object_refs,
            deny_config,
            &store,
        )?;

        let (input_objects, receiving_objects) = store.read_objects_for_synchronous_execution(
            &tx_digest,
            &input_object_kinds,
            &receiving_object_refs,
        )?;

        // `MoveAuthenticator`s are not supported in Simulacrum, so we set the
        // `authenticator_gas_budget` to 0.
        let authenticator_gas_budget = 0;

        // Run the transaction input checks that would run when submitting the txn to a
        // validator for signing
        let (gas_status, checked_input_objects) = iota_transaction_checks::check_transaction_input(
            &self.protocol_config,
            self.epoch_start_state.reference_gas_price(),
            transaction.data().transaction(),
            input_objects,
            &receiving_objects,
            &self.bytecode_verifier_metrics,
            VerifierLimitsSource::NodeConfig(verifier_signing_config),
            authenticator_gas_budget,
            false,
            InputCheckRules::STRICT,
        )?;

        let transaction = transaction.data().transaction();
        let (kind, signer, gas_data) = transaction.execution_parts();
        let (inner_temp_store, gas_status, effects, _timings, result) =
            self.executor.execute_transaction_to_effects(
                store.backing_store(),
                &self.protocol_config,
                self.limits_metrics.clone(),
                false,           // enable_expensive_checks
                &HashSet::new(), // certificate_deny_set
                &self.epoch_start_state.epoch(),
                self.epoch_start_state.epoch_start_timestamp_ms(),
                checked_input_objects,
                gas_data,
                gas_status,
                kind,
                signer,
                tx_digest,
                &mut None,
            );
        Ok((inner_temp_store, gas_status, effects, result))
    }

    /// Simulate a transaction without committing changes.
    /// This is similar to execute_transaction but:
    /// - Takes Transaction instead of VerifiedTransaction (no signature required)
    /// - Takes VmChecks parameter to control validation strictness
    /// - Returns SimulateTransactionResult with input/output objects
    /// - Creates a mock gas object if none provided
    pub fn simulate_transaction(
        &self,
        store: &dyn SimulatorStore,
        deny_config: &TransactionDenyConfig,
        verifier_signing_config: &VerifierSigningConfig,
        transaction: Transaction,
        checks: VmChecks,
    ) -> IotaResult<SimulateTransactionResult> {
        let tx_digest = transaction.digest();
        let env = SimulationEnv {
            protocol_config: &self.protocol_config,
            reference_gas_price: self.epoch_start_state.reference_gas_price(),
            epoch_id: self.epoch_start_state.epoch(),
            epoch_timestamp_ms: self.epoch_start_state.epoch_start_timestamp_ms(),
            deny_config,
            certificate_deny_set: &HashSet::new(),
            verifier_signing_config,
            bytecode_verifier_metrics: &self.bytecode_verifier_metrics,
            limits_metrics: &self.limits_metrics,
        };
        let prepared = simulate::prepare_simulation(
            &env,
            store.backing_store(),
            transaction,
            checks,
            &[],
            true,
            |input_object_kinds, receiving_object_refs| {
                store.read_objects_for_synchronous_execution(
                    &tx_digest,
                    input_object_kinds,
                    receiving_object_refs,
                )
            },
        )?;
        Ok(simulate::execute_simulation(
            &env,
            self.executor.as_ref(),
            store.backing_store(),
            prepared,
        )
        .0)
    }
}
