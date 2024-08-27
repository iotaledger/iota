The iota-test-validator starts a local network that includes a Iota Full node, a Iota validator, a Iota faucet and (optionally)
an indexer.

## Guide

Refer to [iota-local-network.md](../../docs/content/guides/developer/getting-started/local-network.mdx)

## Nightly artifacts

If you don't want to build the `iota-test-validator` yourself, you can download the latest nightly artifacts from the
[nightly build job](https://github.com/iotaledger/iota/actions/workflows/build-nightly.yml) on GitHub. Click on the
latest successful run, scroll down to the bottom and download the artifacts. This contains an `iota` and
`iota-test-validator` binary.

## Experimental Feature - Running with Indexer

**Note** Similar to the fullnode db, all state will be wiped upon restart

1. Follow the [manual setup](../../crates/iota-indexer/README.md) in the `iota-indexer` README to set up the postgresdb on your local machine
2. Make sure the `Posgresdb` starts on your local machine
3. Run `RUST_LOG="consensus=off" ./target/debug/iota-test-validator --with-indexer --pg-password postgres` and change the postgres password if needed.
4. To check your local db, if you use the default db url `postgres://postgres:postgres@localhost:5432/iota_indexer`, you can login to the `postgres` database and run `\dt` to show all tables.

## Run with a persisted state

You can combine this with indexer runs as well to save a persisted state on local development.

1. Generate a config to store db and genesis configs `iota genesis -f --with-faucet --working-dir=[some-directory]`
2. `iota-test-validator --config-dir [some-directory]`
