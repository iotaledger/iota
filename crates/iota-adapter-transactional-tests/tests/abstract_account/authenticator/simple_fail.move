// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

// simple authentication fails using abstract account

//# init --addresses test=0x0 aa=0x0 --accounts A

//# publish-deps --paths crates/iota-adapter-transactional-tests/data/account_abstraction/abstract_account.move

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

//# init-abstract-acc --sender A test authenticate authenticate_hello_world aa::abstract_account::AbstractAccount

//# view-object 4,0

//# abstract --account immshared(4,0) --auth-inputs "test" --ptb-inputs 100 @A
//> 0: SplitCoins(Gas, [Input(0)]);
