// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

// authenticate test for abstract accounts with receiving argument

//# init --addresses test=0x0 --accounts A

//# publish --sender A
module test::abstract_account;

use iota::auth_context::AuthContext;
use iota::coin::Coin;
use iota::iota::IOTA;

public struct AbstractAccount has key {
    id: UID,
}

#[authenticator]
public fun authenticate_receive_coin(
    _account: &AbstractAccount,
    _coin: transfer::Receiving<Coin<IOTA>>,
    _: &AuthContext,
    _ctx: &TxContext,
) {}