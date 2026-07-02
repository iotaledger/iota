// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::collections::VecDeque;

use fastcrypto::{
    ed25519::Ed25519PublicKey, secp256k1::Secp256k1PublicKey, secp256r1::Secp256r1PublicKey,
    traits::ToFromBytes,
};
use iota_sdk_types::crypto::PublicKey;
use iota_types::multisig::MultiSigPublicKey;
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

#[derive(Clone)]
pub struct MultisigValidatePubkeyCostParams {
    pub multisig_multisig_validate_pubkey_cost_base: Option<InternalGas>,
}

/// Implementation of the Move native function
/// `multisig::multisig_validate_pubkey(public_key: &vector<u8>): bool`
///
/// Returns `true` if `public_key` deserializes into a well-formed MultiSig
/// committee that passes canonical validation — 1 to 10 distinct members, every
/// member key a valid curve point, every weight greater than zero, threshold
/// greater than zero, and total weight at least the threshold — and `false`
/// otherwise (including on deserialization failure or trailing bytes).
///
/// gas cost: multisig_multisig_validate_pubkey_cost_base
pub fn multisig_validate_pubkey(
    context: &mut NativeContext,
    ty_args: Vec<Type>,
    mut args: VecDeque<Value>,
) -> PartialVMResult<NativeResult> {
    debug_assert!(ty_args.is_empty());
    debug_assert!(args.len() == 1);

    let cost_base = context
        .extensions()
        .get::<NativesCostTable>()?
        .multisig_validate_pubkey_cost_params
        .multisig_multisig_validate_pubkey_cost_base
        .ok_or_else(|| {
            PartialVMError::new(StatusCode::UNKNOWN_INVARIANT_VIOLATION_ERROR)
                .with_message("Gas cost for multisig_validate_pubkey not available".to_string())
        })?;
    native_charge_gas_early_exit!(context, cost_base);

    let public_key = pop_arg!(args, VectorRef);
    let public_key_ref = public_key.as_bytes_ref();
    let cost = context.gas_used();

    // A valid MultiSig public key must deserialize cleanly (BCS rejects trailing
    // bytes), pass committee validation, and have every member key on its
    // curve. Committee validation alone does not check curve points, so we
    // verify each member explicitly.
    let is_valid = match bcs::from_bytes::<MultiSigPublicKey>(&public_key_ref) {
        Ok(committee) => {
            committee.validate().is_ok()
                && committee
                    .members()
                    .iter()
                    .all(|member| member_pubkey_is_on_curve(member.public_key()))
        }
        Err(_) => false,
    };
    Ok(NativeResult::ok(cost, smallvec![Value::bool(is_valid)]))
}

/// Returns `true` if `public_key` is a valid point on its curve. Mirrors the
/// per-scheme curve checks the single-key `*_validate_pubkey` natives perform,
/// applied to each MultiSig member.
fn member_pubkey_is_on_curve(public_key: &PublicKey) -> bool {
    match public_key {
        PublicKey::Ed25519(pk) => Ed25519PublicKey::from_bytes(pk.inner()).is_ok(),
        PublicKey::Secp256k1(pk) => Secp256k1PublicKey::from_bytes(pk.inner()).is_ok(),
        PublicKey::Secp256r1(pk) => Secp256r1PublicKey::from_bytes(pk.inner()).is_ok(),
        PublicKey::Passkey(pk) => Secp256r1PublicKey::from_bytes(pk.inner().inner()).is_ok(),
        // `PublicKey` is `#[non_exhaustive]`; any future member scheme is rejected until
        // explicitly supported here.
        _ => false,
    }
}
