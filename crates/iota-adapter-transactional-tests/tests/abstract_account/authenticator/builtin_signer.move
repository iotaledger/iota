// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

// Accounts backed by the built-in authenticators, sending transactions signed
// with `--builtin-signer`: by a Secp256k1 signer, an Ed25519 signer and a test
// account. Each call signed by the account's own key succeeds; a call signed
// by another key is rejected.

//# init --addresses test=0x0 --accounts A --signers K1=secp256k1 K2=ed25519

//# publish --sender A
module test::builtin_account;

use iota::account;
use iota::builtin_authenticator_functions;
use iota::package_metadata::PackageMetadataV1;
use iota::public_key;

public struct BuiltinAccount has key {
    id: UID,
}

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

/// Never used by the accounts. `init-abstract-account` passes the package
/// metadata to `create`, and a package has metadata only if it defines an
/// authenticator.
#[authenticator]
public fun unused_authenticator(
    _account: &BuiltinAccount,
    _auth_ctx: &AuthContext,
    _ctx: &TxContext,
) {}

/// Creates a shared account with `prefixed_public_key` attached and the
/// built-in authenticator of its scheme.
public fun create(
    _package_metadata: &PackageMetadataV1,
    prefixed_public_key: vector<u8>,
    ctx: &mut TxContext,
): address {
    let public_key = public_key::from_prefixed_bytes(prefixed_public_key);
    let mut account = BuiltinAccount { id: object::new(ctx) };
    builtin_authenticator_functions::attach_public_key(&mut account.id, public_key);
    let account_address = object::id_address(&account);
    account::create_account_v1(
        account,
        builtin_authenticator_functions::from_signature_scheme(public_key.scheme()),
    );
    account_address
}

// A Secp256k1 signer.

//# init-abstract-account --sender A --package-metadata object(1,6) --inputs pubkey(K1) --create-function test::builtin_account::create --account-type test::builtin_account::BuiltinAccount

//# abstract --account immshared(2,3) --builtin-signer K1 --ptb-inputs object(1,0)
//> 0: test::builtin_account::increment(Input(0));

//# abstract --account immshared(2,3) --builtin-signer K2 --ptb-inputs object(1,0)
//> 0: test::builtin_account::increment(Input(0));

// An Ed25519 signer.

//# init-abstract-account --sender A --package-metadata object(1,6) --inputs pubkey(K2) --create-function test::builtin_account::create --account-type test::builtin_account::BuiltinAccount

//# abstract --account immshared(5,3) --builtin-signer K2 --ptb-inputs object(1,0)
//> 0: test::builtin_account::increment(Input(0));

//# abstract --account immshared(5,3) --builtin-signer A --ptb-inputs object(1,0)
//> 0: test::builtin_account::increment(Input(0));

// A test account's key.

//# init-abstract-account --sender A --package-metadata object(1,6) --inputs pubkey(A) --create-function test::builtin_account::create --account-type test::builtin_account::BuiltinAccount

//# abstract --account immshared(8,3) --builtin-signer A --ptb-inputs object(1,0)
//> 0: test::builtin_account::increment(Input(0));

//# abstract --account immshared(8,3) --builtin-signer K2 --ptb-inputs object(1,0)
//> 0: test::builtin_account::increment(Input(0));

// Three calls went through.

//# view-object 1,0
