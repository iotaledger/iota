// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

// simple authentication using mutable abstract account

//# init --addresses test=0x0 aa=0x0 --accounts A

//# publish-deps --paths crates/iota-adapter-transactional-tests/data/account_abstraction/abstract_account.move

//# publish --sender A --dependencies aa
module test::authenticate;

use aa::abstract_account::AbstractAccount;
use iota::auth_context::AuthContext;

#[authenticator]
public fun authenticate(_account: &AbstractAccount, _auth_ctx: &AuthContext, _ctx: &TxContext) {}

//# init-abstract-acc --sender A --package-metadata object(3,1) authenticate authenticate aa::abstract_account::AbstractAccount

//# view-object 4,0

//# abstract --account object(4,0) --ptb-inputs 100 @A
//> 0: SplitCoins(Gas, [Input(0)]);
