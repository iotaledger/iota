// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Shared transaction preparation, execution, and event decoding.
//!
//! These helpers turn a [`Transaction`] into a checked, ready-to-run
//! [`PreparedSimulation`], drive it through the Move engine via a
//! [`MoveAuthenticator`], and decode emitted events. They operate on a
//! [`SimulationEnv`] or an [`ExecutionEnv`] and a [`BackingStore`] and never
//! touch the [`LocalVm`]'s store directly.

use iota_execution::Executor;
use iota_protocol_config::ProtocolConfig;
use iota_sdk_types::{
    Address, Digest, Event, MoveAuthenticator, ObjectReference, Transaction, UserSignature,
};
use iota_transaction_checks::simulate::{self, PreparedSimulation, SimulationEnv};
use iota_types::{
    account_abstraction::authenticator_function::{
        AuthenticatorFunctionRefForExecution, MoveAuthenticatorForExecution,
        MoveAuthenticatorsForExecution, authenticator_function_ref_v1_from_dynamic_field_object,
        derive_authenticator_function_ref_v1_dynamic_field_id, extract_auth_fun_refs,
    },
    auth_context::AuthContextData,
    error::UserInputError,
    gas::IotaGasStatus,
    layout_resolver::LayoutResolver,
    move_authenticator::MoveAuthenticatorExt,
    object::bounded_visitor::BoundedVisitor,
    storage::BackingStore,
    transaction::{
        CheckedInputObjects, InputObjectKind, InputObjects, ObjectReadResult,
        ReceivingObjectReadResult, ReceivingObjects, TransactionAPI,
    },
    transaction_executor::{SimulateTransactionResult, VmChecks},
};
use move_trace_format::format::MoveTraceBuilder;

use crate::{
    error::{ExecutionError, StoreError, ValidationError, VmError, VmSdkError},
    executor::{
        env::ExecutionEnv,
        types::{DecodedEvent, ExecutionMode},
    },
};

pub(super) fn prepare_transaction(
    env: &SimulationEnv<'_>,
    store: &dyn BackingStore,
    mut transaction: Transaction,
    mode: ExecutionMode,
    tx_signatures: &[UserSignature],
    check_coin_deny_list: bool,
) -> Result<PreparedSimulation, VmSdkError> {
    // Reject an invalid transaction before fetching its gas objects, which
    // can mean a network round trip per gas coin.
    iota_transaction_checks::check_user_transaction_validity(&transaction, env.protocol_config)
        .map_err(|e| ValidationError::new("transaction validity check", e))?;

    // Update gas payment references to match actual object versions in the store.
    let mut updated_gas = Vec::with_capacity(transaction.gas().len());
    for gas_ref in transaction.gas() {
        let obj = store
            .as_object_store()
            .try_get_object(&gas_ref.object_id)
            .map_err(|e| StoreError::new("load gas object", e))?;
        updated_gas.push(obj.map_or(*gas_ref, |o| o.object_ref()));
    }
    transaction.gas_data_mut().objects = updated_gas;

    let (checks, fill_in_gas) = match mode {
        ExecutionMode::DevInspect => (VmChecks::Disabled, true),
        ExecutionMode::DryRun => (VmChecks::Enabled, true),
        // `Execute` commits its effects, so it holds a transaction to its own
        // declared gas the way a validator would.
        ExecutionMode::Execute => (VmChecks::Enabled, false),
    };
    let prepared = simulate::prepare_simulation(
        env,
        store,
        transaction,
        checks,
        tx_signatures,
        fill_in_gas,
        |input_object_kinds, receiving_object_refs| {
            Ok::<_, VmSdkError>((
                load_input_objects(store, input_object_kinds)?,
                load_receiving_objects(store, receiving_object_refs)?,
            ))
        },
    )?;

    if check_coin_deny_list {
        // Regulated-coin deny-list check over the transaction's own inputs and
        // received objects. Authenticator inputs are checked separately in
        // `execute_with_move_authenticators`, where they are resolved.
        run_coin_deny_list_check(
            store,
            prepared.transaction.sender(),
            &prepared.checked_input_objects,
            &prepared.receiving_objects,
            &[],
        )?;
    }

    Ok(prepared)
}

/// Run a transaction whose sender and/or sponsor authorize via a
/// `MoveAuthenticator`, returning the simulation together with the aggregate
/// authentication outcome over all authenticators.
///
/// Every `MoveAuthenticator` on the transaction is resolved and executed — the
/// sender's and, for a sponsored transaction, the sponsor's. A successful run
/// implies all of them accepted. The outcome is the engine's authentication
/// error: `Err` if any rejected, `Ok` if they all passed, even when the body
/// failed.
pub(super) fn execute_with_move_authenticators(
    env: &SimulationEnv<'_>,
    executor: &dyn Executor,
    store: &dyn BackingStore,
    prepared: PreparedSimulation,
    authenticators: Vec<MoveAuthenticator>,
    auth_digests: (Digest, Option<Digest>),
    check_coin_deny_list: bool,
    trace_builder_opt: &mut Option<MoveTraceBuilder>,
) -> Result<
    (
        SimulateTransactionResult,
        Result<(), iota_types::error::ExecutionError>,
        Transaction,
    ),
    VmSdkError,
> {
    let PreparedSimulation {
        transaction,
        gas_status,
        checked_input_objects,
        mock_gas_id,
        ..
    } = prepared;
    // Resolve every authenticator, then union each one's inputs into the
    // transaction's checked inputs, enforcing consistency (matching object read
    // results, compatible shared-object kinds) for ids that appear in more than
    // one set.
    let prepared_auths = prepare_authenticators(store, authenticators, env.protocol_config)?;
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
        .map(|(a, fn_ref, inputs)| MoveAuthenticatorForExecution {
            authenticator: a.clone(),
            function_ref: fn_ref.clone(),
            input_objects: CheckedInputObjects::new_with_checked_transaction_inputs(inputs.clone()),
        })
        .collect::<Vec<_>>();

    let (inner_temp_store, _, effects, _timings, execution_result, authentication_error) = executor
        .authenticate_then_execute_transaction_to_effects(
            store,
            env.protocol_config,
            env.limits_metrics.clone(),
            false,
            env.certificate_deny_set,
            &env.epoch_id,
            env.epoch_timestamp_ms,
            gas_data,
            gas_status,
            MoveAuthenticatorsForExecution::Resolved(exec_authenticators),
            union_checked,
            None,
            kind,
            signer,
            transaction.digest(),
            auth_context_data,
            trace_builder_opt,
        );

    let authenticator_outcome = authentication_error.map_or(Ok(()), |kind| Err(kind.into()));

    // The authenticator engine entry point does not return per-command
    // results, so a signed `MoveAuthenticator` run carries none.
    Ok((
        simulate::simulation_result(
            store,
            inner_temp_store,
            effects,
            execution_result.map(|_| Vec::new()),
            mock_gas_id,
            transaction.gas_data().clone(),
        ),
        authenticator_outcome,
        transaction,
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
/// time (no packages other than those its type arguments name, no address-owned
/// objects, no mutable shared objects, …) are applied to each authenticator's
/// inputs.
pub(super) fn prepare_authenticators(
    store: &dyn BackingStore,
    authenticators: Vec<MoveAuthenticator>,
    protocol_config: &ProtocolConfig,
) -> Result<Vec<PreparedAuthenticator>, VmSdkError> {
    let mut prepared = Vec::with_capacity(authenticators.len());
    for authenticator in authenticators {
        let auth_input_object_kinds = authenticator
            .input_objects(protocol_config.move_authenticator_type_arg_packages_as_inputs());
        let auth_input_objects = load_input_objects(store, &auth_input_object_kinds)?;
        let auth_checked = iota_transaction_checks::check_move_authenticator_input_for_validation(
            auth_input_objects,
        )
        .map_err(|e| ValidationError::new("authenticator input check", e))?;
        let fn_ref = resolve_authenticator_function_ref(store, &authenticator, protocol_config)?;
        prepared.push((authenticator, fn_ref, auth_checked.into_inner()));
    }
    Ok(prepared)
}

/// Build the [`AuthContextData`] an authenticator run needs: the transaction
/// bytes, the sender/sponsor auth digests, and the sender/sponsor authenticator
/// function refs. The function refs are resolved from the full set of
/// authenticators, matching the node, even when only a subset is executed.
pub(super) fn build_auth_context_data(
    transaction: &Transaction,
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
/// returning the aggregate outcome (`Err` if any authenticator rejected).
/// Models the node's pre-consensus signing check.
pub(super) fn authenticate_only(
    env: &ExecutionEnv,
    store: &dyn BackingStore,
    transaction: &Transaction,
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
    let (_gas_status, result) = env.executor.authenticate_transaction(
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
    );
    Ok(result)
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
        tx_input_objects.inner(),
        receiving_objects,
        &per_authenticator_input_objects
            .iter()
            .map(|c| c.inner())
            .collect(),
        store.as_object_store(),
    )
    .map_err(|e| ValidationError::new("coin deny-list check", e).into())
}

/// Load the [`AuthenticatorFunctionRefForExecution`] from the account object's
/// dynamic field in the store.
fn resolve_authenticator_function_ref(
    store: &dyn BackingStore,
    authenticator: &MoveAuthenticator,
    protocol_config: &ProtocolConfig,
) -> Result<AuthenticatorFunctionRefForExecution, VmSdkError> {
    let (account_object_id, _version, _digest) = authenticator
        .object_to_authenticate_components()
        .map_err(|e| VmError::new(format!("invalid object_to_authenticate: {e}")))?;

    if protocol_config.reject_immutable_account_objects() {
        let account_object = store
            .as_object_store()
            .try_get_object(&account_object_id)
            .map_err(|e| StoreError::new("load account object", e))?
            .ok_or(VmSdkError::MissingObject {
                id: account_object_id,
                version: None,
            })?;

        if account_object.is_immutable() {
            return Err(ValidationError::new(
                "account object check",
                UserInputError::ImmutableAccountObjectNotSupported {
                    object_id: account_object_id,
                },
            )
            .into());
        }
    }

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
        .get_annotated_layout(&event.struct_tag)
        .map_err(|e| {
            ExecutionError::new(format!("resolve layout for {}: {e}", event.struct_tag))
        })?;
    // `BoundedVisitor` bounds the deserialized value's depth and allocation,
    // like every node-side decoder of externally-sourced bytes.
    let value = BoundedVisitor::deserialize_value(&event.contents, &layout.into_layout())
        .map_err(|e| ExecutionError::new(format!("bcs deserialize {}: {e}", event.struct_tag)))?;

    Ok(DecodedEvent {
        event: event.clone(),
        value,
    })
}
