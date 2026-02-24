use std::collections::VecDeque;

use iota_types::{
    digests::MoveAuthenticatorDigest,
    transaction::{CallArg, Command},
};
use move_binary_format::errors::{PartialVMError, PartialVMResult};
use move_core_types::{
    gas_algebra::InternalGas, runtime_value::MoveTypeLayout, vm_status::StatusCode,
};
use move_vm_runtime::{native_charge_gas_early_exit, native_functions::NativeContext};
use move_vm_types::{
    loaded_data::runtime_types::Type, natives::function::NativeResult, pop_arg, values::Value,
};
use serde::{Serialize, de::DeserializeOwned};
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

    let auth_context: &AuthenticationContext = get_extension!(context)?;

    let digest = Value::vector_u8(auth_context.digest().into_inner());
    Ok(NativeResult::ok(context.gas_used(), smallvec![digest]))
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
    mut ty_args: Vec<Type>,
    args: VecDeque<Value>,
) -> PartialVMResult<NativeResult> {
    debug_assert!(ty_args.len() == 1);
    debug_assert!(args.is_empty());

    let auth_context_tx_commands_cost_params = get_extension!(context, NativesCostTable)?
        .auth_context_tx_commands_cost_params
        .clone();
    native_charge_gas_early_exit!(
        context,
        auth_context_tx_commands_cost_params.auth_context_tx_commands_cost_base
    );

    let command_type = ty_args.pop().unwrap();
    let command_move_layout = resolve_move_layout(context, &command_type)?;
    let commands_move_layout = MoveTypeLayout::Vector(Box::new(command_move_layout));

    let auth_context: &AuthenticationContext = get_extension!(context)?;

    let commands_value = to_value(&auth_context.tx_commands(), &commands_move_layout)?;

    Ok(NativeResult::ok(
        context.gas_used(),
        smallvec![commands_value],
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
    mut ty_args: Vec<Type>,
    args: VecDeque<Value>,
) -> PartialVMResult<NativeResult> {
    debug_assert!(ty_args.len() == 1);
    debug_assert!(args.is_empty());

    let auth_context_tx_inputs_cost_params = get_extension!(context, NativesCostTable)?
        .auth_context_tx_inputs_cost_params
        .clone();
    native_charge_gas_early_exit!(
        context,
        auth_context_tx_inputs_cost_params.auth_context_tx_inputs_cost_base
    );

    let input_type = ty_args.pop().unwrap();
    let input_move_layout = resolve_move_layout(context, &input_type)?;
    let inputs_move_layout = MoveTypeLayout::Vector(Box::new(input_move_layout));

    let auth_context: &AuthenticationContext = get_extension!(context)?;

    let inputs_value = to_value(&auth_context.tx_inputs(), &inputs_move_layout)?;

    Ok(NativeResult::ok(
        context.gas_used(),
        smallvec![inputs_value],
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
        .map(|value| from_value(value, &command_move_layout))
        .collect::<PartialVMResult<Vec<Command>>>()?;

    let input_type = ty_args.pop().unwrap();
    let input_move_layout = resolve_move_layout(context, &input_type)?;

    let tx_inputs = pop_arg!(args, Vec<Value>)
        .into_iter()
        .map(|value| from_value(value, &input_move_layout))
        .collect::<PartialVMResult<Vec<CallArg>>>()?;

    let auth_digest = MoveAuthenticatorDigest::try_from(pop_arg!(args, Vec<u8>).as_slice())
        .map_err(|err| {
            PartialVMError::new(StatusCode::UNKNOWN_INVARIANT_VIOLATION_ERROR)
                .with_message(err.to_string())
        })?;

    let auth_context: &mut AuthenticationContext = get_extension_mut!(context)?;

    auth_context.replace(auth_digest, tx_inputs, tx_commands)?;

    Ok(NativeResult::ok(context.gas_used(), smallvec![]))
}

fn to_value<T: ?Sized + Serialize>(
    input: &T,
    input_move_layout: &MoveTypeLayout,
) -> PartialVMResult<Value> {
    let bytes = bcs::to_bytes(input).map_err(|err| {
        PartialVMError::new(StatusCode::UNKNOWN_INVARIANT_VIOLATION_ERROR)
            .with_message(format!("Failed to serialize an input: {err}"))
    })?;
    Value::simple_deserialize(&bytes, input_move_layout).ok_or_else(|| {
        PartialVMError::new(StatusCode::UNKNOWN_INVARIANT_VIOLATION_ERROR)
            .with_message("Failed to deserialize an input to a Move value".to_string())
    })
}

fn from_value<T: DeserializeOwned>(
    value: Value,
    value_move_layout: &MoveTypeLayout,
) -> PartialVMResult<T> {
    let bytes = value.simple_serialize(value_move_layout).ok_or_else(|| {
        PartialVMError::new(StatusCode::UNKNOWN_INVARIANT_VIOLATION_ERROR)
            .with_message(format!("Failed to serialize a value"))
    })?;
    bcs::from_bytes::<T>(&bytes).map_err(|err| {
        PartialVMError::new(StatusCode::UNKNOWN_INVARIANT_VIOLATION_ERROR)
            .with_message(format!("Failed to deserialize a value: {err}"))
    })
}

fn resolve_move_layout(context: &NativeContext, ty: &Type) -> PartialVMResult<MoveTypeLayout> {
    match context.type_to_type_layout(ty)? {
        Some(move_layout) => Ok(move_layout),
        None => Err(
            PartialVMError::new(StatusCode::UNKNOWN_INVARIANT_VIOLATION_ERROR)
                .with_message(format!("Can't resolve `MoveTypeLayout` for {ty:?}")),
        ),
    }
}
