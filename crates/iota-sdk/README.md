This crate provides the IOTA Rust SDK, containing APIs to interact with the IOTA network.

## Getting started

Add the `iota-sdk` dependency as following:

```toml
iota-sdk = { git = "https://github.com/iotaledger/iota", package = "iota-sdk" }
tokio = { version = "1.2", features = ["full"] }
anyhow = "1.0"
```

The main building block for the IOTA Rust SDK is the `IotaClientBuilder`, which provides a simple and straightforward way of connecting to a Iota network and having access to the different available APIs.

In the following example, the application connects to the IOTA `testnet` and `devnet` networks and prints out their respective RPC API versions.

```rust
use iota_sdk::IotaClientBuilder;

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    // Iota testnet -- https://fullnode.testnet.iota.io:443
    let iota_testnet = IotaClientBuilder::default().build_testnet().await?;
    println!("Iota testnet version: {}", iota_testnet.api_version());

     // Iota devnet -- https://fullnode.devnet.iota.io:443
    let iota_devnet = IotaClientBuilder::default().build_devnet().await?;
    println!("Iota devnet version: {}", iota_devnet.api_version());

    Ok(())
}
```

## Documentation for iota-sdk crate

[GitHub Pages](https://iotaledger.github.io/iota/iota_sdk/index.html) hosts the generated documentation for all Rust crates in the IOTA repository.

### Building documentation locally

You can also build the documentation locally. To do so,

1. Clone the `iota` repo locally. Open a Terminal or Console and go to the `iota/crates/iota-sdk` directory.

1. Run `cargo doc` to build the documentation into the `iota/target` directory. Take note of location of the generated file from the last line of the output, for example `Generated /Users/foo/iota/target/doc/iota_sdk/index.html`.

1. Use a web browser, like Chrome, to open the `.../target/doc/iota_sdk/index.html` file at the location your console reported in the previous step.

## Rust SDK examples

The [examples](https://github.com/iotaledger/iota/tree/develop/crates/iota-sdk/examples) folder provides both basic and advanced examples.

There are several files ending in `_api.rs` which provide code examples of the corresponding APIs and their methods. These showcase how to use the IOTA Rust SDK, and can be run against the IOTA testnet. Below are instructions on the prerequisites and how to run these examples.

### Prerequisites

Unless otherwise specified, most of these examples assume `Rust` and `cargo` are installed, and that there is an available internet connection. The examples connect to the IOTA testnet (`https://fullnode.testnet.iota.io:443`) and execute different APIs using the active address from the local wallet. If there is no local wallet, it will create one, generate two addresses, set one of them to be active, and it will request 1 IOTA from the testnet faucet for the active address.

### Running the existing examples

In the root folder of the `iota` repository (or in the `iota-sdk` crate folder), you can individually run examples using the command `cargo run --example filename` (without `.rs` extension). For example:

- `cargo run --example iota_client` -- this one requires a local Iota network running (see [#Connecting to Iota Network](https://wiki.iota.cafe/developer/getting-started/local-network#start-the-local-network). If you do not have a local Iota network running, please skip this example.
- `cargo run --example coin_read_api`
- `cargo run --example event_api` -- note that this will subscribe to a stream and thus the program will not terminate unless forced (Ctrl+C)
- `cargo run --example governance_api`
- `cargo run --example read_api`
- `cargo run --example programmable_transactions_api`
- `cargo run --example sign_tx_guide`

### Basic Examples

#### Connecting to IOTA Network

The `IotaClientBuilder` struct provides a connection to the JSON-RPC server that you use for all read-only operations. The default URLs to connect to the IOTA network are:

- Local: http://127.0.0.1:9000
- Devnet: https://fullnode.devnet.iota.io:443
- Testnet: https://fullnode.testnet.iota.io:443
- Mainnet: https://fullnode.mainnet.iota.io:443

For all available servers, see [here](TODO: https://github.com/iotaledger/iota/issues/1614).

For running a local IOTA network, please follow [this guide](https://wiki.iota.cafe/developer/getting-started/iota-install) for installing IOTA and [this guide](https://wiki.iota.cafe/developer/getting-started/local-network#start-the-local-network) for starting the local IOTA network.

```rust
use iota_sdk::IotaClientBuilder;

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    let iota = IotaClientBuilder::default()
        .build("http://127.0.0.1:9000") // local network address
        .await?;
    println!("IOTA local network version: {}", iota.api_version());

    // local IOTA network, like the above one but using the dedicated function
    let iota_local = IotaClientBuilder::default().build_localnet().await?;
    println!("IOTA local network version: {}", iota_local.api_version());

    // IOTA devnet -- https://fullnode.devnet.iota.io:443
    let iota_devnet = IotaClientBuilder::default().build_devnet().await?;
    println!("IOTA devnet version: {}", iota_devnet.api_version());

    // IOTA testnet -- https://fullnode.testnet.iota.io:443
    let iota_testnet = IotaClientBuilder::default().build_testnet().await?;
    println!("IOTA testnet version: {}", iota_testnet.api_version());

    Ok(())
}
```

#### Read the total coin balance for each coin type owned by this address

```rust
use std::str::FromStr;
use iota_sdk::types::base_types::IotaAddress;
use iota_sdk::{ IotaClientBuilder};
#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {

   let iota_local = IotaClientBuilder::default().build_localnet().await?;
   println!("IOTA local network version: {}", iota_local.api_version());

   let active_address = IotaAddress::from_str("<YOUR IOTA ADDRESS>")?; // change to your IOTA address

   let total_balance = iota_local
      .coin_read_api()
      .get_all_balances(active_address)
      .await?;
   println!("The balances for all coins owned by address: {active_address} are {total_balance:?}");
   Ok(())
}
```

## Advanced examples

See the programmable transactions [example](https://github.com/iotaledger/iota/blob/develop/crates/iota-sdk/examples/programmable_transactions_api.rs).

## Games examples

### Tic Tac Toe quick start

1. Prepare the environment

   1. Install `iota` binary following the [IOTA installation](https://wiki.iota.cafe/developer/getting-started/iota-install) docs.
   1. [Connect to IOTA Devnet](https://wiki.iota.cafe/developer/getting-started/connect).
   1. [Make sure you have two addresses with gas](https://wiki.iota.cafe/developer/getting-started/get-address) by using the `new-address` command to create new addresses:
      ```shell
      iota client new-address ed25519
      ```
      You must specify the key scheme, one of `ed25519` or `secp256k1` or `secp256r1`.
      You can skip this step if you are going to play with a friend. :)
   1. [Request IOTA tokens](https://wiki.iota.cafe/developer/getting-started/get-coins) for all addresses that will be used to join the game.

2. Publish the move contract

   1. Download the IOTA source code:
      ```shell
      git clone https://github.com/iotaledger/iota
      ```
   1. Publish the [`games` package(iota/iota_programmability/examples/games)](https://github.com/iotaledger/iota/tree/develop/iota_programmability/examples/games)
      using the IOTA client:
      ```shell
      iota client publish --path /path-to-iota-source-code/iota_programmability/examples/games --gas-budget 10000
      ```
   1. Record the package object ID.

3. Create a new tic-tac-toe game

   1. Run the following command in the IOTA source code directory to start a new game, replacing the game package objects ID with the one you recorded:
      ```shell
      cargo run --example tic-tac-toe -- --game-package-id <<games package object ID>> new-game
      ```
      This will create a game for the first two addresses in your keystore by default. If you want to specify the identity of each player,
      use the following command and replace the variables with the actual player's addresses:
      ```shell
      cargo run --example tic-tac-toe -- --game-package-id <<games package object ID>> new-game --player-x <<player X address>> --player-o <<player O address>>
      ```
   1. Copy the game ID and pass it to your friend to join the game.

4. Joining the game

   Run the following command in the IOTA source code directory to join the game, replacing the game ID and address accordingly:

   ```shell
   cargo run --example tic-tac-toe -- --game-package-id <<games package object ID>> join-game --my-identity <<address>> --game-id <<game ID>>
   ```

## License

[SPDX-License-Identifier: Apache-2.0](https://github.com/iotaledger/iota/blob/develop/LICENSE)
