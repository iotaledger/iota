module conventions::amm {

    use iota::coin::Coin;

    public struct Pool has key {
        id: UID
    }

    // ✅ Correct
    // Return the excess coins even if they have zero value.
    public fun add_liquidity<CoinX, CoinY, LpCoin>(pool: &mut Pool, coin_x: Coin<CoinX>, coin_y: Coin<CoinY>): (Coin<LpCoin>, Coin<CoinX>, Coin<CoinY>) {
        // Implementation omitted.
        abort(0)
    }

    // ✅ Correct
    public fun add_liquidity_and_transfer<CoinX, CoinY, LpCoin>(pool: &mut Pool, coin_x: Coin<CoinX>, coin_y: Coin<CoinY>, recipient: address) {
        let (lp_coin, coin_x, coin_y) = add_liquidity<CoinX, CoinY, LpCoin>(pool, coin_x, coin_y);
        transfer::public_transfer(lp_coin, recipient);
        transfer::public_transfer(coin_x, recipient);
        transfer::public_transfer(coin_y, recipient);
    }

    // ❌ Incorrect
    public fun impure_add_liquidity<CoinX, CoinY, LpCoin>(pool: &mut Pool, coin_x: Coin<CoinX>, coin_y: Coin<CoinY>, ctx: &mut TxContext): Coin<LpCoin> {
        let (lp_coin, coin_x, coin_y) = add_liquidity<CoinX, CoinY, LpCoin>(pool, coin_x, coin_y);
        transfer::public_transfer(coin_x, tx_context::sender(ctx));
        transfer::public_transfer(coin_y, tx_context::sender(ctx));

        lp_coin
    }
}
