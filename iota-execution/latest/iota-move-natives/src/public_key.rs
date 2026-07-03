// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::collections::VecDeque;

use iota_sdk_types::Address as IotaAddress;
use iota_types::{
    crypto::{PublicKey, SignatureScheme},
    multisig::MultiSigPublicKey,
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
    values::{Value, VectorRef},
};
use smallvec::smallvec;

use crate::NativesCostTable;

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

    let address = match SignatureScheme::from_flag_byte(&flag) {
        // MultiSig is not part of the `PublicKey` enum; derive from its committee bytes.
        Ok(SignatureScheme::MultiSig) => bcs::from_bytes::<MultiSigPublicKey>(&raw_bytes_ref)
            .ok()
            .map(|committee| IotaAddress::from(&committee)),
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
