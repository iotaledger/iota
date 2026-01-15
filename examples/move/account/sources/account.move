// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

module account::account;

use iota::package_metadata::PackageMetadataV1;
use iota::account;
use iota::authenticator_function;

public struct Account has key, store {
    id: UID,
}

public struct ACCOUNT has drop {}

fun init(_otw: ACCOUNT, ctx: &mut TxContext) {
    // Shares the account object, anyone can claim it by calling the link_auth function
    transfer::public_share_object(Account {
        id: object::new(ctx),
    });
}

public fun link_auth(account: Account, package: &PackageMetadataV1, module_name: std::ascii::String, function_name: std::ascii::String) {
    let authenticator = authenticator_function::create_auth_function_ref_v1<Account>(package, module_name, function_name);
    account::create_account_v1<Account>(account, authenticator);
}

/// An unsecure example authenticator function that checks if the provided message is "hello".
#[authenticator]
public fun authenticate(
    _account: &Account,
    msg: std::ascii::String,
    // Could also accept shared objects
    // clock: &iota::clock::Clock,
    // Could also accept immutable objects
    // Freeze an empty gas coin for testing:
    // iota client ptb \
    // --split-coins gas "[0]" \
    // --assign coin \
    // --move-call iota::transfer::public_freeze_object "<iota::coin::Coin<iota::iota::IOTA>>" coin
    // coin: &iota::coin::Coin<iota::iota::IOTA>,
    _auth_ctx: &iota::auth_context::AuthContext,
    _ctx: &TxContext,
) {
    assert!(msg == std::ascii::string(b"hello"), 0);
}
