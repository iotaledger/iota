// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

/// A synthetic fungible token backed by a basket of other tokens.
/// Here, we use a basket that is 1:1 IOTA and MANAGED,
/// but this approach would work for a basket with arbitrary assets/ratios.
/// E.g., [SDR](https://www.imf.org/en/About/Factsheets/Sheets/2016/08/01/14/51/Special-Drawing-Right-SDR)
/// could be implemented this way.
module fungible_tokens::basket {
    use fungible_tokens::managed::MANAGED;
    use iota::coin::{Self, Coin};
    use iota::balance::{Self, Balance, Supply};
    use iota::iota::IOTA;

    /// Name of the coin. By convention, this type has the same name as its parent module
    /// and has no fields. The full type of the coin defined by this module will be `COIN<BASKET>`.
    public struct BASKET has drop { }

    /// Singleton shared object holding the reserve assets and the capability.
    public struct Reserve has key {
        id: UID,
        /// capability allowing the reserve to mint and burn BASKET
        total_supply: Supply<BASKET>,
        /// IOTA coins held in the reserve
        iota: Balance<IOTA>,
        /// MANAGED coins held in the reserve
        managed: Balance<MANAGED>,
    }

    /// Needed to deposit a 1:1 ratio of IOTA and MANAGED for minting, but deposited a different ratio
    const EBadDepositRatio: u64 = 0;

    #[allow(unused_function)]
    fun init(witness: BASKET, ctx: &mut TxContext) {
        // Get a treasury cap for the coin put it in the reserve
        let total_supply = balance::create_supply<BASKET>(witness);

        transfer::share_object(Reserve {
            id: object::new(ctx),
            total_supply,
            iota: balance::zero<IOTA>(),
            managed: balance::zero<MANAGED>(),
        })
    }

    /// === Writes ===

    /// Mint BASKET coins by accepting an equal number of IOTA and MANAGED coins
    public fun mint(
        reserve: &mut Reserve, iota: Coin<IOTA>, managed: Coin<MANAGED>, ctx: &mut TxContext
    ): Coin<BASKET> {
        let num_iota = coin::value(&iota);
        assert!(num_iota == coin::value(&managed), EBadDepositRatio);

        coin::put(&mut reserve.iota, iota);
        coin::put(&mut reserve.managed, managed);

        let minted_balance = balance::increase_supply(&mut reserve.total_supply, num_iota);

        coin::from_balance(minted_balance, ctx)
    }

    /// Burn BASKET coins and return the underlying reserve assets
    public fun burn(
        reserve: &mut Reserve, basket: Coin<BASKET>, ctx: &mut TxContext
    ): (Coin<IOTA>, Coin<MANAGED>) {
        let num_basket = balance::decrease_supply(&mut reserve.total_supply, coin::into_balance(basket));
        let iota = coin::take(&mut reserve.iota, num_basket, ctx);
        let managed = coin::take(&mut reserve.managed, num_basket, ctx);

        (iota, managed)
    }

    // === Reads ===

    /// Return the number of `MANAGED` coins in circulation
    public fun total_supply(reserve: &Reserve): u64 {
        balance::supply_value(&reserve.total_supply)
    }

    /// Return the number of IOTA in the reserve
    public fun iota_supply(reserve: &Reserve): u64 {
        balance::value(&reserve.iota)
    }

    /// Return the number of MANAGED in the reserve
    public fun managed_supply(reserve: &Reserve): u64 {
        balance::value(&reserve.managed)
    }

    #[test_only]
    public fun init_for_testing(ctx: &mut TxContext) {
        init(BASKET {}, ctx)
    }
}
