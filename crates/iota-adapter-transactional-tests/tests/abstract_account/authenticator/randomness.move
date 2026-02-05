// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

// simple authentication using abstract account and random object

//# init --addresses test=0x0 simple_abstract_account=0x0 --accounts A

//# publish-dependencies --paths crates/iota-adapter-transactional-tests/data/account_abstraction/simple_abstract_account.move

//# publish --sender A --dependencies simple_abstract_account
module test::authenticate;

use iota::random::Random;
use simple_abstract_account::abstract_account::AbstractAccount;

#[authenticator]
public fun authenticate(
    _account: &AbstractAccount,
    _random: &Random,
    _auth_ctx: &AuthContext,
    _ctx: &TxContext,
) {}
