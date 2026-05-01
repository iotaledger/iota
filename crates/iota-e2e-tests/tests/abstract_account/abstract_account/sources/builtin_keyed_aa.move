// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

/// Provides account creation helpers that wire the standard IOTA built-in
/// authenticators (Ed25519, Secp256k1, Secp256r1, MultiSig, Passkey) to an
/// `AbstractAccount` shared object.
///
/// Each function:
///   1. Asserts that the supplied `PublicKey` carries the expected scheme flag.
///   2. Obtains the appropriate `AuthenticatorFunctionRefV1` from the framework.
///   3. Creates an `AbstractAccountBuilder` via `abstract_account::builder`.
///   4. Attaches the public key using `abstract_account::attach_builtin_public_key`.
///   5. Finalises the account with `.build()`.
module abstract_account::builtin_keyed_aa;

use abstract_account::abstract_account::{Self, AbstractAccount};
use iota::builtin_authenticator_functions;
use iota::public_key::PublicKey;
use iota::signature_scheme;

// === Errors ===

#[error(code = 0)]
const EUnexpectedPublicKeyScheme: vector<u8> =
    b"Public key scheme does not match the expected authenticator scheme.";

// === Public Functions ===

/// Creates a new `AbstractAccount` authenticated by the built-in Ed25519 authenticator.
public fun create_with_ed25519(public_key: PublicKey, ctx: &mut TxContext) {
    assert!(public_key.scheme() == signature_scheme::ed25519(), EUnexpectedPublicKeyScheme);

    let authenticator = builtin_authenticator_functions::ed25519_authenticator_function_ref_v1<
        AbstractAccount,
    >();
    abstract_account::builder(authenticator, ctx).attach_builtin_public_key(public_key).build();
}

/// Creates a new `AbstractAccount` authenticated by the built-in Secp256k1 authenticator.
public fun create_with_secp256k1(public_key: PublicKey, ctx: &mut TxContext) {
    assert!(public_key.scheme() == signature_scheme::secp256k1(), EUnexpectedPublicKeyScheme);

    let authenticator = builtin_authenticator_functions::secp256k1_authenticator_function_ref_v1<
        AbstractAccount,
    >();
    abstract_account::builder(authenticator, ctx).attach_builtin_public_key(public_key).build();
}

/// Creates a new `AbstractAccount` authenticated by the built-in Secp256r1 authenticator.
public fun create_with_secp256r1(public_key: PublicKey, ctx: &mut TxContext) {
    assert!(public_key.scheme() == signature_scheme::secp256r1(), EUnexpectedPublicKeyScheme);

    let authenticator = builtin_authenticator_functions::secp256r1_authenticator_function_ref_v1<
        AbstractAccount,
    >();
    abstract_account::builder(authenticator, ctx).attach_builtin_public_key(public_key).build();
}

/// Creates a new `AbstractAccount` authenticated by the built-in MultiSig authenticator.
public fun create_with_multisig(public_key: PublicKey, ctx: &mut TxContext) {
    assert!(public_key.scheme() == signature_scheme::multisig(), EUnexpectedPublicKeyScheme);

    let authenticator = builtin_authenticator_functions::multisig_authenticator_function_ref_v1<
        AbstractAccount,
    >();
    abstract_account::builder(authenticator, ctx).attach_builtin_public_key(public_key).build();
}

/// Creates a new `AbstractAccount` authenticated by the built-in Passkey authenticator.
public fun create_with_passkey(public_key: PublicKey, ctx: &mut TxContext) {
    assert!(public_key.scheme() == signature_scheme::passkey(), EUnexpectedPublicKeyScheme);

    let authenticator = builtin_authenticator_functions::passkey_authenticator_function_ref_v1<
        AbstractAccount,
    >();
    abstract_account::builder(authenticator, ctx).attach_builtin_public_key(public_key).build();
}
