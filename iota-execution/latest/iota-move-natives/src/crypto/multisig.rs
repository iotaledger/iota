// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::collections::VecDeque;

use iota_sdk_types::{
    Secp256k1PublicKey,
    crypto::{MULTISIG_COMMITTEE_SIZE_MAX, MultisigCommittee},
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

/// The length of the largest valid BCS-encoded MultiSig committee (353 bytes).
///
/// The SDK does not export this bound, so it is derived here from the BCS
/// layout of `MultisigCommittee`:
/// - member count: ULEB128, one byte for up to 127 members;
/// - each of at most `MULTISIG_COMMITTEE_SIZE_MAX` members: a one-byte scheme tag, the key, and a
///   `u8` weight. The longest member key is 33 bytes (Secp256k1, Secp256r1 and Passkey; Ed25519 is
///   32), so `Secp256k1PublicKey::LENGTH` stands for it;
/// - threshold: `u16`, two bytes.
///
/// A new member scheme with a key longer than 33 bytes must update this, or
/// valid committees using it would be refused.
const MULTISIG_PUBLIC_KEY_LENGTH_MAX: usize =
    1 + MULTISIG_COMMITTEE_SIZE_MAX * (1 + Secp256k1PublicKey::LENGTH + 1) + 2;

#[derive(Clone)]
pub struct MultisigValidatePubkeyCostParams {
    pub multisig_multisig_validate_pubkey_cost_base: Option<InternalGas>,
}

/// Implementation of the Move native function
/// `multisig::multisig_validate_pubkey(public_key: &vector<u8>): bool`
///
/// Returns `true` if `public_key` deserializes into a MultiSig committee that
/// passes `MultisigCommittee::validate()` — 1 to 10 distinct members, every
/// weight greater than zero, threshold greater than zero, total weight at
/// least the threshold, and no Ed25519 member together with members of other
/// schemes — and `false` otherwise (including on deserialization failure or
/// trailing bytes). Member keys are not checked against their curves,
/// matching the SDK.
///
/// An input longer than `MULTISIG_PUBLIC_KEY_LENGTH_MAX` (353 bytes) cannot
/// hold a valid committee and returns `false` without being decoded, so the
/// work done is bounded and the flat cost covers it.
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

    let is_valid = validate_pubkey(&public_key_ref).is_ok();

    let cost = context.gas_used();
    Ok(NativeResult::ok(cost, smallvec![Value::bool(is_valid)]))
}

#[derive(Debug, PartialEq, Eq)]
enum MultisigPublicKeyError {
    TooLong,
    Deserialize,
    InvalidCommittee,
}

fn validate_pubkey(public_key: &[u8]) -> Result<(), MultisigPublicKeyError> {
    if public_key.len() > MULTISIG_PUBLIC_KEY_LENGTH_MAX {
        return Err(MultisigPublicKeyError::TooLong);
    }
    // BCS rejects trailing bytes and unsupported member schemes.
    let committee = bcs::from_bytes::<MultisigCommittee>(public_key)
        .map_err(|_| MultisigPublicKeyError::Deserialize)?;
    committee
        .validate()
        .map_err(|_| MultisigPublicKeyError::InvalidCommittee)
}

#[cfg(test)]
mod tests {
    use iota_sdk_types::crypto::MultisigMember;

    use super::*;

    fn committee_bytes(size: usize) -> Vec<u8> {
        let members = (0..size)
            .map(|i| {
                let mut key = [u8::try_from(i).unwrap(); Secp256k1PublicKey::LENGTH];
                key[0] = 0x02;
                MultisigMember::new(Secp256k1PublicKey::new(key), 1)
            })
            .collect();
        bcs::to_bytes(&MultisigCommittee::new_unchecked(
            members,
            size.try_into().unwrap(),
        ))
        .unwrap()
    }

    #[test]
    fn largest_valid_committee_is_accepted() {
        let bytes = committee_bytes(MULTISIG_COMMITTEE_SIZE_MAX);
        assert_eq!(bytes.len(), 353);
        assert_eq!(bytes.len(), MULTISIG_PUBLIC_KEY_LENGTH_MAX);
        assert_eq!(validate_pubkey(&bytes), Ok(()));
    }

    #[test]
    fn input_one_byte_over_the_bound_is_refused_before_decoding() {
        let mut bytes = committee_bytes(MULTISIG_COMMITTEE_SIZE_MAX);
        bytes.push(0);
        assert_eq!(
            validate_pubkey(&bytes),
            Err(MultisigPublicKeyError::TooLong)
        );
    }

    #[test]
    fn committee_with_too_many_members_is_refused_before_decoding() {
        let too_many_members = committee_bytes(100);
        assert_eq!(
            validate_pubkey(&too_many_members),
            Err(MultisigPublicKeyError::TooLong)
        );
    }

    #[test]
    fn truncated_input_within_the_bound_fails_to_deserialize() {
        let mut bytes = committee_bytes(MULTISIG_COMMITTEE_SIZE_MAX);
        bytes.pop();
        assert_eq!(
            validate_pubkey(&bytes),
            Err(MultisigPublicKeyError::Deserialize)
        );
    }

    #[test]
    fn zero_threshold_within_the_bound_fails_committee_validation() {
        let mut bytes = committee_bytes(2);
        let len = bytes.len();
        bytes[len - 2..].fill(0);
        assert_eq!(
            validate_pubkey(&bytes),
            Err(MultisigPublicKeyError::InvalidCommittee)
        );
    }
}
