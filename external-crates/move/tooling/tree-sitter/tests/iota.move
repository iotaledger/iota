// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

/// Coin<IOTA> is the token used to pay for gas in IOTA.
/// It has 9 decimals, and the smallest unit (10^-9) is called "nanos".
module iota::iota {
    use std::option;
    use iota::tx_context::{Self, TxContext};
    use iota::balance::{Self, Balance};
    use iota::transfer;
    use iota::coin;

    const EAlreadyMinted: u64 = 0;
    /// Sender is not @0x0 the system address.
    const ENotSystemAddress: u64 = 1;

    #[allow(unused_const)]
    /// The amount of Nanos per IOTA token based on the the fact that nanos is
    /// 10^-9 of a IOTA token
    const NANOS_PER_IOTA: u64 = 1_000_000_000;

    #[allow(unused_const)]
    /// The total supply of IOTA denominated in whole IOTA tokens (10 Billion)
    const TOTAL_SUPPLY_IOTA: u64 = 10_000_000_000;

    /// The total supply of IOTA denominated in Nanos (10 Billion * 10^9)
    const TOTAL_SUPPLY_NANOS: u64 = 10_000_000_000_000_000_000;

    /// Name of the coin
    struct IOTA has drop {}

    #[allow(unused_function)]
    /// Register the `IOTA` Coin to acquire its `Supply`.
    /// This should be called only once during genesis creation.
    fun new(ctx: &mut TxContext): Balance<IOTA> {
        assert!(tx_context::sender(ctx) == @0x0, ENotSystemAddress);
        assert!(tx_context::epoch(ctx) == 0, EAlreadyMinted);

        let (treasury, metadata) = coin::create_currency(
            IOTA {},
            9,
            b"IOTA",
            b"IOTA",
            // TODO: add appropriate description and logo url
            b"",
            option::none(),
            ctx
        );
        transfer::public_freeze_object(metadata);
        let supply = coin::treasury_into_supply(treasury);
        let total_iota = balance::increase_supply(&mut supply, TOTAL_SUPPLY_NANOS);
        balance::destroy_supply(supply);
        total_iota
    }

    public entry fun transfer(c: coin::Coin<IOTA>, recipient: address) {
        transfer::public_transfer(c, recipient)
    }
}
