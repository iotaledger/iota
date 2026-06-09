// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::collections::VecDeque;

use iota_types::account_abstraction::authenticator_function::AuthenticatorFunctionRefV1;
use move_binary_format::errors::{PartialVMError, PartialVMResult};
use move_core_types::{
    gas_algebra::InternalGas, runtime_value::MoveTypeLayout, vm_status::StatusCode,
};
use move_vm_runtime::{native_charge_gas_early_exit, native_functions::NativeContext};
use move_vm_types::{
    loaded_data::runtime_types::Type,
    natives::function::NativeResult,
    pop_arg,
    values::{Struct, Value},
};
use smallvec::smallvec;

use crate::{
    NativesCostTable, authentication_context::AuthenticationContext, get_extension,
    get_extension_mut, utils,
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

/// ****************************************************************************
/// native fun native_sender_auth_digest
/// Implementation of the Move native function `fun native_sender_auth_digest():
/// &vector<u8>`
///
/// Returns the sender's auth digest. For MoveAuthenticator signatures equals
/// `MoveAuthenticator::digest()`; for others Blake2b256 of the serialized
/// (flag-prefixed) signature bytes.
/// ****************************************************************************
pub fn native_sender_auth_digest(
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
                PartialVMError::new(StatusCode::UNKNOWN_INVARIANT_VIOLATION_ERROR).with_message(
                    "Gas cost base for native_sender_auth_digest not available".to_string(),
                )
            })?
    );

    let auth_context: &mut AuthenticationContext = get_extension_mut!(context)?;

    let digest_ref = auth_context.sender_auth_digest_ref()?;

    Ok(NativeResult::ok(context.gas_used(), smallvec![digest_ref]))
}

/// ****************************************************************************
/// native fun native_sponsor_auth_digest
/// Implementation of the Move native function `fun
/// native_sponsor_auth_digest(): &Option<vector<u8>>`
///
/// Returns `None` for non-sponsored transactions. For sponsored transactions,
/// returns the sponsor's auth digest: `MoveAuthenticator::digest()` for
/// MoveAuthenticator signatures; Blake2b256 of the serialized (flag-prefixed)
/// signature bytes for all others.
/// ****************************************************************************
pub fn native_sponsor_auth_digest(
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
                PartialVMError::new(StatusCode::UNKNOWN_INVARIANT_VIOLATION_ERROR).with_message(
                    "Gas cost base for native_sponsor_auth_digest not available".to_string(),
                )
            })?
    );

    let auth_context: &mut AuthenticationContext = get_extension_mut!(context)?;

    let digest_ref = auth_context.sponsor_auth_digest_ref()?;

    Ok(NativeResult::ok(context.gas_used(), smallvec![digest_ref]))
}

#[derive(Clone)]
pub struct AuthContextAuthenticatorFunctionInfoV1CostParams {
    pub auth_context_authenticator_function_info_v1_cost_base: Option<InternalGas>,
}

/// ****************************************************************************
/// native fun native_sender_authenticator_function_info_v1
/// Implementation of the Move native function
/// `fun native_sender_authenticator_function_info_v1<F>(): &Option<F>`
///
/// Returns `None` if the sender did not use a `MoveAuthenticator` signature.
/// ****************************************************************************
pub fn native_sender_authenticator_function_info_v1(
    context: &mut NativeContext,
    mut ty_args: Vec<Type>,
    args: VecDeque<Value>,
) -> PartialVMResult<NativeResult> {
    debug_assert!(ty_args.len() == 1);
    debug_assert!(args.is_empty());

    let cost_params = get_extension!(context, NativesCostTable)?
        .auth_context_authenticator_function_info_v1_cost_params
        .clone();
    native_charge_gas_early_exit!(
        context,
        cost_params
            .auth_context_authenticator_function_info_v1_cost_base
            .ok_or_else(|| {
                PartialVMError::new(StatusCode::UNKNOWN_INVARIANT_VIOLATION_ERROR).with_message(
                    "Gas cost base for native_sender_authenticator_function_info_v1 not available"
                        .to_string(),
                )
            })?
    );

    let authenticator_function_info_v1_type = ty_args.pop().unwrap();
    let authenticator_function_info_v1_type_layout =
        resolve_move_layout(context, &authenticator_function_info_v1_type)?;

    let auth_context: &mut AuthenticationContext = get_extension_mut!(context)?;

    let sender_authenticator_function_info_v1_value = auth_context
        .sender_authenticator_function_info_v1_ref(authenticator_function_info_v1_type_layout)?;

    Ok(NativeResult::ok(
        context.gas_used(),
        smallvec![sender_authenticator_function_info_v1_value],
    ))
}

/// ****************************************************************************
/// native fun native_sponsor_authenticator_function_info_v1
/// Implementation of the Move native function
/// `fun native_sponsor_authenticator_function_info_v1<F>(): &Option<F>`
///
/// Returns `None` if the transaction is unsponsored or the sponsor did not use
/// a `MoveAuthenticator` signature.
/// ****************************************************************************
pub fn native_sponsor_authenticator_function_info_v1(
    context: &mut NativeContext,
    mut ty_args: Vec<Type>,
    args: VecDeque<Value>,
) -> PartialVMResult<NativeResult> {
    debug_assert!(ty_args.len() == 1);
    debug_assert!(args.is_empty());

    let cost_params = get_extension!(context, NativesCostTable)?
        .auth_context_authenticator_function_info_v1_cost_params
        .clone();
    native_charge_gas_early_exit!(
        context,
        cost_params
            .auth_context_authenticator_function_info_v1_cost_base
            .ok_or_else(|| {
                PartialVMError::new(StatusCode::UNKNOWN_INVARIANT_VIOLATION_ERROR).with_message(
                    "Gas cost base for native_sponsor_authenticator_function_info_v1 not available"
                        .to_string(),
                )
            })?
    );

    let authenticator_function_info_v1_type = ty_args.pop().unwrap();
    let authenticator_function_info_v1_type_layout =
        resolve_move_layout(context, &authenticator_function_info_v1_type)?;

    let auth_context: &mut AuthenticationContext = get_extension_mut!(context)?;

    let sponsor_authenticator_function_info_v1_value = auth_context
        .sponsor_authenticator_function_info_v1_ref(authenticator_function_info_v1_type_layout)?;

    Ok(NativeResult::ok(
        context.gas_used(),
        smallvec![sponsor_authenticator_function_info_v1_value],
    ))
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
pub struct AuthContextTxDataBytesCostParams {
    pub auth_context_tx_data_bytes_cost_base: Option<InternalGas>,
    pub auth_context_tx_data_bytes_cost_per_byte: Option<InternalGas>,
}

/// ****************************************************************************
/// native fun native_tx_data_bytes
/// Implementation of the Move native function `fun native_tx_data_bytes():
/// &vector<u8>`
/// ****************************************************************************
pub fn native_tx_data_bytes(
    context: &mut NativeContext,
    ty_args: Vec<Type>,
    args: VecDeque<Value>,
) -> PartialVMResult<NativeResult> {
    debug_assert!(ty_args.is_empty());
    debug_assert!(args.is_empty());

    let auth_context_tx_data_bytes_cost_params = get_extension!(context, NativesCostTable)?
        .auth_context_tx_data_bytes_cost_params
        .clone();
    native_charge_gas_early_exit!(
        context,
        auth_context_tx_data_bytes_cost_params
            .auth_context_tx_data_bytes_cost_base
            .ok_or_else(|| {
                PartialVMError::new(StatusCode::UNKNOWN_INVARIANT_VIOLATION_ERROR).with_message(
                    "Gas cost base for native_tx_data_bytes not available".to_string(),
                )
            })?
    );

    let auth_context: &mut AuthenticationContext = get_extension_mut!(context)?;

    let (tx_data_bytes_ref, tx_data_bytes_value_size) = auth_context.tx_data_bytes_ref()?;

    native_charge_gas_early_exit!(
        context,
        auth_context_tx_data_bytes_cost_params
            .auth_context_tx_data_bytes_cost_per_byte
            .ok_or_else(|| {
                PartialVMError::new(StatusCode::UNKNOWN_INVARIANT_VIOLATION_ERROR).with_message(
                    "Gas cost per byte for native_tx_data_bytes not available".to_string(),
                )
            })?
            * u64::from(tx_data_bytes_value_size).into()
    );

    Ok(NativeResult::ok(
        context.gas_used(),
        smallvec![tx_data_bytes_ref],
    ))
}

#[derive(Clone)]
pub struct AuthContextReplaceCostParams {
    pub auth_context_replace_cost_base: Option<InternalGas>,
    pub auth_context_replace_cost_per_byte: Option<InternalGas>,
}

/// ****************************************************************************
/// native fun native_replace
/// Implementation of the Move native function
/// `fun native_replace<I, C, F>(auth_digest: vector<u8>, tx_inputs: vector<I>,
/// tx_commands: vector<C>, tx_data_bytes: vector<u8>,
/// sender_auth_digest: vector<u8>, sponsor_auth_digest: Option<vector<u8>>,
/// sender_authenticator_function_info_v1: Option<F>,
/// sponsor_authenticator_function_info_v1: Option<F>)`
/// ****************************************************************************
pub fn native_replace(
    context: &mut NativeContext,
    mut ty_args: Vec<Type>,
    mut args: VecDeque<Value>,
) -> PartialVMResult<NativeResult> {
    debug_assert!(ty_args.len() == 2 || ty_args.len() == 3);
    let args_len = args.len();
    debug_assert!(args_len == 3 || args_len == 4 || args_len == 6 || args_len == 8);

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

    let (sender_authenticator_function_ref_v1_opt, sponsor_authenticator_function_ref_v1_opt) =
        if args_len >= 8 {
            let authenticator_function_info_v1_type = ty_args.pop().unwrap();
            let authenticator_function_info_v1_type_layout =
                resolve_move_layout(context, &authenticator_function_info_v1_type)?;
            let sponsor_authenticator_function_info_v1_struct = pop_arg!(args, Struct);
            let sender_authenticator_function_info_v1_struct = pop_arg!(args, Struct);
            (
                Some(unpack_authenticator_function_info_v1_opt(
                    sender_authenticator_function_info_v1_struct,
                    &authenticator_function_info_v1_type_layout,
                )?),
                Some(unpack_authenticator_function_info_v1_opt(
                    sponsor_authenticator_function_info_v1_struct,
                    &authenticator_function_info_v1_type_layout,
                )?),
            )
        } else {
            (None, None)
        };

    let (sender_auth_digest_opt, sponsor_auth_digest_opt) = if args_len >= 6 {
        let option_struct = pop_arg!(args, Struct);

        let sponsor = unpack_sponsor_digest(option_struct)?;
        let sender: Vec<u8> = pop_arg!(args, Vec<u8>);

        (Some(sender), Some(sponsor))
    } else {
        (None, None)
    };

    let tx_data_bytes_opt = if args_len >= 4 {
        Some(pop_arg!(args, Vec<u8>))
    } else {
        None
    };

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
        tx_data_bytes_opt,
        sender_auth_digest_opt,
        sponsor_auth_digest_opt,
        sender_authenticator_function_ref_v1_opt,
        sponsor_authenticator_function_ref_v1_opt,
    )?;

    Ok(NativeResult::ok(context.gas_used(), smallvec![]))
}

fn resolve_move_layout(context: &NativeContext, ty: &Type) -> PartialVMResult<MoveTypeLayout> {
    context.type_to_type_layout(ty)?.ok_or(
        PartialVMError::new(StatusCode::UNKNOWN_INVARIANT_VIOLATION_ERROR)
            .with_message(format!("Can't resolve `MoveTypeLayout` for {ty:?}")),
    )
}

/// Unpacks a Move `Option<T>` struct (encoded as `struct { v: vector<T> }`)
/// and converts the inner value using `convert`, returning `None` for empty
/// options.
fn unpack_option<T, F>(option_struct: Struct, convert: F) -> PartialVMResult<Option<T>>
where
    F: FnOnce(Value) -> PartialVMResult<T>,
{
    let inner_vec = option_struct
        .unpack()?
        .next()
        .ok_or_else(|| {
            PartialVMError::new(StatusCode::UNKNOWN_INVARIANT_VIOLATION_ERROR)
                .with_message("Option struct has no fields".to_string())
        })?
        .value_as::<Vec<Value>>()?;

    match inner_vec.into_iter().next() {
        None => Ok(None),
        Some(v) => convert(v).map(Some),
    }
}

fn unpack_authenticator_function_info_v1_opt(
    option_struct: Struct,
    layout: &MoveTypeLayout,
) -> PartialVMResult<Option<AuthenticatorFunctionRefV1>> {
    unpack_option(option_struct, |v| {
        utils::from_value::<AuthenticatorFunctionRefV1>(v, layout)
    })
}

fn unpack_sponsor_digest(option_struct: Struct) -> PartialVMResult<Option<Vec<u8>>> {
    unpack_option(option_struct, |v| v.value_as::<Vec<u8>>())
}
