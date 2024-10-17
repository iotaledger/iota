# Services relying on postgres database

This docker-compose configuration allows launching instances of the `iota-indexer` and `iota-graphql-rpc` applications for local development purposes.

These applications require a running postgres server, and in the absence of
persistence for the server data, a local network to sync the database with.

For this configuration we have opted out of persisting the database data. Users
that want to enable persistence should use the `iota-private-network` compose
configuration.

## Requirements

- [Docker Compose](https://docs.docker.com/engine/install/)

## Start the services

In current images when cargo is compiling it relies on `iota-sim` which is a private repository, it can be cloned only through ssh.
As a temporary workaround [until this issue will be closed](https://github.com/iotaledger/iota/issues/2149), before starting services
we need to build them first, even though `docker compose up -d` does automatically build the images in this case it will fail because an explicit ssh mount is needed.

Before proceeding make sure to follow these steps [here](https://docs.github.com/en/authentication/connecting-to-github-with-ssh/generating-a-new-ssh-key-and-adding-it-to-the-ssh-agent#adding-your-ssh-key-to-the-ssh-agent)

> [!NOTE]
> If you're on MacOS and using Docker Desktop make sure to choose file sharing implementation for your containers from `VirtioFS` to `gRPC FUSE`
> as dedcribed in this [issue](https://github.com/docker/for-mac/issues/7204#issuecomment-1969109233)

```
$ docker compose build --ssh default
$ docker compose up -d
```

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

or simply

```
$ docker compose up -d
```

> [!NOTE]
>
> `docker compose up [<service>]` only builds the required docker images if they
> have not been built already.
>
> If you want to rebuild an image, after you made some local changes, say in
> `iota-indexer`, it is advised to use the `build` subcommand for the particular
> image you want to rebuild. E.g.
>
> ```
> $ docker compose build indexer-sync
> ```
>
> Running `docker compose build` without specifying a service, would rebuild all images affected by your
> changes, and thus is not recommended. To understand why, consider that changes
> in `iota-indexer`, or `iota-graphql-rpc` would cause the `iota` image to rebuild.
> The `local-network` service however uses `iota` as a node cluster
> without an indexer service or a GraphQL service. Thus rebuilding the image for every
> change during a normal development workflow would hinder development
> unnecessarily.

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
