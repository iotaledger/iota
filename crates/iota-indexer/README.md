Iota indexer is an off-fullnode service to serve data from Iota protocol, including both data directly generated from chain and derivative data.

## Architecture

![enhanced_FN](../../docs/content/operator/images/indexer-arch.png)

> [!NOTE]
>
> - Indexer sync workers require the `NodeConfig::enable_experimental_rest_api` flag set to `true` in the node
> - Fullnodes expose both read and write json-rpc APIs. Hence transactions may be executed only through fullnodes.
> - Validators expose only read-only JSON-RPC APIs.
> - Read-only APIs in the node variants expose the same methods as the indexer read-only APIs with the following difference
>   - Nodes expose the `NameServiceConfig` API, whereas indexer instances do not.
>   - Indexer instance expose the `ExtendedApi`, but nodes do not.

## Steps to run locally

### Prerequisites

Install local [Postgres server](https://www.postgresql.org/download/). Platform-specific instructions follow.

#### Postgres (MacOS)

- install local [Postgres server](https://www.postgresql.org/download/).
  - You can also `brew install postgresql@15` and then add the following to your `~/.zshrc` or `~/.zprofile`, etc:

```sh
export LDFLAGS="-L/opt/homebrew/opt/postgresql@15/lib"
export CPPFLAGS="-I/opt/homebrew/opt/postgresql@15/include"
export PATH="/opt/homebrew/opt/postgresql@15/bin:$PATH"
```

- make sure you have libpq installed: `brew install libpq`, and in your profile, add `export PATH="/opt/homebrew/opt/libpq/bin:$PATH"`. If this doesn't work, try `brew link --force libpq`.

#### Postgres (Ubuntu)

On ubuntu, install postgres via `apt`:

```sh
sudo apt install postgresql
```

#### Diesel

- install Diesel CLI with `cargo install diesel_cli --no-default-features --features postgres`, refer to [Diesel Getting Started guide](https://diesel.rs/guides/getting-started) for more details
- [optional but handy] Postgres client like [Postico](https://eggerapps.at/postico2/), for local check, query execution etc.

### Start the Postgres Service

Postgres must run as a service in the background for other tools to communicate with. If it was installed using homebrew, it can be started as a service with:

```sh
brew services start postgresql@version
```

### Local Development(Recommended)

Use [iota-test-validator](../../crates/iota-test-validator/README.md)

### Running standalone indexer

1. DB setup, under `iota/crates/iota-indexer` run:

```sh
# an example DATABASE_URL is "postgres://postgres:postgres@localhost/gegao" where postgres:postgres are the credentials.
diesel setup --database-url="<DATABASE_URL>"
diesel database reset --database-url="<DATABASE_URL>"
```

Note that you'll need an existing database for the above to work. Replace `gegao` with the name of the database created.

2. Checkout to your target branch

For example, if you want to be on the DevNet branch

```sh
git fetch upstream devnet && git reset --hard upstream/devnet
```

3. Start indexer binary, under `iota/crates/iota-indexer` run:

- run indexer as a writer, which pulls data from fullnode and writes data to DB

```sh
# Change the RPC_CLIENT_URL to http://0.0.0.0:9000 to run indexer against local validator & fullnode
cargo run --bin iota-indexer -- --db-url "<DATABASE_URL>" --rpc-client-url "https://fullnode.devnet.iota.io:443" --fullnode-sync-worker --reset-db
```

- run indexer as a reader, which is a JSON RPC server with the [interface](https://docs.iota.io/iota-api-ref#iotax_getallbalances)

```
cargo run --bin iota-indexer -- --db-url "<DATABASE_URL>" --rpc-client-url "https://fullnode.devnet.iota.io:443" --rpc-server-worker
```

More flags info can be found in this [file](https://github.com/iotaledger/iota/blob/main/crates/iota-indexer/src/lib.rs#L83-L123).

### DB reset

Run this command under `iota/crates/iota-indexer`, which will wipe DB; In case of schema changes in `.sql` files, this will also update corresponding `schema.rs` file.

```sh
diesel database reset --database-url="<DATABASE_URL>"
```
