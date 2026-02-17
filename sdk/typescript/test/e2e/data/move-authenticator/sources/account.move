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
    transfer::public_share_object(Account {
        id: object::new(ctx),
    });
}

public fun link_auth(account: Account, package: &PackageMetadataV1, module_name: std::ascii::String, function_name: std::ascii::String) {
    let authenticator = authenticator_function::create_auth_function_ref_v1<Account>(package, module_name, function_name);
    account::create_account_v1<Account>(account, authenticator);
}

#[authenticator]
public fun authenticate(
    _account: &Account,
    msg: std::ascii::String,
    _auth_ctx: &iota::auth_context::AuthContext,
    _ctx: &TxContext,
) {
    assert!(msg == std::ascii::string(b"rustisbetterthanjavascript"), 0);
}
