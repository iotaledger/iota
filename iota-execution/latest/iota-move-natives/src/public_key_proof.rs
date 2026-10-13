// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::collections::VecDeque;

use iota_types::account_abstraction::{builtin_authenticator_functions, public_key::MovePublicKey};
use move_binary_format::errors::{PartialVMError, PartialVMResult};
use move_core_types::vm_status::StatusCode;
use move_vm_runtime::{native_charge_gas_early_exit, native_functions::NativeContext};
use move_vm_types::{
    loaded_data::runtime_types::Type,
    natives::function::NativeResult,
    pop_arg,
    values::{StructRef, Value, VectorRef},
};
use smallvec::smallvec;

use crate::{NativesCostTable, get_extension, object_runtime::ObjectRuntime, utils};

/// Implementation of the Move native function
/// `public_key_proof::verify_personal_message(public_key: &PublicKey,
/// message: &vector<u8>, signature: &vector<u8>): bool`
///
/// Returns `true` if `signature` is a valid signature, by `public_key`, of the
/// personal message `message`, using the signature scheme of `public_key`.
///
/// gas cost: builtin_move_authenticator_cost_base
pub fn verify_personal_message(
    context: &mut NativeContext,
    ty_args: Vec<Type>,
    mut args: VecDeque<Value>,
) -> PartialVMResult<NativeResult> {
    debug_assert!(ty_args.is_empty());
    debug_assert!(args.len() == 3);

    let cost_base = get_extension!(context, NativesCostTable)?
        .builtin_authenticator_functions_cost_params
        .builtin_move_authenticator_cost_base
        .ok_or_else(|| {
            PartialVMError::new(StatusCode::UNKNOWN_INVARIANT_VIOLATION_ERROR)
                .with_message("Gas cost for built-in authenticators not available".to_string())
        })?;
    native_charge_gas_early_exit!(context, cost_base);

    let signature = pop_arg!(args, VectorRef);
    let message = pop_arg!(args, VectorRef);
    let public_key: MovePublicKey = utils::from_value(
        pop_arg!(args, StructRef).read_ref()?,
        &MovePublicKey::layout(),
    )?;

    let protocol_config = get_extension!(context, ObjectRuntime)?.protocol_config;

    let is_valid = builtin_authenticator_functions::verify_personal_message_signature(
        protocol_config,
        &public_key,
        &signature.as_bytes_ref(),
        &message.as_bytes_ref(),
    )
    .is_ok();

    Ok(NativeResult::ok(
        context.gas_used(),
        smallvec![Value::bool(is_valid)],
    ))
}
