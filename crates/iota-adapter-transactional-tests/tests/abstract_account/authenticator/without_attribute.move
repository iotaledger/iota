// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

// simple authentication using abstract account without `#[authenticator]` attribute

//# init --addresses test=0x0 aa=0x0 --accounts A

//# publish-dependencies --paths crates/iota-adapter-transactional-tests/data/account_abstraction/abstract_account.move

//# publish --sender A --dependencies aa
module test::authenticate;

use aa::abstract_account::AbstractAccount;
use iota::auth_context::AuthContext;
use std::ascii;

public fun authenticate_hello_world(
    _account: &AbstractAccount,
    msg: ascii::String,
    _auth_ctx: &AuthContext,
    _ctx: &TxContext,
) {
    assert!(msg == ascii::string(b"HelloWorld"), 0);
}

//# init-abstract-account --sender A --package-metadata object(3,1) --inputs "authenticate" "authenticate_hello_world" --aa-create-fn-path aa::abstract_account::create --aa-type AbstractAccount
