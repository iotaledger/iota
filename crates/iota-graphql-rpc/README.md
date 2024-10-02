# iota-graphql-rpc

## Architecture

The GraphQL server provides read access to the indexer database, and enables
execution of transaction through the fullnode JSON-RPC API.

Its architecture can thus be visualized as follows:

![GraphQL server architecture](./graphql-rpc-arch.png)

## Dev setup

Note that we use compilation flags to determine the backend for Diesel. If you're using VS Code, make sure to update settings.json with the appropriate features - there should at least be a "pg_backend" (or other backend.)

```
"rust-analyzer.cargo.features": ["pg_backend"]
```

Consequently, you'll also need to specify the backend when running cargo commands:
`cargo run --features "pg_backend" --bin iota-graphql-rpc start-server --db-url <DB_URL>`

The order is important:

1. --features "pg_backend": This part tells Cargo to enable the pg_backend feature.
2. --bin iota-graphql-rpc: This specifies which binary to run.
3. start-server --db-url: These are arguments to the binary.

## Steps to run a local GraphQL server

### Using docker compose (recommended)

See [pg-services-local](../../docker/pg-services-local/README.md), which automatically sets up the GraphQL server along with a postgres database and local network.

### Using manual setup

Before you can run the GraphQL server, you need to have a running postgres instance.
Follow the [Indexer database setup](../iota-indexer/README.md#database-setup) to set up the database.
You should end up with a running postgres instance on port `5432` with the database `iota_indexer` accessible by user `postgres` with password `postgrespw`.

## Launching the graphql-rpc server

See [src/commands.rs](src/commands.rs) for all CLI options.

Example `.toml` config:

```toml
[limits]
max-query-depth = 15
max-query-nodes = 500
max-output-nodes = 100000
max-query-payload-size = 5000
max-db-query-cost = 20000
default-page-size = 5
max-page-size = 10
request-timeout-ms = 15000
max-type-argument-depth = 16
max-type-argument-width = 32
max-type-nodes = 256
max-move-value-depth = 128

[background-tasks]
watermark-update-ms = 500
```

This will build iota-graphql-rpc and start an IDE:

```
cargo run --bin iota-graphql-rpc start-server [--rpc-url] [--db-url] [--port] [--host] [--config]
```

### Launching the server w/ indexer

For local dev, it might be useful to spin up an indexer as well. You can run it as a single service via [pg-services-local](../../docker/pg-services-local/README.md), through the `iota start` subcommand or as a [standalone service](../iota-indexer/README.md#standalone-indexer-setup)

## Compatibility with json-rpc

`cargo run --features indexer --bin iota start --with-indexer --pg-port 5432 --pg-db-name iota_indexer --with-graphql=0.0.0.0:9125`

`pnpm --filter @iota/graphql-transport test:e2e`

## Running tests

To run the tests, a running postgres database is required.
To do so, follow the [Indexer database setup](../iota-indexer/README.md#database-setup) to set up a database.

Then, run the following command:

```sh
cargo nextest run -p iota-graphql-rpc --features pg_integration --no-fail-fast --test-threads 1
```
