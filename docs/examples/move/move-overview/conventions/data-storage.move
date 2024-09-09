
module conventions::vesting_wallet {

    use iota::iota::IOTA;
    use iota::coin::Coin;
    use iota::table::Table;
    use iota::balance::Balance;

    public struct OwnedWallet has key {
        id: UID,
        balance: Balance<IOTA>
    }

    public struct SharedWallet has key {
        id: UID,
        balance: Balance<IOTA>,
        accounts: Table<address, u64>
    }

    /*
    * A vesting wallet releases a certain amount of coin over a period of time.
    * If the entire balance belongs to one user and the wallet has no additional functionalities, it is best to store it in an owned object.
    */
    public fun new(deposit: Coin<IOTA>, ctx: &mut TxContext): OwnedWallet {
        // Implementation omitted.
        abort(0)
    }

    /*
    * If you wish to add extra functionality to a vesting wallet, it is best to share the object.
    * For example, if you wish the issuer of the wallet to be able to cancel the contract in the future.
    */
    public fun new_shared(deposit: Coin<IOTA>, ctx: &mut TxContext) {
        // Implementation omitted.
        // It shares the `SharedWallet`.
        abort(0)
    }
}
