// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

// simple authentication using abstract account and random object

//# init --addresses test=0x0 simple_abstract_account=0x0 --accounts A

//# publish-dependencies --paths crates/iota-adapter-transactional-tests/data/account_abstraction/simple_abstract_account.move

//# publish --sender A --dependencies simple_abstract_account
module test::randomness_attack;

use iota::random::Random;
use simple_abstract_account::abstract_account::AbstractAccount;

public fun attack(_r: &Random) {}

#[authenticator]
public fun authenticate_random(
    _account: &AbstractAccount,
    _r: &Random,
    _auth_ctx: &AuthContext,
    _ctx: &TxContext,
) {}

//# init-abstract-account --sender A --package-metadata object(3,0) --inputs "randomness_attack" "authenticate_random" --create-function simple_abstract_account::abstract_account::create --account-type simple_abstract_account::abstract_account::AbstractAccount

//# view-object 4,2

//# abstract --account immshared(4,2) --auth-inputs immshared(8) --ptb-inputs immshared(8)
//> test::randomness_attack::attack(Input(0));
