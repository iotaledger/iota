// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

/// Module: test_coin
module test_coin::test_coin {

    use iota::coin;

    public struct TEST_COIN has drop {}

    fun init(witness: TEST_COIN, ctx: &mut TxContext) {
        let (treasury, metadata) = coin::create_currency(
            witness,
            6,
            b"TEST_COIN",
            b"",
            b"",
            option::none(),
            ctx
        );
        transfer::public_freeze_object(metadata);
        transfer::public_transfer(treasury, ctx.sender())
    }
}
