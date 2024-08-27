IOTA Indexer is an off-fullnode service to serve data from the IOTA protocol, including both data directly generated from chain and derivative data.

## Architecture

![enhanced_FN](../../docs/content/operator/images/indexer-arch.png)

> [!NOTE]
>
> - Indexer sync workers require the `NodeConfig::enable_experimental_rest_api` flag set to `true` in the node
> - Fullnodes expose both read and write json-rpc APIs. Hence, transactions may be executed only through fullnodes.
> - Validators expose only read-only JSON-RPC APIs.
> - Read-only APIs in the node variants expose the same methods as the indexer read-only APIs with the following difference
>   - Indexer instance exposes the `ExtendedApi`, but nodes do not.

## Database Schema

For more in depth information check the [Database Schema](./schema.md).

## Steps to run an IOTA Indexer locally

### Using docker compose (recommended)

See [pg-services-local](../../docker/pg-services-local/README.md), which automatically sets up the Indexer Sync worker and the Indexer RPC worker along with a postgres database and local network.
Useful local development and testing.

### Using manual setup

1) Install a local [Postgres server](https://www.postgresql.org/download) and start it.

2) Install [Diesel](https://diesel.rs/):
        
 `cargo install diesel_cli --no-default-features --features postgres`

  refer to [Diesel Getting Started guide](https://diesel.rs/guides/getting-started) for more details

3) Setup the database:

Make sure you are in the `iota/crates/iota-indexer` directory and run the following command to setup the database:

```sh
diesel setup --database-url="postgres://postgres:postgres@localhost/iota_indexer"
```

This command will create a database with the name `iota_indexer` for the indexer to work.
Per default, the user and password are `postgres`.

In case the database already exists, you can run the following command to reset the database:

```sh
diesel database reset --database-url="postgres://postgres:postgres@localhost/iota_indexer"
```

4) Run the Indexer together with a local network or standalone:

#### Running the Indexer with a local network, including fullnode, validator, and faucet

Use [iota-test-validator](../../crates/iota-test-validator/README.md) to run the indexer with a local network.

#### Running a standalone indexer with an existing fullnode

You can run the indexer as a writer (Sync worker), which pulls data from the fullnode and writes data to the database or as a reader (RPC worker), which exposes a JSON RPC server with the [interface](https://docs.iota.io/iota-api-ref).

- to run the indexer as a writer:

```sh
# Change the RPC_CLIENT_URL to http://0.0.0.0:9000 to run indexer against local validator & fullnode
cargo run --bin iota-indexer -- --db-url "postgres://postgres:postgres@localhost/iota_indexer" --rpc-client-url "https://fullnode.devnet.iota.io:443" --fullnode-sync-worker --reset-db
```

- to run indexer as a reader:

```
cargo run --bin iota-indexer -- --db-url "postgres://postgres:postgres@localhost/iota_indexer" --rpc-client-url "https://fullnode.devnet.iota.io:443" --rpc-server-worker
```

More flags that can be passed to the commands can be found in this [file](https://github.com/iotaledger/iota/blob/develop/crates/iota-indexer/src/lib.rs).

### DB reset

To wipe the database, make sure you are in the `iota/crates/iota-indexer` directory and run following command. In case of schema changes in `.sql` files, this will also update corresponding `schema.rs` file:

```sh
diesel database reset --database-url="postgres://postgres:postgres@localhost/iota_indexer"
```

### Running tests

The crate provides following tests currently:
- unit tests for DB models (objects, events) which test the conversion between the database representation and the Rust representation of the objects and events.
- unit tests for the DB query filters, which test the conversion of filters to the correct SQL queries.
- integration tests (see [ingestion_tests](tests/ingestion_tests.rs)) to make sure the indexer correctly indexes transaction data from a full node by comparing the data in the database with the data received from the fullnode.
They require a running postgres instance with the database `iota_indexer` and the `pg_integration` feature enabled.

```sh
cargo test --features pg_integration
```
