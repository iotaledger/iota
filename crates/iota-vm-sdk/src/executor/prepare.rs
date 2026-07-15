// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Shared transaction preparation, execution, and event decoding.
//!
//! These helpers turn a [`TransactionData`] into a checked, ready-to-run
//! [`PreparedTransaction`], drive it through the Move engine (plain or via a
//! [`MoveAuthenticator`]), and decode emitted events. They operate on an
//! [`ExecutionEnv`] and a [`BackingStore`] and never touch the [`LocalVm`]'s
//! store directly.

use std::collections::HashSet;

use iota_config::transaction_deny_config::TransactionDenyConfig;
use iota_sdk_types::{Address, Digest, Event, ObjectId, ObjectReference};
use iota_types::{
    account_abstraction::authenticator_function::{
        AuthenticatorFunctionRefForExecution,
        authenticator_function_ref_v1_from_dynamic_field_object,
        derive_authenticator_function_ref_v1_dynamic_field_id, extract_auth_fun_refs,
    },
    auth_context::AuthContextData,
    effects::{TransactionEffects, TransactionEffectsAPI},
    error::{IotaError, UserInputError},
    gas::{IotaGasStatus, IotaGasStatusAPI},
    gas_coin::mock_simulation_gas_coin,
    inner_temporary_store::InnerTemporaryStore,
    layout_resolver::LayoutResolver,
    move_authenticator::{MoveAuthenticator, MoveAuthenticatorExt},
    object::bounded_visitor::BoundedVisitor,
    signature::GenericSignature,
    storage::BackingStore,
    transaction::{
        CheckedInputObjects, InputObjectKind, InputObjects, ObjectReadResult,
        ReceivingObjectReadResult, ReceivingObjects, TransactionData, TransactionDataAPI,
        merge_authenticator_input_objects,
    },
    transaction_executor::SimulateTransactionResult,
};
use move_trace_format::format::MoveTraceBuilder;

use crate::{
    error::{ExecutionError, StoreError, ValidationError, VmError, VmSdkError},
    executor::{
        env::ExecutionEnv,
        types::{CommandResult, DecodedEvent, ExecutionMode},
    },
};

pub(super) struct PreparedTransaction {
    transaction: TransactionData,
    gas_status: IotaGasStatus,
    checked_input_objects: CheckedInputObjects,
    mock_gas_id: Option<ObjectId>,
}

pub(super) fn prepare_transaction(
    env: &ExecutionEnv,
    store: &dyn BackingStore,
    mut transaction: TransactionData,
    mode: ExecutionMode,
    deny_config: &TransactionDenyConfig,
    tx_signatures: &[GenericSignature],
    move_authenticators: &[MoveAuthenticator],
    check_coin_deny_list: bool,
) -> Result<PreparedTransaction, VmSdkError> {
    if transaction.kind().is_system() {
        return Err(ValidationError::new(
            "transaction validity check",
            IotaError::UnsupportedFeature {
                error: "system transactions are not supported".to_string(),
            },
        )
        .into());
    }
    transaction
        .validity_check_no_gas_check(&env.protocol_config)
        .map_err(|e| ValidationError::new("transaction validity check", e))?;

    // Update gas payment references to match actual object versions in the
    // store, summing the coins' balance for the dev-inspect budget below.
    let mut gas_balance: u64 = 0;
    let mut updated_gas = Vec::with_capacity(transaction.gas().len());
    for gas_ref in transaction.gas() {
        let obj = store
            .as_object_store()
            .try_get_object(&gas_ref.object_id)
            .map_err(|e| StoreError::new("load gas object", e))?;
        updated_gas.push(obj.map_or(*gas_ref, |o| {
            gas_balance = gas_balance.saturating_add(o.as_coin_maybe().map_or(0, |c| c.value()));
            o.object_ref()
        }));
    }
    transaction.gas_data_mut().objects = updated_gas;

    let raw_input_object_kinds = transaction
        .input_objects()
        .map_err(|e| ValidationError::new("collect input objects", e))?;
    let receiving_object_refs = transaction.receiving_objects();

    // Apply the deny-list policy before loading input objects from the store: it
    // gates on transaction-derived data (signers, commands, object ids, the
    // shared/owned kind) that store loading does not change, so a denied
    // transaction is rejected without the intervening object I/O.
    //
    // The node deny-checks `SenderSignedData::input_objects()`, which merges
    // every `MoveAuthenticator`'s input objects into the transaction's; mirror
    // that merge so denied objects are also caught as authenticator inputs.
    let mut deny_check_input_kinds = raw_input_object_kinds.clone();
    merge_authenticator_input_objects(move_authenticators, &mut deny_check_input_kinds)
        .map_err(|e| ValidationError::new("merge authenticator inputs", e))?;
    iota_transaction_checks::deny::check_transaction_for_validation(
        &transaction,
        tx_signatures,
        &deny_check_input_kinds,
        &receiving_object_refs,
        deny_config,
        store,
    )
    .map_err(|e| ValidationError::new("deny-list check", e))?;

    let mut input_objects = load_input_objects(store, &raw_input_object_kinds)?;
    let receiving_objects = load_receiving_objects(store, &receiving_object_refs)?;

    // Mint a one-shot mock gas coin if the transaction carries no gas payment,
    // the same coin the node mints on its simulation paths. `Execute` commits
    // effects to the store, so it requires a real gas payment.
    let mock_gas_id = if transaction.gas().is_empty() {
        if matches!(mode, ExecutionMode::Execute) {
            return Err(ValidationError::new(
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

    // Snapshot the received objects for the coin deny-list check below: the
    // dev-inspect branch consumes `receiving_objects`, which is not `Clone`.
    let coin_deny_receiving = check_coin_deny_list.then(|| {
        receiving_objects
            .objects
            .iter()
            .map(|r| ReceivingObjectReadResult::new(r.object_ref, r.object.clone()))
            .collect::<Vec<_>>()
    });

    let (gas_status, checked_input_objects) = if matches!(mode, ExecutionMode::DevInspect) {
        let checked_input_objects = iota_transaction_checks::check_dev_inspect_input(
            &env.protocol_config,
            transaction.kind(),
            input_objects,
            receiving_objects,
        )
        .map_err(|e| ValidationError::new("dev-inspect input check", e))?;
        // Dev-inspect meters at `max_tx_gas`, not the transaction's declared
        // budget, matching the node's dev-inspect entry point — a run before a
        // budget is settled isn't limited by it. Real gas coins cap the budget
        // at their total balance, since the engine smashes the budget off the
        // coin up front (the mock coin's balance always covers `max_tx_gas`).
        let dev_inspect_gas_budget = if mock_gas_id.is_some() {
            env.protocol_config.max_tx_gas()
        } else {
            env.protocol_config.max_tx_gas().min(gas_balance)
        };
        let gas_status = IotaGasStatus::new(
            dev_inspect_gas_budget,
            transaction.gas_price(),
            env.reference_gas_price,
            &env.protocol_config,
        )
        .map_err(|e| ValidationError::new("gas status", e))?;
        (gas_status, checked_input_objects)
    } else {
        // Offline default: the verifier-signing limits may differ from those a
        // live validator enforces, so this check will not match a real chain.
        let verifier_signing_config =
            iota_config::verifier_signing_config::VerifierSigningConfig::default();
        // Pass `0` for the authenticator gas budget: this crate runs the
        // authenticator and body together to effects, which is the node's
        // post-consensus path, where they share the full transaction budget
        // (`max_auth_gas` caps only the separate pre-consensus signing check,
        // which this crate does not model). A `0` budget makes
        // `check_transaction_input` meter at the full transaction budget, as
        // `check_certificate_input` does. (Move-authentication support is
        // verified before preparation, so the precondition a non-zero budget
        // would enforce is already covered.)
        let (gas_status, checked_input_objects) = iota_transaction_checks::check_transaction_input(
            &env.protocol_config,
            env.reference_gas_price,
            &transaction,
            input_objects,
            &receiving_objects,
            &env.bytecode_verifier_metrics,
            &verifier_signing_config,
            0,
        )
        .map_err(|e| ValidationError::new("transaction input check", e))?;
        (gas_status, checked_input_objects)
    };

    if let Some(receiving) = coin_deny_receiving {
        // Regulated-coin deny-list check over the transaction's own inputs and
        // received objects. Authenticator inputs are checked separately in
        // `execute_with_move_authenticators`, where they are resolved.
        let receiving: ReceivingObjects = receiving.into();
        run_coin_deny_list_check(
            store,
            transaction.sender(),
            &checked_input_objects,
            &receiving,
            &[],
        )?;
    }

    Ok(PreparedTransaction {
        transaction,
        gas_status,
        checked_input_objects,
        mock_gas_id,
    })
}

pub(super) fn execute_prepared(
    env: &ExecutionEnv,
    store: &dyn BackingStore,
    prepared: PreparedTransaction,
    mode: ExecutionMode,
) -> Result<SimulateTransactionResult, VmSdkError> {
    let PreparedTransaction {
        transaction,
        gas_status,
        checked_input_objects,
        mock_gas_id,
    } = prepared;

    let dev_inspect = matches!(mode, ExecutionMode::DevInspect);
    let (kind, signer, gas_data) = transaction.execution_parts();
    // `dev_inspect_transaction` accepts no `MoveTraceBuilder`; tracing is only
    // available on the `authenticate_then_execute_transaction_to_effects` path.
    let (inner_temp_store, _, effects, execution_result) = env.executor.dev_inspect_transaction(
        store,
        &env.protocol_config,
        env.limits_metrics.clone(),
        false,
        &HashSet::new(),
        &env.epoch_id,
        env.epoch_timestamp_ms,
        checked_input_objects,
        gas_data,
        gas_status,
        kind,
        signer,
        transaction.digest(),
        dev_inspect,
    );

    Ok(simulation_result(
        inner_temp_store,
        effects,
        execution_result,
        mock_gas_id,
    ))
}

/// Assemble the engine's raw outputs into a [`SimulateTransactionResult`].
fn simulation_result(
    inner_temp_store: InnerTemporaryStore,
    effects: TransactionEffects,
    execution_result: Result<Vec<CommandResult>, iota_types::error::ExecutionError>,
    mock_gas_id: Option<ObjectId>,
) -> SimulateTransactionResult {
    SimulateTransactionResult {
        input_objects: inner_temp_store.input_objects,
        output_objects: inner_temp_store.written,
        events: effects.events_digest().map(|_| inner_temp_store.events),
        effects,
        execution_result,
        mock_gas_id,
        suggested_gas_price: None,
    }
}

/// Run a transaction whose sender and/or sponsor authorize via a
/// `MoveAuthenticator`, returning the simulation together with the aggregate
/// authentication outcome over all authenticators.
///
/// Every `MoveAuthenticator` on the transaction is resolved and executed — the
/// sender's and, for a sponsored transaction, the sponsor's. A successful run
/// implies all of them accepted. On a failed run the failure may come from any
/// authenticator or from the transaction body, so the authenticators are
/// re-executed alone for an unambiguous outcome: `Err` if any rejected, `Ok` if
/// they all passed and the body was at fault. The re-run is sound — the
/// authentication phase discards writes.
pub(super) fn execute_with_move_authenticators(
    env: &ExecutionEnv,
    store: &dyn BackingStore,
    prepared: PreparedTransaction,
    authenticators: Vec<MoveAuthenticator>,
    auth_digests: (Digest, Option<Digest>),
    check_coin_deny_list: bool,
    trace_builder_opt: &mut Option<MoveTraceBuilder>,
) -> Result<
    (
        SimulateTransactionResult,
        Result<(), iota_types::error::ExecutionError>,
    ),
    VmSdkError,
> {
    let PreparedTransaction {
        transaction,
        gas_status,
        checked_input_objects,
        mock_gas_id,
    } = prepared;
    // Captured before `gas_status` is consumed so the re-run below can meter at
    // the same budget as the combined run.
    let run_gas_budget = gas_status.gas_budget();

    // Resolve every authenticator, then union each one's inputs into the
    // transaction's checked inputs, enforcing consistency (matching object read
    // results, compatible shared-object kinds) for ids that appear in more than
    // one set.
    let prepared_auths = prepare_authenticators(store, authenticators)?;
    let mut union_checked = checked_input_objects;
    for (_, _, inputs) in &prepared_auths {
        let auth_checked = CheckedInputObjects::new_with_checked_transaction_inputs(inputs.clone());
        union_checked =
            iota_transaction_checks::checked_input_objects_union(union_checked, &auth_checked)
                .map_err(|e| ValidationError::new("union authenticator inputs", e))?;
    }

    if check_coin_deny_list {
        // Regulated-coin deny-list check over the authenticator inputs; the
        // transaction's own inputs and received objects were checked in
        // `prepare_transaction`.
        let auth_checked = prepared_auths
            .iter()
            .map(|(_, _, inputs)| {
                CheckedInputObjects::new_with_checked_transaction_inputs(inputs.clone())
            })
            .collect::<Vec<_>>();
        let auth_refs: Vec<&CheckedInputObjects> = auth_checked.iter().collect();
        run_coin_deny_list_check(
            store,
            transaction.sender(),
            &CheckedInputObjects::new_with_checked_transaction_inputs(Vec::new().into()),
            &Vec::new().into(),
            &auth_refs,
        )?;
    }

    let auth_context_data = build_auth_context_data(&transaction, &prepared_auths, auth_digests)?;

    let (kind, signer, gas_data) = transaction.execution_parts();
    // `CheckedInputObjects` is not `Clone`; rebuild the per-authenticator inputs
    // for the combined run, which takes the full
    // `AuthenticatorFunctionRefForExecution`.
    let exec_authenticators = prepared_auths
        .iter()
        .map(|(a, fn_ref, inputs)| {
            (
                a.clone(),
                fn_ref.clone(),
                CheckedInputObjects::new_with_checked_transaction_inputs(inputs.clone()),
            )
        })
        .collect::<Vec<_>>();

    let (inner_temp_store, _, effects, execution_result) = env
        .executor
        .authenticate_then_execute_transaction_to_effects(
            store,
            &env.protocol_config,
            env.limits_metrics.clone(),
            false,
            &HashSet::new(),
            &env.epoch_id,
            env.epoch_timestamp_ms,
            gas_data,
            gas_status,
            exec_authenticators,
            union_checked,
            kind,
            signer,
            transaction.digest(),
            auth_context_data.clone(),
            trace_builder_opt,
        );

    let authenticator_outcome = if effects.status().is_success() {
        Ok(())
    } else if effects.status().error_command().is_some_and(|cmd| cmd > 0) {
        // The authenticators run as a fake command 0, so a failure in any later
        // command is a transaction-body abort, never an authentication
        // rejection: the authenticators passed. Skip the re-run.
        // TODO(https://github.com/iotaledger/iota/issues/11986): once the fake
        // command-0 mapping is resolved an authentication rejection becomes
        // unambiguous and this whole re-run branch can be removed.
        Ok(())
    } else {
        // The failure is in command 0 or unattributed, so it may be an
        // authentication rejection or a body abort. Re-run the authenticators
        // alone to tell them apart. Meter at the same budget the combined run
        // shared between the authenticators and body (the full transaction
        // budget outside `DevInspect`, matching the node's post-consensus
        // execution), so the re-run never reports a rejection for a run the
        // combined execution had enough gas for.
        let rerun_gas_status = IotaGasStatus::new(
            run_gas_budget,
            transaction.gas_price(),
            env.reference_gas_price,
            &env.protocol_config,
        )
        .map_err(|e| ValidationError::new("authenticator re-run gas status", e))?;
        authenticate_only(
            env,
            store,
            &transaction,
            &prepared_auths,
            rerun_gas_status,
            auth_context_data,
        )?
    };

    // The authenticator engine entry point does not return per-command
    // results, so a signed `MoveAuthenticator` run carries none.
    Ok((
        simulation_result(
            inner_temp_store,
            effects,
            execution_result.map(|_| Vec::new()),
            mock_gas_id,
        ),
        authenticator_outcome,
    ))
}

/// A `MoveAuthenticator` resolved for execution: paired with its account's
/// authenticator function ref and its checked input objects.
type PreparedAuthenticator = (
    MoveAuthenticator,
    AuthenticatorFunctionRefForExecution,
    InputObjects,
);

/// Resolve each authenticator's checked input objects and function ref from the
/// store. The per-authenticator object restrictions a node enforces at signing
/// time (no packages, no address-owned objects, no mutable shared objects, …)
/// are applied to each authenticator's inputs.
pub(super) fn prepare_authenticators(
    store: &dyn BackingStore,
    authenticators: Vec<MoveAuthenticator>,
) -> Result<Vec<PreparedAuthenticator>, VmSdkError> {
    let mut prepared = Vec::with_capacity(authenticators.len());
    for authenticator in authenticators {
        let auth_input_object_kinds = authenticator.input_objects();
        let auth_input_objects = load_input_objects(store, &auth_input_object_kinds)?;
        let auth_checked = iota_transaction_checks::check_move_authenticator_input_for_validation(
            auth_input_objects,
        )
        .map_err(|e| ValidationError::new("authenticator input check", e))?;
        let fn_ref = resolve_authenticator_function_ref(store, &authenticator)?;
        prepared.push((authenticator, fn_ref, auth_checked.into_inner()));
    }
    Ok(prepared)
}

/// Build the [`AuthContextData`] an authenticator run needs: the transaction
/// bytes, the sender/sponsor auth digests, and the sender/sponsor authenticator
/// function refs. The function refs are resolved from the full set of
/// authenticators, matching the node, even when only a subset is executed.
pub(super) fn build_auth_context_data(
    transaction: &TransactionData,
    prepared_auths: &[PreparedAuthenticator],
    auth_digests: (Digest, Option<Digest>),
) -> Result<AuthContextData, VmSdkError> {
    let tx_data_bytes = bcs::to_bytes(transaction)
        .map_err(|e| VmError::new(format!("serialize transaction data: {e}")))?;
    let (sender_auth_digest, sponsor_auth_digest) = auth_digests;
    let (sender_authenticator_function_ref, sponsor_authenticator_function_ref) =
        extract_auth_fun_refs(
            transaction.sender(),
            transaction.gas_data().owner,
            |address| {
                prepared_auths
                    .iter()
                    .find(|(a, _, _)| a.address() == address)
                    .map(|(_, fn_ref, _)| fn_ref.authenticator_function_ref.clone())
            },
        );
    Ok(AuthContextData {
        transaction_data_bytes: tx_data_bytes,
        sender_auth_digest,
        sponsor_auth_digest,
        sender_authenticator_function_ref,
        sponsor_authenticator_function_ref,
    })
}

/// Run `authenticate_transaction` over `auths_to_run` under `gas_status`,
/// returning the aggregate outcome (`Err` if any authenticator rejected). Used
/// to disambiguate a failed combined run and, on its own, to model the node's
/// pre-consensus signing check.
pub(super) fn authenticate_only(
    env: &ExecutionEnv,
    store: &dyn BackingStore,
    transaction: &TransactionData,
    auths_to_run: &[PreparedAuthenticator],
    gas_status: IotaGasStatus,
    auth_context_data: AuthContextData,
) -> Result<Result<(), iota_types::error::ExecutionError>, VmSdkError> {
    // `authenticate_transaction` takes the inner `AuthenticatorFunctionRef`;
    // `CheckedInputObjects` is not `Clone`, so rebuild it for the call.
    let authenticators = auths_to_run
        .iter()
        .map(|(a, fn_ref, inputs)| {
            (
                a.clone(),
                fn_ref.authenticator_function_ref.clone(),
                CheckedInputObjects::new_with_checked_transaction_inputs(inputs.clone()),
            )
        })
        .collect::<Vec<_>>();
    let checked_refs: Vec<&CheckedInputObjects> = authenticators
        .iter()
        .map(|(_, _, checked)| checked)
        .collect();
    let aggregated_auth_inputs =
        iota_transaction_checks::aggregate_authenticator_input_objects(&checked_refs)
            .map_err(|e| ValidationError::new("aggregate authenticator inputs", e))?;
    let (kind, signer, gas_data) = transaction.execution_parts();
    Ok(env.executor.authenticate_transaction(
        store,
        &env.protocol_config,
        env.limits_metrics.clone(),
        &env.epoch_id,
        env.epoch_timestamp_ms,
        gas_data,
        gas_status,
        authenticators,
        aggregated_auth_inputs,
        kind,
        signer,
        transaction.digest(),
        auth_context_data,
        &mut None,
    ))
}

/// Run the regulated-coin deny-list check over the given inputs, mapping a
/// denial to a validation error.
fn run_coin_deny_list_check(
    store: &dyn BackingStore,
    sender: Address,
    tx_input_objects: &CheckedInputObjects,
    receiving_objects: &ReceivingObjects,
    per_authenticator_input_objects: &[&CheckedInputObjects],
) -> Result<(), VmSdkError> {
    iota_types::deny_list_v1::check_coin_deny_list_v1(
        sender,
        tx_input_objects,
        receiving_objects,
        &per_authenticator_input_objects.to_vec(),
        store.as_object_store(),
    )
    .map_err(|e| ValidationError::new("coin deny-list check", e).into())
}

/// Load the [`AuthenticatorFunctionRefForExecution`] from the account object's
/// dynamic field in the store.
fn resolve_authenticator_function_ref(
    store: &dyn BackingStore,
    authenticator: &MoveAuthenticator,
) -> Result<AuthenticatorFunctionRefForExecution, VmSdkError> {
    let (account_object_id, _version, _digest) = authenticator
        .object_to_authenticate_components()
        .map_err(|e| VmError::new(format!("invalid object_to_authenticate: {e}")))?;

    let field_id = derive_authenticator_function_ref_v1_dynamic_field_id(account_object_id)
        .map_err(|e| ValidationError::new("derive authenticator field id", e))?;

    let field_obj = store
        .as_object_store()
        .try_get_object(&field_id)
        .map_err(|e| StoreError::new("load authenticator field", e))?
        .ok_or(VmSdkError::MissingObject {
            id: field_id,
            version: None,
        })?;

    authenticator_function_ref_v1_from_dynamic_field_object(account_object_id, &field_obj)
        .map_err(|e| ValidationError::new("decode authenticator field", e).into())
}

/// Build `InputObjects` from a store, using the latest fetched versions.
fn load_input_objects(
    store: &dyn BackingStore,
    input_object_kinds: &[InputObjectKind],
) -> Result<InputObjects, VmSdkError> {
    let mut input_objects = Vec::new();
    for kind in input_object_kinds {
        let obj = store
            .as_object_store()
            .try_get_object(&kind.object_id())
            .map_err(|e| StoreError::new("load input object", e))?
            .ok_or(VmSdkError::MissingObject {
                id: kind.object_id(),
                version: kind.version(),
            })?;

        let loaded_kind = match kind {
            InputObjectKind::MovePackage(_) => *kind,
            InputObjectKind::ImmOrOwnedMoveObject(_) => {
                InputObjectKind::ImmOrOwnedMoveObject(obj.object_ref())
            }
            InputObjectKind::SharedMoveObject {
                initial_shared_version,
                mutable,
                ..
            } => InputObjectKind::SharedMoveObject {
                id: obj.id(),
                initial_shared_version: *initial_shared_version,
                mutable: *mutable,
            },
        };

        input_objects.push(ObjectReadResult::new(loaded_kind, obj.into()));
    }
    Ok(input_objects.into())
}

fn load_receiving_objects(
    store: &dyn BackingStore,
    receiving_object_refs: &[ObjectReference],
) -> Result<ReceivingObjects, VmSdkError> {
    let mut receiving_objects = Vec::new();
    for objref in receiving_object_refs {
        let obj = store
            .as_object_store()
            .try_get_object(&objref.object_id)
            .map_err(|e| StoreError::new("load receiving object", e))?
            .ok_or(VmSdkError::MissingObject {
                id: objref.object_id,
                version: Some(objref.version),
            })?;
        // Keep the declared reference, as the node does at signing: the
        // sign-time checks compare it against the loaded object, so a stale
        // version or digest is rejected instead of silently accepted.
        receiving_objects.push(ReceivingObjectReadResult::new(*objref, obj.into()));
    }
    Ok(receiving_objects.into())
}

/// Decode a single event against the resolver.
pub(super) fn decode_one_event(
    event: &Event,
    resolver: &mut dyn LayoutResolver,
) -> Result<DecodedEvent, VmSdkError> {
    let layout = resolver
        .get_annotated_layout(&event.type_)
        .map_err(|e| ExecutionError::new(format!("resolve layout for {}: {e}", event.type_)))?;
    // `BoundedVisitor` bounds the deserialized value's depth and allocation,
    // like every node-side decoder of externally-sourced bytes.
    let value = BoundedVisitor::deserialize_value(&event.contents, &layout.into_layout())
        .map_err(|e| ExecutionError::new(format!("bcs deserialize {}: {e}", event.type_)))?;

    Ok(DecodedEvent {
        event: event.clone(),
        value,
    })
}
