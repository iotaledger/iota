module conventions::amm {

    use iota::coin::Coin;

    public struct Pool has key {
        id: UID
    }

    // ✅ Correct
    public fun swap<CoinX, CoinY>(coin_in: Coin<CoinX>): Coin<CoinY> {
        // Implementation omitted.
        abort(0)
    }

    // ❌ Incorrect
    public fun exchange<CoinX, CoinY>(coin_in: &mut Coin<CoinX>, value: u64): Coin<CoinY> {
        // Implementation omitted.
        abort(0)
    }
}
