// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::collections::VecDeque;

use move_binary_format::errors::PartialVMResult;
use move_core_types::{gas_algebra::InternalGas, vm_status::StatusCode};
use move_vm_runtime::{native_charge_gas_early_exit, native_functions::NativeContext};
use move_vm_types::{
    loaded_data::runtime_types::Type,
    natives::function::NativeResult,
    pop_arg,
    values::{StructRef, Value},
};
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
/// Implementation of the Move native function `fun native_tx_commands():
/// &vector<Command>`
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
/// Implementation of the Move native function `fun native_tx_inputs():
/// &vector<CallArg>`
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
/// Implementation of the Move native function `fun native_replace(auth_digest:
/// vector<u8>,tx_inputs: vector<CallArg>,tx_commands: vector<Command>)`
/// ****************************************************************************
pub fn native_replace(
    context: &mut NativeContext,
    ty_args: Vec<Type>,
    mut args: VecDeque<Value>,
) -> PartialVMResult<NativeResult> {
    assert!(ty_args.is_empty());
    debug_assert!(args.len() == 3);

    let auth_context_replace_cost_params = get_extension!(context, NativesCostTable)?
        .auth_context_replace_cost_params
        .clone();
    native_charge_gas_early_exit!(
        context,
        auth_context_replace_cost_params.auth_context_replace_cost_base
    );

    let tx_commands_value = pop_arg!(args, Vec<Value>);
    let tx_inputs_value = pop_arg!(args, Vec<Value>);
    let auth_digest_value = pop_arg!(args, Vec<u8>);

    let auth_context: &mut AuthenticationContext = get_extension_mut!(context)?;

    auth_context.replace(auth_digest_value, tx_inputs_value, tx_commands_value)?;

    Ok(NativeResult::ok(context.gas_used(), smallvec![]))
}
