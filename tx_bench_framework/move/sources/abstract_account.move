// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

module abstract_account_with_pub_key::abstract_account;

use iota::account;
use iota::auth_context::AuthContext;
use iota::dynamic_field;
use iota::ed25519;
use iota::package_metadata::PackageMetadataV1;
use std::ascii;
use iota::hex::decode;

public struct AbstractAccount has key {
    id: UID,
}

public struct OwnerPublicKey has copy, drop, store {}

public fun create(
    package_metadata: &PackageMetadataV1,
    module_name: ascii::String,
    function_name: ascii::String,
    public_key: vector<u8>,
    ctx: &mut TxContext,
): address {
    let authenticator = account::create_auth_info_v1<AbstractAccount>(
        package_metadata,
        module_name,
        function_name,
    );

    let mut account = AbstractAccount { id: object::new(ctx) };

    dynamic_field::add(&mut account.id, OwnerPublicKey {}, public_key);

    let account_address = object::id_address(&account);

    account::create_account_v1(account, authenticator);

    account_address
}

public fun borrow_public_key(account: &AbstractAccount): &vector<u8> {
    dynamic_field::borrow(&account.id, OwnerPublicKey {})
}

/// Ed25519 signature authenticator.
#[authenticator]
public fun authenticate_ed25519(
    account: &AbstractAccount,
    signature: vector<u8>,
    _: &AuthContext,
    ctx: &TxContext,
) {
    // Check the signature.
    assert!(
        ed25519::ed25519_verify(
            &decode(signature),
            account.borrow_public_key(),
            ctx.digest(),
        ),
        0,
    );
}

/// Ed25519 signature authenticator.
#[authenticator]
public fun authenticate_ed25519_heavy(
    account: &AbstractAccount,
    signature: vector<u8>,
    _: &AuthContext,
    ctx: &TxContext,
) {
    let mut i = 0;
    while (i < 25000) {
        i = i + 1;
    };
    // Check the signature.
    assert!(
        ed25519::ed25519_verify(
            &decode(signature),
            account.borrow_public_key(),
            ctx.digest(),
        ),
        0,
    );
}

#[authenticator]
public fun authenticate_hello_world(
    _account: &AbstractAccount,
    msg: ascii::String,
    _auth_ctx: &AuthContext,
    _ctx: &TxContext,
) {
    assert!(msg == ascii::string(b"HelloWorld"), 0);
}
