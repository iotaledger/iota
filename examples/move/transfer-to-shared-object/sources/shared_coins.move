// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

/// This example demonstrates how to receive objects to a shared object and transfer them again.
module shared_coins::shared_coins {
    use iota::coin::Coin;
    use iota::transfer::{Receiving};
    use iota::iota::IOTA;

    /// Coins, anyone can deposit and transfer.
    public struct SharedCoins has key {
        id: UID,
        coins: vector<Coin<IOTA>>
    }

    /// Create and share a SharedCoins object.
    public fun create(ctx: &mut TxContext) {
        transfer::share_object(SharedCoins {
            id: object::new(ctx),
            coins: vector[],
        })
    }

    /// Delete a SharedCoins object, works only if there are no coins anymore.
    public fun delete(shared_coins: SharedCoins) {
        let SharedCoins {id, coins} = shared_coins;
        id.delete();
        vector::destroy_empty(coins);
    }

    /// Deposit (receive) a coin to the SharedCoins object.
    public fun deposit_coin(
        obj: &mut SharedCoins,
        coin: Receiving<Coin<IOTA>>,
    ) {
        let coin: Coin<IOTA> = transfer::public_receive(&mut obj.id, coin);
        vector::push_back(&mut obj.coins, coin);
    }

    /// Transfer a coin to the sender.
    public fun transfer_coin(
        obj: &mut SharedCoins,
        recipient: address,
    ) {
        let coin = vector::pop_back(&mut obj.coins);
        transfer::public_transfer(coin, recipient)
    }
}

#[test_only]
module shared_coins::shared_coins_test {
    use iota::coin::{Self, Coin};
    use iota::iota::IOTA;
    use iota::test_scenario as ts;
    use shared_coins::shared_coins::{Self, SharedCoins};

    #[test]
    fun test_shared_coins() {
        let user0 = @0xA;

        let mut ts = ts::begin(user0);

        // Create the SharedCoins object.
        {
            ts.next_tx(user0);
            shared_coins::create(ts.ctx());
        };

        // Send the coin to the address of the shared object.
        {
            ts.next_tx(user0);
            let shared_coins: SharedCoins = ts.take_shared();
            let shared_coin_address = object::id_to_address(&object::id(&shared_coins));

            let coin = coin::mint_for_testing<IOTA>(100, ts.ctx());
            transfer::public_transfer(coin, shared_coin_address);
            ts::return_shared(shared_coins);
        };

        // Deposit the Receiving<Coin<IOTA>> to the shared object.
        {
            ts.next_tx(user0);
            let mut shared_coins: SharedCoins = ts.take_shared();
            let shared_coin_address = object::id_to_address(&object::id(&shared_coins));
            std::debug::print(&shared_coin_address);

            let coin = coin::mint_for_testing<IOTA>(100, ts.ctx());
            transfer::public_transfer(coin, shared_coin_address);

            let id_opt: Option<ID> = ts::most_recent_id_for_address<Coin<IOTA>>(shared_coin_address);
            let id = id_opt.destroy_some();
            let receiving_coin = ts::receiving_ticket_by_id<Coin<IOTA>>(id);

            shared_coins::deposit_coin(&mut shared_coins, receiving_coin);

            ts::return_shared(shared_coins);
        };

        // Transfer the coin from the SharedCoins to the sender.
        {
            ts.next_tx(user0);
            let mut shared_coins: SharedCoins = ts.take_shared();

            shared_coins::transfer_coin(&mut shared_coins, tx_context::sender(ts.ctx()));
            ts::return_shared(shared_coins);
        };

        // Delete the SharedCoins object.
        {
            ts.next_tx(user0);
            let shared_coins: SharedCoins = ts.take_shared();

            shared_coins::delete(shared_coins);
        };

        ts.end();
    }
}


/* CLI commands

# Create a new SharedCoins object:
iota client call \
--package "0x9f6a4c3b71ada16ada9acea1cd35cb245caec0eb28a6de86bb8b6bd3e8f62197" \
--module shared_coins \
--function create

# Send a coin to the SharedCoins address:
iota client ptb \
--split-coins gas "[1000000000]" \
--assign new_coins \
--transfer-objects "[new_coins]" @0xb1b2c1f2f2e33943e8fe7954fca9da40b56cd33cf89c00ac1a45f7a1b028ec11 \
--gas-budget 10000000

# Deposit the coin to the SharedCoins object:
COIN_ID=$(iota client gas 0xb1b2c1f2f2e33943e8fe7954fca9da40b56cd33cf89c00ac1a45f7a1b028ec11 --json | jq -r '.[0].gasCoinId')
iota client ptb \
--move-call 0x9f6a4c3b71ada16ada9acea1cd35cb245caec0eb28a6de86bb8b6bd3e8f62197::shared_coins::deposit_coin @0xb1b2c1f2f2e33943e8fe7954fca9da40b56cd33cf89c00ac1a45f7a1b028ec11 @$COIN_ID \
--gas-budget 10000000

# Transfer a coin from the SharedCoins object to the sender:
iota client ptb \
--move-call iota::tx_context::sender \
--assign sender \
--move-call 0x9f6a4c3b71ada16ada9acea1cd35cb245caec0eb28a6de86bb8b6bd3e8f62197::shared_coins::transfer_coin @0xb1b2c1f2f2e33943e8fe7954fca9da40b56cd33cf89c00ac1a45f7a1b028ec11 sender \
--gas-budget 10000000

# Delete an empty SharedCoins object:
iota client ptb \
--move-call 0x9f6a4c3b71ada16ada9acea1cd35cb245caec0eb28a6de86bb8b6bd3e8f62197::shared_coins::delete @0xb1b2c1f2f2e33943e8fe7954fca9da40b56cd33cf89c00ac1a45f7a1b028ec11 \
--gas-budget 10000000

*/
