// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

module basics::bee_hive {
    /// A shared counter.
    public struct Bee has key {
        id: UID,
    }

    public struct Hive has key {
        id: UID,
        bees_enraged: u64,
    }

    public fun create_public_hive(ctx: &mut TxContext) {
        transfer::share_object(Hive {
            id: object::new(ctx),
            bees_enraged: 0,
        })
    }

    public fun create_private_hive(ctx: &mut TxContext) {
        transfer::transfer(Hive {
            id: object::new(ctx),
            bees_enraged: 0,
        }, ctx.sender())
    }

    /// Increment a counter by 1.
    public fun hit_the_hive(hive: &mut Hive, how_hard: u64, ctx: &mut TxContext) {
        let mut bees_produced = 0;
        while (bees_produced < how_hard) {
            transfer::share_object(Bee {
                id: object::new(ctx),
            });
            bees_produced = bees_produced + 1;
        };
        hive.bees_enraged = hive.bees_enraged + how_hard;
    }
}
