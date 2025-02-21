module example::token {
    use iota::coin;
    use iota::coin_manager;

    /// One-Time-Witness of kind: `Coin<package_object::token::TOKEN>`
    public struct TOKEN has drop {}

    #[allow(lint(share_owned))]
    fun init(witness: TOKEN, ctx: &mut TxContext) {
        let (treasurycap, metadata) = coin::create_currency(
            witness,
            6,                  // decimals
            b"EXAMPLE",         // symbol
            b"Example Coin",     // name
            b"Just an example",  // description
            option::none(),     // icon URL
            ctx
        );
    
        // Creating the Manager, transferring ownership of the `TreasuryCap` to it
        let (newtreasurycap, metacap, mut manager) = coin_manager::new(treasurycap, metadata, ctx);

        // Limiting the maximum supply to `100`
        newtreasurycap.enforce_maximum_supply(&mut manager, 100);

        // Returning a new `CoinManagerTreasuryCap` to the creator of the `Coin`
        transfer::public_transfer(newtreasurycap, ctx.sender());

        // Returning a new `CoinManagerMetadataCap` to the creator of the `Coin`
        transfer::public_transfer(metacap, ctx.sender());

        // Publicly sharing the `CoinManager` object for convenient usage by anyone interested
        transfer::public_share_object(manager);
    }
}