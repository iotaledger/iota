// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

/// Coin<IOTA> is the token used to pay for gas in Iota.
/// It has 9 decimals, and the smallest unit (10^-9) is called "nanos".
module iota::iota {
    use iota::balance::Balance;
    use iota::coin;

    const EAlreadyMinted: u64 = 0;
    /// Sender is not @0x0 the system address.
    const ENotSystemAddress: u64 = 1;

    #[allow(unused_const)]
    /// The amount of Nanos per Iota token based on the fact that nanos is
    /// 10^-9 of a Iota token
    const NANOS_PER_IOTA: u64 = 1_000_000_000;

    #[allow(unused_const)]
    /// The total supply of Iota denominated in whole Iota tokens (10 Billion)
    const TOTAL_SUPPLY_IOTA: u64 = 10_000_000_000;

    /// The total supply of Iota denominated in Nanos (10 Billion * 10^9)
    const TOTAL_SUPPLY_NANOS: u64 = 10_000_000_000_000_000_000;

    /// Name of the coin
    public struct IOTA has drop {}

    #[allow(unused_function)]
    /// Register the `IOTA` Coin to acquire its `Supply`.
    /// This should be called only once during genesis creation.
    fun new(ctx: &mut TxContext): Balance<IOTA> {
        assert!(ctx.sender() == @0x0, ENotSystemAddress);
        assert!(ctx.epoch() == 0, EAlreadyMinted);

        let (treasury, metadata) = coin::create_currency(
            IOTA {},
            9,
            b"IOTA",
            b"Iota",
            // TODO: add appropriate description and logo url
            b"",
            option::none(),
            ctx
        );
        transfer::public_freeze_object(metadata);
        let mut supply = treasury.treasury_into_supply();
        let total_iota = supply.increase_supply(TOTAL_SUPPLY_NANOS);
        supply.destroy_supply();
        total_iota
    }

    public entry fun transfer(c: coin::Coin<IOTA>, recipient: address) {
        transfer::public_transfer(c, recipient)
    }
}
