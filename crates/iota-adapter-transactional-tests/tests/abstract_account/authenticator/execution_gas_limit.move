// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

// With only the sponsor's authenticator run before consensus, the sender's
// authenticator of a sponsored transaction runs first at execution. There it
// gets at most `max_auth_gas`, as before consensus, and the transaction body
// gets the rest of the budget.

//# init --addresses test=0x0 simple_abstract_account=0x0 --accounts A --move-auth true

//# publish-dependencies --paths crates/iota-adapter-transactional-tests/data/account_abstraction/simple_abstract_account.move

//# publish --sender A --dependencies simple_abstract_account
module test::authenticate;

use simple_abstract_account::abstract_account::AbstractAccount;

#[authenticator]
public fun authenticate(
    _account: &AbstractAccount,
    iterations: u64,
    _auth_ctx: &AuthContext,
    _ctx: &TxContext,
) {
    spend_gas(iterations);
}

public fun spend_gas(iterations: u64) {
    let mut i = 0;
    while (i < iterations) i = i + 1;
}

//# init-abstract-account --sender A --package-metadata object(3,5) --inputs "authenticate" "authenticate" --create-function simple_abstract_account::abstract_account::create --account-type simple_abstract_account::abstract_account::AbstractAccount

// The authenticator stays within `max_auth_gas` and the body goes beyond it:
// the transaction succeeds.
//# abstract --account immshared(4,2) --sponsor A --auth-inputs 1 --ptb-inputs 30000
//> 0: test::authenticate::spend_gas(Input(0));

//# view-object 0,0

// The authenticator goes beyond `max_auth_gas`: it runs out of gas and the
// body does not run. The sponsor pays for `max_auth_gas` and the input
// reads, not for the whole loop.
//# abstract --account immshared(4,2) --sponsor A --auth-inputs 30000 --ptb-inputs 1
//> 0: test::authenticate::spend_gas(Input(0));

//# view-object 0,0
