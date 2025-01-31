#!/bin/bash
ROOT=$(git rev-parse --show-toplevel || realpath "$(dirname "$0")/../..")

#
# INPUTS
#

# Running all the tests will compile different sets of crates and take a lot of storage (>500GB)
# If your machine has less storage, you can run only part of the tests (at a time),
# use the name of the function to run as a subcommand, for instance:
# ./scripts/tests_like_ci/rust_tests.sh simtests
export RUN_ONLY_STEP=${1:-${RUN_ONLY_STEP:-}}
# the possible steps are:
export VALID_STEPS=(rust_crates unused_deps external_crates test_extra simtests tests_using_postgres stress_new_tests_check_for_flakiness)

# CI will only test crates that have changed in the PR
# For local tests, tests all crates by default. Override with TEST_ONLY_CHANGED_CRATES=true
export TEST_ONLY_CHANGED_CRATES=${TEST_ONLY_CHANGED_CRATES:-false}

# CI uses an action to detect changed_crates. It needs to be able to override changed crates with the ones detected by that action.
# Override with CHANGED_CRATES.
# Locally, you don't need to provide this variable, this script will detect changed crates.
# Format of CHANGED_CRATES: one string, space-separated: CHANGED_CRATES="crate1 crate2 crate3" ./this_script.sh

# CI uses postgres provided via a github CI service. It needs to be able to not restart postgres.
# Locally, this script restarts postgres by default. Override by passing RESTART_POSTGRES=false
# only the tests that need postgres will automatically (re-)start it
export RESTART_POSTGRES=${RESTART_POSTGRES:-true}

#
# END INPUTS
#

function changed_crates() {
    if ! yq --version | grep -q "v4." 2>/dev/null; then
        echo -e "\033[31m'yq' v4.0+ is not installed in PATH. Please ensure you installed \033[92myq v4.0+.\033[0m" >&2
        if [ "$(uname -s)" == "Linux" ]; then echo -e "On Ubuntu/Linux via snap: \033[92msnap install yq\033[0m" >&2; fi
        if [ "$(uname -s)" == "Darwin" ]; then echo -e "On MacOS via Brew: \033[92mbrew install yq\033[0m" >&2; fi
        echo -e "More installation options at https://github.com/mikefarah/yq/#install" >&2
        exit 1
    fi

    # assuming PRs merge into origin/develop, we diff the current branch with origin/develop
    CHANGED_FILES=$(git diff --name-only origin/develop..HEAD)
    CRATES_FILTERS_YML="${ROOT}/.github/crates-filters.yml"

    TUPLES_CRATE_NAME_PATH=$(yq -r 'to_entries[] | .key + " " + (.value[] | sub("/\\*\\*$",""))' $CRATES_FILTERS_YML)

    MATCHING_CRATES=()
    while IFS= read -r tuple; do
        crate_name=$(echo "$tuple" | cut -d' ' -f1)
        crate_path_starts_with=$(echo "$tuple" | cut -d' ' -f2)
        for CHANGED_FILE in $CHANGED_FILES; do
            if [[ "$CHANGED_FILE" == "$crate_path_starts_with"* ]]; then
                MATCHING_CRATES+=($crate_name)
            fi
        done
    done <<<"$TUPLES_CRATE_NAME_PATH"
    echo "${MATCHING_CRATES[@]}" | tr ' ' '\n' | sort -u | tr '\n' ' '
}

function mk_test_filterset() {
    if [ "$TEST_ONLY_CHANGED_CRATES" == "false" ]; then
        # test all crates (return empty filterset)
        return
    fi
    # else TEST_ONLY_CHANGED_CRATES is true
    # detect changed_crates() only if variable unset. If set to empty string, keep it as is.
    CHANGED_CRATES="${CHANGED_CRATES-"$(changed_crates)"}"
    echo "Using CHANGED_CRATES: ${CHANGED_CRATES[@]}" >&2

    # only include changed crates and all their dependent crates
    FILTERSET=""
    for crate in ${CHANGED_CRATES[@]}; do
        # rdeps selects the crate plus all crates that depend on it
        add_filter="-E rdeps(${crate})"

        if [ -z "$FILTERSET" ]; then
            FILTERSET="$add_filter"
        else
            FILTERSET="$FILTERSET $add_filter"
        fi
    done
    # if filterset is sill empty here, it means no changed crates were detected, there are no crates to test
    if [ -z "$FILTERSET" ]; then
        echo "test_none"
        return
    fi
    echo "${FILTERSET}"
}

function retry_only_tests() {
    FILTERSET=""
    for test_name in "${RETRY_ONLY_TESTS[@]}"; do
        FILTERSET="${FILTERSET} -E 'test(${test_name})'"
    done
    echo "FILTERSET: ${FILTERSET}"
    echo "Running: cargo nextest run --profile ci ${FILTERSET} --test-threads 1"
    cargo nextest run --profile ci ${FILTERSET} --test-threads 1
}

function rust_crates() {
    # Tests written with #[sim_test] are often flaky if run as #[tokio::test] - this var
    # causes #[sim_test] to only run under the deterministic `simtest` job, and not the
    # non-deterministic `test` job.
    export IOTA_SKIP_SIMTESTS=1

    FILTERSET="$(mk_test_filterset)"
    if [ "$FILTERSET" == "test_none" ]; then
        echo "No changed crates detected. Skipping"
        return
    fi

    command="cargo nextest run --config-file .config/nextest.toml --profile ci $FILTERSET"
    echo "Running: $command"
    cargo nextest run --config-file .config/nextest.toml --profile ci $FILTERSET
}

function external_crates() {
    # WARNING: this has  a side effect of updating the Cargo.lock file
    FILTERSET="$(mk_test_filterset) -E '!test(prove) and !test(run_all::simple_build_with_docs/args.txt) and !test(run_test::nested_deps_bad_parent/Move.toml)"
    command="cargo nextest run --config-file .config/nextest.toml --profile ci --manifest-path external-crates/move/Cargo.toml $FILTERSET"
    echo "Running: $command"
    cargo nextest run --config-file .config/nextest.toml --profile ci --manifest-path external-crates/move/Cargo.toml $FILTERSET
}

function unused_deps() {
    cargo +nightly ci-udeps --all-features
    cargo +nightly ci-udeps --no-default-features
}

function test_extra() {
    export IOTA_SKIP_SIMTESTS=1
    cargo run --package iota-benchmark --bin stress -- --log-path ${ROOT}/.cache/stress.log --num-client-threads 10 --num-server-threads 24 --num-transfer-accounts 2 bench --target-qps 100 --num-workers 10 --transfer-object 50 --shared-counter 50 --run-duration 10s --stress-stat-collection
    cargo test --doc
    cargo doc --all-features --workspace --no-deps
    ${ROOT}/scripts/execution_layer.py generate-lib
    ${ROOT}/scripts/changed-files.sh
}

function simtests() {
    export MSIM_WATCHDOG_TIMEOUT_MS=${MSIM_WATCHDOG_TIMEOUT_MS:-60000}
    echo "Running: scripts/simtest/cargo-simtest simtest --profile ci --color always"
    scripts/simtest/cargo-simtest simtest --profile ci --color always
}

function stress_new_tests_check_for_flakiness() {
    scripts/simtest/stress-new-tests.sh
}

# restart postgres
function restart_postgres() {
    if ! command -v psql &>/dev/null; then
        echo "'psql' is not installed in PATH. Please ensure it is installed and available."
        exit 1
    fi
    docker rm -f -v $(docker ps -a | grep postgres | awk '{print $1}')
    export POSTGRES_PASSWORD=${POSTGRES_PASSWORD:-postgrespw}
    export POSTGRES_USER=${POSTGRES_USER:-postgres}
    export POSTGRES_DB=${POSTGRES_DB:-iota_indexer}
    export POSTGRES_HOST=${POSTGRES_HOST:-postgres}
    # assuming you run the indexer's postgres using docker-compose
    cd ${ROOT}/dev-tools/pg-services-local
    docker-compose down -v postgres
    docker-compose up -d postgres
    PGPASSWORD=$POSTGRES_PASSWORD psql -h localhost -U $POSTGRES_USER -c 'CREATE DATABASE IF NOT EXISTS iota_indexer;' -c 'ALTER SYSTEM SET max_connections = 500;' 2>/dev/null
}

function tests_using_postgres() {
    if [ "$RESTART_POSTGRES" == "true" ]; then restart_postgres; fi
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
    if [[ " ${VALID_STEPS[*]} " =~ " ${RUN_ONLY_STEP} " ]]; then # if VALID_STEPS contains RUN_ONLY_STEP
        "$RUN_ONLY_STEP"
    else
        echo "Invalid step RUN_ONLY_STEP: $RUN_ONLY_STEP"
        exit 1
    fi
else
    for step in "${VALID_STEPS[@]}"; do
        if [ "$step" != "external_crates" ]; then
            echo "Running step: $step"
            $step
        fi
    done
fi
