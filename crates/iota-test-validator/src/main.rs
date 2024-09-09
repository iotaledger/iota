// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

fn main() {
    println!("iota-test-validator binary has been deprecated in favor of iota start, which is a more powerful command that allows you to start the local network with more options.

How to install/build the iota binary IF:
    A: you only need the basic functionality, so just faucet and no persistence (no indexer, no GraphQL service), build from source as usual (cargo build --bin iota) or download latest archive from release archives (starting from testnet v1.28.1 or devnet v1.29) and use iota binary.
    B: you need to also start an indexer (--with-indexer ), or a GraphQL service (--with-graphql), you either:
    - download latest archive from release archives (starting from testnet v1.28.1 or devnet v1.29) and use iota-pg binary.
  OR
    - build from source. This requires passing the indexer feature when building the iota binary, as well as having libpq/postgresql dependencies installed (just as when using iota-test-validator):
        - cargo build --bin iota --features indexer
        - cargo run --features indexer --bin iota -- start --with-faucet --force-regenesis --with-indexer --with-graphql

Running the local network:
 - (Preferred) In the simplest form, you can replace iota-test-validator with iota start --with-faucet --force-regenesis. This will create a network from a new genesis and start a faucet (127.0.0.1:9123). This will not persist state.
 - Use the drop-in replacement script: iota/scripts/iota-test-validator.sh and pass in all the flags/options as you used to.

Use iota start --help to see all the flags and options, such as:
  * --with-indexer --> to start the indexer on the default host and port. Note that this requires \
a Postgres database to be running locally, or you need to set the different options to connect to a \
remote indexer database.
  * --with-graphql --> to start the GraphQL server on the default host and port");
}
