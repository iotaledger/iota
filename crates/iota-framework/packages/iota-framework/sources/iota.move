// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

/// Coin<IOTA> is the token used to pay for gas in IOTA.
/// It has 9 decimals, and the smallest unit (10^-9) is called "nano".
module iota::iota {
    use iota::coin::{Self, TreasuryCap};
    use iota::url;

    const EAlreadyMinted: u64 = 0;
    /// Sender is not @0x0 the system address.
    const ENotSystemAddress: u64 = 1;

    #[allow(unused_const)]
    /// The amount of Nanos per IOTA token based on the fact that nano is
    /// 10^-9 of a IOTA token
    const NANO_PER_IOTA: u64 = 1_000_000_000;
    
    #[allow(unused_const)]
    /// The total supply of IOTA denominated in Nano (4.6 Billion * 10^9)
    const TOTAL_SUPPLY_NANO: u64 = 4_600_000_000_000_000_000;

    /// Name of the coin
    public struct IOTA has drop {}

    #[allow(unused_function)]
    /// Register the `IOTA` Coin to acquire its `TreasuryCap`.
    /// This should be called only once during genesis creation.
    fun new(ctx: &mut TxContext): TreasuryCap<IOTA> {
        assert!(ctx.sender() == @0x0, ENotSystemAddress);
        assert!(ctx.epoch() == 0, EAlreadyMinted);

        let (treasury, metadata) = coin::create_currency(
            IOTA {},
            9,
            b"IOTA",
            b"IOTA",
            b"The main (gas)token of the IOTA Network.",
            option::some(url::new_unsafe_from_bytes(b"https://iota.org/logo.png")),
            ctx
        );

        transfer::public_freeze_object(metadata);

        treasury
    }

    public entry fun transfer(c: coin::Coin<IOTA>, recipient: address) {
        transfer::public_transfer(c, recipient)
    }
}
