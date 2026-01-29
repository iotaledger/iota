// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

// rotate the authenticator using an immutable abstract account

//# init --addresses test=0x0 simple_abstract_account=0x0 --accounts A

//# publish-dependencies --paths crates/iota-adapter-transactional-tests/data/account_abstraction/simple_abstract_account.move

//# publish --sender A --dependencies simple_abstract_account
module test::authenticate;

use simple_abstract_account::abstract_account::AbstractAccount;

#[authenticator]
public fun authenticate1(_account: &AbstractAccount, _auth_ctx: &AuthContext, _ctx: &TxContext) {}

#[authenticator]
public fun authenticate2(_account: &AbstractAccount, _auth_ctx: &AuthContext, _ctx: &TxContext) {}

//# init-abstract-account --sender A --package-metadata object(3,1) --inputs "authenticate" "authenticate1" --create-function simple_abstract_account::abstract_account::create --account-type simple_abstract_account::abstract_account::AbstractAccount

//# view-object 4,2

//# abstract --account immshared(4,2) --ptb-inputs object(4,2) object(3,1) "authenticate" "authenticate2"
//> 0: simple_abstract_account::abstract_account::rotate_auth_function_ref(Input(0), Input(1), Input(2), Input(3))
