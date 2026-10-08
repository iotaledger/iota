// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

/// An Ed25519 authenticator for `SmartAccount`, adapted from
/// `public_key_iotaccount::ed25519_authenticator` in the `public_key_authentication` example.
///
/// The account keeps the Ed25519 public key for this authenticator in its own field, set with
/// `set_public_key`. It can't use the built-in authenticator's key: `smart_account_builtin_auth`
/// detaches that key whenever the account rotates to a custom authenticator.
module smart_account_allowed_authenticators::ed25519_authenticator;

use iota::ed25519;
use iota::smart_account::SmartAccount;

// === Errors ===

#[error(code = 0)]
const EPublicKeyMissing: vector<u8> = b"No Ed25519 public key set for this authenticator.";
#[error(code = 1)]
const EEd25519VerificationFailed: vector<u8> = b"Ed25519 signature verification failed.";

// === Structs ===

/// Field key of the account's Ed25519 public key for this authenticator.
public struct Ed25519PublicKeyFieldName has copy, drop, store {}

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
/// account's Ed25519 public key.
public fun verify_signature(account: &SmartAccount, signature: &vector<u8>, ctx: &TxContext) {
    assert!(account.has_field(Ed25519PublicKeyFieldName {}), EPublicKeyMissing);
    let public_key: &vector<u8> = account.borrow_field(Ed25519PublicKeyFieldName {});
    assert!(
        ed25519::ed25519_verify(signature, public_key, ctx.digest()),
        EEd25519VerificationFailed,
    );
}

// === Admin Functions ===

/// Sets the account's Ed25519 public key for this authenticator, replacing any previous one.
///
/// Aborts if the transaction sender is not the account.
public fun set_public_key(account: &mut SmartAccount, public_key: vector<u8>, ctx: &TxContext) {
    if (account.has_field(Ed25519PublicKeyFieldName {})) {
        account.rotate_field<_, vector<u8>>(Ed25519PublicKeyFieldName {}, public_key, ctx);
    } else {
        account.add_field(Ed25519PublicKeyFieldName {}, public_key, ctx);
    }
}

// === View Functions ===

/// Returns the account's Ed25519 public key for this authenticator, if one is set.
public fun public_key(account: &SmartAccount): Option<vector<u8>> {
    if (account.has_field(Ed25519PublicKeyFieldName {})) {
        option::some(*account.borrow_field(Ed25519PublicKeyFieldName {}))
    } else {
        option::none()
    }
}
