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
# Change the RPC_CLIENT_URL to http://0.0.0.0:9000 to run indexer against local validator & fullnode
cargo run --bin iota-indexer -- --db-url "postgres://postgres:postgrespw@localhost/iota_indexer" --rpc-client-url "https://api.devnet.iota.cafe:443" --fullnode-sync-worker --reset-db
```

- to run indexer as a reader which exposes a JSON RPC service with following [APIs](https://docs.iota.org/iota-api-ref).

```
cargo run --bin iota-indexer -- --db-url "postgres://postgres:postgrespw@localhost/iota_indexer" --rpc-client-url "https://api.devnet.iota.cafe:443" --rpc-server-worker
```

More available flags can be found in this [file](https://github.com/iotaledger/iota/blob/develop/crates/iota-indexer/src/lib.rs).

### DB reset

To wipe the database, make sure you are in the `iota/crates/iota-indexer` directory and run following command. In case of schema changes in `.sql` files, this will also update corresponding `schema.rs` file:

```sh
diesel database reset --database-url="postgres://postgres:postgrespw@localhost/iota_indexer"
```

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
> rpc tests which relies on postgres for every test it applies migrations, we need to run tests sequencially to avoid errors

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

## Steps to run locally with TiDB (experimental)

### Prerequisites

1. Install TiDB

```sh
curl --proto '=https' --tlsv1.2 -sSf https://tiup-mirrors.pingcap.com/install.sh | sh
```

2. Install a compatible version of MySQL (At the time of writing, this is MySQL 8.0 -- note that 8.3 is incompatible).

```sh
brew install mysql@8.0
```

3. Install a version of `diesel_cli` that supports MySQL (and probably also Postgres). This version of the CLI needs to be built against the version of MySQL that was installed in the previous step (compatible with the local installation of TiDB, 8.0.37 at time of writing).

```sh
MYSQLCLIENT_LIB_DIR=/opt/homebrew/Cellar/mysql@8.0/8.0.37/lib/ cargo install diesel_cli --no-default-features --features postgres --features mysql --force
```

### Run the indexer

1.Run TiDB

```sh
tiup playground
```

2.Verify tidb is running by connecting to it using the mysql client, create database `test`

```sh
mysql --comments --host 127.0.0.1 --port 4000 -u root
create database test;
```

3.DB setup, under `iota/crates/iota-indexer` run:

```sh
# an example DATABASE_URL is "mysql://root:password@127.0.0.1:4000/test"
diesel setup --database-url="<DATABASE_URL>" --migration-dir='migrations/mysql'
diesel database reset --database-url="<DATABASE_URL>" --migration-dir='migrations/mysql'
```

Note that you need an existing database for this to work. Using the DATABASE_URL example in the comment of the previous code, replace `test` with the name of your database.
4. Run indexer as a writer, which pulls data from fullnode and writes data to DB

```sh
# Change the RPC_CLIENT_URL to http://0.0.0.0:9000 to run indexer against local validator & fullnode
cargo run --bin iota-indexer --features mysql-feature --no-default-features -- --db-url "<DATABASE_URL>" --rpc-client-url "https://api.devnet.iota.cafe:443" --fullnode-sync-worker --reset-db
```
