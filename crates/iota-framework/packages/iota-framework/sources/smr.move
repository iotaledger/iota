// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

/// Coin<SMR> is the token used for migrated Shimmer users.
/// It has 6 decimals, and the smallest unit (10^-6) is called "glow".
module iota::smr {
    use iota::coin;
    use iota::url;

    const EAlreadyMinted: u64 = 0;
    /// Sender is not @0x0 the system address.
    const ENotSystemAddress: u64 = 1;

    #[allow(unused_const)]
    /// The amount of Glows per Shimmer token based on the fact that micros is
    /// 10^-6 of a Shimmer token
    const GLOW_PER_SMR: u64 = 1_000_000;

    // The total supply of Shimmer denominated in whole Shimmer tokens
    // const TOTAL_SMR_SUPPLY: u64 = 1_813_620_509;

    #[allow(unused_const)]
    /// The total supply of Shimmer denominated in Glows
    const TOTAL_SUPPLY_GLOWS: u64 = 1_813_620_509_061_365;

    /// Name of the coin
    public struct SMR has drop {}

    /// Register the `SHIMMER` coin.
    fun init(witness: SMR, ctx: &mut TxContext) {
        assert!(ctx.sender() == @0x0, ENotSystemAddress);
        assert!(ctx.epoch() == 0, EAlreadyMinted);

        let (treasury, metadata) = coin::create_currency(
                witness,
                6,
                b"Shimmer",
                b"SMR",
                b"The original Shimmer (SMR) token as inherited from the Shimmer Network.",
                option::some(url::new_unsafe_from_bytes(b"https://iota.org/smr-logo.png")),
                ctx
            );
        transfer::public_freeze_object(metadata);
        let supply = treasury.treasury_into_supply();
        supply.destroy_supply();
    }

    public entry fun transfer(c: coin::Coin<SMR>, recipient: address) {
        transfer::public_transfer(c, recipient)
    }
}