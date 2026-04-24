// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

#[allow(unused_const, unused_function)]
module iota::zklogin_verified_issuer;

use std::string::String;

const EFunctionDisabled: u64 = 0;

/// Possession of a VerifiedIssuer proves that the user's address was created using zklogin and with the given issuer
/// (identity provider).
#[deprecated]
public struct VerifiedIssuer has key {
    /// The ID of this VerifiedIssuer
    id: UID,
    /// The address this VerifiedID is associated with
    owner: address,
    /// The issuer
    issuer: String,
}

/// Returns the address associated with the given VerifiedIssuer
#[deprecated, allow(deprecated_usage)]
public fun owner(verified_issuer: &VerifiedIssuer): address {
    verified_issuer.owner
}

/// Returns the issuer associated with the given VerifiedIssuer
#[deprecated, allow(deprecated_usage)]
public fun issuer(verified_issuer: &VerifiedIssuer): &String {
    &verified_issuer.issuer
}

/// Delete a VerifiedIssuer
#[deprecated, allow(deprecated_usage)]
public fun delete(verified_issuer: VerifiedIssuer) {
    let VerifiedIssuer { id, owner: _, issuer: _ } = verified_issuer;
    id.delete();
}

/// This function has been disabled.
#[deprecated]
public fun verify_zklogin_issuer(_address_seed: u256, _issuer: String, _ctx: &mut TxContext) {
    assert!(false, EFunctionDisabled);
}

/// This function has been disabled.
#[deprecated]
public fun check_zklogin_issuer(
    _address: address,
    _address_seed: u256,
    _issuer: &String,
): bool {
    assert!(false, EFunctionDisabled);
    false
}
