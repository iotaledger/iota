#!/bin/bash
WD=$(git rev-parse --show-toplevel)

# INPUTS

# Running all the tests will compile different sets of crates and take a lot of storage (>500GB)
# If your machine has less storage, you can run only part of the tests (at a time), 
# use the name of the function to run as a subcommand, for instance:
# ./scripts/tests_like_ci/rust_tests.sh simtests
# the possible steps are: check_unused_deps, test_rust_crates, test_external_crates, test_extra, tests_using_postgres, simtests
# the tests that need postgres will automatically (re-)start it
RUN_ONLY_STEP=$1

# restart postgres
function restart_postgres() {
    if ! command -v psql &> /dev/null; then
        echo "'psql' is not installed in PATH. Please ensure it is installed and available."
        exit 1
    fi
    docker rm -f -v $(docker ps -a | grep postgres | awk '{print $1}')
    export POSTGRES_PASSWORD=${POSTGRES_PASSWORD:-postgrespw}
    export POSTGRES_USER=${POSTGRES_USER:-postgres}
    export POSTGRES_DB=${POSTGRES_DB:-iota_indexer}
    export POSTGRES_HOST=${POSTGRES_HOST:-postgres}
    # assuming you run the indexer's postgres using docker-compose
    cd ${WD}/docker/pg-services-local; docker-compose down -v postgres; docker-compose up -d postgres
    PGPASSWORD=$POSTGRES_PASSWORD psql -h localhost -U $POSTGRES_USER -c 'CREATE DATABASE IF NOT EXISTS iota_indexer;' -c 'ALTER SYSTEM SET max_connections = 500;' 2>/dev/null
}

function retry_failing_only() {
    filterset=""
    for line in "${FAILING_NONSIM_TESTS[@]}"; do
        arr=(${line// / })
        if [ ${#arr[@]} -eq 2 ]; then
            package=${arr[0]%%::*}
            test_name=${arr[-1]#*::}
            echo "package:$package test_name:$test_name"
            filterset="${filterset} -E 'test(${test_name})'"
            break   
        fi
    done
    echo "FILTERSET: ${filterset}"
    command="cargo nextest run --profile ci ${filterset} --test-threads 1"
    set -x
    eval $command
}

function test_rust_crates() {
    # Tests written with #[sim_test] are often flaky if run as #[tokio::test] - this var
    # causes #[sim_test] to only run under the deterministic `simtest` job, and not the
    # non-deterministic `test` job.
    export IOTA_SKIP_SIMTESTS=1
    cargo nextest run --config-file .config/nextest.toml --profile ci
}

function test_external_crates() {
    cargo nextest run --config-file .config/nextest.toml --manifest-path external-crates/move/Cargo.toml -E '!test(prove) and !test(run_all::simple_build_with_docs/args.txt) and !test(run_test::nested_deps_bad_parent/Move.toml)' --profile ci
}



function check_unused_deps() {
    cargo +nightly ci-udeps --all-features
    cargo +nightly ci-udeps --no-default-features
}

function test_extra() {
    export IOTA_SKIP_SIMTESTS=1
    cargo run --package iota-benchmark --bin stress -- --log-path ${WD}/.cache/stress.log --num-client-threads 10 --num-server-threads 24 --num-transfer-accounts 2 bench --target-qps 100 --num-workers 10  --transfer-object 50 --shared-counter 50 --run-duration 10s --stress-stat-collection
    cargo test --doc
    cargo doc --all-features --workspace --no-deps
    ${WD}/scripts/execution_layer.py generate-lib;
    ${WD}/scripts/changed-files.sh;
}

function simtests() {
    export MSIM_WATCHDOG_TIMEOUT_MS=60000
    scripts/simtest/cargo-simtest simtest --profile ci --color always
    scripts/simtest/stress-new-tests.sh
}

function tests_using_postgres() {
    restart_postgres
    cargo nextest run --no-fail-fast --test-threads 1 --package iota-graphql-rpc --test e2e_tests --test examples_validation_tests --features pg_integration
    cargo nextest run --no-fail-fast --test-threads 1 --package iota-graphql-rpc --lib --features pg_integration -- test_query_cost
    cargo nextest run --no-fail-fast --test-threads 8 --package iota-graphql-e2e-tests --features pg_integration
    cargo nextest run --no-fail-fast --test-threads 1 --package iota-cluster-test --test local_cluster_test --features pg_integration
    cargo nextest run --no-fail-fast --test-threads 1 --package iota-indexer --test ingestion_tests --features pg_integration
    # Iota-indexer's RPC tests, which depend on a shared runtime, are incompatible with nextest due to its process-per-test execution model.
    # cargo test, on the other hand, allows tests to share state and resources by default.
    cargo test --profile simulator --package iota-indexer --test rpc-tests --features shared_test_runtime
}


# Running all the tests will compile different sets of crates and take a lot of storage (>500GB)
# If your machine has less storage, you can run only part of the tests (at a time), 
# use the name of the function to run as a subcommand, for instance:
# ./scripts/tests_like_ci/rust_tests.sh simtests
if [ -n "$RUN_ONLY_STEP" ]; then
    if declare -f "$RUN_ONLY_STEP" > /dev/null; then
        "$RUN_ONLY_STEP"
    else
        # run all steps
        set -euxo pipefail
        check_unused_deps
        test_rust_crates
        test_external_crates
        test_extra
        tests_using_postgres
        simtests
    fi
fi

