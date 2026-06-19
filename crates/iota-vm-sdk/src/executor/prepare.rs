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

use iota_sdk_types::{Event, ObjectId, Owner};
use iota_types::{
    account_abstraction::{
        account::AuthenticatorFunctionRefV1Key,
        authenticator_function::{
            AuthenticatorFunctionRefForExecution, AuthenticatorFunctionRefV1, extract_auth_fun_refs,
        },
    },
    auth_context::AuthContextData,
    digests::TransactionDigest,
    dynamic_field::{self, Field},
    effects::TransactionEffectsAPI,
    gas::IotaGasStatus,
    layout_resolver::LayoutResolver,
    move_authenticator::MoveAuthenticator,
    object::{MoveObject, MoveObjectExt, OBJECT_START_VERSION, Object},
    storage::BackingStore,
    transaction::{
        CheckedInputObjects, InputObjectKind, InputObjects, ObjectReadResult,
        ReceivingObjectReadResult, ReceivingObjects, TransactionData, TransactionDataAPI,
    },
    transaction_executor::SimulateTransactionResult,
};
use move_core_types::annotated_value::{MoveDatatypeLayout, MoveTypeLayout, MoveValue};
use move_trace_format::format::MoveTraceBuilder;

use crate::{
    error::{ExecutionError, ValidationError, VmError, VmSdkError},
    executor::{
        env::ExecutionEnv,
        types::{DecodedEvent, ExecutionMode},
    },
};

/// Balance of the mock gas coin minted for a gasless transaction; same as the
/// node's `SIMULATION_GAS_COIN_VALUE`.
const SIMULATION_GAS_COIN_VALUE: u64 = 1_000_000_000 * iota_types::gas_coin::NANOS_PER_IOTA;

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
    authenticator_gas_budget: u64,
) -> Result<PreparedTransaction, VmSdkError> {
    transaction
        .validity_check_no_gas_check(&env.protocol_config)
        .map_err(|e| ValidationError::new("transaction validity check", e))?;

    // Update gas payment references to match actual object versions in the store.
    let updated_gas: Vec<_> = transaction
        .gas()
        .iter()
        .map(|gas_ref| {
            store
                .as_object_store()
                .get_object(&gas_ref.object_id)
                .map(|obj| obj.object_ref())
                .unwrap_or(*gas_ref)
        })
        .collect();
    transaction.gas_data_mut().objects = updated_gas;

    let raw_input_object_kinds = transaction
        .input_objects()
        .map_err(|e| ValidationError::new("collect input objects", e))?;
    let receiving_object_refs = transaction.receiving_objects();

    let (input_object_kinds, mut input_objects) =
        build_input_objects(store, &raw_input_object_kinds)?;
    let receiving_objects = build_receiving_objects(store, &receiving_object_refs)?;

    // Mint a one-shot mock gas coin if the transaction carries no gas payment,
    // the same coin the node mints in this case.
    let mock_gas_id = if transaction.gas().is_empty() {
        let mock_gas_object = Object::new_move(
            MoveObject::new_gas_coin(
                OBJECT_START_VERSION,
                ObjectId::MAX,
                SIMULATION_GAS_COIN_VALUE,
            ),
            Owner::Address(transaction.gas_data().owner),
            TransactionDigest::GENESIS_MARKER,
        );
        let mock_gas_object_ref = mock_gas_object.object_ref();
        transaction.gas_data_mut().objects = vec![mock_gas_object_ref];
        input_objects.push(ObjectReadResult::new_from_gas_object(&mock_gas_object));
        Some(mock_gas_object.id())
    } else {
        None
    };

    // Offline default: an empty deny-list. A live validator may be configured
    // with denied addresses/packages, so this check will not match a real chain.
    let deny_config = iota_config::transaction_deny_config::TransactionDenyConfig::default();
    iota_transaction_checks::deny::check_transaction_for_validation(
        &transaction,
        &[],
        &input_object_kinds,
        &receiving_object_refs,
        &deny_config,
        store,
    )
    .map_err(|e| ValidationError::new("deny-list check", e))?;

    let (gas_status, checked_input_objects) = if matches!(mode, ExecutionMode::DevInspect) {
        let checked_input_objects = iota_transaction_checks::check_dev_inspect_input(
            &env.protocol_config,
            transaction.kind(),
            input_objects,
            receiving_objects,
        )
        .map_err(|e| ValidationError::new("dev-inspect input check", e))?;
        let gas_status = IotaGasStatus::new(
            transaction.gas_budget(),
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
        let (gas_status, checked_input_objects) = iota_transaction_checks::check_transaction_input(
            &env.protocol_config,
            env.reference_gas_price,
            &transaction,
            input_objects,
            &receiving_objects,
            &env.bytecode_verifier_metrics,
            &verifier_signing_config,
            authenticator_gas_budget,
        )
        .map_err(|e| ValidationError::new("transaction input check", e))?;
        // `check_transaction_input` meters the signing phase and caps the budget
        // at `max_auth_gas` when an authenticator budget is set. The combined
        // authenticator + body run executes to effects, so meter it at the full
        // transaction budget; the standalone verdict re-run keeps `max_auth_gas`.
        let gas_status = if authenticator_gas_budget > 0 {
            IotaGasStatus::new(
                transaction.gas_budget(),
                transaction.gas_price(),
                env.reference_gas_price,
                &env.protocol_config,
            )
            .map_err(|e| ValidationError::new("gas status", e))?
        } else {
            gas_status
        };
        (gas_status, checked_input_objects)
    };

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

/// Run a transaction whose sender and/or sponsor authorize via a
/// `MoveAuthenticator`, returning the simulation together with an aggregate
/// verdict over all authenticators.
///
/// Every `MoveAuthenticator` on the transaction is resolved and executed — the
/// sender's and, for a sponsored transaction, the sponsor's. A successful run
/// implies all of them accepted. On a failed run the failure may come from any
/// authenticator or from the transaction body, so the authenticators are
/// re-executed alone for an unambiguous verdict: `Err` if any rejected, `Ok` if
/// they all passed and the body was at fault. The re-run is sound — the
/// authentication phase discards writes.
pub(super) fn execute_with_move_authenticators(
    env: &ExecutionEnv,
    store: &dyn BackingStore,
    prepared: PreparedTransaction,
    authenticators: Vec<MoveAuthenticator>,
    auth_digests: (
        iota_types::digests::Digest,
        Option<iota_types::digests::Digest>,
    ),
    authenticator_gas_budget: u64,
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

    // Resolve each authenticator's input objects and function ref, unioning
    // every authenticator's inputs into the transaction's checked inputs.
    // Offline default: the per-authenticator object restrictions a node enforces
    // (`check_move_authenticator_objects`) are not applied here, so inputs a live
    // chain would reject at signing time still run.
    let mut union_inputs = checked_input_objects.into_inner();
    let mut prepared_auths = Vec::with_capacity(authenticators.len());
    for authenticator in authenticators {
        let auth_input_object_kinds = authenticator.input_objects();
        let (_, auth_input_objects) = build_input_objects(store, &auth_input_object_kinds)?;
        for obj in auth_input_objects.iter() {
            if union_inputs.find_object_id_mut(obj.id()).is_none() {
                union_inputs.push(obj.clone());
            }
        }
        let fn_ref = resolve_authenticator_function_ref(store, &authenticator)?;
        prepared_auths.push((authenticator, fn_ref, auth_input_objects));
    }
    let union_checked = CheckedInputObjects::new_with_checked_transaction_inputs(union_inputs);

    let tx_data_bytes = bcs::to_bytes(&transaction)
        .map_err(|e| VmError::new(format!("serialize transaction data: {e}")))?;
    let (kind, signer, gas_data) = transaction.execution_parts();

    // Map each signer (sender / sponsor) to its authenticator function ref.
    let (sender_auth_digest, sponsor_auth_digest) = auth_digests;
    let (sender_authenticator_function_ref, sponsor_authenticator_function_ref) =
        extract_auth_fun_refs(signer, gas_data.owner, |address| {
            prepared_auths
                .iter()
                .find(|(a, _, _)| a.address().ok() == Some(address))
                .map(|(_, fn_ref, _)| fn_ref.authenticator_function_ref.clone())
        });
    let auth_context_data = AuthContextData {
        transaction_data_bytes: tx_data_bytes,
        sender_auth_digest,
        sponsor_auth_digest,
        sender_authenticator_function_ref,
        sponsor_authenticator_function_ref,
    };

    // `CheckedInputObjects` is not `Clone`; rebuild the per-authenticator inputs
    // (as `CheckedInputObjects`) for each engine call.
    let exec_authenticators = || {
        prepared_auths
            .iter()
            .map(|(a, fn_ref, inputs)| {
                (
                    a.clone(),
                    fn_ref.clone(),
                    CheckedInputObjects::new_with_checked_transaction_inputs(inputs.clone()),
                )
            })
            .collect::<Vec<_>>()
    };

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
            gas_data.clone(),
            gas_status,
            exec_authenticators(),
            union_checked,
            kind.clone(),
            signer,
            transaction.digest(),
            auth_context_data.clone(),
            trace_builder_opt,
        );

    let verdict = if effects.status().is_success() {
        Ok(())
    } else {
        // The combined run failed; re-run the authenticators alone to learn
        // whether an authenticator rejected the transaction or the body failed.
        // Meter with the authenticator budget the signing phase uses
        // (`max_auth_gas`), not the transaction budget — a smaller tx budget
        // would starve the re-run and look like a rejection.
        let verdict_gas_status = IotaGasStatus::new(
            authenticator_gas_budget,
            transaction.gas_price(),
            env.reference_gas_price,
            &env.protocol_config,
        )
        .map_err(|e| ValidationError::new("authenticator verdict gas status", e))?;
        // `authenticate_transaction` takes the inner `AuthenticatorFunctionRef`
        // and the union of all authenticator input objects.
        let verdict_authenticators = prepared_auths
            .iter()
            .map(|(a, fn_ref, inputs)| {
                (
                    a.clone(),
                    fn_ref.authenticator_function_ref.clone(),
                    CheckedInputObjects::new_with_checked_transaction_inputs(inputs.clone()),
                )
            })
            .collect::<Vec<_>>();
        let per_auth_checked: Vec<CheckedInputObjects> = prepared_auths
            .iter()
            .map(|(_, _, inputs)| {
                CheckedInputObjects::new_with_checked_transaction_inputs(inputs.clone())
            })
            .collect();
        let per_auth_checked_refs: Vec<&CheckedInputObjects> = per_auth_checked.iter().collect();
        let aggregated_auth_inputs =
            iota_transaction_checks::aggregate_authenticator_input_objects(&per_auth_checked_refs)
                .map_err(|e| ValidationError::new("aggregate authenticator inputs", e))?;
        env.executor.authenticate_transaction(
            store,
            &env.protocol_config,
            env.limits_metrics.clone(),
            &env.epoch_id,
            env.epoch_timestamp_ms,
            gas_data,
            verdict_gas_status,
            verdict_authenticators,
            aggregated_auth_inputs,
            kind,
            signer,
            transaction.digest(),
            auth_context_data,
            &mut None,
        )
    };

    Ok((
        SimulateTransactionResult {
            input_objects: inner_temp_store.input_objects,
            output_objects: inner_temp_store.written,
            events: effects.events_digest().map(|_| inner_temp_store.events),
            effects,
            // The authenticator engine entry point does not return per-command
            // results, so a signed `MoveAuthenticator` run carries none.
            execution_result: execution_result.map(|_| Vec::new()),
            mock_gas_id,
            suggested_gas_price: None,
        },
        verdict,
    ))
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

    let field_id = dynamic_field::derive_dynamic_field_id(
        account_object_id,
        &AuthenticatorFunctionRefV1Key::tag().into(),
        &AuthenticatorFunctionRefV1Key::default().to_bcs_bytes(),
    )
    .map_err(|e| VmError::new(format!("derive authenticator field id: {e}")))?;

    let field_obj = store
        .as_object_store()
        .get_object(&field_id)
        .ok_or(VmSdkError::missing_object(field_id, None))?;

    let field_move_object = field_obj.data.as_struct_opt().ok_or_else(|| {
        VmError::new("authenticator dynamic field: field object is not a Move object")
    })?;

    let field: Field<AuthenticatorFunctionRefV1Key, AuthenticatorFunctionRefV1> = field_move_object
        .to_rust()
        .map_err(|e| VmError::new(format!("deserialize AuthenticatorFunctionRefV1: {e}")))?;

    Ok(AuthenticatorFunctionRefForExecution::new_v1(
        field.value,
        field_obj.object_ref(),
        field_obj.owner,
        field_obj.storage_rebate,
        field_obj.previous_transaction,
    ))
}

/// Build `InputObjects` from a store, using the latest fetched versions.
fn build_input_objects(
    store: &dyn BackingStore,
    input_object_kinds: &[InputObjectKind],
) -> Result<(Vec<InputObjectKind>, InputObjects), VmSdkError> {
    let mut updated_kinds = Vec::new();
    let mut input_objects = Vec::new();
    for kind in input_object_kinds {
        let obj = store
            .as_object_store()
            .get_object(&kind.object_id())
            .ok_or(VmSdkError::missing_object(kind.object_id(), kind.version()))?;

        let updated_kind = match kind {
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

        input_objects.push(ObjectReadResult::new(updated_kind, obj.into()));
        updated_kinds.push(updated_kind);
    }
    Ok((updated_kinds, input_objects.into()))
}

fn build_receiving_objects(
    store: &dyn BackingStore,
    receiving_object_refs: &[iota_types::base_types::ObjectRef],
) -> Result<ReceivingObjects, VmSdkError> {
    let mut receiving_objects = Vec::new();
    for objref in receiving_object_refs {
        let obj = store
            .as_object_store()
            .get_object(&objref.object_id)
            .ok_or(VmSdkError::missing_object(
                objref.object_id,
                Some(objref.version),
            ))?;
        let updated_ref = obj.object_ref();
        receiving_objects.push(ReceivingObjectReadResult::new(updated_ref, obj.into()));
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
    let value = match layout {
        MoveDatatypeLayout::Struct(s) => {
            MoveValue::simple_deserialize(&event.contents, &MoveTypeLayout::Struct(s))
        }
        MoveDatatypeLayout::Enum(e_layout) => {
            MoveValue::simple_deserialize(&event.contents, &MoveTypeLayout::Enum(e_layout))
        }
    }
    .map_err(|e| ExecutionError::new(format!("bcs deserialize {}: {e}", event.type_)))?;

    Ok(DecodedEvent {
        event: event.clone(),
        value,
    })
}
