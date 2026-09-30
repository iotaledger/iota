// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

// A SmartAccount created with builder_v1 and a custom authenticator, without a
// key, attaches a built-in Ed25519 key and then rotates to the built-in Ed25519
// authenticator, each in its own transaction sent by the account itself and
// authenticated by the custom authenticator.

//# init --addresses test=0x0 --accounts A

//# publish --sender A
module test::custom_smart_account;

use iota::authenticator_function;
use iota::package_metadata::PackageMetadataV1;
use iota::smart_account::{Self, SmartAccount};
use std::ascii;

/// Accepts every transaction.
#[authenticator]
public fun authenticate(_account: &SmartAccount, _auth_ctx: &AuthContext, _ctx: &TxContext) {}

/// Creates a shared SmartAccount with `module_name::function_name` as its
/// authenticator and no built-in key.
public fun create(
    package_metadata: &PackageMetadataV1,
    module_name: ascii::String,
    function_name: ascii::String,
    ctx: &mut TxContext,
): address {
    let authenticator = authenticator_function::create_auth_function_ref_v1<SmartAccount>(
        package_metadata,
        module_name,
        function_name,
    );
    smart_account::builder_v1(authenticator, ctx).build_v1()
}

//# init-abstract-account --sender A --package-metadata object(1,5) --inputs "custom_smart_account" "authenticate" --create-function test::custom_smart_account::create --account-type iota::smart_account::SmartAccount

//# view-object 2,2

// Attach the Ed25519 key.

//# abstract --account immshared(2,2) --ptb-inputs object(2,2) x"cc62332e34bb2d5cd69f60efbb2a36cb916c7eb458301ea36636c4dbb012bd88"
//> 0: iota::signature_scheme::ed25519();
//> 1: iota::public_key::create(Result(0), Input(1));
//> 2: iota::smart_account::attach_builtin_auth_public_key(Input(0), Result(1));

// Rotate to the built-in Ed25519 authenticator.

//# abstract --account immshared(2,2) --ptb-inputs object(2,2)
//> 0: iota::builtin_authenticator_functions::ed25519_authenticator_function_ref_v1<iota::smart_account::SmartAccount>();
//> 1: iota::smart_account::rotate_auth_function_ref_v1(Input(0), Result(0));

//# view-object 2,2

// The attached key and the authenticator, both dynamic fields of the account.

//# view-object 4,0

//# view-object 2,1

// The account is now authenticated by the built-in Ed25519 authenticator, which
// requires a signature: a transaction without one is rejected.

//# abstract --account immshared(2,2) --ptb-inputs object(2,2)
//> 0: iota::smart_account::has_builtin_auth_public_key(Input(0));
