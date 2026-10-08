// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

/// An Ed25519 authenticator for `SmartAccount`, adapted from
/// `public_key_iotaccount::ed25519_authenticator` in the `public_key_authentication` example.
///
/// That example's helpers take the account's `UID`, which only the framework can reach for a
/// `SmartAccount`. This module reads the public key attached with `smart_account_public_key`
/// instead: the same key the built-in authenticator reads, so an account can rotate between the
/// two without touching the key.
///
/// `BuiltinAuthRule` refuses detaching the key only while the built-in authenticator is set:
/// detaching it while this authenticator is set leaves the account unable to authenticate.
module smart_account_allowed_authenticators::ed25519_authenticator;

use iota::ed25519;
use iota::signature_scheme;
use iota::smart_account::SmartAccount;
use iota::smart_account_public_key;

// === Errors ===

#[error(code = 0)]
const ENotAnEd25519Key: vector<u8> = b"The account's public key is not an Ed25519 key.";
#[error(code = 1)]
const EEd25519VerificationFailed: vector<u8> = b"Ed25519 signature verification failed.";

// === Authenticators ===

/// Authenticates a transaction with an Ed25519 signature over its digest.
#[authenticator]
public fun authenticate(
    account: &SmartAccount,
    signature: vector<u8>,
    _: &AuthContext,
    ctx: &TxContext,
) {
    verify_signature(account, &signature, ctx);
}

// === Public Functions ===

/// Aborts unless `signature` is a valid Ed25519 signature of the transaction digest by the
/// account's public key.
public fun verify_signature(account: &SmartAccount, signature: &vector<u8>, ctx: &TxContext) {
    let public_key = smart_account_public_key::borrow_public_key(account);
    assert!(public_key.scheme() == signature_scheme::ed25519(), ENotAnEd25519Key);
    assert!(
        ed25519::ed25519_verify(signature, public_key.raw_bytes(), ctx.digest()),
        EEd25519VerificationFailed,
    );
}
