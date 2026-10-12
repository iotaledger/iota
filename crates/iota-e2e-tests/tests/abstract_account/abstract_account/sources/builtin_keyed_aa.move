// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

/// Provides an account creation helper that wires the standard IOTA built-in
/// authenticator (Ed25519, Secp256k1, Secp256r1, MultiSig, Passkey) to an
/// `AbstractAccount` shared object.
module abstract_account::builtin_keyed_aa;

use abstract_account::abstract_account::{Self, AbstractAccount};
use iota::builtin_authenticator_functions;
use iota::public_key::PublicKey;

// === Public Functions ===

/// Creates a new `AbstractAccount` authenticated by the built-in authenticator with
/// `public_key`.
public fun create(public_key: PublicKey, ctx: &mut TxContext) {
    let authenticator = builtin_authenticator_functions::builtin_authenticator_function_ref_v1<
        AbstractAccount,
    >();
    abstract_account::builder(authenticator, ctx).attach_builtin_public_key(public_key).build();
}
