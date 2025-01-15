// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! This example uses the coin read api to showcase the available functions to
//! retrieve coin related information for a specific address. The example will
//! use the active address in the wallet (if it exists or create one if it
//! doesn't) check if it has coins and request coins from the faucet if there
//! aren't any. If there is no wallet, it will create a wallet and two
//! addresses, set one address as active, and add 1 IOTA to the active address.
//! By default, the example will use the IOTA testnet network (https://api.testnet.iota.cafe).
//!
//! cargo run --example coin_read_api

mod utils;

use futures::{future, stream::StreamExt};
use utils::setup_for_read;

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    let (client, active_address) = setup_for_read().await?;

    // Get coins for this address. Coins can be filtered by `coin_type`
    // (e.g., 0x168da5bf1f48dafc111b0a488fa454aca95e0b5e::usdc::USDC) or
    // use `None` for the default `Coin<IOTA>` which is represented as
    // "0x2::iota::IOTA"
    let coin_type = "0x2::iota::IOTA".to_string();
    let coins = client
        .coin_read_api()
        .get_coins(active_address, coin_type.clone(), None, 5) // get the first five coins
        .await?;
    println!(" *** Coins ***");
    println!("{:?}", coins);
    println!(" *** Coins ***\n");

    // Get all coins
    // This function works very similar to the get_coins function, except it does
    // not take a coin_type filter argument and it returns all coin types
    // associated with this address
    let all_coins = client
        .coin_read_api()
        .get_all_coins(active_address, None, 5) // get the first five coins
        .await?;
    println!(" *** All coins ***");
    println!("{:?}", all_coins);
    println!(" *** All coins ***\n");

    // Get coins as a stream
    // Similar to the previous functions, except it returns the coins as a stream.
    let coins_stream = client
        .coin_read_api()
        .get_coins_stream(active_address, None);

    println!(" *** Coins Stream ***");
    coins_stream
        .for_each(|coin| {
            println!("{:?}", coin);
            future::ready(())
        })
        .await;
    println!(" *** Coins Stream ***\n");

    // Select coins based on the provided coin type (IOTA in this example). Use
    // `None` for the default IOTA coin
    let select_coins = client
        .coin_read_api()
        .select_coins(active_address, coin_type.clone(), 1, vec![])
        .await?;

    println!(" *** Select Coins ***");
    println!("{:?}", select_coins);
    println!(" *** Select Coins ***\n");

    // Balance
    // Returns the balance for the specified coin type for this address,
    // or if None is passed, it will use Coin<IOTA> as the coin type
    let balance = client
        .coin_read_api()
        .get_balance(active_address, None)
        .await?;

    // Total balance
    // Returns the balance for each coin owned by this address
    let total_balance = client
        .coin_read_api()
        .get_all_balances(active_address)
        .await?;
    println!(" *** Balance + Total Balance *** ");
    println!("Balance: {:?}", balance);
    println!("Total Balance: {:?}", total_balance);
    println!(" *** Balance + Total Balance ***\n ");

    // Return the coin metadata for the Coin<IOTA>
    let coin_metadata = client.coin_read_api().get_coin_metadata(&coin_type).await?;

    println!(" *** Coin Metadata *** ");
    println!("{:?}", coin_metadata);
    println!(" *** Coin Metadata ***\n ");

    // Total Supply
    let total_supply = client.coin_read_api().get_total_supply(coin_type).await?;
    println!(" *** Total Supply *** ");
    println!("{:?}", total_supply);
    println!(" *** Total Supply ***\n ");

    Ok(())
}
