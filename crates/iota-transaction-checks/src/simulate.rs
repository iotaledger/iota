// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{collections::HashSet, sync::Arc};

use iota_config::verifier_signing_config::VerifierSigningConfig;
use iota_execution::Executor;
use iota_protocol_config::ProtocolConfig;
use iota_sdk_types::{EpochId, ObjectReference, Transaction, TransactionDigest};
use iota_types::{
    deny_rule_governance::DenyRuleConfig,
    effects::TransactionEffectsAPI,
    error::{IotaError, IotaResult},
    gas::{
        IotaGasStatus, check_gas_coins_cover_budget_in_simulation, fill_in_unset_simulation_gas,
    },
    gas_coin::mock_simulation_gas_coin,
    metrics::{BytecodeVerifierMetrics, LimitsMetrics},
    storage::{BackingStore, extend_input_objects_with_loaded_runtime_objects},
    transaction::{
        InputObjectKind, InputObjects, ObjectReadResult, ReceivingObjects, TransactionAPI,
    },
    transaction_executor::{SimulateTransactionResult, VmChecks},
};

use crate::{
    VerifierLimitsSource, check_non_system_packages_to_be_published, check_simulation_input,
    check_transaction_input, deny::check_transaction_for_validation,
};

/// The chain and node settings a simulation runs under.
pub struct SimulationEnv<'a> {
    pub protocol_config: &'a ProtocolConfig,
    pub reference_gas_price: u64,
    pub epoch_id: EpochId,
    pub epoch_timestamp_ms: u64,
    pub deny_config: &'a dyn DenyRuleConfig,
    pub certificate_deny_set: &'a HashSet<TransactionDigest>,
    pub verifier_signing_config: &'a VerifierSigningConfig,
    pub bytecode_verifier_metrics: &'a Arc<BytecodeVerifierMetrics>,
    pub limits_metrics: &'a Arc<LimitsMetrics>,
}

/// Simulate `transaction` without committing it.
///
/// `checks` selects the Move VM semantics: `VmChecks::Enabled` runs the
/// transaction as it would run on chain (a dry run), while
/// `VmChecks::Disabled` relaxes the checks around entry functions and
/// argument values (a dev inspect). Both report the per-command return
/// values in [`SimulateTransactionResult::execution_result`]. System
/// transactions are rejected.
///
/// Under either `checks`, the simulation fills in whatever gas the
/// transaction leaves unset, so that a caller with no gas to declare can
/// leave all of it out: no gas payment mints a mock gas coin, whose ID is
/// reported back in [`SimulateTransactionResult::mock_gas_id`]; a zero gas
/// price becomes `env.reference_gas_price`; and a zero gas budget becomes as
/// much as the gas coins can back, up to
/// [`max_tx_gas`](ProtocolConfig::max_tx_gas). Anything the transaction does
/// declare is metered as given, so a dry run still rejects the gas a validator
/// would.
///
/// Under either `checks`, the packages the transaction publishes are run
/// through the bytecode verifier with the limits in
/// `env.verifier_signing_config`, so a package those limits reject at signing
/// is rejected here too, with
/// [`UserInputError::PackageVerificationTimedout`](iota_types::error::UserInputError::PackageVerificationTimedout).
///
/// Whatever the budget resolves to, the gas coins have to cover it, since
/// execution reserves the whole budget from them before running any command
/// and refunds it afterwards. A caller leaving the budget at zero to have the
/// cost estimated therefore gets an estimate whatever its coins hold, but the
/// reserved budget is off limits for the duration of the programmable
/// transaction: a transaction that also pays out of its gas coin has to
/// declare a budget leaving room for that, exactly as it would on chain. A
/// balance too small to declare the minimum budget at all is rejected with
/// [`UserInputError::GasBalanceTooLow`](iota_types::error::UserInputError::GasBalanceTooLow).
///
/// `load_objects` loads the transaction's input and receiving objects. The
/// returned `suggested_gas_price` is always `None`.
pub fn simulate_transaction(
    env: &SimulationEnv<'_>,
    executor: &dyn Executor,
    store: &dyn BackingStore,
    mut transaction: Transaction,
    checks: VmChecks,
    load_objects: impl FnOnce(
        &[InputObjectKind],
        &[ObjectReference],
    ) -> IotaResult<(InputObjects, ReceivingObjects)>,
) -> IotaResult<SimulateTransactionResult> {
    let protocol_config = env.protocol_config;

    if transaction.kind().is_system() {
        return Err(IotaError::UnsupportedFeature {
            error: "simulate does not support system transactions".to_string(),
        });
    }

    transaction.check_serialized_size(protocol_config)?;

    // Cheap validity checks for a transaction, including input size limits.
    // This does not check if gas objects are missing since we may create a
    // mock gas object. It checks for other transaction input validity.
    transaction.validity_check_no_gas_check(protocol_config)?;

    // The full validity check caps the gas payment size alongside requiring a
    // gas payment at all, which a simulation relaxes so it can mock one. The cap
    // still applies, and is cheapest before any object is loaded.
    transaction.check_gas_payment_size(protocol_config)?;

    let input_object_kinds = transaction.input_objects()?;
    let receiving_object_refs = transaction.receiving_objects();

    // Reject denied transaction elements, as signing would.
    check_transaction_for_validation(
        &transaction,
        &[],
        &input_object_kinds,
        &receiving_object_refs,
        env.deny_config,
        store,
    )?;

    let (mut input_objects, receiving_objects) =
        load_objects(&input_object_kinds, &receiving_object_refs)?;

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

    fill_in_unset_simulation_gas(
        &mut transaction,
        &input_objects,
        env.reference_gas_price,
        protocol_config,
    );

    // `MoveAuthenticator`s are not supported in simulation, so we set the
    // `authenticator_gas_budget` to 0.
    let authenticator_gas_budget = 0;

    // Checks enabled -> DRY-RUN, it means we are simulating a real TX
    // Checks disabled -> DEV-INSPECT, more relaxed Move VM checks
    let (gas_status, checked_input_objects) = if checks.enabled() {
        check_transaction_input(
            protocol_config,
            env.reference_gas_price,
            &transaction,
            input_objects,
            &receiving_objects,
            env.bytecode_verifier_metrics,
            VerifierLimitsSource::NodeConfig(env.verifier_signing_config),
            authenticator_gas_budget,
            false,
        )?
    } else {
        // Execution smashes the gas coins and reserves the whole budget from them
        // before running any command, treating the input checks as having verified
        // that they are gas coins at all — so with those checks skipped here, this
        // has to stand in for them. With the checks enabled,
        // `check_transaction_input` covers it.
        check_gas_coins_cover_budget_in_simulation(
            &input_objects,
            transaction.gas(),
            transaction.gas_budget(),
        )?;

        let checked_input_objects = check_simulation_input(
            protocol_config,
            transaction.kind(),
            input_objects,
            receiving_objects,
        )?;

        // Execution verifies the packages a transaction publishes with neither a
        // limit nor a meter, trusting signing to have bounded them — so with the
        // signing-time checks skipped here, this has to bound them. With the
        // checks enabled, `check_transaction_input` covers it.
        check_non_system_packages_to_be_published(
            &transaction,
            protocol_config,
            env.bytecode_verifier_metrics,
            VerifierLimitsSource::NodeConfig(env.verifier_signing_config),
        )?;

        let gas_status = IotaGasStatus::new(
            transaction.gas_budget(),
            transaction.gas_price(),
            env.reference_gas_price,
            protocol_config,
        )?;

        (gas_status, checked_input_objects)
    };

    let (kind, signer, gas_data) = transaction.execution_parts();
    let (inner_temp_store, _, effects, execution_result) = executor.dev_inspect_transaction(
        store,
        protocol_config,
        env.limits_metrics.clone(),
        false, // expensive_checks
        env.certificate_deny_set,
        &env.epoch_id,
        env.epoch_timestamp_ms,
        checked_input_objects,
        gas_data,
        gas_status,
        kind,
        signer,
        transaction.digest(),
        checks.disabled(),
    );

    let mut input_objects = inner_temp_store.input_objects;
    extend_input_objects_with_loaded_runtime_objects(
        &mut input_objects,
        &effects,
        &inner_temp_store.loaded_runtime_objects,
        store.as_object_store(),
    );

    Ok(SimulateTransactionResult {
        input_objects,
        output_objects: inner_temp_store.written,
        events: effects.events_digest().map(|_| inner_temp_store.events),
        effects,
        execution_result,
        suggested_gas_price: None,
        mock_gas_id,
        gas_data: transaction.gas_data().clone(),
    })
}
