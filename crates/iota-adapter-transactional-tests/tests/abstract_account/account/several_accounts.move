// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

// simple authentication using abstract account

//# init --addresses test=0x0 simple_abstract_account=0x0 --accounts A

//# publish-dependencies --paths crates/iota-adapter-transactional-tests/data/account_abstraction/simple_abstract_account.move

//# publish --sender A --dependencies simple_abstract_account
module test::authenticate;

use simple_abstract_account::abstract_account::AbstractAccount;
use std::ascii;

#[authenticator]
public fun authenticate_hello_world(
    _account: &AbstractAccount,
    msg: ascii::String,
    _auth_ctx: &AuthContext,
    _ctx: &TxContext,
) {
    assert!(msg == ascii::string(b"HelloWorld"), 0);
}

//# init-abstract-account --sender A --package-metadata object(3,1) --inputs "authenticate" "authenticate_hello_world" --create-function simple_abstract_account::abstract_account::create --account-type simple_abstract_account::abstract_account::AbstractAccount

//# init-abstract-account --sender A --package-metadata object(3,1) --inputs "authenticate" "authenticate_hello_world" --create-function simple_abstract_account::abstract_account::create --account-type simple_abstract_account::abstract_account::AbstractAccount

//# init-abstract-account --sender A --package-metadata object(3,1) --inputs "authenticate" "authenticate_hello_world" --create-function simple_abstract_account::abstract_account::create --account-type simple_abstract_account::abstract_account::AbstractAccount

//# view-object 4,1

//# view-object 4,2

//# view-object 5,1

//# view-object 5,2

//# view-object 6,1

//# view-object 6,2
