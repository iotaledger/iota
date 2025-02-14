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
export VALID_STEPS=(rust_crates unused_deps external_crates test_extra simtests tests_using_postgres stress_new_tests_check_for_flakiness audit_deps audit_deps_external move_examples_rdeps_tests move_examples_rdeps_simtests)

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

function mk_test_filterset_changed_crates() {
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
        add_filter="rdeps(${crate})"

        if [ -z "$FILTERSET" ]; then
            FILTERSET="$add_filter"
        else
            FILTERSET="$FILTERSET | $add_filter"
        fi
    done

    # if no crates were changed, we want to run all tests.
    # because changes that trigger the workflow but which aren't explicitly in a crate can potentially affect the entire workspace
    # returning an empty filterset does that

    echo "${FILTERSET}"
}

function mk_move_examples_rdeps_tests_filterset() {
    # iota-test-transaction-builder + iota-core provide functions that publish packages from the Move examples for other crates to use.
    # iota-framework-tests, iota-json, iota-json-rpc-tests, iota-rosetta use the Move examples directly as part of their tests.
    INCLUDED=(
        "rdeps(iota-test-transaction-builder)"
        "rdeps(iota-core)"
        "package(iota-framework-tests)"
        "package(iota-json) & test(test_basic_args_linter_top_level)"
        "package(iota-json-rpc-tests) & (test(try_get_past_object_deleted) | test(test_publish))"
        "package(iota-rosetta) & test(test_publish_and_move_call)"
    )

    FILTERSET=""
    for item in "${INCLUDED[@]}"; do
        add_filter="(${item})"

        if [ -z "$FILTERSET" ]; then
            FILTERSET="$add_filter"
        else
            FILTERSET="$FILTERSET | $add_filter"
        fi
    done

    echo "${FILTERSET}"
}

function mk_exclude_filterset() {
    EXCLUDE_SET=""

    # These require extra config which is applied in `tests_using_postgres` below.
    # They are excluded from the main tests group, which runs with all features enabled,
    # because they will fail due to cross-thread contamination.
    EXCLUDED=(
        "package(iota-graphql-rpc) & (binary(e2e_tests) | binary(examples_validation_tests) | test(test_query_cost))"
        "package(iota-graphql-e2e-tests)"
        "package(iota-cluster-test) & binary(local_cluster_test)"
        "package(iota-indexer) & (binary(ingestion_tests) | binary(rpc-tests))"
    )

    for item in "${EXCLUDED[@]}"; do
        add_filter="!(${item})"

        if [ -z "$EXCLUDE_SET" ]; then
            EXCLUDE_SET="$add_filter"
        else
            EXCLUDE_SET="$EXCLUDE_SET & $add_filter"
        fi
    done

    echo "${EXCLUDE_SET}"
}

function mk_exclude_filterset_external() {
    EXCLUDE_SET=""

    EXCLUDED=(
        "test(prove)"
        "test(run_all::simple_build_with_docs/args.txt)"
        "test(run_test::nested_deps_bad_parent/Move.toml)"
    )

    for item in "${EXCLUDED[@]}"; do
        add_filter="!(${item})"

        if [ -z "$EXCLUDE_SET" ]; then
            EXCLUDE_SET="$add_filter"
        else
            EXCLUDE_SET="$EXCLUDE_SET & $add_filter"
        fi
    done

    echo "${EXCLUDE_SET}"
}

function print_and_run_command() {
    command="$1"
    echo "Running: $command"
    eval ${command}
}

function retry_only_tests() {
    FILTERSET=""
    for test_name in "${RETRY_ONLY_TESTS[@]}"; do
        FILTERSET="${FILTERSET} -E 'test(${test_name})'"
    done
    echo "FILTERSET: ${FILTERSET}"

    print_and_run_command "cargo nextest run --config-file .config/nextest.toml --profile ci ${FILTERSET} --test-threads 1 --no-tests=warn ${ENABLE_NO_CAPTURE:+--nocapture}"
}

function rust_crates() {
    # we run this in a subshell to avoid polluting the environment with the variables set in this function
    (
        # Tests written with #[sim_test] are often flaky if run as #[tokio::test] - this var
        # causes #[sim_test] to only run under the deterministic `simtest` job, and not the
        # non-deterministic `test` job.
        export IOTA_SKIP_SIMTESTS=1

        # if no crates were changed, we want to run all tests.
        # because changes that trigger the workflow but which aren't explicitly in a crate can potentially affect the entire workspace
        # mk_test_filterset_changed_crates returns an empty filterset in this case
        FILTERSET="$(mk_test_filterset_changed_crates)"

        EXCLUDE_SET="$(mk_exclude_filterset)"

        if [ -z "$FILTERSET" ]; then
            FILTERSET="-E '$EXCLUDE_SET'"
        else
            FILTERSET="-E '($FILTERSET) & ($EXCLUDE_SET)'"
        fi

        print_and_run_command "cargo nextest run --config-file .config/nextest.toml --profile ci --all-features $FILTERSET --no-tests=warn ${ENABLE_NO_CAPTURE:+--nocapture}"
    )
}

function external_crates() {
    # we run this in a subshell to avoid polluting the environment with the variables set in this function
    (
        # Tests written with #[sim_test] are often flaky if run as #[tokio::test] - this var
        # causes #[sim_test] to only run under the deterministic `simtest` job, and not the
        # non-deterministic `test` job.
        export IOTA_SKIP_SIMTESTS=1

        # if no crates were changed, we want to run all tests.
        # because changes that trigger the workflow but which aren't explicitly in a crate can potentially affect the entire workspace
        # mk_test_filterset_changed_crates returns an empty filterset in this case
        FILTERSET="$(mk_test_filterset_changed_crates)"

        EXCLUDE_SET="$(mk_exclude_filterset_external)"

        if [ -z "$FILTERSET" ]; then
            FILTERSET="-E '$EXCLUDE_SET'"
        else
            FILTERSET="-E '($FILTERSET) & ($EXCLUDE_SET)'"
        fi

        # WARNING: this has a side effect of updating the Cargo.lock file
        print_and_run_command "cargo nextest run --config-file .config/nextest.toml --manifest-path external-crates/move/Cargo.toml --profile ci $FILTERSET --no-tests=warn ${ENABLE_NO_CAPTURE:+--nocapture}"
    )
}

function unused_deps() {
    print_and_run_command "cargo +nightly ci-udeps --all-features"
    print_and_run_command "cargo +nightly ci-udeps --no-default-features"
}

function test_extra() {
    # we run this in a subshell to avoid polluting the environment with the variables set in this function
    (
        # Tests written with #[sim_test] are often flaky if run as #[tokio::test] - this var
        # causes #[sim_test] to only run under the deterministic `simtest` job, and not the
        # non-deterministic `test` job.
        export IOTA_SKIP_SIMTESTS=1
        
        print_and_run_command "cargo run --package iota-benchmark --bin stress -- --log-path ${ROOT}/.cache/stress.log --num-client-threads 10 --num-server-threads 24 --num-transfer-accounts 2 bench --target-qps 100 --num-workers 10 --transfer-object 50 --shared-counter 50 --run-duration 10s --stress-stat-collection"
        print_and_run_command "cargo test --doc"
        print_and_run_command "cargo doc --all-features --workspace --no-deps"
        print_and_run_command "${ROOT}/scripts/execution_layer.py generate-lib"
        print_and_run_command "${ROOT}/scripts/changed-files.sh"
    )
}

function simtests() {
    # we run this in a subshell to avoid polluting the environment with the variables set in this function
    (
        export MSIM_WATCHDOG_TIMEOUT_MS=${MSIM_WATCHDOG_TIMEOUT_MS:-180000}

        FILTERSET=""
        # Example of how to run only a specific test
        #FILTERSET="-E 'test(test_example_function_name)'"
        
        print_and_run_command "scripts/simtest/cargo-simtest simtest --profile ci --color always $FILTERSET --no-tests=warn ${ENABLE_NO_CAPTURE:+--nocapture}"
    )
}

function stress_new_tests_check_for_flakiness() {
    # we run this in a subshell to avoid polluting the environment with the variables set in this function
    (
        export MSIM_WATCHDOG_TIMEOUT_MS=${MSIM_WATCHDOG_TIMEOUT_MS:-180000}
    
        print_and_run_command "scripts/simtest/stress-new-tests.sh ${ENABLE_NO_CAPTURE:+--nocapture}"
    )
}

# restart postgres
function restart_postgres() {
    if ! command -v psql &>/dev/null; then
        echo "'psql' is not installed in PATH. Please ensure it is installed and available."
        exit 1
    fi

    # we run this in a subshell to avoid polluting the environment with the variables set in this function
    (
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
    )
}

function tests_using_postgres() {
    # we run this in a subshell to avoid polluting the environment with the variables set in this function
    (
        # Tests written with #[sim_test] are often flaky if run as #[tokio::test] - this var
        # causes #[sim_test] to only run under the deterministic `simtest` job, and not the
        # non-deterministic `test` job.
        export IOTA_SKIP_SIMTESTS=1

        if [ "$RESTART_POSTGRES" == "true" ]; then restart_postgres; fi

        NEXTEST_RUN="cargo nextest run --config-file .config/nextest.toml --features pg_integration"
        print_and_run_command "$NEXTEST_RUN -p iota-graphql-rpc --test-threads 1 --test e2e_tests --test examples_validation_tests ${ENABLE_NO_CAPTURE:+--nocapture}"
        print_and_run_command "$NEXTEST_RUN -p iota-graphql-rpc --test-threads 1 --lib -- test_query_cost ${ENABLE_NO_CAPTURE:+--nocapture}"
        print_and_run_command "$NEXTEST_RUN -p iota-graphql-e2e-tests --test-threads 8 ${ENABLE_NO_CAPTURE:+--nocapture}"
        print_and_run_command "$NEXTEST_RUN -p iota-cluster-test --test-threads 1 --test local_cluster_test ${ENABLE_NO_CAPTURE:+--nocapture}"
        print_and_run_command "$NEXTEST_RUN -p iota-indexer --test-threads 1 --test ingestion_tests ${ENABLE_NO_CAPTURE:+--nocapture}"
        # Iota-indexer's RPC tests, which depend on a shared runtime, are incompatible with nextest due to its process-per-test execution model.
        # cargo test, on the other hand, allows tests to share state and resources by default.
        print_and_run_command "cargo test --profile simulator --package iota-indexer --test rpc-tests --features shared_test_runtime ${ENABLE_NO_CAPTURE:+--nocapture}"
    )
}

function audit_deps() {
    local MANIFEST_PATH=${MANIFEST_PATH:-"./Cargo.toml"}
    print_and_run_command "cargo deny --manifest-path "$MANIFEST_PATH" check bans licenses sources"
    # check security advisories (in-house crates)
    print_and_run_command "cargo deny --manifest-path "$MANIFEST_PATH" check advisories"
}

function audit_deps_external() {
   print_and_run_command "MANIFEST_PATH="./external-crates/move/Cargo.toml" audit_deps"
}

function move_examples_rdeps_tests() {
    # we run this in a subshell to avoid polluting the environment with the variables set in this function
    (
        # Tests written with #[sim_test] are often flaky if run as #[tokio::test] - this var
        # causes #[sim_test] to only run under the deterministic `simtest` job, and not the
        # non-deterministic `test` job.
        export IOTA_SKIP_SIMTESTS=1

        FILTERSET="$(mk_move_examples_rdeps_tests_filterset)"

        if [ -n "$FILTERSET" ]; then
            FILTERSET="-E '($FILTERSET)'"
        fi

        print_and_run_command "cargo nextest run --config-file .config/nextest.toml --profile ci $FILTERSET --no-tests=warn ${ENABLE_NO_CAPTURE:+--nocapture}"
    )
}

function move_examples_rdeps_simtests() {
    # we run this in a subshell to avoid polluting the environment with the variables set in this function
    (
        export MSIM_WATCHDOG_TIMEOUT_MS=${MSIM_WATCHDOG_TIMEOUT_MS:-180000}

        FILTERSET="$(mk_move_examples_rdeps_tests_filterset)"

        if [ -n "$FILTERSET" ]; then
            FILTERSET="-E '($FILTERSET)'"
        fi

        print_and_run_command "scripts/simtest/cargo-simtest simtest --profile ci --color always $FILTERSET --no-tests=warn ${ENABLE_NO_CAPTURE:+--nocapture}"
    )    
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
