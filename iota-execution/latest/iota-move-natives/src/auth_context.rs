// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::collections::VecDeque;

use move_binary_format::errors::{PartialVMError, PartialVMResult};
use move_core_types::{
    gas_algebra::InternalGas, runtime_value::MoveTypeLayout, vm_status::StatusCode,
};
use move_vm_runtime::{native_charge_gas_early_exit, native_functions::NativeContext};
use move_vm_types::{
    loaded_data::runtime_types::Type, natives::function::NativeResult, pop_arg, values::Value,
};
use smallvec::smallvec;

use crate::{
    NativesCostTable, authentication_context::AuthenticationContext, get_extension,
    get_extension_mut,
};

#[derive(Clone)]
pub struct AuthContextDigestCostParams {
    pub auth_context_digest_cost_base: Option<InternalGas>,
}

/// ****************************************************************************
/// native fun native_digest
/// Implementation of the Move native function `fun native_digest():
/// &vector<u8>`
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
        auth_context_digest_cost_params
            .auth_context_digest_cost_base
            .ok_or_else(|| {
                PartialVMError::new(StatusCode::UNKNOWN_INVARIANT_VIOLATION_ERROR)
                    .with_message("Gas cost base for native_digest not available".to_string())
            })?
    );

    let auth_context: &mut AuthenticationContext = get_extension_mut!(context)?;

    let digest_ref = auth_context.digest_ref()?;

    Ok(NativeResult::ok(context.gas_used(), smallvec![digest_ref]))
}

#[derive(Clone)]
pub struct AuthContextTxCommandsCostParams {
    pub auth_context_tx_commands_cost_base: Option<InternalGas>,
    pub auth_context_tx_commands_cost_per_byte: Option<InternalGas>,
}

/// ****************************************************************************
/// native fun native_tx_commands
/// Implementation of the Move native function `fun native_tx_commands():
/// &vector<Command>`
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
        auth_context_tx_commands_cost_params
            .auth_context_tx_commands_cost_base
            .ok_or_else(|| {
                PartialVMError::new(StatusCode::UNKNOWN_INVARIANT_VIOLATION_ERROR)
                    .with_message("Gas cost base for native_tx_commands not available".to_string())
            })?
    );

    let command_type = ty_args.pop().unwrap();
    let command_move_layout = resolve_move_layout(context, &command_type)?;

    let auth_context: &mut AuthenticationContext = get_extension_mut!(context)?;

    let (tx_commands_ref, tx_commands_value_size) =
        auth_context.tx_commands_ref(command_move_layout)?;

    native_charge_gas_early_exit!(
        context,
        auth_context_tx_commands_cost_params
            .auth_context_tx_commands_cost_per_byte
            .ok_or_else(|| {
                PartialVMError::new(StatusCode::UNKNOWN_INVARIANT_VIOLATION_ERROR).with_message(
                    "Gas cost per byte for native_tx_commands not available".to_string(),
                )
            })?
            * u64::from(tx_commands_value_size).into()
    );

    Ok(NativeResult::ok(
        context.gas_used(),
        smallvec![tx_commands_ref],
    ))
}

#[derive(Clone)]
pub struct AuthContextTxInputsCostParams {
    pub auth_context_tx_inputs_cost_base: Option<InternalGas>,
    pub auth_context_tx_inputs_cost_per_byte: Option<InternalGas>,
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
        auth_context_tx_inputs_cost_params
            .auth_context_tx_inputs_cost_base
            .ok_or_else(
                || PartialVMError::new(StatusCode::UNKNOWN_INVARIANT_VIOLATION_ERROR)
                    .with_message("Gas cost base for native_tx_inputs not available".to_string())
            )?
    );

    let input_type = ty_args.pop().unwrap();
    let input_move_layout = resolve_move_layout(context, &input_type)?;

    let auth_context: &mut AuthenticationContext = get_extension_mut!(context)?;

    let (tx_inputs_ref, tx_inputs_value_size) = auth_context.tx_inputs_ref(input_move_layout)?;

    native_charge_gas_early_exit!(
        context,
        auth_context_tx_inputs_cost_params
            .auth_context_tx_inputs_cost_per_byte
            .ok_or_else(
                || PartialVMError::new(StatusCode::UNKNOWN_INVARIANT_VIOLATION_ERROR).with_message(
                    "Gas cost per byte for native_tx_inputs not available".to_string()
                )
            )?
            * u64::from(tx_inputs_value_size).into()
    );

    Ok(NativeResult::ok(
        context.gas_used(),
        smallvec![tx_inputs_ref],
    ))
}

#[derive(Clone)]
pub struct AuthContextReplaceCostParams {
    pub auth_context_replace_cost_base: Option<InternalGas>,
    pub auth_context_replace_cost_per_byte: Option<InternalGas>,
}

/// ****************************************************************************
/// native fun replace
/// Implementation of the Move native function `fun native_replace(auth_digest:
/// vector<u8>,tx_inputs: vector<CallArg>,tx_commands: vector<Command>)`
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
        auth_context_replace_cost_params
            .auth_context_replace_cost_base
            .ok_or_else(|| {
                PartialVMError::new(StatusCode::UNKNOWN_INVARIANT_VIOLATION_ERROR)
                    .with_message("Gas cost base for native_replace not available".to_string())
            })?
    );

    let args_size = args
        .iter()
        .fold(0_u64, |acc, v| acc + u64::from(v.legacy_size()));
    native_charge_gas_early_exit!(
        context,
        auth_context_replace_cost_params
            .auth_context_replace_cost_per_byte
            .ok_or_else(|| {
                PartialVMError::new(StatusCode::UNKNOWN_INVARIANT_VIOLATION_ERROR)
                    .with_message("Gas cost per byte for native_replace not available".to_string())
            })?
            * args_size.into()
    );

    let command_type = ty_args.pop().unwrap();
    let command_move_layout = resolve_move_layout(context, &command_type)?;
    let tx_commands_value = pop_arg!(args, Vec<Value>);

    let input_type = ty_args.pop().unwrap();
    let input_move_layout = resolve_move_layout(context, &input_type)?;
    let tx_inputs_value = pop_arg!(args, Vec<Value>);

    let auth_digest_value = pop_arg!(args, Vec<u8>);

    let auth_context: &mut AuthenticationContext = get_extension_mut!(context)?;

    auth_context.replace(
        auth_digest_value,
        tx_inputs_value,
        input_move_layout,
        tx_commands_value,
        command_move_layout,
    )?;

    Ok(NativeResult::ok(context.gas_used(), smallvec![]))
}

fn resolve_move_layout(context: &NativeContext, ty: &Type) -> PartialVMResult<MoveTypeLayout> {
    context.type_to_type_layout(ty)?.ok_or(
        PartialVMError::new(StatusCode::UNKNOWN_INVARIANT_VIOLATION_ERROR)
            .with_message(format!("Can't resolve `MoveTypeLayout` for {ty:?}")),
    )
}
