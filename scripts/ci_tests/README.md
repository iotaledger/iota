# CI Test Script

This repository includes a CI test script that helps automate running various test suites. Below, you will find details on how to use the script, the available commands, and the relevant environment variables.

## Usage

To run all CI tests, execute:

```sh
./scripts/ci_tests/rust_tests.sh
```

By default, this will run all available test suites. You can also run specific test steps as subcommands:

```sh
./scripts/ci_tests/rust_tests.sh <step_name>
```

## Available Test Steps

The script supports the following test steps:

- `rust_crates` - Test Rust crates
- `external_crates` - Test external crates
- `simtests` - Run simulation tests
- `tests_using_postgres` - Run tests that require PostgreSQL
- `move_examples_rdeps_tests` - Run tests for Move examples
- `move_examples_rdeps_simtests` - Run Move example simulation tests
- `test_extra` - Run extra tests, including stress tests and documentation tests
- `run_tests` - Fine grained option to run test cases (used by CI, needs additional ENV vars, see below)
- `run_simtests` - Fine grained option to run simtest cases (used by CI, needs additional ENV vars, see below)
- `stress_new_tests_check_for_flakiness` - Verify new tests for flakiness
- `unused_deps` - Check for unused dependencies
- `audit_deps` - Audit dependencies for security and compliance
- `audit_deps_external` - Audit external dependencies

## Environment Variables

You can control the behavior of the test script using the following environment variables:

- `TEST_ONLY_CHANGED_CRATES` - If `true`, only test crates that have changed (default: `false`).
- `CI_CHANGED_CRATES` - Space-separated list of changed crates. Example: `iota-proxy iota-framework`
- `CI_CHANGED_EXTERNAL_CRATES` - Space-separated list of changed external crates. Example: `bytecode-verifier-tests bytecode-verifier-transactional-tests`
- `RESTART_POSTGRES` - If `true`, it will restart the local docker container for postgres.

Additional Environment Variables for `run_tests` and `run_simtests`:

- `CI_IS_RUST` - If `true`, the cargo nextests for `crates/` are run.
- `CI_IS_EXTERNAL_CRATES` - If `true`, the cargo nextests for `external-crates/` are run.
- `CI_IS_PG_INTEGRATION` - If `true`, the tests that depend on `postgres` are run.
- `CI_IS_MOVE_EXAMPLE_USED_BY_OTHERS` - If `true`, the tests that depend on the move examples are run.

## Some examples

Run all rust nextests (except tests using postgres):

```sh
./scripts/ci_tests/rust_tests.sh rust_crates
```

Automatically detect and test changed crates compared to `develop` (only committed changes):

```sh
TEST_ONLY_CHANGED_CRATES=true ./scripts/ci_tests/rust_tests.sh rust_crates
```

You can manually test specific crates based on your needs. Examples:

```sh
TEST_ONLY_CHANGED_CRATES=true CI_CHANGED_CRATES="iota-proxy iota-framework" ./scripts/ci_tests/rust_tests.sh rust_crates
```

Run all simtests (except tests using postgres):

```sh
./scripts/ci_tests/rust_tests.sh simtests
```

## Notes

If you are running tests using PostgreSQL, you need to have a local postgres docker instance running. This can be done with this command:

```sh
pushd docker/pg-services-local && docker compose up -d postgres && popd
```
