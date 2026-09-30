// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

// The transitions out of the green state: a SmartAccount with a key and the
// built-in authenticator of the key's scheme. After every transition the
// account itself calls `increment`: the call succeeds while the account can
// authenticate, and is rejected once it cannot.

//# init --addresses test=0x0 --accounts A --signers K1=secp256k1

//# publish --sender A
module test::green_account;

use iota::package_metadata::PackageMetadataV1;
use iota::public_key;
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

/// The custom authenticator an account rotates to; accepts every transaction.
#[authenticator]
public fun authenticate(_account: &SmartAccount, _auth_ctx: &AuthContext, _ctx: &TxContext) {}

/// Creates a shared SmartAccount with `prefixed_public_key` and the built-in
/// authenticator of its scheme. `init-abstract-account` passes the package
/// metadata first.
public fun create(
    _package_metadata: &PackageMetadataV1,
    prefixed_public_key: vector<u8>,
    ctx: &mut TxContext,
): address {
    let public_key = public_key::from_prefixed_bytes(prefixed_public_key);
    smart_account::builtin_auth_builder_v1(public_key, ctx).build_v1()
}

// === Account 1: builtin_auth_builder_v1, then rotate_pk + rotate_auth(builtin), then rotate_auth(custom) ===

//# init-abstract-account --sender A --package-metadata object(1,6) --inputs pubkey(A) --create-function test::green_account::create --account-type iota::smart_account::SmartAccount

//# abstract --account immshared(2,3) --builtin-signer A --ptb-inputs object(1,0)
//> 0: test::green_account::increment(Input(0));

// Switch from Ed25519 to Secp256k1: the key and the authenticator together.

//# abstract --account immshared(2,3) --builtin-signer A --ptb-inputs object(2,3) pubkey(K1)
//> 0: iota::builtin_authenticator_functions::secp256k1_authenticator_function_ref_v1<iota::smart_account::SmartAccount>();
//> 1: iota::smart_account::rotate_auth_function_ref_v1(Input(0), Result(0));
//> 2: iota::public_key::from_prefixed_bytes(Input(1));
//> 3: iota::smart_account::rotate_builtin_auth_public_key(Input(0), Result(2));

//# abstract --account immshared(2,3) --builtin-signer K1 --ptb-inputs object(1,0)
//> 0: test::green_account::increment(Input(0));

// To a custom authenticator: the key stays attached (yellow).

//# abstract --account immshared(2,3) --builtin-signer K1 --ptb-inputs object(2,3) object(1,6) "green_account" "authenticate"
//> 0: iota::authenticator_function::create_auth_function_ref_v1<iota::smart_account::SmartAccount>(Input(1), Input(2), Input(3));
//> 1: iota::smart_account::rotate_auth_function_ref_v1(Input(0), Result(0));

//# abstract --account immshared(2,3) --ptb-inputs object(1,0)
//> 0: test::green_account::increment(Input(0));

// === Account 2: detach_pk locks the account ===

//# init-abstract-account --sender A --package-metadata object(1,6) --inputs pubkey(A) --create-function test::green_account::create --account-type iota::smart_account::SmartAccount

//# abstract --account immshared(8,3) --builtin-signer A --ptb-inputs object(8,3)
//> 0: iota::smart_account::detach_builtin_auth_public_key(Input(0));

//# abstract --account immshared(8,3) --builtin-signer A --ptb-inputs object(1,0)
//> 0: test::green_account::increment(Input(0));

// === Account 3: rotate_pk alone to another scheme locks the account ===

//# init-abstract-account --sender A --package-metadata object(1,6) --inputs pubkey(A) --create-function test::green_account::create --account-type iota::smart_account::SmartAccount

//# abstract --account immshared(11,3) --builtin-signer A --ptb-inputs object(11,3) pubkey(K1)
//> 0: iota::public_key::from_prefixed_bytes(Input(1));
//> 1: iota::smart_account::rotate_builtin_auth_public_key(Input(0), Result(0));

// Neither the new key nor the old one can sign any more.

//# abstract --account immshared(11,3) --builtin-signer K1 --ptb-inputs object(1,0)
//> 0: test::green_account::increment(Input(0));

//# abstract --account immshared(11,3) --builtin-signer A --ptb-inputs object(1,0)
//> 0: test::green_account::increment(Input(0));

// === Account 4: rotate_auth(builtin) alone to another scheme locks the account ===

//# init-abstract-account --sender A --package-metadata object(1,6) --inputs pubkey(A) --create-function test::green_account::create --account-type iota::smart_account::SmartAccount

//# abstract --account immshared(15,3) --builtin-signer A --ptb-inputs object(15,3)
//> 0: iota::builtin_authenticator_functions::secp256k1_authenticator_function_ref_v1<iota::smart_account::SmartAccount>();
//> 1: iota::smart_account::rotate_auth_function_ref_v1(Input(0), Result(0));

// Neither the attached key nor a key of the new scheme can sign any more.

//# abstract --account immshared(15,3) --builtin-signer A --ptb-inputs object(1,0)
//> 0: test::green_account::increment(Input(0));

//# abstract --account immshared(15,3) --builtin-signer K1 --ptb-inputs object(1,0)
//> 0: test::green_account::increment(Input(0));

// Three calls went through, all from account 1.

//# view-object 1,0
