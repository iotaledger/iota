// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

// A SmartAccount created with builder_v1 and the built-in Ed25519
// authenticator, but no key, is locked for good: attaching a key and rotating
// the authenticator need the account itself as the sender, and the account
// cannot authenticate without a key. Each is tried from another sender, which
// the account's sender check refuses, and from the account, which is rejected
// before execution. The account cannot call `increment` either.

//# init --addresses test=0x0 --accounts A

//# publish --sender A
module test::keyless_smart_account;

use iota::builtin_authenticator_functions;
use iota::package_metadata::PackageMetadataV1;
use iota::smart_account::{Self, SmartAccount};

public struct Counter has key {
    id: UID,
    value: u64,
}

fun init(ctx: &mut TxContext) {
    transfer::share_object(Counter { id: object::new(ctx), value: 0 })
}

public fun increment(counter: &mut Counter) {
    counter.value = counter.value + 1;
}

/// The custom authenticator the account tries to rotate to; accepts every
/// transaction.
#[authenticator]
public fun authenticate(_account: &SmartAccount, _auth_ctx: &AuthContext, _ctx: &TxContext) {}

/// Creates a shared SmartAccount with the built-in Ed25519 authenticator and
/// no key.
public fun create(_package_metadata: &PackageMetadataV1, ctx: &mut TxContext): address {
    let authenticator = builtin_authenticator_functions::ed25519_authenticator_function_ref_v1();
    smart_account::builder_v1(authenticator, ctx).build_v1()
}

//# init-abstract-account --sender A --package-metadata object(1,5) --create-function test::keyless_smart_account::create --account-type iota::smart_account::SmartAccount

//# view-object 2,2

// attach_pk.

//# programmable --sender A --inputs object(2,2) pubkey(A)
//> 0: iota::public_key::from_prefixed_bytes(Input(1));
//> 1: iota::smart_account::attach_builtin_auth_public_key(Input(0), Result(0));

//# abstract --account immshared(2,2) --builtin-signer A --ptb-inputs object(2,2) pubkey(A)
//> 0: iota::public_key::from_prefixed_bytes(Input(1));
//> 1: iota::smart_account::attach_builtin_auth_public_key(Input(0), Result(0));

// rotate_auth(builtin).

//# programmable --sender A --inputs object(2,2)
//> 0: iota::builtin_authenticator_functions::secp256k1_authenticator_function_ref_v1<iota::smart_account::SmartAccount>();
//> 1: iota::smart_account::rotate_auth_function_ref_v1(Input(0), Result(0));

//# abstract --account immshared(2,2) --builtin-signer A --ptb-inputs object(2,2)
//> 0: iota::builtin_authenticator_functions::secp256k1_authenticator_function_ref_v1<iota::smart_account::SmartAccount>();
//> 1: iota::smart_account::rotate_auth_function_ref_v1(Input(0), Result(0));

// rotate_auth(custom).

//# programmable --sender A --inputs object(2,2) object(1,5) "keyless_smart_account" "authenticate"
//> 0: iota::authenticator_function::create_auth_function_ref_v1<iota::smart_account::SmartAccount>(Input(1), Input(2), Input(3));
//> 1: iota::smart_account::rotate_auth_function_ref_v1(Input(0), Result(0));

//# abstract --account immshared(2,2) --builtin-signer A --ptb-inputs object(2,2) object(1,5) "keyless_smart_account" "authenticate"
//> 0: iota::authenticator_function::create_auth_function_ref_v1<iota::smart_account::SmartAccount>(Input(1), Input(2), Input(3));
//> 1: iota::smart_account::rotate_auth_function_ref_v1(Input(0), Result(0));

// Nothing else can be sent from the account either.

//# abstract --account immshared(2,2) --builtin-signer A --ptb-inputs object(1,0)
//> 0: test::keyless_smart_account::increment(Input(0));

// No call went through.

//# view-object 1,0
