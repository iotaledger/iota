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

use iota_sdk_types::ObjectId;
use iota_types::{
    account_abstraction::{
        account::AuthenticatorFunctionRefV1Key,
        authenticator_function::{
            AuthenticatorFunctionRefForExecution, AuthenticatorFunctionRefV1,
        },
    },
    digests::TransactionDigest,
    dynamic_field::{self, Field},
    effects::TransactionEffectsAPI,
    event::Event,
    gas::IotaGasStatus,
    gas_coin::NANOS_PER_IOTA,
    layout_resolver::LayoutResolver,
    move_authenticator::MoveAuthenticator,
    object::{MoveObject, MoveObjectExt, Object, Owner},
    storage::BackingStore,
    transaction::{
        CheckedInputObjects, InputObjectKind, InputObjects, ObjectReadResult,
        ReceivingObjectReadResult, ReceivingObjects, TransactionData, TransactionDataAPI,
    },
    transaction_executor::SimulateTransactionResult,
};
use move_core_types::annotated_value::{MoveDatatypeLayout, MoveTypeLayout, MoveValue};
use move_trace_format::format::MoveTraceBuilder;

use super::{
    env::ExecutionEnv,
    types::{DecodedEvent, ExecutionMode},
};
use crate::error::{ExecutionError, ValidationError, VmSdkError};

/// Value the VM stuffs into a mock gas coin when a transaction has no explicit
/// gas payment. One IOTA's worth of NANOs — wide enough to cover any realistic
/// single-tx gas budget.
const MOCK_GAS_COIN_NANOS: u64 = 1_000_000_000 * NANOS_PER_IOTA;

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
                .map(|obj| obj.compute_object_reference())
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

    // Mint a one-shot mock gas coin if the transaction carries no gas payment.
    let mock_gas_id = if transaction.gas().is_empty() {
        let mock_gas_object = Object::new_move(
            MoveObject::new_gas_coin(1.into(), ObjectId::MAX, MOCK_GAS_COIN_NANOS),
            Owner::Address(transaction.gas_data().owner),
            TransactionDigest::ZERO,
        );
        let mock_gas_object_ref = mock_gas_object.compute_object_reference();
        transaction.gas_data_mut().objects = vec![mock_gas_object_ref];
        input_objects.push(ObjectReadResult::new_from_gas_object(&mock_gas_object));
        Some(mock_gas_object.id())
    } else {
        None
    };

    let deny_config = iota_config::transaction_deny_config::TransactionDenyConfig::default();
    let receiving_object_refs = transaction.receiving_objects();
    iota_transaction_checks::deny::check_transaction_for_signing(
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
        let verifier_signing_config =
            iota_config::verifier_signing_config::VerifierSigningConfig::default();
        iota_transaction_checks::check_transaction_input(
            &env.protocol_config,
            env.reference_gas_price,
            &transaction,
            input_objects,
            &receiving_objects,
            &env.bytecode_verifier_metrics,
            &verifier_signing_config,
            authenticator_gas_budget,
        )
        .map_err(|e| ValidationError::new("transaction input check", e))?
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
    // The current `dev_inspect_transaction` engine entry point does not accept a
    // `MoveTraceBuilder`; instruction tracing is only available on the
    // authenticator path (`authenticate_then_execute_transaction_to_effects`).
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

pub(super) fn execute_with_move_authenticator(
    env: &ExecutionEnv,
    store: &dyn BackingStore,
    prepared: PreparedTransaction,
    authenticator: MoveAuthenticator,
    auth_digests: (
        iota_types::digests::Digest,
        Option<iota_types::digests::Digest>,
    ),
    trace_builder_opt: &mut Option<MoveTraceBuilder>,
) -> Result<SimulateTransactionResult, VmSdkError> {
    use iota_types::{
        account_abstraction::authenticator_function::extract_auth_fun_refs,
        auth_context::AuthContextData,
    };

    let PreparedTransaction {
        transaction,
        gas_status,
        checked_input_objects,
        mock_gas_id,
    } = prepared;

    // Resolve the authenticator's input objects (separate from tx inputs).
    let auth_input_object_kinds = authenticator.input_objects();
    let (_, auth_input_objects) = build_input_objects(store, &auth_input_object_kinds)?;

    // Union of transaction + authenticator inputs for the main execution.
    let mut union_inputs = checked_input_objects.into_inner();
    for obj in auth_input_objects.iter() {
        if union_inputs.find_object_id_mut(obj.id()).is_none() {
            union_inputs.push(obj.clone());
        }
    }
    let union_checked = CheckedInputObjects::new_with_checked_transaction_inputs(union_inputs);
    let auth_checked = CheckedInputObjects::new_with_checked_transaction_inputs(auth_input_objects);

    let authenticator_fn_ref = resolve_authenticator_function_ref(store, &authenticator)?;
    let tx_data_bytes =
        bcs::to_bytes(&transaction).expect("TransactionData serialization cannot fail");

    let (kind, signer, gas_data) = transaction.execution_parts();

    // Build the auth context: map the authenticator's address to its function
    // ref for the sender (and sponsor, if sponsored).
    let (sender_auth_digest, sponsor_auth_digest) = auth_digests;
    let authenticator_address = authenticator.address().ok();
    let (sender_authenticator_function_ref, sponsor_authenticator_function_ref) =
        extract_auth_fun_refs(signer, gas_data.owner, |address| {
            if authenticator_address == Some(address) {
                Some(authenticator_fn_ref.authenticator_function_ref.clone())
            } else {
                None
            }
        });
    let auth_context_data = AuthContextData {
        transaction_data_bytes: tx_data_bytes,
        sender_auth_digest,
        sponsor_auth_digest,
        sender_authenticator_function_ref,
        sponsor_authenticator_function_ref,
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
            gas_data,
            gas_status,
            vec![(authenticator, authenticator_fn_ref, auth_checked)],
            union_checked,
            kind,
            signer,
            transaction.digest(),
            auth_context_data,
            trace_builder_opt,
        );

    Ok(SimulateTransactionResult {
        input_objects: inner_temp_store.input_objects,
        output_objects: inner_temp_store.written,
        events: effects.events_digest().map(|_| inner_temp_store.events),
        effects,
        execution_result: execution_result.map(|_| Vec::new()),
        mock_gas_id,
        suggested_gas_price: None,
    })
}

/// Load the [`AuthenticatorFunctionRefForExecution`] from the account object's
/// dynamic field in the store.
fn resolve_authenticator_function_ref(
    store: &dyn BackingStore,
    authenticator: &MoveAuthenticator,
) -> Result<AuthenticatorFunctionRefForExecution, VmSdkError> {
    let (account_object_id, _version, _digest) = authenticator
        .object_to_authenticate_components()
        .map_err(|e| ValidationError::new("invalid object_to_authenticate", e))?;

    let field_id = dynamic_field::derive_dynamic_field_id(
        account_object_id,
        &AuthenticatorFunctionRefV1Key::tag().into(),
        &AuthenticatorFunctionRefV1Key::default().to_bcs_bytes(),
    )
    .map_err(|e| ValidationError::new("derive authenticator field id", e))?;

    let field_obj =
        store
            .as_object_store()
            .get_object(&field_id)
            .ok_or(VmSdkError::MissingObject {
                id: field_id,
                version: None,
            })?;

    let field_move_object = field_obj.data.as_struct_opt().ok_or_else(|| {
        ValidationError::new(
            "authenticator dynamic field",
            "field object is not a Move object",
        )
    })?;

    let field: Field<AuthenticatorFunctionRefV1Key, AuthenticatorFunctionRefV1> = field_move_object
        .to_rust()
        .map_err(|e| ValidationError::new("deserialize AuthenticatorFunctionRefV1", e))?;

    Ok(AuthenticatorFunctionRefForExecution::new_v1(
        field.value,
        field_obj.compute_object_reference(),
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
            .ok_or(VmSdkError::MissingObject {
                id: kind.object_id(),
                version: None,
            })?;

        let updated_kind = match kind {
            InputObjectKind::MovePackage(_) => *kind,
            InputObjectKind::ImmOrOwnedMoveObject(_) => {
                InputObjectKind::ImmOrOwnedMoveObject(obj.compute_object_reference())
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
            .ok_or(VmSdkError::MissingObject {
                id: objref.object_id,
                version: Some(objref.version),
            })?;
        let updated_ref = obj.compute_object_reference();
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
        package_id: event.package_id,
        module: event.module.clone(),
        name: event.type_.name().clone(),
        sender: event.sender,
        type_tag: event.type_.clone(),
        value,
    })
}
