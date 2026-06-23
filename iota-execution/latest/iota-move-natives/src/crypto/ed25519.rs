// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::collections::VecDeque;

use fastcrypto::{
    ed25519::{Ed25519PublicKey, Ed25519Signature},
    traits::{ToFromBytes, VerifyingKey},
};
use move_binary_format::errors::{PartialVMError, PartialVMResult};
use move_core_types::{gas_algebra::InternalGas, vm_status::StatusCode};
use move_vm_runtime::{native_charge_gas_early_exit, native_functions::NativeContext};
use move_vm_types::{
    loaded_data::runtime_types::Type,
    natives::function::NativeResult,
    pop_arg,
    values::{Value, VectorRef},
};
use smallvec::smallvec;

use crate::NativesCostTable;

const ED25519_BLOCK_SIZE: usize = 128;

#[derive(Clone)]
pub struct Ed25519VerifyCostParams {
    /// Base cost for invoking the `ed25519_verify` function
    pub ed25519_ed25519_verify_cost_base: InternalGas,
    /// Cost per byte of `msg`
    pub ed25519_ed25519_verify_msg_cost_per_byte: InternalGas,
    /// Cost per block of `msg`, where a block is 128 bytes
    pub ed25519_ed25519_verify_msg_cost_per_block: InternalGas,
}
/// ****************************************************************************
/// ********************* native fun ed25519_verify
/// Implementation of the Move native function
/// `ed25519::ed25519_verify(signature: &vector<u8>, public_key: &vector<u8>,
/// msg: &vector<u8>): bool;`   gas cost: ed25519_ed25519_verify_cost_base
/// | base cost for function call and fixed opers
///              + ed25519_ed25519_verify_msg_cost_per_byte * msg.len()   | cost
///                depends on length of message
///              + ed25519_ed25519_verify_msg_cost_per_block * num_blocks(msg) |
///                cost depends on number of blocks in message
/// Note: each block is of size `ED25519_BLOCK_SIZE` bytes, and we round up.
///       `signature` and `public_key` are fixed size, so their costs are
/// included in the base cost. *************************************************
/// **********************************************
pub fn ed25519_verify(
    context: &mut NativeContext,
    ty_args: Vec<Type>,
    mut args: VecDeque<Value>,
) -> PartialVMResult<NativeResult> {
    debug_assert!(ty_args.is_empty());
    debug_assert!(args.len() == 3);

    // Load the cost parameters from the protocol config
    let ed25519_verify_cost_params = &context
        .extensions()
        .get::<NativesCostTable>()?
        .ed25519_verify_cost_params
        .clone();
    // Charge the base cost for this oper
    native_charge_gas_early_exit!(
        context,
        ed25519_verify_cost_params.ed25519_ed25519_verify_cost_base
    );

    let msg = pop_arg!(args, VectorRef);
    let msg_ref = msg.as_bytes_ref();
    let public_key_bytes = pop_arg!(args, VectorRef);
    let public_key_bytes_ref = public_key_bytes.as_bytes_ref();
    let signature_bytes = pop_arg!(args, VectorRef);
    let signature_bytes_ref = signature_bytes.as_bytes_ref();

    // Charge the arg size dependent costs
    native_charge_gas_early_exit!(
        context,
        ed25519_verify_cost_params.ed25519_ed25519_verify_msg_cost_per_byte
            * (msg_ref.len() as u64).into()
            + ed25519_verify_cost_params.ed25519_ed25519_verify_msg_cost_per_block
                * (msg_ref.len().div_ceil(ED25519_BLOCK_SIZE) as u64).into()
    );
    let cost = context.gas_used();

    let Ok(signature) = <Ed25519Signature as ToFromBytes>::from_bytes(&signature_bytes_ref) else {
        return Ok(NativeResult::ok(cost, smallvec![Value::bool(false)]));
    };

    let Ok(public_key) = <Ed25519PublicKey as ToFromBytes>::from_bytes(&public_key_bytes_ref)
    else {
        return Ok(NativeResult::ok(cost, smallvec![Value::bool(false)]));
    };

    Ok(NativeResult::ok(
        cost,
        smallvec![Value::bool(public_key.verify(&msg_ref, &signature).is_ok())],
    ))
}

#[derive(Clone)]
pub struct Ed25519ValidatePubkeyCostParams {
    pub ed25519_ed25519_validate_pubkey_cost_base: Option<InternalGas>,
}
/// Implementation of the Move native function
/// `ed25519::ed25519_validate_pubkey(public_key: &vector<u8>): bool`
///
/// Returns `true` if `public_key` is a valid point on the Ed25519 curve: the
/// encoded y-coordinate must have a corresponding x-coordinate on the curve
/// (i.e. x² must be a quadratic residue mod p) and y must be less than p.
/// Returns `false` otherwise.
///
/// gas cost: ed25519_ed25519_validate_pubkey_cost_base
pub fn ed25519_validate_pubkey(
    context: &mut NativeContext,
    ty_args: Vec<Type>,
    mut args: VecDeque<Value>,
) -> PartialVMResult<NativeResult> {
    debug_assert!(ty_args.is_empty());
    debug_assert!(args.len() == 1);

    let cost_base = context
        .extensions()
        .get::<NativesCostTable>()?
        .ed25519_validate_pubkey_cost_params
        .ed25519_ed25519_validate_pubkey_cost_base
        .ok_or_else(|| {
            PartialVMError::new(StatusCode::UNKNOWN_INVARIANT_VIOLATION_ERROR)
                .with_message("Gas cost for ed25519_validate_pubkey not available".to_string())
        })?;
    native_charge_gas_early_exit!(context, cost_base);

    let public_key_bytes = pop_arg!(args, VectorRef);
    let public_key_bytes_ref = public_key_bytes.as_bytes_ref();
    let cost = context.gas_used();

    let is_valid = Ed25519PublicKey::from_bytes(&public_key_bytes_ref).is_ok();
    Ok(NativeResult::ok(cost, smallvec![Value::bool(is_valid)]))
}
