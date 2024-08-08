# Services relying on postgres db

This docker-compose configuration allows launching instances of the `iota-indexer` and `iota-graphql-rpc` applications for local development purposes.

These applications require a running postgres server, and in the absence of
persistence for the server data, a local network to sync the database with.

For this configuration we have opted out of persisting the database data. Users
that want to enable persistence should use the `iota-private-network` compose
configuration.

## Requirements

- [Docker Compose](https://docs.docker.com/engine/install/)

## Start the services

### `iota-indexer` rpc worker

```
$ docker compose up -d indexer-rpc
```

### `iota-graphql-rpc` server

```
$ docker compose up -d graphql-server
```

### `iota-indexer` rpc worker and `iota-graphql-rpc` server

```
$ docker compose up -d graphql-server indexer-rpc
```

### Dependencies

As mentioned, these applications depend on the following services that start by default:

- A running local network with test data.

  To start in isolation

  ```
  $ docker compose up -d local-network
  ```

- A running postgres server

  To start in isolation

  ```
  $ docker compose up -d postgres
  ```

- An `iota-indexer` sync worker on top of `local-network`, and `postgres`

  To start

  ```
  $ docker compose up -d indexer-sync
  ```

  It should be noted that this does not expose any public interface, its sole
  purpose being synchronizing the database with the ledger state.
