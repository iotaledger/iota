// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

#[deprecated, allow(unused_field)]
module iota::authenticator_state;

use std::string::String;

#[deprecated]
public struct AuthenticatorState has key {
    id: UID,
    version: u64,
}

#[deprecated, allow(deprecated_usage)]
public struct AuthenticatorStateInner has store {
    version: u64,
    /// List of currently active JWKs.
    active_jwks: vector<ActiveJwk>,
}

#[deprecated]
/// Must match the JWK struct in fastcrypto-zkp
public struct JWK has copy, drop, store {
    kty: String,
    e: String,
    n: String,
    alg: String,
}

#[deprecated]
/// Must match the JwkId struct in fastcrypto-zkp
public struct JwkId has copy, drop, store {
    iss: String,
    kid: String,
}

#[deprecated, allow(deprecated_usage)]
public struct ActiveJwk has copy, drop, store {
    jwk_id: JwkId,
    jwk: JWK,
    epoch: u64,
}