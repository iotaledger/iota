// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

// an immutable abstract account can no longer be created, a mutable one still can

//# init --addresses test=0x0 simple_abstract_account=0x0 --accounts A

//# publish-dependencies --paths crates/iota-adapter-transactional-tests/data/account_abstraction/simple_abstract_account.move

//# publish --sender A --dependencies simple_abstract_account
module test::authenticate;

use simple_abstract_account::abstract_account::AbstractAccount;

#[authenticator]
public fun authenticate(_account: &AbstractAccount, _auth_ctx: &AuthContext, _ctx: &TxContext) {}

//# programmable --sender A --inputs object(3,5) "authenticate" "authenticate"
//> 0: simple_abstract_account::abstract_account::create_immutable(Input(0), Input(1), Input(2));

//# programmable --sender A --inputs object(3,5) "authenticate" "authenticate"
//> 0: simple_abstract_account::abstract_account::create(Input(0), Input(1), Input(2));
