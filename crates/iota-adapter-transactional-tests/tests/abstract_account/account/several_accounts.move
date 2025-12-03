// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

// simple authentication using abstract account

//# init --addresses test=0x0 --accounts A --default-aa

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

//# init-abstract-acc --sender A test authenticate authenticate_hello_world

//# init-abstract-acc --sender A test authenticate authenticate_hello_world

//# init-abstract-acc --sender A test authenticate authenticate_hello_world

//# view-object 2,0

//# view-object 2,1

//# view-object 3,0

//# view-object 3,1

//# view-object 4,0

//# view-object 4,1