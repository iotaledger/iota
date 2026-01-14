IOTA Indexer is an off-fullnode service to serve data from the IOTA protocol, including both data directly generated from chain and derivative data.

## Architecture

![enhanced_FN](../../docs/site/static/img/operator/indexer-functions/indexer-arch.png)

> [!NOTE]
>
> - Indexer sync workers require the `NodeConfig::enable_rest_api` flag set to `true` in the node
> - Fullnodes expose read and transaction execution JSON-RPC APIs. Hence, transactions can be executed through fullnodes.
> - Validators expose only read-only JSON-RPC APIs.
> - Indexer instances expose read, write and extended JSON-RPC APIs.

## Database Schema

For more in depth information check the [Database Schema](./schema.md).

## Steps to run an Indexer locally

### Using docker compose (recommended)

See [pg-services-local](../../dev-tools/pg-services-local/README.md), which automatically sets up the Indexer Sync worker and the Indexer RPC worker along with a postgres database and local network.

### Using manual setup

To run an Indexer, a running postgres instance is required.

#### Database setup

You can either spin up the postgres instance as a single service via [docker-compose](../../dev-tools/pg-services-local/README.md) or manually set up it up.
If you choose for manual setup, follow the steps below:

1. Install a local [Postgres server](https://www.postgresql.org/download) and start it.

2. Install [Diesel](https://diesel.rs/):

`cargo install diesel_cli --no-default-features --features postgres`

3. Make sure you are in the `iota/crates/iota-indexer` directory and run the following command to create the database:

```sh
diesel setup --database-url="postgres://postgres:postgrespw@localhost/iota_indexer"
```

This command will create a database with the name `iota_indexer` to store the indexed data.
Per default, the user is `postgres` and the password is `postgrespw`.

In case the database already exists, you can run the following command to reset the database:

```sh
diesel database reset --database-url="postgres://postgres:postgrespw@localhost/iota_indexer"
```

#### Indexer setup

You can spin up an Indexer through the `iota start` subcommand which creates a simple local network or as a standalone service that connects to an existing fullnode.

To run the indexer as a standalone service with an existing fullnode, follow the steps below.

#### Standalone Indexer setup

- to run the indexer as a writer (Sync worker), which pulls data from a fullnode and writes data to the database

```sh
cargo run --bin iota-indexer -- --db-url "postgres://postgres:postgrespw@localhost/iota_indexer" indexer --remote-store-url "http://0.0.0.0:9000/api/v1" --reset-db
```

- to run indexer as a reader which exposes a JSON RPC service with following [APIs](https://docs.iota.org/iota-api-ref).

```sh
cargo run --bin iota-indexer -- --db-url "postgres://postgres:postgrespw@localhost/iota_indexer" json-rpc-service --rpc-client-url "http://0.0.0.0:9000" --rpc-address "0.0.0.0:9124"
```

Then the JSON RPC can be accessed like this:

```sh
curl http://localhost:9124 \
--header 'content-type: application/json' \
--data '{
    "jsonrpc": "2.0",
    "id": 1,
    "method": "iota_getChainIdentifier"
}'
```

> [!NOTE]
> To have a fully functional indexer that serves data via the JSON RPC interface at `--rpc-address`, you need to run both the writer (sync worker) instance to populate the database with data from the fullnode and the reader (RPC server worker) instance to expose the API. Running only the reader will not provide data unless the database has been populated by a writer.

More available flags can be found in this [file](https://github.com/iotaledger/iota/blob/develop/crates/iota-indexer/src/lib.rs).

### Backfilling of data

Sometimes when the schema changes (e.g. adding a new table or column), backfilling may be required to populate historical data.
The CLI provides a `run-backfill` command to facilitate this process:

```sh
Usage: iota-indexer run-backfill [OPTIONS] <START> <END> <COMMAND>

Commands:
  sql        Run a SQL backfill
  ingestion  Run a backfill driven by the ingestion engine
  help       Print this message or the help of the given subcommand(s)

Arguments:
  <START>  Start of the range to backfill, inclusive. It can be a checkpoint number or an epoch or any other identifier that
           can be used to slice the backfill range
  <END>    End of the range to backfill, inclusive

Options:
      --max-concurrency <MAX_CONCURRENCY>  Maximum number of concurrent tasks to run [default: 10]
      --chunk-size <CHUNK_SIZE>            Size of the data chunks processed per task [default: 1000]
  -h, --help                               Print help
```

It supports following backfill options:

- `sql`: Executes a SQL statement directly against the database in chunks, filtering on a specified column (typically a sequence number). Conflict resolution is handled automatically with `ON CONFLICT DO NOTHING`.
- `ingestion`: Fetches and buffers checkpoint data from a provided ingestion source, then slices the buffered checkpoint data into chunks to backfill the database. Supported ingestion sources:
  - `--data-ingestion-path <DIR>`: Path to a directory containing checkpoint (`.chk`) files.
  - `--remote-store-url <REMOTE_STORE_URL>`: Remote store URL to fetch checkpoint data from, e.g., `http://0.0.0.0:9000/api/v1`.
  - `--rpc-client-url <RPC_CLIENT_URL>`: RPC client URL to fetch checkpoint data from, e.g., `http://0.0.0.0:9000`.

#### Backfill job: `tx-wrapped-or-deleted-objects`

This job backfills the `tx_wrapped_or_deleted_objects` table, which indexes transactions that either wrapped or deleted given objects.
Replace `<START>` and `<END>` with the desired checkpoint range to backfill (e.g., `0` `10000`, both inclusive), and `<REMOTE_STORE_URL>` with the fullnode REST API URL used to fetch checkpoint data.

```sh
cargo run --bin iota-indexer -- --database-url <DATABASE_URL> run-backfill <START> <END> ingestion tx-wrapped-or-deleted-objects --remote-store-url <REMOTE_STORE_URL>
```

#### Error Handling

If any errors occur during the backfill, the error log will indicate the exact chunk (`{start}`-`{end}`) where the failure occurred. To prevent data gaps, you can restart the backfill from the calculated restart point:

`restart_from = failed_chunk_start - (max_concurrency * chunk_size)`

This ensures any unprocessed chunks are covered also in the worst-case, preventing data gaps.

### DB reset

To wipe the database, make sure you are in the `iota/crates/iota-indexer` directory and run following command. In case of schema changes in `.sql` files, this will also update corresponding `schema.rs` file:

```sh
diesel database reset --database-url="postgres://postgres:postgrespw@localhost/iota_indexer"
```

### CLI Reference

The IOTA Indexer is currently transitioning from the old CLI to a new one.
While both versions are still supported, the old CLI will be deprecated in the future.
Users are encouraged to start using the new CLI.

To view help information for each version:

```sh
# Old CLI
cargo run --bin iota-indexer -- help-deprecated

# New CLI
cargo run --bin iota-indexer -- help
```

### Experimental Features

#### Historic Fallback (REST KV Store)

The indexer supports an experimental historic fallback feature via the `--fallback-kv-url` flag.
This feature allows the indexer to fall back to a REST KV store for historical data when it's not available in the primary database.
It depends on the API served by the `iota-rest-kv` crate, which is still being finalized and subject to change.

> [!WARNING]
> This is an experimental feature and is subject to change without notice.

### Running tests

To run the tests, a running postgres instance is required.

```sh
docker run --name iota-indexer-tests -e POSTGRES_PASSWORD=postgrespw -e POSTGRES_USER=postgres -e POSTGRES_DB=iota_indexer -d -p 5432:5432 postgres
```

The crate provides following tests currently:

- unit tests for DB models (objects, events) which test the conversion between the database representation and the Rust representation of the objects and events.
- unit tests for the DB query filters, which test the conversion of filters to the correct SQL queries.
- integration tests (see [ingestion_tests](tests/ingestion_tests.rs)) to make sure the indexer correctly indexes transaction data from a full node by comparing the data in the database with the data received from the fullnode.
- rpc tests (see [rpc-tests](tests/rpc-tests/main.rs))

> [!NOTE]
> rpc tests which relies on postgres for every test it applies migrations, we need to run tests sequentially to avoid errors

```sh
# run tests requiring only postgres integration
cargo test --features pg_integration -- --test-threads 1
# run rpc tests with shared runtime
cargo test --profile simulator --features shared_test_runtime
```

For a better testing experience is possible to use [nextest](https://nexte.st/)

> [!NOTE]
> rpc tests which rely on a shared runtime are not supported with `nextest`
>
> This is because `cargo nextest` process-per-test execution model makes extremely difficult to share state and resources between tests.
>
> On the other hand `cargo test` does not run tests in separate processes by default. This means that tests can share state and resources.

```sh
# run tests requiring only postgres integration
cargo nextest run --features pg_integration --test-threads 1
```
