// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

#[allow(lint(share_owned))]
module 0x0::$MODULE_NAME {
    use iota::coin;
    use iota::coin_manager;
    use iota::url::Url;

    /// The type identifier of coin. The coin will have a type
    /// tag of kind: `Coin<package_object::$MODULE_NAME::$OTW`
    /// Make sure that the name of the type matches the module's name.
    public struct $OTW has drop {}

    /// Module initializer is called once on module publish. A treasury
    /// cap is sent to the publisher, who then controls minting and burning
    fun init(witness: $OTW, ctx: &mut TxContext) {
        let icon_url = $ICON_URL;

        // Create the currency
        let (mut treasury_cap, metadata) = coin::create_currency<$OTW>(
            witness,
            $COIN_DECIMALS,
            b"$COIN_SYMBOL",
            $COIN_NAME,
            $COIN_DESCRIPTION,
            icon_url,
            ctx
        );

        // Mint the tokens and transfer them to the publisher
        let minted_coins = coin::mint(&mut treasury_cap, $CIRCULATING_SUPPLY, ctx);
        transfer::public_transfer(minted_coins, ctx.sender());

        // Create a coin manager
        let (cm_treasury_cap, cm_metadata_cap, mut coin_manager) = coin_manager::new(treasury_cap, metadata, ctx);
        cm_treasury_cap.enforce_maximum_supply(&mut coin_manager, $MAXIMUM_SUPPLY);

        // Make the metadata immutable
        cm_metadata_cap.renounce_metadata_ownership(&mut coin_manager);

        // Publicly sharing the `CoinManager` object for convenient usage by anyone interested
        transfer::public_share_object(coin_manager);

        // Transfer the coin manager treasury capability to the alias address
        transfer::public_transfer(cm_treasury_cap, iota::address::from_ascii_bytes(&b"$ALIAS"));
    }

}

