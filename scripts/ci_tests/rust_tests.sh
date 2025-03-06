#!/bin/bash -e
ROOT=$(git rev-parse --show-toplevel || realpath "$(dirname "$0")/../..")

#
# INPUTS
#

# Running all the tests will compile different sets of crates and take a lot of storage (>500GB)
# If your machine has less storage, you can run only part of the tests (at a time),
# use the name of the function to run as a subcommand, for instance:
# ./scripts/tests_like_ci/rust_tests.sh simtests

# The next line sets "RUN_ONLY_STEP" to the first argument passed to the script, or to the value of the environment variable "RUN_ONLY_STEP" if the first argument is not set.
# If neither the first argument nor the environment variable are set, "RUN_ONLY_STEP" is set to an empty string.
export RUN_ONLY_STEP=${1:-${RUN_ONLY_STEP:-}}

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

# the possible steps for RUN_ONLY_STEP are:
VALID_STEPS=(
    "unused_deps"
    "test_extra"
    "stress_new_tests_check_for_flakiness"
    "audit_deps"
    "audit_deps_external"
    "run_tests"
    "run_simtests"
    "rust_crates"
    "external_crates"
    "simtests"
    "tests_using_postgres"
    "move_examples_rdeps_tests"
    "move_examples_rdeps_simtests"
)

EXCLUDE_SET_EXTERNAL=(
    "test(prove)"
    "test(run_all::simple_build_with_docs/args.txt)"
    "test(run_test::nested_deps_bad_parent/Move.toml)"
)

TEST_TYPE_NEXTEST="nextest"
TEST_TYPE_SIMTEST="simtest"

# filter_set for tests that depend on Postgres
FILTERSET_TESTS_PORTGRES=(
    "(package(iota-graphql-rpc) and (binary(e2e_tests) or binary(examples_validation_tests) or test(test_query_cost)))"
    "package(iota-graphql-e2e-tests)"
    "(package(iota-cluster-test) and binary(local_cluster_test))"
    "(package(iota-indexer) and (binary(ingestion_tests) or binary(rpc-tests)))"
)

# filter_set for tests that depend on the Move examples
# iota-test-transaction-builder + iota-core provide functions that publish packages from the Move examples for other crates to use.
# iota-framework-tests, iota-json, iota-json-rpc-tests, iota-rosetta use the Move examples directly as part of their tests.
FILTERSET_TESTS_MOVE_EXAMPLES_RDEPS=(
    "rdeps(iota-test-transaction-builder)"
    "rdeps(iota-core)"
    "package(iota-framework-tests)"
    "(package(iota-json) and test(test_basic_args_linter_top_level))"
    "(package(iota-json-rpc-tests) and (test(try_get_past_object_deleted) or test(test_publish)))"
    "(package(iota-rosetta) and test(test_publish_and_move_call))"
)

# search_changed_crates returns the crates that have changed compared to origin/develop
function search_changed_crates() {
    if ! yq --version | grep -q "v4." 2>/dev/null; then
        echo -e "\033[31m'yq' v4.0+ is not installed in PATH. Please ensure you installed \033[92myq v4.0+.\033[0m" >&2
        if [ "$(uname -s)" == "Linux" ]; then echo -e "On Ubuntu/Linux via snap: \033[92msnap install yq\033[0m" >&2; fi
        if [ "$(uname -s)" == "Darwin" ]; then echo -e "On MacOS via Brew: \033[92mbrew install yq\033[0m" >&2; fi
        echo -e "More installation options at https://github.com/mikefarah/yq/#install" >&2
        exit 1
    fi

    # assuming PRs merge into origin/develop, we diff the current branch with origin/develop
    local changed_files=$(git diff --name-only origin/develop..HEAD)
    local crates_filters_yml="${ROOT}/.github/crates-filters.yml"

    local tuples_crate_name_path=$(yq -r 'to_entries[] | .key + " " + (.value[] | sub("/\\*\\*$",""))' $crates_filters_yml)

    local matching_crates=()
    while IFS= read -r tuple; do
        crate_name=$(echo "$tuple" | cut -d' ' -f1)
        crate_path_starts_with=$(echo "$tuple" | cut -d' ' -f2)
        for changed_file in $changed_files; do
            if [[ "$changed_file" == "$crate_path_starts_with"* ]]; then
                matching_crates+=($crate_name)
            fi
        done
    done <<<"$tuples_crate_name_path"

    echo "${matching_crates[@]}" | tr ' ' '\n' | sort -u | tr '\n' ' '
}

# print_and_run_command prints the command and then runs it
function print_and_run_command() {
    command="$1"
    echo "Running: $command"
    eval ${command}
}

# append_filter appends a filter with "or" condition to the filter set
function append_filter_item_or() {
    local filter_set="${1:-}" # filter set
    local item="${2:-}"       # item to append

    if [ -z "$item" ]; then
        echo "$filter_set"
    else
        if [ -z "$filter_set" ]; then
            echo "$item"
        else
            echo "$filter_set or $item"
        fi
    fi
}

# append_filter_item_and appends a filter with "and" condition to the filter set
function append_filter_item_and() {
    local filter_set="${1:-}" # filter set
    local item="${2:-}"       # item to append

    if [ -z "$item" ]; then
        echo "$filter_set"
    else
        if [ -z "$filter_set" ]; then
            echo "$item"
        else
            echo "$filter_set and $item"
        fi
    fi
}

# build_filterset_included builds a filter set for tests that should be included
function build_filterset_included() {
    local items=("$@")

    local filter_set=""
    for item in "${items[@]}"; do
        # skip if item is empty
        if [ -z "$item" ]; then
            continue
        fi

        filter_set=$(append_filter_item_or "$filter_set" "$item")
    done

    echo "$filter_set"
}

# build_filterset_included_rdeps builds a filter set for tests that should be included,
# based on the rdeps of the given items
function build_filterset_included_rdeps() {
    local items=("$@")

    local filter_set=""
    for item in "${items[@]}"; do
        # skip if item is empty
        if [ -z "$item" ]; then
            continue
        fi

        filter_set=$(append_filter_item_or "$filter_set" "rdeps($item)")
    done

    echo "$filter_set"
}

# build_filterset_changed_crates builds a filter set for tests that should be included
# based on the crates that have changed, either given or searched if the variable is unset.
# If no crates have changed, an empty filter set is returned, because we want to run all tests in that case.
function build_filterset_changed_crates() {
    local test_only_changed_crates="${1:false}"
    local changed_crates="${2:-}"

    if [ "$test_only_changed_crates" == "false" ]; then
        # test all crates (return empty filter_set)
        return
    fi

    # detected changed crates if "changed_crates" variable is unset (achieved by "+x")
    if [ -z "${changed_crates+x}" ]; then
        changed_crates=$(search_changed_crates)
    fi

    # if no crates were changed, we want to run all tests.
    # because changes that trigger the workflow but which aren't explicitly in a crate can potentially affect the entire workspace
    # returning an empty filter_set does that
    echo $(build_filterset_included_rdeps "${changed_crates}")
}

# build_filterset_excluded builds a filter set for tests that should be excluded
function build_filterset_excluded() {
    local items=("$@")

    local filter_set=""
    for item in "${items[@]}"; do
        # skip if item is empty
        if [ -z "$item" ]; then
            continue
        fi
        
        filter_set=$(append_filter_item_and "$filter_set" "!($item)")
    done

    echo "$filter_set"
}

# build_filterset_combined builds a filter set combining the filter set and exclude set.
function build_filterset_combined() {
    local filter_set="${1:-}"  # First parameter is stored in filter_set
    local exclude_set="${2:-}" # Second parameter is stored in exclude_set

    local combined_set=""

    # Check if filter_set is not empty
    if [[ -n "$filter_set" ]]; then
        combined_set="$filter_set"
    fi

    # Check if exclude_set is not empty and append it with 'and'
    if [[ -n "$exclude_set" ]]; then
        if [[ -n "$combined_set" ]]; then
            combined_set="($combined_set) and ($exclude_set)"
        else
            combined_set="$exclude_set"
        fi
    fi

    echo "$combined_set"
}

# build_filterset_tests builds a combined filter set for tests based on the given conditions
# run_rust_tests: run tests for rust crates
# run_tests_using_postgres: run tests that depend on Postgres
# run_move_examples_rdeps_tests: run tests that depend on the Move examples
# test_only_changed_crates: run tests only for the crates that have changed
# changed_crates_rust: the list of changed crates for rust
function build_filterset_tests() {
    local run_rust_tests=${1:-false}
    local run_tests_using_postgres=${2:-false}
    local run_move_examples_rdeps_tests=${3:-false}
    local test_only_changed_crates=${4:-false}
    local changed_crates_rust=${5:-}

    local filter_set=""
    local exclude_set=""

    if [ "$run_rust_tests" == "true" ]; then
        local changed_crates_rust_filter=$(build_filterset_changed_crates "${test_only_changed_crates}" "${changed_crates_rust}")
        filter_set=$(append_filter_item_or "$filter_set" "$changed_crates_rust_filter")
    fi

    if [ "$run_tests_using_postgres" == "true" ]; then
        local postgres_tests_filter=$(build_filterset_included "${FILTERSET_TESTS_PORTGRES[@]}")
        filter_set=$(append_filter_item_or "$filter_set" "$postgres_tests_filter")
    else
        local postgres_tests_exclude_filter=$(build_filterset_excluded "${FILTERSET_TESTS_PORTGRES[@]}")
        exclude_set=$(append_filter_item_and "$exclude_set" "$postgres_tests_exclude_filter")
    fi

    if [ "$run_move_examples_rdeps_tests" == "true" ]; then
        local move_examples_rdeps_tests_filter=$(build_filterset_included "${FILTERSET_TESTS_MOVE_EXAMPLES_RDEPS[@]}")
        filter_set=$(append_filter_item_or "$filter_set" "$move_examples_rdeps_tests_filter")
    fi

    echo "$(build_filterset_combined "$filter_set" "$exclude_set")"
}

# restart postgres docker container and create the iota_indexer database
function restart_postgres_docker() {
    if ! command -v psql &>/dev/null; then
        echo "'psql' is not installed in PATH. Please ensure it is installed and available."
        exit 1
    fi
    if ! command -v pg_isready &>/dev/null; then 
        echo "'pg_isready' is not installed in PATH. Please ensure it is installed and available."
        exit 1
    fi 
    function await_postgres() {
        export POSTGRES_PORT=${POSTGRES_PORT-5432}
        while ! [ -n "`pg_isready -h 0.0.0.0 -p $POSTGRES_PORT | grep "accepting"`" ]; do
            echo "waiting on postgres (port $POSTGRES_PORT)..."; sleep 0.3;
        done
    }

    # we run this in a subshell to avoid polluting the environment with the variables set in this function
    (
        docker rm -f -v $(docker ps -a | grep postgres | awk '{print $1}')
        export POSTGRES_PASSWORD=${POSTGRES_PASSWORD:-postgrespw}
        export POSTGRES_USER=${POSTGRES_USER:-postgres}
        export POSTGRES_DB=${POSTGRES_DB:-iota_indexer}
        export POSTGRES_HOST=${POSTGRES_HOST:-postgres}
        export PGPASSWORD="${POSTGRES_PASSWORD}" # for psql
        # assuming you run the indexer's postgres using docker-compose
        cd ${ROOT}/dev-tools/pg-services-local
        docker-compose down -v postgres
        docker-compose up -d postgres
        await_postgres
        echo "SELECT 'CREATE DATABASE $POSTGRES_DB' WHERE NOT EXISTS (SELECT FROM pg_database WHERE datname = '$POSTGRES_DB')\gexec" | psql -h localhost -U $POSTGRES_USER
        psql -h localhost -U $POSTGRES_USER -c 'ALTER SYSTEM SET max_connections = 500;'
    )
}

# finalize_filter_set appends "-E" to the beginning of the string if it is not empty
function finalize_filter_set() {
    local filter_set="${1:-}" # filter set for tests (first parameter)

    # If filter_set is not empty, append "-E" to the beginning of the string
    if [[ -n "$filter_set" ]]; then
        echo "-E '$filter_set'"
    else
        echo ""
    fi
}

# run_cargo_nextest runs cargo-nextest with the given filter set, config path and manifest path
function run_cargo_nextest() {
    # we run this in a subshell to avoid polluting the environment with the variables set in this function
    (
        local filter_set="${1:-}"                        # filter set for tests (first parameter)
        local config_path="${2:-'.config/nextest.toml'}" # config path for tests (second parameter)
        local manifest_path="${3:-}"                     # manifest path for tests (third parameter)

        # if config path is not empty, set it to --config-file flag
        if [[ -n "$config_path" ]]; then
            config_path="--config-file $config_path"
        fi

        # if manifest path is not empty, set it to --manifest-path flag
        if [[ -n "$manifest_path" ]]; then
            manifest_path="--manifest-path $manifest_path"
        fi

        # Tests written with #[sim_test] are often flaky if run as #[tokio::test] - this var
        # causes #[sim_test] to only run under the deterministic `simtest` job, and not the
        # non-deterministic `test` job.
        export IOTA_SKIP_SIMTESTS=1
        
        local filter_set=$(finalize_filter_set "$filter_set")
        
        print_and_run_command "cargo nextest run $config_path $manifest_path --profile ci --all-features $filter_set --no-tests=warn ${ENABLE_NO_CAPTURE:+--nocapture}"
    )
}

# run_cargo_simtest runs cargo-simtest with the given filter set and exclude set
function run_cargo_simtest() {
    # we run this in a subshell to avoid polluting the environment with the variables set in this function
    (
        local filter_set="${1:-}" # filter set for tests (first parameter))

        export MSIM_WATCHDOG_TIMEOUT_MS=${MSIM_WATCHDOG_TIMEOUT_MS:-180000}

        local filter_set=$(finalize_filter_set "$filter_set")
        
        print_and_run_command "scripts/simtest/cargo-simtest simtest --profile ci --color always $filter_set --no-tests=warn ${ENABLE_NO_CAPTURE:+--nocapture}"
    )
}

# run cargo-udeps to check for unused dependencies
function unused_deps() {
    print_and_run_command "cargo +nightly ci-udeps --all-features"
    print_and_run_command "cargo +nightly ci-udeps --no-default-features"
}

# run extra tests like stresstest, doc tests, doc generation, changed files, etc.
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

# run stress tests for new tests to check for flakiness
function stress_new_tests_check_for_flakiness() {
    # we run this in a subshell to avoid polluting the environment with the variables set in this function
    (
        export MSIM_WATCHDOG_TIMEOUT_MS=${MSIM_WATCHDOG_TIMEOUT_MS:-180000}

        print_and_run_command "scripts/simtest/stress-new-tests.sh ${ENABLE_NO_CAPTURE:+--nocapture}"
    )
}

# audit dependencies
function audit_deps() {
    local manifest_path=${MANIFEST_PATH:-"./Cargo.toml"}
    print_and_run_command "cargo deny --manifest-path "$manifest_path" check bans licenses sources"
    # check security advisories (in-house crates)
    print_and_run_command "cargo deny --manifest-path "$manifest_path" check advisories"
}

# audit external dependencies
function audit_deps_external() {
    print_and_run_command "MANIFEST_PATH="./external-crates/move/Cargo.toml" audit_deps"
}

function filter_and_run_tests() {
    local test_type="${1}"
    shift

    if [ "$test_type" != $TEST_TYPE_NEXTEST ] && [ "$test_type" != $TEST_TYPE_SIMTEST ]; then
        echo "Invalid test type specified. Use '$TEST_TYPE_NEXTEST' or '$TEST_TYPE_SIMTEST'."
        exit 1
    fi

    local run_rust_tests=${CI_IS_RUST:-false}
    local run_external_crates=${CI_IS_EXTERNAL_CRATES:-false}
    local run_tests_using_postgres=${CI_IS_PG_INTEGRATION:-false}
    local run_move_examples_rdeps_tests=${CI_IS_MOVE_EXAMPLE_USED_BY_OTHERS:-false}
    local test_only_changed_crates=${TEST_ONLY_CHANGED_CRATES:-false}
    local changed_crates_rust=${CI_CHANGED_CRATES}
    local changed_crates_external=${CI_CHANGED_EXTERNAL_CRATES}
    local restart_postgres=${RESTART_POSTGRES:-false}

    # check if all conditions are false and early return
    if [ "$run_rust_tests" == "false" ] && [ "$run_external_crates" == "false" ] && [ "$run_tests_using_postgres" == "false" ] && [ "$run_move_examples_rdeps_tests" == "false" ]; then
        echo "No conditions are set to run tests. Exiting."
        exit 1
    fi

    # check if external crates are set
    if [ "$run_external_crates" == "true" ]; then
        local changed_crates_external_filter=$(build_filterset_changed_crates "${test_only_changed_crates}" "${changed_crates_external}")
        local exclude_set_external=$(build_filterset_excluded "${EXCLUDE_SET_EXTERNAL[@]}")
        local combined_set_external=$(build_filterset_combined "$changed_crates_external_filter" "$exclude_set_external")

        # first run tests for external crates (they are not part of the workspace)
        if [ "$test_type" == $TEST_TYPE_NEXTEST ]; then
            run_cargo_nextest "$combined_set_external" ".config/nextest_external.toml" "external-crates/move/Cargo.toml"
        elif [ "$test_type" == $TEST_TYPE_SIMTEST ]; then
            run_cargo_simtest "$combined_set_external" "external-crates/move/Cargo.toml"
        fi
    fi

    # check again if any of the other conditions are set, in case only external crates were set
    if [ "$run_rust_tests" == "false" ] && [ "$run_tests_using_postgres" == "false" ] && [ "$run_move_examples_rdeps_tests" == "false" ]; then
        exit 0
    fi

    local combined_set=$(build_filterset_tests "$run_rust_tests" "$run_tests_using_postgres" "$run_move_examples_rdeps_tests" "$test_only_changed_crates" "$changed_crates_rust")

    # check if a restart of postgres is needed
    if [ "$run_tests_using_postgres" == "true" ] && [ "$restart_postgres" == "true" ]; then
        restart_postgres_docker
    fi

    # run tests
    if [ "$test_type" == $TEST_TYPE_NEXTEST ]; then
        run_cargo_nextest "$combined_set"
    elif [ "$test_type" == $TEST_TYPE_SIMTEST ]; then
        run_cargo_simtest "$combined_set"
    fi

    if [ "$test_type" == $TEST_TYPE_NEXTEST ] && [ "$run_tests_using_postgres" == "true" ]; then
        # Iota-indexer's RPC tests, which depend on a shared runtime, are incompatible with nextest due to its process-per-test execution model.
        # cargo test, on the other hand, allows tests to share state and resources by default.
        print_and_run_command "cargo test --profile simulator --package iota-indexer --test rpc-tests --all-features ${ENABLE_NO_CAPTURE:+--nocapture}"
    fi
}

function run_tests() {
    filter_and_run_tests $TEST_TYPE_NEXTEST
}

function run_simtests() {
    filter_and_run_tests $TEST_TYPE_SIMTEST
}

function rust_crates() {
    # we run this in a subshell to avoid polluting the environment with the variables set in this function
    (
        export CI_IS_RUST=true
        export CI_CHANGED_CRATES=${CI_CHANGED_CRATES}

        run_tests
    )
}

function external_crates() {
    # we run this in a subshell to avoid polluting the environment with the variables set in this function
    (
        export CI_IS_EXTERNAL_CRATES=true
        export CI_CHANGED_EXTERNAL_CRATES=${CI_CHANGED_EXTERNAL_CRATES}

        run_tests
    )
}

function simtests() {
    # we run this in a subshell to avoid polluting the environment with the variables set in this function
    (
        export CI_IS_RUST=true
        export CI_CHANGED_CRATES=${CI_CHANGED_CRATES}

        run_simtests
    )
}

function tests_using_postgres() {
    # we run this in a subshell to avoid polluting the environment with the variables set in this function
    (
        export CI_IS_PG_INTEGRATION=true

        run_tests
    )
}

function move_examples_rdeps_tests() {
    # we run this in a subshell to avoid polluting the environment with the variables set in this function
    (
        export CI_IS_MOVE_EXAMPLE_USED_BY_OTHERS=true

        run_tests
    )
}

function move_examples_rdeps_simtests() {
    # we run this in a subshell to avoid polluting the environment with the variables set in this function
    (
        export CI_IS_MOVE_EXAMPLE_USED_BY_OTHERS=true

        run_simtests
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
        echo "Running step: $step"
        $step
    done
fi
