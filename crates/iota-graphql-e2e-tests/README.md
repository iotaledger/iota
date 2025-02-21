End-to-end tests for GraphQL service, built on top of the transactional test
runner.

# Testing through the transactional test runner

This crate specifically tests GraphQL queries in an e2e manner against the Indexer database.
The tests are executed through the [transactional test runner](../iota-transactional-test-runner), mediated by the [test.rs](tests/tests.rs) module.

Each test is defined in a `.move` file, which contains various statements that are executed in a transactional manner.
In addition to the `.move` file, there is a corresponding `.exp` file that contains the expected output of the transactional test.

# Local Set-up

These tests require a running instance of the `postgres` service, with a
database set-up.

It is recommended that the database server is started in a docker container.

## Using `docker compose`

```sh
$ POSTGRES_USER=postgres POSTGRES_DB=postgres POSTGRES_PASSWORD=postgrespw POSTGRES_INITDB_ARGS="-U postgres" docker compose -f dev-tools/pg-services-local/docker-compose.yaml up -d postgres
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
