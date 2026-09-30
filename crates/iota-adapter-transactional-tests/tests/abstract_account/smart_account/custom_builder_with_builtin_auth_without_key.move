// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

// A SmartAccount created with builder_v1 and the built-in Ed25519
// authenticator, but no key, can never gain one: attaching a key needs the
// account itself as the sender, and the account cannot authenticate without a
// key.

//# init --addresses test=0x0 --accounts A

//# publish --sender A
module test::keyless_smart_account;

use iota::builtin_authenticator_functions;
use iota::package_metadata::PackageMetadataV1;
use iota::smart_account::{Self, SmartAccount};

/// Never used by the account. `init-abstract-account` passes the package
/// metadata to `create`, and a package has metadata only if it defines an
/// authenticator.
#[authenticator]
public fun unused_authenticator(
    _account: &SmartAccount,
    _auth_ctx: &AuthContext,
    _ctx: &TxContext,
) {}

/// Creates a shared SmartAccount with the built-in Ed25519 authenticator and
/// no key.
public fun create(_package_metadata: &PackageMetadataV1, ctx: &mut TxContext): address {
    let authenticator = builtin_authenticator_functions::ed25519_authenticator_function_ref_v1();
    smart_account::builder_v1(authenticator, ctx).build_v1()
}

//# init-abstract-account --sender A --package-metadata object(1,4) --create-function test::keyless_smart_account::create --account-type iota::smart_account::SmartAccount

//# view-object 2,2

// Another sender cannot attach a key: the account itself must be the sender.

//# programmable --sender A --inputs object(2,2) x"cc62332e34bb2d5cd69f60efbb2a36cb916c7eb458301ea36636c4dbb012bd88"
//> 0: iota::signature_scheme::ed25519();
//> 1: iota::public_key::create(Result(0), Input(1));
//> 2: iota::smart_account::attach_builtin_auth_public_key(Input(0), Result(1));

// The account itself cannot send the transaction: its built-in authenticator
// finds no key to verify the signature against.

//# abstract --account immshared(2,2) --auth-inputs x"00" --ptb-inputs object(2,2) x"cc62332e34bb2d5cd69f60efbb2a36cb916c7eb458301ea36636c4dbb012bd88"
//> 0: iota::signature_scheme::ed25519();
//> 1: iota::public_key::create(Result(0), Input(1));
//> 2: iota::smart_account::attach_builtin_auth_public_key(Input(0), Result(1));
