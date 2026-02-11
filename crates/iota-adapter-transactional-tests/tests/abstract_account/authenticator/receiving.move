// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

// authenticate test for abstract accounts with receiving argument

//# init --addresses test=0x0 simple_abstract_account=0x0 --accounts A

//# publish-dependencies --paths crates/iota-adapter-transactional-tests/data/account_abstraction/simple_abstract_account.move

//# publish --sender A --dependencies simple_abstract_account
module test::authenticate;

use iota::coin::Coin;
use iota::iota::IOTA;
use simple_abstract_account::abstract_account::AbstractAccount;

#[authenticator]
public fun authenticate_receive_coin(
    _account: &AbstractAccount,
    _coin: transfer::Receiving<Coin<IOTA>>,
    _: &AuthContext,
    _ctx: &TxContext,
) {}
