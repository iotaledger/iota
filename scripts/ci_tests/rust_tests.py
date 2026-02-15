#!/usr/bin/env python3
"""
Rust Test Orchestration Script

This script manages different types of Rust tests:
- Regular tests (nextest) 
- Simulation tests (simtest)
- External crate tests
- PostgreSQL integration tests
- Selective testing based on changed crates
"""

import argparse
import logging
import os
import re
import subprocess
import sys
import time
from pathlib import Path
from typing import Dict, List, Optional

class RustTestOrchestrator:
    """Main class for orchestrating Rust tests with various configurations."""
    
    # valid test steps that can be run
    VALID_STEPS = [
        "run_tests",
        "run_simtests", 
        "rust_crates",
        "external_crates",
        "tests_using_postgres",
        "simtests",
        "stress_new_tests_check_for_flakiness",
        "move_examples_rdeps_tests",
        "move_examples_rdeps_simtests",
        "test_extra",
        "unused_deps",
        "audit_deps",
        "audit_deps_external"
    ]

    TEST_TYPE_NEXTEST="nextest"
    TEST_TYPE_SIMTEST="simtest"

    EXCLUDE_SET_EXTERNAL = [
        "test(prove)",
        "test(run_all::simple_build_with_docs/args.txt)",
        "test(run_test::nested_deps_bad_parent/Move.toml)"
    ]
    
    # filter_set for tests that depend on postgres and "pg_integration" feature
    FILTERSET_TESTS_POSTGRES_PG_INTEGRATION = [
        "(package(iota-cluster-test) and (test(test_iota_cluster)))",
        "(package(iota-graphql-e2e-tests) and (binary(tests)))",
        "(package(iota-graphql-rpc) and (binary(e2e_tests) or (test(test_query_cost)) or binary(examples_validation_tests)))",
        "(package(iota-indexer) and (binary(ingestion_tests)))"
    ]
    
    # filter_set for tests that depend on postgres and "shared_test_runtime" feature.
    # those tests are incompatible with nextest due to their shared state and should be run with "cargo test"
    FILTERSET_TESTS_POSTGRES_SHARED_TEST_RUNTIME = [
        "(package(iota-indexer) and (binary(rpc-tests)))"
    ]
    
    # filter_set for tests that depend on the Move examples
    # iota-test-transaction-builder + iota-core provide functions that publish packages from the Move examples for other crates to use.
    # iota-framework-tests, iota-json, iota-json-rpc-tests, iota-rosetta use the Move examples directly as part of their tests.
    FILTERSET_TESTS_MOVE_EXAMPLES_RDEPS = [
        "rdeps(iota-test-transaction-builder)",
        "rdeps(iota-core)",
        "package(iota-framework-tests)",
        "(package(iota-json) and test(test_basic_args_linter_top_level))",
        "(package(iota-json-rpc-tests) and (test(try_get_past_object_deleted) or test(test_publish)))",
        "(package(iota-rosetta) and test(test_publish_and_move_call))"
    ]
    
    # initialize the orchestrator with environment configuration
    def __init__(self):
        self.setup_logging()
        self.root_dir = self._get_root_directory()
        self.env_config = self._load_environment_config(os.environ)
        
    # setup_logging configures logging for the script.
    def setup_logging(self) -> None:
        logging.basicConfig(
            level=logging.INFO,
            format='%(message)s',
            stream=sys.stdout
        )
        self.logger = logging.getLogger(__name__)
        
    # get the repository root directory
    def _get_root_directory(self) -> Path:
        try:
            # Try git rev-parse first
            result = subprocess.run(
                ["git", "rev-parse", "--show-toplevel"],
                capture_output=True,
                text=True,
                check=True
            )
            return Path(result.stdout.strip())
        except (subprocess.CalledProcessError, FileNotFoundError):
            # Fallback to script directory navigation
            script_dir = Path(__file__).parent
            return script_dir.parent.parent
            
    # load and validate environment configuration
    def _load_environment_config(self, env_source: Dict[str, str]) -> Dict[str, str]:
        if env_source is None:
            env_source = os.environ

        def get_env_bool(key: str, default: bool = False) -> bool:
            value = env_source.get(key, str(default).lower())
            return value.lower() in ('true', '1', 'yes')
            
        def get_env_str(key: str, default: str = "") -> str:
            return env_source.get(key, default)
            
        def get_env_int(key: str, default: int) -> int:
            try:
                return int(env_source.get(key, str(default)))
            except ValueError:
                return default
        
        return {
            # Test execution control
            
            # CI will only test crates that have changed in the PR
            # For local tests, tests all crates by default. Override with TEST_ONLY_CHANGED_CRATES=true
            'test_only_changed_crates': get_env_bool('TEST_ONLY_CHANGED_CRATES', False),

            # CI uses postgres provided via a github CI service. It needs to be able to not restart postgres.
            # Locally, this script restarts postgres by default. Override by passing RESTART_POSTGRES=false
            # only the tests that need postgres will automatically (re-)start it
            'restart_postgres': get_env_bool('RESTART_POSTGRES', True),
            
            # Test type flags
            'ci_is_rust': get_env_bool('CI_IS_RUST', False),
            'ci_is_external_crates': get_env_bool('CI_IS_EXTERNAL_CRATES', False),
            'ci_is_pg_integration': get_env_bool('CI_IS_PG_INTEGRATION', False),
            'ci_is_move_example_used_by_others': get_env_bool('CI_IS_MOVE_EXAMPLE_USED_BY_OTHERS', False),
            
            # Changed crates lists
            
            # CI uses an action to detect changed_crates. It needs to be able to override changed crates with the ones detected by that action.
            # Override with CHANGED_CRATES.
            # Locally, you don't need to provide this variable, this script will detect changed crates.
            # Format of CHANGED_CRATES: one string, space-separated: CHANGED_CRATES="crate1 crate2 crate3" ./this_script.sh
            'ci_changed_crates': get_env_str('CI_CHANGED_CRATES'),
            'changed_crates_rust_given': 'CI_CHANGED_CRATES' in env_source,
            'ci_changed_external_crates': get_env_str('CI_CHANGED_EXTERNAL_CRATES'),
            'changed_crates_external_given': 'CI_CHANGED_EXTERNAL_CRATES' in env_source,
            
            # PostgreSQL configuration
            'postgres_password': get_env_str('POSTGRES_PASSWORD', 'postgrespw'),
            'postgres_user': get_env_str('POSTGRES_USER', 'postgres'),
            'postgres_db': get_env_str('POSTGRES_DB', 'iota_indexer'),
            'postgres_host': get_env_str('POSTGRES_HOST', 'postgres'),
            'postgres_port': get_env_int('POSTGRES_PORT', 5432),
            
            # Test execution settings
            'msim_watchdog_timeout_ms': get_env_int('MSIM_WATCHDOG_TIMEOUT_MS', 180000),
            'enable_no_capture': get_env_bool('ENABLE_NO_CAPTURE', False),
            'manifest_path': get_env_str('MANIFEST_PATH', './Cargo.toml'),
        }
    
    # parse the crates-filters.yml file using regex.
    def parse_crates_filters(self, yaml_path: Path) -> Dict[str, List[str]]:
        crate_mappings = {}
        current_crate = None
        
        try:
            with open(yaml_path, 'r') as f:
                content = f.read()
        except FileNotFoundError:
            self.logger.error(f"Crates filter file not found: {yaml_path}")
            return {}
            
        for line in content.split('\n'):
            line = line.rstrip()
            if not line or line.startswith('#'):
                continue
                
            # Match crate name (key at start of line)
            crate_match = re.match(r'^([a-zA-Z0-9_-]+):\s*$', line)
            if crate_match:
                current_crate = crate_match.group(1)
                crate_mappings[current_crate] = []
                continue
                
            # Match path entry (indented with - "path/**)
            path_match = re.match(r'^\s*-\s*"([^"]+)"\s*$', line)
            if path_match and current_crate:
                path = path_match.group(1)
                # Remove trailing /** if present
                path = re.sub(r'/\*\*$', '', path)
                crate_mappings[current_crate].append(path)
                
        return crate_mappings
    
    # find crates that have changed by comparing current branch with origin/develop.
    def search_changed_crates(self) -> List[str]:
        try:
            # Log that we are using the fallback method to detect changed crates
            self.logger.info("Detecting changed crates by comparing with origin/develop...")

            # Get changed files
            result = subprocess.run(
                ["git", "diff", "--name-only", "origin/develop..HEAD"],
                capture_output=True,
                text=True,
                check=True,
                cwd=self.root_dir
            )
            changed_files = [f.strip() for f in result.stdout.split('\n') if f.strip()]
            
            # Load crate mappings
            crates_filters_path = self.root_dir / '.github' / 'crates-filters.yml'
            crate_mappings = self.parse_crates_filters(crates_filters_path)
            
            # Find matching crates
            matching_crates = set()
            for crate_name, paths in crate_mappings.items():
                for path_prefix in paths:
                    for changed_file in changed_files:
                        if changed_file.startswith(path_prefix):
                            matching_crates.add(crate_name)
                            break
            
            # Log detected changed crates
            if matching_crates:
                self.logger.info(f"Detected changed crates: {', '.join(sorted(matching_crates))}")
            else:
                self.logger.info("No changed crates detected.")
            
            return sorted(list(matching_crates))
            
        except subprocess.CalledProcessError as e:
            self.logger.error(f"Failed to get changed files from git: {e}")
            return []
        except Exception as e:
            self.logger.error(f"Error detecting changed crates: {e}")
            return []
    
    # print command and execute it, returning exit code
    def print_and_run_command(self, command: str, env: Optional[Dict[str, str]] = None) -> int:
        self.logger.info(f"Running: {command}")
        
        # Prepare environment
        exec_env = os.environ.copy()
        if env:
            exec_env.update(env)
        
        # Execute command
        result = subprocess.run(
            command,
            shell=True,
            env=exec_env,
            cwd=self.root_dir
        )
        return result.returncode
    
    # append_filter appends a filter with "or" condition to the filter set
    def append_filter_item_or(self, filter_set: str, item: str) -> str:
        if not item:
            return filter_set
        if not filter_set:
            return item
        return f"{filter_set} or {item}"
    
    # append_filter_item_and appends a filter with "and" condition to the filter set
    def append_filter_item_and(self, filter_set: str, item: str) -> str:
        if not item:
            return filter_set
        if not filter_set:
            return item
        return f"{filter_set} and {item}"
    
    # build_filterset_included builds a filter set for tests that should be included
    def build_filterset_included(self, items: List[str]) -> str:
        filter_set = ""
        for item in items:
            if item:  # Skip empty items
                filter_set = self.append_filter_item_or(filter_set, item)
        return filter_set
    
    # build_filterset_included_rdeps builds a filter set for tests that should be included,
    # based on the rdeps of the given items
    def build_filterset_included_rdeps(self, items: List[str]) -> str:
        filter_set = ""
        for item in items:
            if item:  # Skip empty items
                filter_set = self.append_filter_item_or(filter_set, f"rdeps({item})")
        return filter_set
        
    # build_filterset_excluded builds a filter set for tests that should be excluded
    def build_filterset_excluded(self, items: List[str]) -> str:
        filter_set = ""
        for item in items:
            if item:  # Skip empty items
                filter_set = self.append_filter_item_and(filter_set, f"!({item})")
        return filter_set
    
    # build_filterset_combined builds a filter set combining the filter set and exclude set.
    def build_filterset_combined(self, include_set: str, exclude_set: str) -> str:
        if include_set and exclude_set:
            return f"({include_set}) and ({exclude_set})"
        elif include_set:
            return include_set
        elif exclude_set:
            return exclude_set
        else:
            return ""
    
    # build_filterset_changed_crates builds a filter set for tests that should be included
    # based on the crates that have changed, either given or searched if the variable is unset.
    # If no crates have changed, an empty filter set is returned, because we want to run all tests in that case.
    def build_filterset_changed_crates(self, test_only_changed_crates: bool, 
                                     changed_crates: str, changed_crates_given: bool) -> str:
        if not test_only_changed_crates:
            # test all crates (return empty filter_set)
            return ""
            
        # detected changed crates if "changed_crates" variable is empty,
        # and the changed crates were not given.
        if not changed_crates and not changed_crates_given:
            detected_crates = self.search_changed_crates()
            changed_crates = " ".join(detected_crates)
        
        if changed_crates:
            crate_list = [c.strip() for c in changed_crates.split() if c.strip()]
            return self.build_filterset_included_rdeps(crate_list)
        
        # if no crates were changed, we want to run all tests.
        # because changes that trigger the workflow but which aren't explicitly in a crate can potentially affect the entire workspace
        # returning an empty filter_set does that
        return ""
    
    # build_filterset_tests builds a combined filter set for tests based on the given conditions
    # run_rust_tests: run tests for rust crates
    # run_tests_using_postgres: run tests that depend on Postgres
    # run_move_examples_rdeps_tests: run tests that depend on the Move examples
    # test_only_changed_crates: run tests only for the crates that have changed
    # changed_crates_rust: the list of changed crates for rust
    def build_filterset_tests(self, run_rust_tests: bool, run_tests_using_postgres: bool,
                            run_move_examples_rdeps_tests: bool, test_only_changed_crates: bool,
                            changed_crates_rust: str, changed_crates_rust_given: bool) -> str:
        filter_set = ""
        
        # we always exclude the following tests, because they need shared state and are incompatible with nextest.
        # they are run separately after the nextest tests via "cargo test"
        exclude_set = self.build_filterset_excluded(self.FILTERSET_TESTS_POSTGRES_SHARED_TEST_RUNTIME)
        
        if run_rust_tests:
            changed_crates_rust_filter = self.build_filterset_changed_crates(
                test_only_changed_crates, changed_crates_rust, changed_crates_rust_given
            )
            filter_set = self.append_filter_item_or(filter_set, changed_crates_rust_filter)
        
        if run_tests_using_postgres:
            postgres_tests_filter = self.build_filterset_included(self.FILTERSET_TESTS_POSTGRES_PG_INTEGRATION)
            filter_set = self.append_filter_item_or(filter_set, postgres_tests_filter)
        else:
            postgres_tests_exclude_filter = self.build_filterset_excluded(self.FILTERSET_TESTS_POSTGRES_PG_INTEGRATION)
            exclude_set = self.append_filter_item_and(exclude_set, postgres_tests_exclude_filter)
        
        if run_move_examples_rdeps_tests:
            move_examples_rdeps_tests_filter = self.build_filterset_included(self.FILTERSET_TESTS_MOVE_EXAMPLES_RDEPS)
            filter_set = self.append_filter_item_or(filter_set, move_examples_rdeps_tests_filter)
        
        return self.build_filterset_combined(filter_set, exclude_set)
    
    # finalize_filter_set appends "-E" to the beginning of the string if it is not empty
    def finalize_filter_set(self, filter_set: str) -> str:
        if filter_set:
            return f"-E '{filter_set}'"
        return ""
    
    # check_postgres_tool_available checks if a required tool for postgres handling is available, and exits with an error if not.
    def check_postgres_tool_available(self, tool: str, error_msg: str) -> None:
        try:
            subprocess.run([tool, "--version"], capture_output=True, check=True)
        except (subprocess.CalledProcessError, FileNotFoundError):
            self.logger.error(error_msg)
            sys.exit(1)
    
    # await_postgres waits for the PostgreSQL service to be ready by repeatedly checking with pg_isready.
    def await_postgres(self) -> None:
        port = self.env_config['postgres_port']
        self.logger.info(f"Waiting for postgres on port {port}...")
        
        while True:
            try:
                result = subprocess.run(
                    ["pg_isready", "-h", "0.0.0.0", "-p", str(port)],
                    capture_output=True,
                    text=True
                )
                if "accepting" in result.stdout:
                    break
            except FileNotFoundError:
                self.logger.error("'pg_isready' not found in PATH")
                sys.exit(1)
            
            time.sleep(0.3)
    
    # restart postgres docker container and create the iota_indexer database
    def restart_postgres_docker(self) -> None:
        # Check required tools
        self.check_postgres_tool_available("psql", "'psql' is not installed in PATH. Please ensure it is installed and available.")
        self.check_postgres_tool_available("pg_isready", "'pg_isready' is not installed in PATH. Please ensure it is installed and available.")
        
        # Prepare environment variables
        postgres_env = {
            'POSTGRES_PASSWORD': self.env_config['postgres_password'],
            'POSTGRES_USER': self.env_config['postgres_user'],
            'POSTGRES_DB': self.env_config['postgres_db'],
            'POSTGRES_HOST': self.env_config['postgres_host'],
            'PGPASSWORD': self.env_config['postgres_password']
        }
        
        # Remove existing postgres containers
        self.print_and_run_command(
            "docker rm -f -v $(docker ps -a | grep postgres | awk '{print $1}') || true"
        )
        
        # Navigate to docker-compose directory and restart postgres
        pg_services_dir = self.root_dir / 'dev-tools' / 'pg-services-local'
        compose_commands = [
            f"cd {pg_services_dir} && docker-compose down -v postgres",
            f"cd {pg_services_dir} && docker-compose up -d postgres"
        ]
        
        for cmd in compose_commands:
            if self.print_and_run_command(cmd, postgres_env) != 0:
                self.logger.error(f"Failed to execute: {cmd}")
                sys.exit(1)
        
        # Wait for postgres to be ready
        self.await_postgres()
        
        # Create database and configure
        db_name = self.env_config['postgres_db']
        user = self.env_config['postgres_user']
        
        create_db_cmd = f'''echo "SELECT 'CREATE DATABASE {db_name}' WHERE NOT EXISTS (SELECT FROM pg_database WHERE datname = '{db_name}')\\gexec" | psql -h localhost -U {user}'''
        self.print_and_run_command(create_db_cmd, postgres_env)
        
        config_cmd = f"psql -h localhost -U {user} -c 'ALTER SYSTEM SET max_connections = 500;'"
        self.print_and_run_command(config_cmd, postgres_env)
    
    # run_cargo_nextest runs cargo-nextest with the given filter set, config path and manifest path
    def run_cargo_nextest(self, filter_set: str = "", config_path: str = ".config/nextest.toml",
                         manifest_path: str = "", feature_set: str = "") -> int:
        # Prepare command parts
        parts = ["cargo", self.TEST_TYPE_NEXTEST, "run"]
        
        # if config path is not empty, set it to --config-file flag
        if config_path:
            parts.extend(["--config-file", config_path])
        
        # if manifest path is not empty, set it to --manifest-path flag
        if manifest_path:
            parts.extend(["--manifest-path", manifest_path])
        
        parts.extend(["--profile", "ci"])
        
        # if feature set is not empty, set it to --features flag.
        # --all-features is used otherwise.
        if feature_set:
            parts.extend(["--features", feature_set])
        else:
            parts.append("--all-features")
            
        # Add filter if present
        finalized_filter = self.finalize_filter_set(filter_set)
        if finalized_filter:
            parts.append(finalized_filter)
        
        parts.extend(["--no-tests=warn"])
        
        if self.env_config['enable_no_capture']:
            parts.append("--nocapture")
        
        # Tests written with #[sim_test] are often flaky if run as #[tokio::test] - this var
        # causes #[sim_test] to only run under the deterministic `simtest` job, and not the
        # non-deterministic `test` job.
        test_env = {'IOTA_SKIP_SIMTESTS': '1'}
        
        command = " ".join(parts)
        return self.print_and_run_command(command, test_env)
    
    # run_cargo_simtest runs cargo-simtest with the given filter set and exclude set
    def run_cargo_simtest(self, filter_set: str = "") -> int:
        parts = ["scripts/simtest/cargo-simtest", self.TEST_TYPE_SIMTEST, "--profile", "ci", "--color", "always"]
        
        # Add filter if present
        finalized_filter = self.finalize_filter_set(filter_set)
        if finalized_filter:
            parts.append(finalized_filter)
            
        parts.extend(["--no-tests=warn"])
        
        if self.env_config['enable_no_capture']:
            parts.append("--nocapture")
        
        # Set simtest timeout
        test_env = {
            'MSIM_WATCHDOG_TIMEOUT_MS': str(self.env_config['msim_watchdog_timeout_ms'])
        }
        
        command = " ".join(parts)
        return self.print_and_run_command(command, test_env)
    
    # main test execution logic handling all test types
    def filter_and_run_tests(self, test_type: str, env_overrides: Optional[Dict[str, str]] = None) -> int:
        if test_type not in [self.TEST_TYPE_NEXTEST, self.TEST_TYPE_SIMTEST]:
            self.logger.error(f"Invalid test type specified. Use 'nextest' or 'simtest'. Got: {test_type}")
            return 1
        
        config = self.env_config
        if env_overrides:
            combined_env = dict(os.environ)
            combined_env.update(env_overrides)
            config = self._load_environment_config(combined_env)
        
        run_rust_tests = config['ci_is_rust']
        run_external_crates = config['ci_is_external_crates']
        run_tests_using_postgres = config['ci_is_pg_integration']
        run_move_examples_rdeps_tests = config['ci_is_move_example_used_by_others']
        test_only_changed_crates = config['test_only_changed_crates']
        changed_crates_rust = config['ci_changed_crates']
        changed_crates_rust_given = config['changed_crates_rust_given']
        changed_crates_external = config['ci_changed_external_crates']
        changed_crates_external_given = config['changed_crates_external_given']
        restart_postgres = config['restart_postgres']
        
        # Early return if no conditions are set
        if not any([run_rust_tests, run_external_crates, run_tests_using_postgres, run_move_examples_rdeps_tests]):
            self.logger.error("No conditions are set to run tests. Exiting.")
            return 1
        
        # check if external crates are set
        if run_external_crates:
            external_filter = self.build_filterset_changed_crates(
                test_only_changed_crates, changed_crates_external, changed_crates_external_given
            )
            exclude_external = self.build_filterset_excluded(self.EXCLUDE_SET_EXTERNAL)
            combined_external = self.build_filterset_combined(external_filter, exclude_external)
            
            # first run tests for external crates (they are not part of the workspace)
            if test_type == self.TEST_TYPE_NEXTEST:
                result = self.run_cargo_nextest(
                    combined_external,
                    ".config/nextest_external.toml", 
                    "external-crates/move/Cargo.toml",
                    "tracing"
                )
                if result != 0:
                    return result
        
        # check again if any of the other conditions are set, in case only external crates were set
        if not any([run_rust_tests, run_tests_using_postgres, run_move_examples_rdeps_tests]):
            return 0
        
        # Build main test filter set
        combined_set = self.build_filterset_tests(
            run_rust_tests, run_tests_using_postgres, run_move_examples_rdeps_tests,
            test_only_changed_crates, changed_crates_rust, changed_crates_rust_given
        )
        
        # check if a restart of postgres is needed
        if run_tests_using_postgres and restart_postgres:
            self.restart_postgres_docker()
        
        # Run tests based on type
        if test_type == self.TEST_TYPE_NEXTEST:
            result = self.run_cargo_nextest(combined_set)
            if result != 0:
                return result
                
            # Run special postgres shared runtime tests with cargo test
            if run_tests_using_postgres:
                # Iota-indexer's RPC tests, which depend on a shared runtime, are incompatible with nextest due to its process-per-test execution model.
                # "cargo test", on the other hand, allows tests to share state and resources by default.
                #
                # Normally the following line can't be run with "all-features", because it would execute the "pg_integration" tests as well,
                # which rather should be run by "cargo nextest" and also not in parallel. "shared_test_runtime" feature flag should actually be used here,
                # but since we filter by "rpc-tests", there are no "shared_test_runtime" tests in the scope and it is fine to run with "all-features" here,
                # which reduces compilation time because we already run the nextest tests with "all-features" beforehand.
                rpc_test_cmd = "cargo test --profile simulator --package iota-indexer --test rpc-tests --all-features"
                if self.env_config['enable_no_capture']:
                    rpc_test_cmd += " --nocapture"
                result = self.print_and_run_command(rpc_test_cmd)
                if result != 0:
                    return result
                    
        elif test_type == self.TEST_TYPE_SIMTEST:
            result = self.run_cargo_simtest(combined_set)
            if result != 0:
                return result
        
        return 0
    
    ### Step execution methods

    # run nextest with current configuration
    def run_tests(self, env_overrides: Optional[Dict[str, str]] = None) -> int:
        return self.filter_and_run_tests(self.TEST_TYPE_NEXTEST, env_overrides)
    
    # run simtest with current configuration
    def run_simtests(self, env_overrides: Optional[Dict[str, str]] = None) -> int:
        return self.filter_and_run_tests(self.TEST_TYPE_SIMTEST, env_overrides)
    
    # test only Rust workspace crates
    def rust_crates(self) -> int:
        return self.run_tests(env_overrides={'CI_IS_RUST': 'true'})
    
    # test only external/Move crates
    def external_crates(self) -> int:
        return self.run_tests(env_overrides={'CI_IS_EXTERNAL_CRATES': 'true'})
    
    # run simulation tests for Rust crates
    def simtests(self) -> int:
        return self.run_simtests(env_overrides={'CI_IS_RUST': 'true'})
    
    # test only PostgreSQL-dependent tests
    def tests_using_postgres(self) -> int:
        return self.run_tests(env_overrides={'CI_IS_PG_INTEGRATION': 'true'})
    
    # test crates dependent on Move examples
    def move_examples_rdeps_tests(self) -> int:
        return self.run_tests(env_overrides={'CI_IS_MOVE_EXAMPLE_USED_BY_OTHERS': 'true'})
    
    # simtest for Move example dependencies
    def move_examples_rdeps_simtests(self) -> int:
        return self.run_simtests(env_overrides={'CI_IS_MOVE_EXAMPLE_USED_BY_OTHERS': 'true'})
    
    # run stress tests for new tests to check for flakiness
    def stress_new_tests_check_for_flakiness(self) -> int:
        test_env = {
            'MSIM_WATCHDOG_TIMEOUT_MS': str(self.env_config['msim_watchdog_timeout_ms'])
        }
        
        cmd = "scripts/simtest/stress-new-tests.sh"
        if self.env_config['enable_no_capture']:
            cmd += " --nocapture"
            
        return self.print_and_run_command(cmd, test_env)
    
    # run extra tests like stresstest, doc tests, doc generation, changed files, etc.
    def test_extra(self) -> int:
        # Tests written with #[sim_test] are often flaky if run as #[tokio::test] - this var
        # causes #[sim_test] to only run under the deterministic `simtest` job, and not the
        # non-deterministic `test` job.
        test_env = {'IOTA_SKIP_SIMTESTS': '1'}
        
        commands = [
            f"cargo run --package iota-benchmark --bin stress -- --log-path {self.root_dir}/.cache/stress.log --num-client-threads 10 --num-server-threads 24 --num-transfer-accounts 2 bench --target-qps 100 --num-workers 10 --transfer-object 50 --shared-counter 50 --run-duration 10s --stress-stat-collection",
            "cargo test --doc",
            "cargo doc --all-features --workspace --no-deps",
            f"{self.root_dir}/scripts/execution_layer.py generate-lib",
            f"{self.root_dir}/scripts/changed-files.sh"
        ]
        
        for cmd in commands:
            result = self.print_and_run_command(cmd, test_env)
            if result != 0:
                return result
        
        return 0
    
    # check for unused dependencies with cargo-udeps.
    def unused_deps(self) -> int:
        commands = [
            "cargo +nightly-2026-01-07 ci-udeps --all-features",
            "cargo +nightly-2026-01-07 ci-udeps --no-default-features"
        ]
        
        for cmd in commands:
            result = self.print_and_run_command(cmd)
            if result != 0:
                return result
        
        return 0
    
    # audit dependencies for security/license issues
    def audit_deps(self) -> int:
        manifest_path = self.env_config['manifest_path']
        
        commands = [
            f'cargo deny --manifest-path "{manifest_path}" check bans licenses sources',
            f'cargo deny --manifest-path "{manifest_path}" check advisories' # check security advisories (in-house crates)
        ]
        
        for cmd in commands:
            result = self.print_and_run_command(cmd)
            if result != 0:
                return result
        
        return 0
    
    # audit external dependencies
    def audit_deps_external(self) -> int:
        external_manifest = "./external-crates/move/Cargo.toml"
        
        # Temporarily set manifest path and call audit_deps
        try:
            old_manifest = self.env_config['manifest_path']
            self.env_config['manifest_path'] = external_manifest
            
            result = self.audit_deps()
        finally:
            # Restore original manifest path
            self.env_config['manifest_path'] = old_manifest
        
        return result

if __name__ == "__main__":    
    # Running all the tests will compile different sets of crates and take a lot of storage (>500GB)
    # If your machine has less storage, you can run only part of the tests (at a time),
    # use the name of the function to run as a subcommand.

    parser = argparse.ArgumentParser(
        description='Rust Test Orchestration Script',
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog=f"""
Valid steps: {', '.join(RustTestOrchestrator.VALID_STEPS)}

Environment variables:
  TEST_ONLY_CHANGED_CRATES: Only test changed crates (default: false)
  CI_CHANGED_CRATES: Space-separated list of changed crates
  RESTART_POSTGRES: Restart PostgreSQL (default: true)
  CI_IS_RUST: Run Rust tests
  CI_IS_EXTERNAL_CRATES: Run external crate tests
  CI_IS_PG_INTEGRATION: Run PostgreSQL integration tests
  CI_IS_MOVE_EXAMPLE_USED_BY_OTHERS: Run Move example dependent tests
"""
    )
    
    parser.add_argument(
        'step',
        nargs='?',
        choices=RustTestOrchestrator.VALID_STEPS + [None],
        help='Specific test step to run (if not provided, runs all steps)'
    )
    
    parser.add_argument(
        '--verbose', '-v',
        action='store_true',
        help='Enable verbose logging'
    )
    
    args = parser.parse_args()
    
    # Set up logging level
    if args.verbose:
        logging.getLogger().setLevel(logging.DEBUG)
    
    # Create orchestrator and run
    orchestrator = RustTestOrchestrator()
    
    if args.step:
        # Run specific step
        step_method = getattr(orchestrator, args.step, None)
        if step_method and callable(step_method):
            try:
                result = step_method()
                sys.exit(result if isinstance(result, int) else 0)
            except Exception as e:
                orchestrator.logger.error(f"Error running step '{args.step}': {e}")
                sys.exit(1)
        else:
            orchestrator.logger.error(f"Unknown step: {args.step}")
            sys.exit(1)
    else:
        # Run all steps (excluding run_tests and run_simtests as they're called by other steps)
        skip_steps = {"run_tests", "run_simtests"}
        for step in RustTestOrchestrator.VALID_STEPS:
            if step in skip_steps:
                continue
            
            orchestrator.logger.info(f"Running step: {step}")
            step_method = getattr(orchestrator, step, None)
            if step_method and callable(step_method):
                try:
                    result = step_method()
                    if isinstance(result, int) and result != 0:
                        orchestrator.logger.error(f"Step '{step}' failed with exit code {result}")
                        sys.exit(result)
                except Exception as e:
                    orchestrator.logger.error(f"Error running step '{step}': {e}")
                    sys.exit(1)
        
        orchestrator.logger.info("All steps completed successfully")