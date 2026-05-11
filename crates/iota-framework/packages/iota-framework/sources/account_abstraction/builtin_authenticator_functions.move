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
use iota::protocol_config;
use iota::public_key::PublicKey;
use std::ascii;

// === Errors ===

#[error(code = 0)]
const EBuiltinAuthenticatorsNotEnabled: vector<u8> = b"Built-in Move authenticators not enabled.";

#[error(code = 10)]
const EPublicKeyMissing: vector<u8> = b"Public key missing.";
#[error(code = 11)]
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

/// Returns an `AuthenticatorFunctionRefV1` that references the built-in Ed25519 authenticator.
///
/// `MoveAuthenticator` must carry exactly one call argument — the signature — and no
/// type arguments. `call_args[0]` must be a `Pure` argument containing a BCS-encoded
/// `vector<u8>` whose decoded bytes are the flag-prefixed Ed25519 signature wire format:
///
/// ```
/// 0x00 || sig[64B] || pk[32B]   (97 bytes total)
/// ```
///
/// `sig` is the 64-byte Ed25519 signature over
/// `IntentMessage(Intent::iota_transaction(), TransactionData)`.
/// `pk` is the 32-byte Ed25519 public key. The signature is verified against the address
/// derived from the public key stored as a dynamic field on the account.
///
/// Aborts if `enable_builtin_move_authenticators` is not enabled in the protocol config.
public fun ed25519_authenticator_function_ref_v1<Account: key>(): AuthenticatorFunctionRefV1<
    Account,
> {
    check_builtin_authenticators_enabled();

    authenticator_function::create_auth_function_ref_v1_inner(
        @iota,
        ascii::string(BUILTIN_AUTHENTICATOR_FUNCTIONS_MODULE_NAME),
        ascii::string(ED25519_AUTHENTICATOR_FUN_NAME_V1),
    )
}

/// Returns an `AuthenticatorFunctionRefV1` that references the built-in Secp256k1 authenticator.
///
/// `MoveAuthenticator` must carry exactly one call argument — the signature — and no
/// type arguments. `call_args[0]` must be a `Pure` argument containing a BCS-encoded
/// `vector<u8>` whose decoded bytes are the flag-prefixed Secp256k1 signature wire format:
///
/// ```
/// 0x01 || sig[64B] || pk[33B]   (98 bytes total)
/// ```
///
/// `sig` is the 64-byte compact (r, s) Secp256k1 signature over
/// `IntentMessage(Intent::iota_transaction(), TransactionData)`.
/// `pk` is the 33-byte compressed Secp256k1 public key. The signature is verified against the
/// address derived from the public key stored as a dynamic field on the account.
///
/// Aborts if `enable_builtin_move_authenticators` is not enabled in the protocol config.
public fun secp256k1_authenticator_function_ref_v1<Account: key>(): AuthenticatorFunctionRefV1<
    Account,
> {
    check_builtin_authenticators_enabled();

    authenticator_function::create_auth_function_ref_v1_inner(
        @iota,
        ascii::string(BUILTIN_AUTHENTICATOR_FUNCTIONS_MODULE_NAME),
        ascii::string(SECP256K1_AUTHENTICATOR_FUN_NAME_V1),
    )
}

/// Returns an `AuthenticatorFunctionRefV1` that references the built-in Secp256r1 authenticator.
///
/// `MoveAuthenticator` must carry exactly one call argument — the signature — and no
/// type arguments. `call_args[0]` must be a `Pure` argument containing a BCS-encoded
/// `vector<u8>` whose decoded bytes are the flag-prefixed Secp256r1 signature wire format:
///
/// ```
/// 0x02 || sig[64B] || pk[33B]   (98 bytes total)
/// ```
///
/// `sig` is the 64-byte compact (r, s) Secp256r1 signature over
/// `IntentMessage(Intent::iota_transaction(), TransactionData)`.
/// `pk` is the 33-byte compressed Secp256r1 public key. The signature is verified against the
/// address derived from the public key stored as a dynamic field on the account.
///
/// Aborts if `enable_builtin_move_authenticators` is not enabled in the protocol config.
public fun secp256r1_authenticator_function_ref_v1<Account: key>(): AuthenticatorFunctionRefV1<
    Account,
> {
    check_builtin_authenticators_enabled();

    authenticator_function::create_auth_function_ref_v1_inner(
        @iota,
        ascii::string(BUILTIN_AUTHENTICATOR_FUNCTIONS_MODULE_NAME),
        ascii::string(SECP256R1_AUTHENTICATOR_FUN_NAME_V1),
    )
}

/// Returns an `AuthenticatorFunctionRefV1` that references the built-in MultiSig authenticator.
///
/// `MoveAuthenticator` must carry exactly one call argument — the signature — and no
/// type arguments. `call_args[0]` must be a `Pure` argument containing a BCS-encoded
/// `vector<u8>` whose decoded bytes are the flag-prefixed MultiSig signature wire format:
///
/// ```
/// 0x03 || <MultiSig wire bytes>   (variable length)
/// ```
///
/// The MultiSig wire bytes encode the bitmap of participating signers, their individual
/// signatures, and the composite public key. The composite signature is verified against
/// the address derived from the `MultiSigPublicKey` stored as a dynamic field on the account.
///
/// Aborts if `enable_builtin_move_authenticators` is not enabled in the protocol config.
public fun multisig_authenticator_function_ref_v1<Account: key>(): AuthenticatorFunctionRefV1<
    Account,
> {
    check_builtin_authenticators_enabled();

    authenticator_function::create_auth_function_ref_v1_inner(
        @iota,
        ascii::string(BUILTIN_AUTHENTICATOR_FUNCTIONS_MODULE_NAME),
        ascii::string(MULTISIG_AUTHENTICATOR_FUN_NAME_V1),
    )
}

/// Returns an `AuthenticatorFunctionRefV1` that references the built-in Passkey authenticator.
///
/// `MoveAuthenticator` must carry exactly one call argument — the signature — and no
/// type arguments. `call_args[0]` must be a `Pure` argument containing a BCS-encoded
/// `vector<u8>` whose decoded bytes are the flag-prefixed Passkey (WebAuthn) signature wire
/// format:
///
/// ```
/// 0x06 || <PasskeyAuthenticator wire bytes>   (variable length)
/// ```
///
/// The Passkey wire bytes encode the authenticator data, client data JSON, and the Secp256r1
/// signature produced by the WebAuthn credential. The challenge embedded in `clientDataJSON`
/// must equal `Blake2b256(IntentMessage(Intent::iota_transaction(), TransactionData))`.
/// The signature is verified against the address derived from the Secp256r1 public key stored
/// as a dynamic field on the account.
///
/// Aborts if `enable_builtin_move_authenticators` is not enabled in the protocol config.
public fun passkey_authenticator_function_ref_v1<Account: key>(): AuthenticatorFunctionRefV1<
    Account,
> {
    check_builtin_authenticators_enabled();

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

/// Aborts if the built-in Move authenticators feature is disabled in the protocol config.
fun check_builtin_authenticators_enabled() {
    assert!(
        protocol_config::is_feature_enabled(b"enable_builtin_move_authenticators"),
        EBuiltinAuthenticatorsNotEnabled,
    );
}

// === Test Functions ===
