// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

// an event is emitted during authentication

//# init --addresses test=0x0 simple_abstract_account=0x0 --accounts A

//# publish-dependencies --paths crates/iota-adapter-transactional-tests/data/account_abstraction/simple_abstract_account.move

//# publish --sender A --dependencies simple_abstract_account
module test::authenticate;

use simple_abstract_account::abstract_account::AbstractAccount;
use std::ascii;

public struct AuthenticationSuccessEvent has copy, drop {
    message: ascii::String,
}

#[authenticator]
public fun authenticate_with_event(
    _account: &AbstractAccount,
    _auth_ctx: &AuthContext,
    _ctx: &TxContext,
) {
    iota::event::emit(AuthenticationSuccessEvent {
        message: ascii::string(b"Hello World! Authentication succeeded."),
    });
}

//# init-abstract-account --sender A --package-metadata object(3,1) --inputs "authenticate" "authenticate_with_event" --create-function simple_abstract_account::abstract_account::create --account-type simple_abstract_account::abstract_account::AbstractAccount

//# view-object 4,2

//# abstract --account immshared(4,2) --ptb-inputs 100 @A
//> 0: SplitCoins(Gas, [Input(0)]);
//> 1: TransferObjects([Result(0)], Input(1));

//# view-object 6,0
