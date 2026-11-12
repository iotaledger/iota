// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{collections::HashSet, sync::Arc};

use iota_config::verifier_signing_config::VerifierSigningConfig;
use iota_execution::Executor;
use iota_protocol_config::ProtocolConfig;
use iota_sdk_types::{
    EpochId, GasPayment, ObjectId, ObjectReference, Transaction, TransactionDigest,
    TransactionEffects, UserSignature,
};
use iota_types::{
    deny_rule_governance::DenyRuleConfig,
    effects::TransactionEffectsAPI,
    error::{ExecutionError, IotaError, UserInputError},
    execution::ExecutionResult,
    gas::{IotaGasStatus, fill_in_unset_simulation_gas},
    gas_coin::mock_simulation_gas_coin,
    inner_temporary_store::InnerTemporaryStore,
    metrics::{BytecodeVerifierMetrics, LimitsMetrics},
    storage::{BackingStore, extend_input_objects_with_loaded_runtime_objects},
    transaction::{
        CheckedInputObjects, InputObjectKind, InputObjects, ObjectReadResult, ReceivingObjects,
        TransactionAPI, merge_authenticator_input_objects,
    },
    transaction_executor::{InputCheckRules, SimulateTransactionResult, VmChecks},
};

use crate::{
    VerifierLimitsSource, check_transaction_input, check_user_transaction_validity,
    deny::check_transaction_for_validation,
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

/// A check in [`prepare_simulation`] rejected the transaction.
#[derive(Debug)]
pub struct SimulationCheckError {
    /// The check that rejected the transaction, e.g. `"deny-list check"`.
    pub check: &'static str,
    pub source: IotaError,
}

impl SimulationCheckError {
    fn new(check: &'static str, source: impl Into<IotaError>) -> Self {
        Self {
            check,
            source: source.into(),
        }
    }
}

impl From<SimulationCheckError> for IotaError {
    fn from(error: SimulationCheckError) -> Self {
        error.source
    }
}

/// A transaction that passed the checks of [`prepare_simulation`], ready for
/// [`execute_simulation`].
pub struct PreparedSimulation {
    /// The transaction with its gas filled in.
    pub transaction: Transaction,
    pub checks: VmChecks,
    pub gas_status: IotaGasStatus,
    pub checked_input_objects: CheckedInputObjects,
    pub receiving_objects: ReceivingObjects,
    pub mock_gas_id: Option<ObjectId>,
}

/// Check `transaction` and fill in its gas, ready for [`execute_simulation`]
/// to simulate it without committing it.
///
/// `checks` selects the Move VM semantics: `VmChecks::Enabled` runs the
/// transaction as it would run on chain (a dry run), while
/// `VmChecks::Disabled` relaxes the checks around entry functions and
/// argument values and drops the input checks named in
/// [`InputCheckRules::RELAXED_UNSAFE`] (a dev inspect). Both report the
/// per-command return values in
/// [`SimulateTransactionResult::execution_result`]. System transactions are
/// rejected.
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
/// `signatures` are deny-checked along with the transaction, and so are the
/// input objects of the `MoveAuthenticator`s among them. With `fill_in_gas`
/// set to `false`, a transaction without a gas payment is rejected and its gas
/// is metered as declared, as on chain. `load_objects` loads the transaction's
/// input and receiving objects.
pub fn prepare_simulation<E: From<SimulationCheckError>>(
    env: &SimulationEnv<'_>,
    store: &dyn BackingStore,
    mut transaction: Transaction,
    checks: VmChecks,
    signatures: &[UserSignature],
    fill_in_gas: bool,
    load_objects: impl FnOnce(
        &[InputObjectKind],
        &[ObjectReference],
    ) -> Result<(InputObjects, ReceivingObjects), E>,
) -> Result<PreparedSimulation, E> {
    let protocol_config = env.protocol_config;

    // A missing gas payment is allowed here, since we may create a mock gas
    // object below.
    check_user_transaction_validity(&transaction, protocol_config)
        .map_err(|e| SimulationCheckError::new("transaction validity check", e))?;

    let input_object_kinds = transaction
        .input_objects()
        .map_err(|e| SimulationCheckError::new("collect input objects", e))?;
    let receiving_object_refs = transaction.receiving_objects();

    // Reject denied transaction elements, as signing would. Signing also
    // checks the input objects of the `MoveAuthenticator`s.
    let mut deny_check_input_kinds = input_object_kinds.clone();
    merge_authenticator_input_objects(
        signatures.iter().filter_map(|signature| match signature {
            UserSignature::MoveAuthenticator(authenticator) => Some(authenticator),
            _ => None,
        }),
        protocol_config.move_authenticator_type_arg_packages_as_inputs(),
        &mut deny_check_input_kinds,
    )
    .map_err(|e| SimulationCheckError::new("merge authenticator inputs", e))?;
    check_transaction_for_validation(
        &transaction,
        signatures,
        &deny_check_input_kinds,
        &receiving_object_refs,
        env.deny_config,
        store,
    )
    .map_err(|e| SimulationCheckError::new("deny-list check", e))?;

    let (mut input_objects, receiving_objects) =
        load_objects(&input_object_kinds, &receiving_object_refs)?;

    // Create a mock gas object if one was not provided
    let mock_gas_id = if transaction.gas().is_empty() {
        if !fill_in_gas {
            return Err(SimulationCheckError::new(
                "transaction validity check",
                UserInputError::MissingGasPayment,
            )
            .into());
        }
        let mock_gas_object = mock_simulation_gas_coin(transaction.gas_data().owner);
        let mock_gas_object_ref = mock_gas_object.object_ref();
        transaction.gas_data_mut().objects = vec![mock_gas_object_ref];
        input_objects.push(ObjectReadResult::new_from_gas_object(&mock_gas_object));
        Some(mock_gas_object.id())
    } else {
        None
    };

    if fill_in_gas {
        fill_in_unset_simulation_gas(
            &mut transaction,
            &input_objects,
            env.reference_gas_price,
            protocol_config,
        );
    }

    // The authenticators and the transaction share the transaction's gas
    // budget, so the authenticator gas budget is 0.
    let authenticator_gas_budget = 0;

    let (gas_status, checked_input_objects) = check_transaction_input(
        protocol_config,
        env.reference_gas_price,
        &transaction,
        input_objects,
        &receiving_objects,
        env.bytecode_verifier_metrics,
        VerifierLimitsSource::NodeConfig(env.verifier_signing_config),
        authenticator_gas_budget,
        false,
        InputCheckRules::from(checks),
    )
    .map_err(|e| {
        let check = if checks.disabled() {
            "simulation input check"
        } else {
            "transaction input check"
        };
        SimulationCheckError::new(check, e)
    })?;

    Ok(PreparedSimulation {
        transaction,
        checks,
        gas_status,
        checked_input_objects,
        receiving_objects,
        mock_gas_id,
    })
}

/// Execute a transaction [`prepare_simulation`] prepared, without committing
/// it, returning the result along with the transaction as it ran. The
/// result's `suggested_gas_price` is `None`.
pub fn execute_simulation(
    env: &SimulationEnv<'_>,
    executor: &dyn Executor,
    store: &dyn BackingStore,
    prepared: PreparedSimulation,
) -> (SimulateTransactionResult, Transaction) {
    let PreparedSimulation {
        transaction,
        checks,
        gas_status,
        checked_input_objects,
        mock_gas_id,
        ..
    } = prepared;
    let (kind, signer, gas_data) = transaction.execution_parts();
    let (inner_temp_store, _, effects, execution_result) = executor.dev_inspect_transaction(
        store,
        env.protocol_config,
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
    let result = simulation_result(
        store,
        inner_temp_store,
        effects,
        execution_result,
        mock_gas_id,
        transaction.gas_data().clone(),
    );
    (result, transaction)
}

/// Assemble a [`SimulateTransactionResult`] from what the executor returned
/// for a run, with no `suggested_gas_price`.
pub fn simulation_result(
    store: &dyn BackingStore,
    inner_temp_store: InnerTemporaryStore,
    effects: TransactionEffects,
    execution_result: Result<Vec<ExecutionResult>, ExecutionError>,
    mock_gas_id: Option<ObjectId>,
    gas_data: GasPayment,
) -> SimulateTransactionResult {
    let mut input_objects = inner_temp_store.input_objects;
    extend_input_objects_with_loaded_runtime_objects(
        &mut input_objects,
        &effects,
        &inner_temp_store.loaded_runtime_objects,
        store.as_object_store(),
    );

    SimulateTransactionResult {
        input_objects,
        output_objects: inner_temp_store.written,
        events: effects.events_digest().map(|_| inner_temp_store.events),
        effects,
        execution_result,
        suggested_gas_price: None,
        mock_gas_id,
        gas_data,
    }
}
