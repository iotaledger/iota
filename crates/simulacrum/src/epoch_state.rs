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
use iota_types::{
    base_types::ObjectID,
    committee::{Committee, EpochId},
    digests::TransactionDigest,
    effects::{TransactionEffects, TransactionEffectsAPI},
    error::IotaResult,
    gas::IotaGasStatus,
    gas_coin::NANOS_PER_IOTA,
    inner_temporary_store::InnerTemporaryStore,
    iota_system_state::{
        IotaSystemState, IotaSystemStateTrait,
        epoch_start_iota_system_state::{EpochStartSystemState, EpochStartSystemStateTrait},
    },
    metrics::{BytecodeVerifierMetrics, LimitsMetrics},
    object::{MoveObject, Object, Owner},
    transaction::{ObjectReadResult, TransactionData, TransactionDataAPI, VerifiedTransaction},
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
        let registry = prometheus::Registry::new();
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
        let tx_data = &transaction.data().intent_message().value;
        let input_object_kinds = tx_data.input_objects()?;
        let receiving_object_refs = tx_data.receiving_objects();

        iota_transaction_checks::deny::check_transaction_for_signing(
            tx_data,
            transaction.tx_signatures(),
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
            transaction.data().transaction_data(),
            input_objects,
            &receiving_objects,
            &self.bytecode_verifier_metrics,
            verifier_signing_config,
            authenticator_gas_budget,
        )?;

        let transaction_data = transaction.data().transaction_data();
        let (kind, signer, gas_data) = transaction_data.execution_parts();
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
    /// - Takes TransactionData instead of VerifiedTransaction (no signature
    ///   required)
    /// - Takes VmChecks parameter to control validation strictness
    /// - Returns SimulateTransactionResult with input/output objects
    /// - Creates a mock gas object if none provided
    pub fn simulate_transaction(
        &self,
        store: &dyn SimulatorStore,
        deny_config: &TransactionDenyConfig,
        verifier_signing_config: &VerifierSigningConfig,
        mut transaction: TransactionData,
        checks: VmChecks,
    ) -> IotaResult<SimulateTransactionResult> {
        // Cheap validity checks for a transaction, including input size limits.
        transaction.validity_check_no_gas_check(&self.protocol_config)?;

        let input_object_kinds = transaction.input_objects()?;
        let receiving_object_refs = transaction.receiving_objects();

        // Check if some transaction elements are denied
        iota_transaction_checks::deny::check_transaction_for_signing(
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
        const SIMULATION_GAS_COIN_VALUE: u64 = 1_000_000_000 * NANOS_PER_IOTA; // 1B IOTA
        let mock_gas_id = if transaction.gas().is_empty() {
            let mock_gas_object = Object::new_move(
                MoveObject::new_gas_coin(1.into(), ObjectID::MAX, SIMULATION_GAS_COIN_VALUE),
                Owner::AddressOwner(transaction.gas_data().owner),
                TransactionDigest::genesis_marker(),
            );
            let mock_gas_object_ref = mock_gas_object.compute_object_reference();
            transaction.gas_data_mut().payment = vec![mock_gas_object_ref];
            input_objects.push(ObjectReadResult::new_from_gas_object(&mock_gas_object));
            Some(mock_gas_object.id())
        } else {
            None
        };

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
            let checked_input_objects = iota_transaction_checks::check_dev_inspect_input(
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
        })
    }
}
