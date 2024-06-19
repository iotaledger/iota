# IOTA Test Validator

## Download nightly artifacts

Download the latest nightly artifacts from the
[nightly build job](https://github.com/iotaledger/iota/actions/workflows/build-nightly.yml) on GitHub. Click on the
latest successful run, scroll down to the bottom and download the artifacts. This contains an `iota` and
`iota-test-validator` binary.

## Prepare your system

The iota-indexer uses `postgresql` as its database, so we need to install it.

```sh
sudo apt install postgresql
```

Install the `diesel_cli` which we need to create the database tables for the indexer. You may have to install additional
postgresql-related libraries if the installation fails.

```sh
cargo install diesel_cli --no-default-features --features postgres
```

Create the database tables. Note that `postgres:postgres` is the username and password here. If you changed one of
those, you have to change those parts in the url as well.

```sh
diesel setup --database-url="postgres://postgres:postgres@localhost:5432/iota_indexer"
diesel database reset --database-url="postgres://postgres:postgres@localhost:5432/iota_indexer"
```

Tip: You can run the `reset` command if you want to reset the indexer contents.

Assuming you are in the extracted directory of the nightly binaries, bootstrap a new network in a directory of your
choosing:

```sh
./iota genesis -f --with-faucet --working-dir=/opt/iota-localnet-config
```

Finally, run the `iota-test-validator` with an indexer and the provided postgres password. It will print the URLs at
which its RPC endpoint, faucet or indexer can be reached. Note that you can omit the `--config-dir` argument, in which
case the state of the network will be gone when you quit the validator, which may or may not be desirable.

```sh
RUST_LOG="consensus=off" ./iota-test-validator --with-indexer --pg-password=postgres --config-dir=/opt/iota-localnet-config
```

To confirm it is working, setup an address in your client and request funds.

```sh
./iota client faucet
./iota client balance
```

The last command should print the funds received from the faucet.
