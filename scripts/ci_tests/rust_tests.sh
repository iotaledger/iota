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
export VALID_STEPS=(rust_crates unused_deps external_crates test_extra simtests using_postgres)

# CI will only test crates that have changed in the PR
# For local tests, tests all crates by default. Override with TEST_ONLY_CHANGED_CRATES=true
# if specifying TEST_ONLY_CRATES, TEST_ONLY_CHANGED_CRATES will be ignored
export TEST_ONLY_CHANGED_CRATES=${TEST_ONLY_CHANGED_CRATES:-false}

# CI uses an action to detect changed_crates. It needs to be able to override changed crates with the ones detected by that action.
# Locally, you don't need to provide this variable, this script will detect changed crates.
# If overriding, format it in one string, space-separated: CHANGED_CRATES="crate1 crate2 crate3" ./this_script.sh
# if specifying TEST_ONLY_CRATES, TEST_ONLY_CHANGED_CRATES will be ignored. All of TEST_ONLY_CRATES will be tested regardless of changed crates.
export TEST_ONLY_CRATES=${TEST_ONLY_CRATES:-}

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
    # if both TEST_ONLY_CRATES and TEST_ONLY_CHANGED_CRATES are not set, test all crates (empty filterset)
    if [ ! -n "$TEST_ONLY_CRATES" ] && [ "$TEST_ONLY_CHANGED_CRATES" == "false" ]; then
        return
    fi

    TEST_ONLY_CRATES=(${TEST_ONLY_CRATES:-"$(changed_crates)"})
    # if [ -n "$TEST_ONLY_CRATES" ]; then
    #     export TEST_ONLY_CRATES="$(changed_crates)"
    # fi
    echo "Using TEST_ONLY_CRATES: ${TEST_ONLY_CRATES[@]}" >&2

    # only include changed crates and all their dependent crates
    FILTERSET=""
    for crate in ${TEST_ONLY_CRATES[@]}; do
        # rdeps selects the crate plus all crates that depend on it
        add_filter="-E rdeps(${crate})"

        if [ -z "$FILTERSET" ]; then
            FILTERSET="$add_filter"
        else
            FILTERSET="$FILTERSET $add_filter"
        fi
    done
    echo "${FILTERSET}"
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
    cd ${ROOT}/docker/pg-services-local
    docker-compose down -v postgres
    docker-compose up -d postgres
    PGPASSWORD=$POSTGRES_PASSWORD psql -h localhost -U $POSTGRES_USER -c 'CREATE DATABASE IF NOT EXISTS iota_indexer;' -c 'ALTER SYSTEM SET max_connections = 500;' 2>/dev/null
}

# function retry_failing_only() {
#     filterset=""
#     for line in "${FAILING_NONSIM_TESTS[@]}"; do
#         arr=(${line// / })
#         if [ ${#arr[@]} -eq 2 ]; then
#             package=${arr[0]%%::*}
#             test_name=${arr[-1]#*::}
#             echo "package:$package test_name:$test_name"
#             filterset="${filterset} -E 'test(${test_name})'"
#             break
#         fi
#     done
#     echo "FILTERSET: ${filterset}"
#     command="cargo nextest run --profile ci ${filterset} --test-threads 1"
#     set -x
#     eval $command
# }

function rust_crates() {
    # Tests written with #[sim_test] are often flaky if run as #[tokio::test] - this var
    # causes #[sim_test] to only run under the deterministic `simtest` job, and not the
    # non-deterministic `test` job.
    export IOTA_SKIP_SIMTESTS=1
    FILTERSET=$(mk_test_filterset)
    command="cargo nextest run --config-file .config/nextest.toml --profile ci $FILTERSET"
    echo "Running: $command"
    cargo nextest run --config-file .config/nextest.toml --profile ci $FILTERSET
}

function external_crates() {
    FILTERSET=$(mk_test_filterset)
    command="cargo nextest run --config-file .config/nextest.toml --profile ci --manifest-path external-crates/move/Cargo.toml $FILTERSET"
    echo "Running: $command"
    cargo nextest run --config-file .config/nextest.toml --manifest-path external-crates/move/Cargo.toml -E '!test(prove) and !test(run_all::simple_build_with_docs/args.txt) and !test(run_test::nested_deps_bad_parent/Move.toml)' --profile ci
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
    scripts/simtest/cargo-simtest simtest --profile ci --color always
}

function stress_new_tests_check_for_flakiness() {
    scripts/simtest/stress-new-tests.sh
}

function using_postgres() {
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
    if [[ " ${VALID_STEPS[*]} " =~ " ${RUN_ONLY_STEP} " ]]; then # if VALID_STEPS contains RUN_ONLY_STEP
        "$RUN_ONLY_STEP"
    else
        echo "Invalid step RUN_ONLY_STEP: $RUN_ONLY_STEP"
        exit 1
    fi
else
    for step in "${VALID_STEPS[@]}"; do
        echo "Running step: $step"
        $step
    done
fi
