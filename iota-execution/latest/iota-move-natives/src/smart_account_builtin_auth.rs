// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::collections::VecDeque;

use iota_sdk_types::{Address, key_proof::key_proof_message_v1};
use move_binary_format::errors::{PartialVMError, PartialVMResult};
use move_core_types::{
    account_address::AccountAddress, gas_algebra::InternalGas, vm_status::StatusCode,
};
use move_vm_runtime::{native_charge_gas_early_exit, native_functions::NativeContext};
use move_vm_types::{
    loaded_data::runtime_types::Type, natives::function::NativeResult, pop_arg, values::Value,
};
use smallvec::smallvec;

use crate::NativesCostTable;

/// Raised when the chain identifier or the nonce is not 32 bytes.
const E_INVALID_KEY_PROOF_MESSAGE_VALUE: u64 = 5;

#[derive(Clone)]
pub struct SmartAccountBuiltinAuthKeyProofMessageCostParams {
    pub smart_account_builtin_auth_key_proof_message_cost_base: Option<InternalGas>,
}
/// Implementation of the Move native function
/// `smart_account_builtin_auth::key_proof_message(chain_identifier:
/// vector<u8>, account: address, new_account: bool, nonce: vector<u8>,
/// valid_until_epoch: u64): vector<u8>`
///
/// Returns the UTF-8 bytes of the text that the SDK's `key_proof_message_v1`
/// writes for these values, the text a key holder signs to approve attaching
/// their key to a `SmartAccount`. Clients call the same SDK function, so they
/// sign the bytes this returns.
///
/// Aborts with `E_INVALID_KEY_PROOF_MESSAGE_VALUE` if `chain_identifier` or
/// `nonce` is not 32 bytes.
///
/// gas cost: smart_account_builtin_auth_key_proof_message_cost_base
pub fn key_proof_message(
    context: &mut NativeContext,
    ty_args: Vec<Type>,
    mut args: VecDeque<Value>,
) -> PartialVMResult<NativeResult> {
    debug_assert!(ty_args.is_empty());
    debug_assert!(args.len() == 5);

    let cost_base = context
        .extensions()
        .get::<NativesCostTable>()?
        .smart_account_builtin_auth_key_proof_message_cost_params
        .smart_account_builtin_auth_key_proof_message_cost_base
        .ok_or_else(|| {
            PartialVMError::new(StatusCode::UNKNOWN_INVARIANT_VIOLATION_ERROR)
                .with_message("Gas cost for key_proof_message not available".to_string())
        })?;
    native_charge_gas_early_exit!(context, cost_base);

    let valid_until_epoch = pop_arg!(args, u64);
    let nonce = pop_arg!(args, Vec<u8>);
    let new_account = pop_arg!(args, bool);
    let account = pop_arg!(args, AccountAddress);
    let chain_identifier = pop_arg!(args, Vec<u8>);
    let cost = context.gas_used();

    let (Ok(chain_identifier), Ok(nonce)) = (
        <[u8; 32]>::try_from(chain_identifier),
        <[u8; 32]>::try_from(nonce),
    ) else {
        return Ok(NativeResult::err(cost, E_INVALID_KEY_PROOF_MESSAGE_VALUE));
    };

    let message = key_proof_message_v1(
        &chain_identifier,
        &Address::new(account.into_bytes()),
        new_account,
        &nonce,
        valid_until_epoch,
    );

    Ok(NativeResult::ok(
        cost,
        smallvec![Value::vector_u8(message.into_bytes())],
    ))
}
