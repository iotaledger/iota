// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

#[allow(unused_const, unused_function)]
module iota::zklogin_verified_id;

use std::string::String;

const EFunctionDisabled: u64 = 0;

/// Possession of a VerifiedID proves that the user's address was created using zklogin and the given parameters.
#[deprecated]
public struct VerifiedID has key {
    /// The ID of this VerifiedID
    id: UID,
    /// The address this VerifiedID is associated with
    owner: address,
    /// The name of the key claim
    key_claim_name: String,
    /// The value of the key claim
    key_claim_value: String,
    /// The issuer
    issuer: String,
    /// The audience (wallet)
    audience: String,
}

/// Returns the address associated with the given VerifiedID
#[deprecated, allow(deprecated_usage)]
public fun owner(verified_id: &VerifiedID): address {
    verified_id.owner
}

/// Returns the name of the key claim associated with the given VerifiedID
#[deprecated, allow(deprecated_usage)]
public fun key_claim_name(verified_id: &VerifiedID): &String {
    &verified_id.key_claim_name
}

/// Returns the value of the key claim associated with the given VerifiedID
#[deprecated, allow(deprecated_usage)]
public fun key_claim_value(verified_id: &VerifiedID): &String {
    &verified_id.key_claim_value
}

/// Returns the issuer associated with the given VerifiedID
#[deprecated, allow(deprecated_usage)]
public fun issuer(verified_id: &VerifiedID): &String {
    &verified_id.issuer
}

/// Returns the audience (wallet) associated with the given VerifiedID
#[deprecated, allow(deprecated_usage)]
public fun audience(verified_id: &VerifiedID): &String {
    &verified_id.audience
}

/// Delete a VerifiedID
#[deprecated, allow(deprecated_usage)]
public fun delete(verified_id: VerifiedID) {
    let VerifiedID { id, owner: _, key_claim_name: _, key_claim_value: _, issuer: _, audience: _ } =
        verified_id;
    id.delete();
}

/// This function has been disabled.
#[deprecated]
public fun verify_zklogin_id(
    _key_claim_name: String,
    _key_claim_value: String,
    _issuer: String,
    _audience: String,
    _pin_hash: u256,
    _ctx: &mut TxContext,
) {
    assert!(false, EFunctionDisabled);
}

/// This function has been disabled.
#[deprecated]
public fun check_zklogin_id(
    _address: address,
    _key_claim_name: &String,
    _key_claim_value: &String,
    _issuer: &String,
    _audience: &String,
    _pin_hash: u256,
): bool {
    assert!(false, EFunctionDisabled);
    false
}
