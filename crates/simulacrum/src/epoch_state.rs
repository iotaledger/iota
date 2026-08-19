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
use iota_types::{
    committee::{Committee, EpochId},
    effects::TransactionEffectsAPI,
    error::IotaResult,
    gas::IotaGasStatus,
    gas_coin::mock_simulation_gas_coin,
    inner_temporary_store::InnerTemporaryStore,
    iota_system_state::{
        IotaSystemState, IotaSystemStateTrait,
        epoch_start_iota_system_state::{EpochStartSystemState, EpochStartSystemStateTrait},
    },
    metrics::{BytecodeVerifierMetrics, LimitsMetrics},
    transaction::{ObjectReadResult, TransactionAPI, VerifiedTransaction},
    transaction_executor::{SimulateTransactionResult, VmChecks},
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
        let committee = epoch_start_state.get_iota_committee();
        let protocol_config =
            ProtocolConfig::get_for_version(epoch_start_state.protocol_version(), Chain::Unknown);
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
            verifier_signing_config,
            authenticator_gas_budget,
        )?;

        let transaction = transaction.data().transaction();
        let (kind, signer, gas_data) = transaction.execution_parts();
        Ok(self.executor.execute_transaction_to_effects(
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
        ))
    }

    /// Simulate a transaction without committing changes.
    /// This is similar to execute_transaction but:
    /// - Takes Transaction instead of VerifiedTransaction (no signature
    ///   required)
    /// - Takes VmChecks parameter to control validation strictness
    /// - Returns SimulateTransactionResult with input/output objects
    /// - Creates a mock gas object if none provided
    pub fn simulate_transaction(
        &self,
        store: &dyn SimulatorStore,
        deny_config: &TransactionDenyConfig,
        verifier_signing_config: &VerifierSigningConfig,
        mut transaction: Transaction,
        checks: VmChecks,
    ) -> IotaResult<SimulateTransactionResult> {
        // Cheap validity checks for a transaction, including input size limits.
        transaction.validity_check_no_gas_check(&self.protocol_config)?;

        // The full validity check caps the gas payment size alongside requiring a
        // gas payment at all, which a simulation relaxes so it can mock one. The cap
        // still applies, and is cheapest before any object is loaded.
        transaction.check_gas_payment_size(&self.protocol_config)?;

        let input_object_kinds = transaction.input_objects()?;
        let receiving_object_refs = transaction.receiving_objects();

        // Check if some transaction elements are denied
        iota_transaction_checks::deny::check_transaction_for_validation(
            &transaction,
            &[],
            &input_object_kinds,
            &receiving_object_refs,
            deny_config,
            store,
        )?;

        // Load input and receiving objects
        let (mut input_objects, receiving_objects) = store.read_objects_for_synchronous_execution(
            &transaction.digest(),
            &input_object_kinds,
            &receiving_object_refs,
        )?;

        // Create a mock gas object if one was not provided
        let mock_gas_id = if transaction.gas().is_empty() {
            let mock_gas_object = mock_simulation_gas_coin(transaction.gas_data().owner);
            let mock_gas_object_ref = mock_gas_object.object_ref();
            transaction.gas_data_mut().objects = vec![mock_gas_object_ref];
            input_objects.push(ObjectReadResult::new_from_gas_object(&mock_gas_object));
            Some(mock_gas_object.id())
        } else {
            None
        };

        iota_types::gas::fill_in_unset_simulation_gas(
            &mut transaction,
            &input_objects,
            self.epoch_start_state.reference_gas_price(),
            &self.protocol_config,
        );

        // `MoveAuthenticator`s are not supported in Simulacrum, so we set the
        // `authenticator_gas_budget` to 0.
        let authenticator_gas_budget = 0;

        // Checks enabled -> DRY-RUN (simulating a real TX)
        // Checks disabled -> DEV-INSPECT (more relaxed Move VM checks)
        let (gas_status, checked_input_objects) = if checks.enabled() {
            iota_transaction_checks::check_transaction_input(
                &self.protocol_config,
                self.epoch_start_state.reference_gas_price(),
                &transaction,
                input_objects,
                &receiving_objects,
                &self.bytecode_verifier_metrics,
                verifier_signing_config,
                authenticator_gas_budget,
            )?
        } else {
            // Execution smashes the gas coins and reserves the whole budget from them
            // before running any command, treating the input checks as having verified
            // that they are gas coins at all — so with those checks skipped here, this
            // has to stand in for them. With the checks enabled,
            // `check_transaction_input` covers it.
            iota_types::gas::check_gas_coins_cover_budget_in_simulation(
                &input_objects,
                transaction.gas(),
                transaction.gas_budget(),
            )?;

            let checked_input_objects = iota_transaction_checks::check_simulation_input(
                &self.protocol_config,
                transaction.kind(),
                input_objects,
                receiving_objects,
            )?;
            let gas_status = IotaGasStatus::new(
                transaction.gas_budget(),
                transaction.gas_price(),
                self.epoch_start_state.reference_gas_price(),
                &self.protocol_config,
            )?;

            (gas_status, checked_input_objects)
        };

        // Execute the simulation
        let (kind, signer, gas_data) = transaction.execution_parts();
        let (inner_temp_store, _, effects, execution_result) =
            self.executor.dev_inspect_transaction(
                store.backing_store(),
                &self.protocol_config,
                self.limits_metrics.clone(),
                false,           // expensive_checks
                &HashSet::new(), // certificate_deny_set
                &self.epoch_start_state.epoch(),
                self.epoch_start_state.epoch_start_timestamp_ms(),
                checked_input_objects,
                gas_data,
                gas_status,
                kind,
                signer,
                transaction.digest(),
                checks.disabled(),
            );

        Ok(SimulateTransactionResult {
            input_objects: inner_temp_store.input_objects,
            output_objects: inner_temp_store.written,
            events: effects.events_digest().map(|_| inner_temp_store.events),
            effects,
            execution_result,
            mock_gas_id,
            suggested_gas_price: None,
            gas_data: transaction.gas_data().clone(),
        })
    }
}
