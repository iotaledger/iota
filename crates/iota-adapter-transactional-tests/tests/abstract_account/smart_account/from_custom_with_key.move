// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

// The transitions out of the yellow state: a SmartAccount with a key and a
// custom authenticator. The account starts without a key, gains one, changes
// it and its authenticator, loses it and gains it again, and finally switches
// to the built-in authenticator of the key's scheme. After every transition
// the account itself calls `increment`, which must succeed.

//# init --addresses test=0x0 --accounts A B

//# publish --sender A
module test::yellow_account;

use iota::authenticator_function;
use iota::package_metadata::PackageMetadataV1;
use iota::smart_account::{Self, SmartAccount};
use std::ascii;

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

/// The account's first custom authenticator; accepts every transaction.
#[authenticator]
public fun authenticate(_account: &SmartAccount, _auth_ctx: &AuthContext, _ctx: &TxContext) {}

/// The custom authenticator the account rotates to; accepts every
/// transaction.
#[authenticator]
public fun authenticate2(_account: &SmartAccount, _auth_ctx: &AuthContext, _ctx: &TxContext) {}

/// Creates a shared SmartAccount with `module_name::function_name` as its
/// authenticator and no key.
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

// builder_v1(custom), then build_v1: no key yet.

//# init-abstract-account --sender A --package-metadata object(1,5) --inputs "yellow_account" "authenticate" --create-function test::yellow_account::create --account-type iota::smart_account::SmartAccount

//# abstract --account immshared(2,2) --ptb-inputs object(1,0)
//> 0: test::yellow_account::increment(Input(0));

// attach_pk: A's key (yellow).

//# abstract --account immshared(2,2) --ptb-inputs object(2,2) pubkey(A)
//> 0: iota::public_key::from_prefixed_bytes(Input(1));
//> 1: iota::smart_account::attach_builtin_auth_public_key(Input(0), Result(0));

//# abstract --account immshared(2,2) --ptb-inputs object(1,0)
//> 0: test::yellow_account::increment(Input(0));

// rotate_pk: to B's key.

//# abstract --account immshared(2,2) --ptb-inputs object(2,2) pubkey(B)
//> 0: iota::public_key::from_prefixed_bytes(Input(1));
//> 1: iota::smart_account::rotate_builtin_auth_public_key(Input(0), Result(0));

//# abstract --account immshared(2,2) --ptb-inputs object(1,0)
//> 0: test::yellow_account::increment(Input(0));

// rotate_auth(custom): to the second custom authenticator.

//# abstract --account immshared(2,2) --ptb-inputs object(2,2) object(1,5) "yellow_account" "authenticate2"
//> 0: iota::authenticator_function::create_auth_function_ref_v1<iota::smart_account::SmartAccount>(Input(1), Input(2), Input(3));
//> 1: iota::smart_account::rotate_auth_function_ref_v1(Input(0), Result(0));

//# abstract --account immshared(2,2) --ptb-inputs object(1,0)
//> 0: test::yellow_account::increment(Input(0));

// detach_pk: back to a custom authenticator without a key.

//# abstract --account immshared(2,2) --ptb-inputs object(2,2)
//> 0: iota::smart_account::detach_builtin_auth_public_key(Input(0));

//# abstract --account immshared(2,2) --ptb-inputs object(1,0)
//> 0: test::yellow_account::increment(Input(0));

// attach_pk: A's key again (yellow).

//# abstract --account immshared(2,2) --ptb-inputs object(2,2) pubkey(A)
//> 0: iota::public_key::from_prefixed_bytes(Input(1));
//> 1: iota::smart_account::attach_builtin_auth_public_key(Input(0), Result(0));

//# abstract --account immshared(2,2) --ptb-inputs object(1,0)
//> 0: test::yellow_account::increment(Input(0));

// rotate_auth(builtin): the built-in Ed25519 authenticator (green). From now on
// the account is authenticated by A's signature.

//# abstract --account immshared(2,2) --ptb-inputs object(2,2)
//> 0: iota::builtin_authenticator_functions::ed25519_authenticator_function_ref_v1<iota::smart_account::SmartAccount>();
//> 1: iota::smart_account::rotate_auth_function_ref_v1(Input(0), Result(0));

//# abstract --account immshared(2,2) --builtin-signer A --ptb-inputs object(1,0)
//> 0: test::yellow_account::increment(Input(0));

// Seven calls went through.

//# view-object 1,0
