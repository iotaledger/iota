// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

// authenticate test for abstract accounts with receiving argument

//# init --addresses test=0x0 aa=0x0 --accounts A

//# publish-deps --paths crates/iota-adapter-transactional-tests/data/account_abstraction/abstract_account.move

//# publish --sender A --dependencies aa
module test::authenticate;

use aa::abstract_account::AbstractAccount;
use iota::auth_context::AuthContext;
use iota::coin::Coin;
use iota::iota::IOTA;


#[authenticator]
public fun authenticate_receive_coin(
    _account: &AbstractAccount,
    _coin: transfer::Receiving<Coin<IOTA>>,
    _: &AuthContext,
    _ctx: &TxContext,
) {}