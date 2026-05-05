// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

/// Provides built-in authenticator function references for the standard IOTA signature schemes
/// (Ed25519, Secp256k1, Secp256r1, MultiSig, Passkey) together with the public-key lifecycle
/// primitives needed to set up and rotate those authenticators on an account.
///
/// # Account creation
/// To create a new account backed by a built-in authenticator, attach the public key and obtain
/// the authenticator function ref, then pass it to `account::create_account_v1`:
///
/// ```move
/// builtin_authenticator_functions::attach_public_key(&mut account.id, public_key::create(scheme, raw_pk_bytes));
/// let authenticator_function_ref = builtin_authenticator_functions::<scheme>_authenticator_function_ref_v1<Account>();
/// account::create_account_v1(account, authenticator_function_ref);
/// ```
///
/// # Authenticator rotation
/// To replace the public key or switch to a different built-in scheme, rotate the stored key and
/// obtain a new authenticator function ref, then pass it to `account::rotate_auth_function_ref_v1`:
///
/// ```move
/// let old_public_key = builtin_authenticator_functions::rotate_public_key(&mut account.id, public_key::create(new_scheme, new_raw_pk_bytes));
/// let new_authenticator_function_ref = builtin_authenticator_functions::<scheme>_authenticator_function_ref_v1<Account>();
/// account::rotate_auth_function_ref_v1(account, new_authenticator_function_ref);
/// ```
///
/// # Switching to a custom authenticator
/// To migrate away from a built-in authenticator entirely, detach the stored public key and obtain
/// an authenticator function ref from the target authenticator module:
///
/// ```move
/// let old_public_key = builtin_authenticator_functions::detach_public_key(&mut account.id);
/// let new_authenticator_function_ref = ...;
/// account::rotate_auth_function_ref_v1(account, new_authenticator_function_ref);
/// ```
module iota::builtin_authenticator_functions;

use iota::authenticator_function::{Self, AuthenticatorFunctionRefV1};
use iota::dynamic_field as df;
use iota::public_key::PublicKey;
use std::ascii;

// === Errors ===

#[error(code = 0)]
const EPublicKeyMissing: vector<u8> = b"Public key missing.";
#[error(code = 1)]
const EPublicKeyAlreadyAttached: vector<u8> = b"Public key already attached.";

// === Constants ===

const BUILTIN_AUTHENTICATOR_FUNCTIONS_MODULE_NAME: vector<u8> = b"builtin_authenticator_functions";

const ED25519_AUTHENTICATOR_FUN_NAME_V1: vector<u8> = b"ed25519_authenticator_function_ref_v1";
const SECP256K1_AUTHENTICATOR_FUN_NAME_V1: vector<u8> = b"secp256k1_authenticator_function_ref_v1";
const SECP256R1_AUTHENTICATOR_FUN_NAME_V1: vector<u8> = b"secp256r1_authenticator_function_ref_v1";
const MULTISIG_AUTHENTICATOR_FUN_NAME_V1: vector<u8> = b"multisig_authenticator_function_ref_v1";
const PASSKEY_AUTHENTICATOR_FUN_NAME_V1: vector<u8> = b"passkey_authenticator_function_ref_v1";

// === Structs ===

/// Dynamic field key, where the system will look for a potential public key.
public struct PublicKeyFieldName has copy, drop, store {}

// === Public Functions ===

/// Returns an `AuthenticatorFunctionRefV1` that references the built-in ed25519 authenticator function.
public fun ed25519_authenticator_function_ref_v1<Account: key>(): AuthenticatorFunctionRefV1<
    Account,
> {
    authenticator_function::create_auth_function_ref_v1_inner(
        @iota,
        ascii::string(BUILTIN_AUTHENTICATOR_FUNCTIONS_MODULE_NAME),
        ascii::string(ED25519_AUTHENTICATOR_FUN_NAME_V1),
    )
}

/// Returns an `AuthenticatorFunctionRefV1` that references the built-in secp256k1 authenticator function.
public fun secp256k1_authenticator_function_ref_v1<Account: key>(): AuthenticatorFunctionRefV1<
    Account,
> {
    authenticator_function::create_auth_function_ref_v1_inner(
        @iota,
        ascii::string(BUILTIN_AUTHENTICATOR_FUNCTIONS_MODULE_NAME),
        ascii::string(SECP256K1_AUTHENTICATOR_FUN_NAME_V1),
    )
}

/// Returns an `AuthenticatorFunctionRefV1` that references the built-in secp256r1 authenticator function.
public fun secp256r1_authenticator_function_ref_v1<Account: key>(): AuthenticatorFunctionRefV1<
    Account,
> {
    authenticator_function::create_auth_function_ref_v1_inner(
        @iota,
        ascii::string(BUILTIN_AUTHENTICATOR_FUNCTIONS_MODULE_NAME),
        ascii::string(SECP256R1_AUTHENTICATOR_FUN_NAME_V1),
    )
}

/// Returns an `AuthenticatorFunctionRefV1` that references the built-in multisig authenticator function.
public fun multisig_authenticator_function_ref_v1<Account: key>(): AuthenticatorFunctionRefV1<
    Account,
> {
    authenticator_function::create_auth_function_ref_v1_inner(
        @iota,
        ascii::string(BUILTIN_AUTHENTICATOR_FUNCTIONS_MODULE_NAME),
        ascii::string(MULTISIG_AUTHENTICATOR_FUN_NAME_V1),
    )
}

/// Returns an `AuthenticatorFunctionRefV1` that references the built-in passkey authenticator function.
public fun passkey_authenticator_function_ref_v1<Account: key>(): AuthenticatorFunctionRefV1<
    Account,
> {
    authenticator_function::create_auth_function_ref_v1_inner(
        @iota,
        ascii::string(BUILTIN_AUTHENTICATOR_FUNCTIONS_MODULE_NAME),
        ascii::string(PASSKEY_AUTHENTICATOR_FUN_NAME_V1),
    )
}

/// Attaches `public_key` to the account. Aborts if a public key is already attached.
///
/// Call this before obtaining an authenticator function ref and passing it to
/// `account::create_account_v1`.
public fun attach_public_key(account_id: &mut UID, public_key: PublicKey) {
    assert!(!has_public_key(account_id), EPublicKeyAlreadyAttached);
    df::add(account_id, public_key_field_name(), public_key)
}

/// Replaces the existing public key with `public_key` and returns the previous key.
/// Aborts if no public key is currently attached.
///
/// Call this before obtaining a new authenticator function ref and passing it to
/// `account::rotate_auth_function_ref_v1`.
public fun rotate_public_key(account_id: &mut UID, public_key: PublicKey): PublicKey {
    assert!(has_public_key(account_id), EPublicKeyMissing);

    let df_name = public_key_field_name();

    let prev_public_key = df::remove(account_id, df_name);
    df::add(account_id, df_name, public_key);
    prev_public_key
}

/// Detaches and returns the public key attached to the account. Aborts if no public key is
/// currently attached.
///
/// Use this when migrating away from a built-in authenticator to a custom one.
public fun detach_public_key(account_id: &mut UID): PublicKey {
    assert!(has_public_key(account_id), EPublicKeyMissing);

    df::remove(account_id, public_key_field_name())
}

// === View Functions ===

/// Returns true if the account has a public key attached.
public fun has_public_key(account_id: &UID): bool {
    df::exists_(account_id, public_key_field_name())
}

/// Borrows the public key attached to the account. Aborts if no public key is
/// currently attached.
public fun borrow_public_key(account_id: &UID): &PublicKey {
    assert!(has_public_key(account_id), EPublicKeyMissing);

    df::borrow(account_id, public_key_field_name())
}

// === Admin Functions ===

// === Package Functions ===

// === Private Functions ===

/// A utility function to construct the dynamic field name for the public key field.
fun public_key_field_name(): PublicKeyFieldName {
    PublicKeyFieldName {}
}

// === Test Functions ===
