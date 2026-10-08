// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

/// Provides the built-in authenticator for the standard IOTA signature schemes (Ed25519,
/// Secp256k1, Secp256r1, MultiSig, Passkey) together with the public-key lifecycle primitives
/// needed to set up and rotate it on an account.
///
/// The built-in authenticator verifies a signature with the public key attached to the account,
/// using that key's signature scheme.
///
/// # Account creation
/// To create a new account backed by the built-in authenticator, attach the public key and obtain
/// the authenticator function ref, then pass it to `account::create_account_v1`:
///
/// ```move
/// builtin_authenticator_functions::attach_public_key(&mut account.id, public_key::create(scheme, raw_pk_bytes));
/// let authenticator_function_ref = builtin_authenticator_functions::builtin_authenticator_function_ref_v1<Account>();
/// account::create_account_v1(account, authenticator_function_ref);
/// ```
///
/// # Public key rotation
/// To replace the public key, including switching to a different signature scheme, rotate the
/// stored key. The authenticator function ref stays the same:
///
/// ```move
/// let old_public_key = builtin_authenticator_functions::rotate_public_key(&mut account.id, public_key::create(new_scheme, new_raw_pk_bytes));
/// ```
///
/// # Switching to a custom authenticator
/// To migrate away from the built-in authenticator entirely, detach the stored public key and
/// obtain an authenticator function ref from the target authenticator module:
///
/// ```move
/// let old_public_key = builtin_authenticator_functions::detach_public_key(&mut account.id);
/// let new_authenticator_function_ref = ...;
/// account::rotate_auth_function_ref_v1(account, new_authenticator_function_ref);
/// ```
module iota::builtin_authenticator_functions;

use iota::authenticator_function::{Self, AuthenticatorFunctionRefV1};
use iota::dynamic_field as df;
use iota::event;
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

#[error(code = 20)]
const EInvalidSignature: vector<u8> = b"Invalid signature.";

// === Constants ===

const BUILTIN_AUTHENTICATOR_FUNCTIONS_MODULE_NAME: vector<u8> = b"builtin_authenticator_functions";

const BUILTIN_AUTHENTICATOR_FUN_NAME_V1: vector<u8> = b"builtin_authenticator_v1";

// === Events ===

/// Event: emitted when a public key is attached to an account.
public struct PublicKeyAttached has copy, drop {
    account_id: ID,
    public_key: PublicKey,
}

/// Event: emitted when a public key is detached from an account.
public struct PublicKeyDetached has copy, drop {
    account_id: ID,
    public_key: PublicKey,
}

/// Event: emitted when a public key is rotated on an account.
public struct PublicKeyRotated has copy, drop {
    account_id: ID,
    from: PublicKey,
    to: PublicKey,
}

// === Structs ===

/// Dynamic field key, where the system will look for a potential public key.
public struct PublicKeyFieldName has copy, drop, store {}

// === Public Functions ===

/// Returns an `AuthenticatorFunctionRefV1` that references the built-in authenticator.
///
/// The built-in authenticator verifies the signature with the public key attached to the
/// account (see `attach_public_key`), using that key's signature scheme.
///
/// `MoveAuthenticator` must carry exactly one call argument — the signature — and no
/// type arguments. `call_args[0]` must be a `Pure` argument containing a BCS-encoded
/// `vector<u8>` whose decoded bytes are the flag-prefixed signature wire format of the
/// attached key's scheme, signing `IntentMessage(Intent::iota_transaction(), TransactionData)`:
///
/// ```
/// Ed25519:   0x00 || sig[64B] || pk[32B]                (97 bytes total)
/// Secp256k1: 0x01 || sig[64B] || pk[33B]                (98 bytes total)
/// Secp256r1: 0x02 || sig[64B] || pk[33B]                (98 bytes total)
/// MultiSig:  0x03 || <MultiSig wire bytes>              (variable length)
/// Passkey:   0x06 || <PasskeyAuthenticator wire bytes>  (variable length)
/// ```
///
/// The Secp256k1 and Secp256r1 signatures are compact (r, s) signatures and the public keys are
/// compressed. The MultiSig wire bytes encode the bitmap of participating signers, their
/// individual signatures, and the composite public key. The Passkey wire bytes encode the
/// authenticator data, client data JSON, and the Secp256r1 signature produced by the WebAuthn
/// credential; the challenge embedded in `clientDataJSON` must equal
/// `Blake2b256(IntentMessage(Intent::iota_transaction(), TransactionData))`.
///
/// Aborts if `enable_builtin_move_authenticators` is not enabled in the protocol config.
public fun builtin_authenticator_function_ref_v1<Account: key>(): AuthenticatorFunctionRefV1<
    Account,
> {
    check_builtin_authenticators_enabled();

    authenticator_function::create_auth_function_ref_v1_inner(
        @iota,
        ascii::string(BUILTIN_AUTHENTICATOR_FUNCTIONS_MODULE_NAME),
        ascii::string(BUILTIN_AUTHENTICATOR_FUN_NAME_V1),
    )
}

/// Attaches `public_key` to the account. Aborts if a public key is already attached.
///
/// Call this before obtaining an authenticator function ref and passing it to
/// `account::create_account_v1`.
///
/// Emits a `PublicKeyAttached` event on success.
public fun attach_public_key(account_id: &mut UID, public_key: PublicKey) {
    assert!(!has_public_key(account_id), EPublicKeyAlreadyAttached);

    df::add(account_id, public_key_field_name(), public_key);

    let event = PublicKeyAttached {
        account_id: account_id.to_inner(),
        public_key,
    };
    event::emit(event);
}

/// Detaches and returns the public key attached to the account. Aborts if no public key is
/// currently attached.
///
/// Use this when migrating away from a built-in authenticator to a custom one.
///
/// Emits a `PublicKeyDetached` event on success.
public fun detach_public_key(account_id: &mut UID): PublicKey {
    assert!(has_public_key(account_id), EPublicKeyMissing);

    let public_key = df::remove(account_id, public_key_field_name());

    let event = PublicKeyDetached {
        account_id: account_id.to_inner(),
        public_key,
    };
    event::emit(event);

    public_key
}

/// Replaces the existing public key with `public_key` and returns the previous key.
/// Aborts if no public key is currently attached.
///
/// Call this before obtaining a new authenticator function ref and passing it to
/// `account::rotate_auth_function_ref_v1`.
///
/// Emits a `PublicKeyRotated` event on success.
public fun rotate_public_key(account_id: &mut UID, public_key: PublicKey): PublicKey {
    assert!(has_public_key(account_id), EPublicKeyMissing);

    let df_name = public_key_field_name();

    let prev_public_key = df::remove(account_id, df_name);
    df::add(account_id, df_name, public_key);

    let event = PublicKeyRotated {
        account_id: account_id.to_inner(),
        from: prev_public_key,
        to: public_key,
    };
    event::emit(event);

    prev_public_key
}

// === View Functions ===

/// Returns true if `authenticator` references the built-in authenticator.
public fun is_builtin_authenticator<Account: key>(
    authenticator: &AuthenticatorFunctionRefV1<Account>,
): bool {
    let module_name = ascii::string(BUILTIN_AUTHENTICATOR_FUNCTIONS_MODULE_NAME);
    let function_name = ascii::string(BUILTIN_AUTHENTICATOR_FUN_NAME_V1);

    authenticator.package() == object::id_from_address(@iota)
        && authenticator.module_name() == &module_name
        && authenticator.function_name() == &function_name
}

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

#[allow(unused_function)]
/// Authenticates a transaction sent by `account` with the public key attached to it.
///
/// Called by the executor for accounts using `builtin_authenticator_function_ref_v1`.
/// See that function for the `signature` format.
fun builtin_authenticator_v1<Account: key>(account: &Account, signature: vector<u8>) {
    check_builtin_authenticators_enabled();

    let public_key = borrow_public_key(borrow_account_uid(account));
    assert!(verify_builtin_signature(public_key, &signature), EInvalidSignature);
}

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

// === Native Functions ===

/// Borrows the account `UID`.
///
/// IMPORTANT: This function is allowed to be called only by the built-in authenticator.
native fun borrow_account_uid<Account: key>(account: &Account): &UID;

/// Returns true if `signature` is a valid signature of the transaction being authenticated
/// by `public_key`, using the signature scheme of `public_key`.
native fun verify_builtin_signature(public_key: &PublicKey, signature: &vector<u8>): bool;

// === Test Functions ===

#[test_only]
public fun builtin_authenticator_v1_for_testing<Account: key>(
    account: &Account,
    signature: vector<u8>,
) {
    builtin_authenticator_v1(account, signature)
}
