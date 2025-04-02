# Mainnet Unlocks Store

This repository contains a module that allows you to query Mainnet unlock data over time.
It reads the aggregated unlock data from the crate root (`data/aggregated_mainnet_unlocks.csv`) and maintains an in-memory map of `DateTime` instances to the amount of tokens still locked at each point.

## Regenerate Unlock Data

The provided binary `bin/generate_aggregated_data.rs` can be used to regenerate the aggregated unlock data from the [IOTA Foundation's Mainnet Unlocks GitHub repository](https://github.com/iotaledger/new_supply.git).

First, make sure you have `git` installed on your system.
To regenerate the data, run the following command:

```sh
cargo run --bin generate_aggregated_data
```

## Running Tests

```sh
cargo test
```
