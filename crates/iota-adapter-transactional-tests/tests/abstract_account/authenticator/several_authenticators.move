// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

// several authenticators in 1 module

//# init --addresses test=0x0 aa=0x0 --accounts A

//# publish-dependencies --paths crates/iota-adapter-transactional-tests/data/account_abstraction/abstract_account.move

//# publish --sender A --dependencies aa
module test::authenticate;

use aa::abstract_account::AbstractAccount;
use iota::auth_context::AuthContext;
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

#[authenticator]
public fun authenticate_hello_world2(
    _account: &AbstractAccount,
    msg: ascii::String,
    _auth_ctx: &AuthContext,
    _ctx: &TxContext,
) {
    assert!(msg == ascii::string(b"HelloWorld"), 0);
}

//# programmable --sender A --inputs object(3,1) "authenticate" "authenticate_hello_world"
//> 0: aa::abstract_account::create(Input(0), Input(1), Input(2));

//# view-object 4,0

//# abstract --account immshared(4,0) --auth-inputs "HelloWorld" --ptb-inputs 100 @A
//> 0: SplitCoins(Gas, [Input(0)]);
//> 1: TransferObjects([Result(0)], Input(1));

//# view-object 6,0
