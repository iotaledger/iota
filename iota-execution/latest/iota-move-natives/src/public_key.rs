// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::collections::VecDeque;

use iota_sdk_types::{Address as IotaAddress, SignatureScheme, crypto::MultisigCommittee};
use iota_types::{
    account_abstraction::{builtin_authenticator_functions, public_key::MovePublicKey},
    crypto::PublicKey,
};
use move_binary_format::errors::{PartialVMError, PartialVMResult};
use move_core_types::{
    account_address::AccountAddress, gas_algebra::InternalGas, vm_status::StatusCode,
};
use move_vm_runtime::{native_charge_gas_early_exit, native_functions::NativeContext};
use move_vm_types::{
    loaded_data::runtime_types::Type,
    natives::function::NativeResult,
    pop_arg,
    values::{StructRef, Value, VectorRef},
};
use smallvec::smallvec;

use crate::{NativesCostTable, get_extension, object_runtime::ObjectRuntime, utils};

/// Raised when the `(flag, raw_bytes)` pair does not form a public key whose
/// address can be derived. `public_key::create` validates keys before calling
/// this native, so this abort is not expected to be reachable for
/// `public_key::PublicKey`-derived calls.
const E_INVALID_PUBLIC_KEY: u64 = 0;

#[derive(Clone)]
pub struct PublicKeyToIotaAddressImplCostParams {
    pub public_key_to_iota_address_impl_cost_base: Option<InternalGas>,
}
/// Implementation of the Move native function
/// `public_key::to_iota_address_impl(flag: u8, raw_bytes: &vector<u8>):
/// address`
///
/// Derives the IOTA address for the `flag`-typed public key with raw key
/// material `raw_bytes` (no scheme flag prefix), delegating to the node's
/// canonical address derivation so Move and Rust cannot diverge.
///
/// Aborts with `E_INVALID_PUBLIC_KEY` if `flag` is unrecognized or `raw_bytes`
/// is not a valid public key for it.
///
/// gas cost: public_key_to_iota_address_impl_cost_base
pub fn to_iota_address_impl(
    context: &mut NativeContext,
    ty_args: Vec<Type>,
    mut args: VecDeque<Value>,
) -> PartialVMResult<NativeResult> {
    debug_assert!(ty_args.is_empty());
    debug_assert!(args.len() == 2);

    let cost_base = context
        .extensions()
        .get::<NativesCostTable>()?
        .public_key_to_iota_address_impl_cost_params
        .public_key_to_iota_address_impl_cost_base
        .ok_or_else(|| {
            PartialVMError::new(StatusCode::UNKNOWN_INVARIANT_VIOLATION_ERROR)
                .with_message("Gas cost for to_iota_address_impl not available".to_string())
        })?;
    native_charge_gas_early_exit!(context, cost_base);

    let raw_bytes = pop_arg!(args, VectorRef);
    let raw_bytes_ref = raw_bytes.as_bytes_ref();
    let flag = pop_arg!(args, u8);
    let cost = context.gas_used();

    let address = match SignatureScheme::from_byte(flag) {
        // MultiSig is not part of the `PublicKey` enum; derive from its committee bytes.
        Ok(SignatureScheme::Multisig) => bcs::from_bytes::<MultisigCommittee>(&raw_bytes_ref)
            .ok()
            .map(|committee| committee.derive_address()),
        Ok(scheme) => PublicKey::try_from_bytes(scheme, &raw_bytes_ref)
            .ok()
            .map(|public_key| IotaAddress::from(&public_key)),
        Err(_) => None,
    };

    Ok(match address {
        Some(address) => NativeResult::ok(
            cost,
            smallvec![Value::address(AccountAddress::new(address.into_bytes()))],
        ),
        None => NativeResult::err(cost, E_INVALID_PUBLIC_KEY),
    })
}

#[derive(Clone)]
pub struct PublicKeyVerifyPersonalMessageImplCostParams {
    /// Base cost for invoking the `verify_personal_message_impl` function
    pub public_key_verify_personal_message_impl_cost_base: Option<InternalGas>,
    /// Cost per byte of `message`
    pub public_key_verify_personal_message_impl_msg_cost_per_byte: Option<InternalGas>,
    /// Cost per byte of `signature`
    pub public_key_verify_personal_message_impl_sig_cost_per_byte: Option<InternalGas>,
}
/// Implementation of the Move native function
/// `public_key::verify_personal_message_impl(public_key: &PublicKey,
/// signature: &vector<u8>, message: &vector<u8>): bool`
///
/// Returns `true` if `signature` is a valid signature, by the public key, of
/// `message` signed as an IOTA personal message, using the signature scheme of
/// the public key. Returns `false` for a MultiSig public key.
///
/// gas cost: public_key_verify_personal_message_impl_cost_base
///     + public_key_verify_personal_message_impl_msg_cost_per_byte * message.len()
///     + public_key_verify_personal_message_impl_sig_cost_per_byte * signature.len()
pub fn verify_personal_message_impl(
    context: &mut NativeContext,
    ty_args: Vec<Type>,
    mut args: VecDeque<Value>,
) -> PartialVMResult<NativeResult> {
    debug_assert!(ty_args.is_empty());
    debug_assert!(args.len() == 3);

    let cost_params = get_extension!(context, NativesCostTable)?
        .public_key_verify_personal_message_impl_cost_params
        .clone();
    let (Some(cost_base), Some(msg_cost_per_byte), Some(sig_cost_per_byte)) = (
        cost_params.public_key_verify_personal_message_impl_cost_base,
        cost_params.public_key_verify_personal_message_impl_msg_cost_per_byte,
        cost_params.public_key_verify_personal_message_impl_sig_cost_per_byte,
    ) else {
        return Err(
            PartialVMError::new(StatusCode::UNKNOWN_INVARIANT_VIOLATION_ERROR).with_message(
                "Gas cost for verify_personal_message_impl not available".to_string(),
            ),
        );
    };
    native_charge_gas_early_exit!(context, cost_base);

    let message = pop_arg!(args, VectorRef);
    let message_ref = message.as_bytes_ref();
    let signature = pop_arg!(args, VectorRef);
    let signature_ref = signature.as_bytes_ref();
    let public_key: MovePublicKey = utils::from_value(
        pop_arg!(args, StructRef).read_ref()?,
        &MovePublicKey::layout(),
    )?;

    native_charge_gas_early_exit!(
        context,
        msg_cost_per_byte * (message_ref.len() as u64).into()
    );
    // Charged before the check decodes the signature, whose length the caller
    // chooses.
    native_charge_gas_early_exit!(
        context,
        sig_cost_per_byte * (signature_ref.len() as u64).into()
    );

    let protocol_config = get_extension!(context, ObjectRuntime)?.protocol_config;

    let is_valid = builtin_authenticator_functions::verify_personal_message(
        protocol_config,
        &public_key,
        &signature_ref,
        &message_ref,
    )
    .is_ok();

    Ok(NativeResult::ok(
        context.gas_used(),
        smallvec![Value::bool(is_valid)],
    ))
}
