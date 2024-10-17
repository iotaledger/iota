End-to-end tests for GraphQL service, built on top of the transactional test
runner.

# Local Set-up

These tests require a running instance of the `postgres` service, with a
database set-up.

It is recommended that the database server is started in a docker container.

## Using `docker compose`

```sh
$ POSTGRES_USER=postgres POSTGRES_DB=postgres POSTGRES_PASSWORD=postgrespw POSTGRES_INITDB_ARGS="-U postgres" docker compose -f docker/pg-services-local/docker-compose.yaml up -d postgres
```

## Using `docker`

```sh
docker run -d --name postgres \
 -e POSTGRES_PASSWORD=postgrespw \
 -e POSTGRES_INITDB_ARGS="-U postgres" \
 -p 5432:5432 \
 postgres:15 \
 -c max_connections=1000
```

# Running Locally

When running the tests locally, they must be run with the `pg_integration`
feature enabled:

```sh
$ cargo nextest run --features pg_integration
```

# Snapshot Stability

Tests are pinned to an existing protocol version that has already been used on a
production network. The protocol version controls the protocol config and also
the version of the framework that gets used by tests. By using a version that
has already been used in a production setting, we are guaranteeing that it will
not be changed by later modifications to the protocol or framework (this would
be a bug).

When adding a new test, **remember to set the `--protocol-version`** for that
test to ensure stability.
