// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::collections::VecDeque;

use iota_types::{
    auth_context::{AuthContextCallArg, AuthContextCommand},
    digests::MoveAuthenticatorDigest,
};
use move_binary_format::errors::{PartialVMError, PartialVMResult};
use move_core_types::{
    gas_algebra::InternalGas, runtime_value::MoveTypeLayout, vm_status::StatusCode,
};
use move_vm_runtime::{native_charge_gas_early_exit, native_functions::NativeContext};
use move_vm_types::{
    loaded_data::runtime_types::Type,
    natives::function::NativeResult,
    pop_arg,
    values::{StructRef, Value},
};
use serde::de::DeserializeOwned;
use smallvec::smallvec;

use crate::{
    NativesCostTable, authentication_context::AuthenticationContext, get_extension,
    get_extension_mut,
};

#[derive(Clone)]
pub struct AuthContextDigestCostParams {
    pub auth_context_digest_cost_base: InternalGas,
}

/// ****************************************************************************
/// native fun native_digest
/// Implementation of the Move native function `fun native_digest(): vector<u8>`
/// ****************************************************************************
pub fn native_digest(
    context: &mut NativeContext,
    ty_args: Vec<Type>,
    args: VecDeque<Value>,
) -> PartialVMResult<NativeResult> {
    debug_assert!(ty_args.is_empty());
    debug_assert!(args.is_empty());

    let auth_context_digest_cost_params = get_extension!(context, NativesCostTable)?
        .auth_context_digest_cost_params
        .clone();
    native_charge_gas_early_exit!(
        context,
        auth_context_digest_cost_params.auth_context_digest_cost_base
    );

    let auth_context: &mut AuthenticationContext = get_extension_mut!(context)?;

    let digest_ref = auth_context
        .struct_with_digest()
        .borrow_global()
        .inspect_err(|err| assert!(err.major_status() != StatusCode::MISSING_DATA))?
        .value_as::<StructRef>()?
        .borrow_field(0)?;

    Ok(NativeResult::ok(context.gas_used(), smallvec![digest_ref]))
}

#[derive(Clone)]
pub struct AuthContextTxCommandsCostParams {
    pub auth_context_tx_commands_cost_base: InternalGas,
}

/// ****************************************************************************
/// native fun native_tx_commands
/// Implementation of the Move native function `fun native_tx_commands<C>():
/// vector<C>`
/// ****************************************************************************
pub fn native_tx_commands(
    context: &mut NativeContext,
    ty_args: Vec<Type>,
    args: VecDeque<Value>,
) -> PartialVMResult<NativeResult> {
    assert!(ty_args.is_empty());
    debug_assert!(args.is_empty());

    let auth_context_tx_commands_cost_params = get_extension!(context, NativesCostTable)?
        .auth_context_tx_commands_cost_params
        .clone();
    native_charge_gas_early_exit!(
        context,
        auth_context_tx_commands_cost_params.auth_context_tx_commands_cost_base
    );

    let auth_context: &mut AuthenticationContext = get_extension_mut!(context)?;
    let tx_commands_ref = auth_context
        .struct_with_tx_commands()
        .borrow_global()
        .inspect_err(|err| assert!(err.major_status() != StatusCode::MISSING_DATA))?
        .value_as::<StructRef>()?
        .borrow_field(0)?;

    Ok(NativeResult::ok(
        context.gas_used(),
        smallvec![tx_commands_ref],
    ))
}

#[derive(Clone)]
pub struct AuthContextTxInputsCostParams {
    pub auth_context_tx_inputs_cost_base: InternalGas,
}

/// ****************************************************************************
/// native fun native_tx_inputs
/// Implementation of the Move native function `fun native_tx_inputs<I>():
/// vector<I>`
/// ****************************************************************************
pub fn native_tx_inputs(
    context: &mut NativeContext,
    ty_args: Vec<Type>,
    args: VecDeque<Value>,
) -> PartialVMResult<NativeResult> {
    assert!(ty_args.is_empty());
    debug_assert!(args.is_empty());

    let auth_context_tx_inputs_cost_params = get_extension!(context, NativesCostTable)?
        .auth_context_tx_inputs_cost_params
        .clone();
    native_charge_gas_early_exit!(
        context,
        auth_context_tx_inputs_cost_params.auth_context_tx_inputs_cost_base
    );

    let auth_context: &mut AuthenticationContext = get_extension_mut!(context)?;

    let tx_inputs_ref = auth_context
        .struct_with_tx_inputs()
        .borrow_global()
        .inspect_err(|err| assert!(err.major_status() != StatusCode::MISSING_DATA))?
        .value_as::<StructRef>()?
        .borrow_field(0)?;

    Ok(NativeResult::ok(
        context.gas_used(),
        smallvec![tx_inputs_ref],
    ))
}

#[derive(Clone)]
pub struct AuthContextReplaceCostParams {
    pub auth_context_replace_cost_base: InternalGas,
}

/// ****************************************************************************
/// native fun replace
/// Implementation of the Move native function `fun native_replace<I,
/// C>(auth_digest: vector<u8>, tx_inputs: vector<I>, tx_commands: vector<C>)`
/// ****************************************************************************
pub fn native_replace(
    context: &mut NativeContext,
    mut ty_args: Vec<Type>,
    mut args: VecDeque<Value>,
) -> PartialVMResult<NativeResult> {
    debug_assert!(ty_args.len() == 2);
    debug_assert!(args.len() == 3);

    let auth_context_replace_cost_params = get_extension!(context, NativesCostTable)?
        .auth_context_replace_cost_params
        .clone();
    native_charge_gas_early_exit!(
        context,
        auth_context_replace_cost_params.auth_context_replace_cost_base
    );

    let command_type = ty_args.pop().unwrap();
    let command_move_layout = resolve_move_layout(context, &command_type)?;

    let tx_commands = pop_arg!(args, Vec<Value>)
        .into_iter()
        .map(|value| from_value::<AuthContextCommand>(value, &command_move_layout))
        .collect::<PartialVMResult<Vec<AuthContextCommand>>>()?;

    let input_type = ty_args.pop().unwrap();
    let input_move_layout = resolve_move_layout(context, &input_type)?;

    let tx_inputs = pop_arg!(args, Vec<Value>)
        .into_iter()
        .map(|value| from_value::<AuthContextCallArg>(value, &input_move_layout))
        .collect::<PartialVMResult<Vec<AuthContextCallArg>>>()?;

    let auth_digest = MoveAuthenticatorDigest::try_from(pop_arg!(args, Vec<u8>).as_slice())
        .map_err(|err| {
            PartialVMError::new(StatusCode::UNKNOWN_INVARIANT_VIOLATION_ERROR)
                .with_message(err.to_string())
        })?;

    let auth_context: &mut AuthenticationContext = get_extension_mut!(context)?;

    auth_context.replace(auth_digest, tx_inputs, tx_commands)?;

    Ok(NativeResult::ok(context.gas_used(), smallvec![]))
}

fn from_value<T: DeserializeOwned>(
    value: Value,
    value_move_layout: &MoveTypeLayout,
) -> PartialVMResult<T> {
    let bytes = value.simple_serialize(value_move_layout).ok_or_else(|| {
        PartialVMError::new(StatusCode::UNKNOWN_INVARIANT_VIOLATION_ERROR)
            .with_message("Failed to serialize a value".to_string())
    })?;
    bcs::from_bytes::<T>(&bytes).map_err(|err| {
        PartialVMError::new(StatusCode::UNKNOWN_INVARIANT_VIOLATION_ERROR)
            .with_message(format!("Failed to deserialize a value: {err}"))
    })
}

fn resolve_move_layout(context: &NativeContext, ty: &Type) -> PartialVMResult<MoveTypeLayout> {
    context.type_to_type_layout(ty)?.ok_or(
        PartialVMError::new(StatusCode::UNKNOWN_INVARIANT_VIOLATION_ERROR)
            .with_message(format!("Can't resolve `MoveTypeLayout` for {ty:?}")),
    )
}
