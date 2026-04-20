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
        "run_sim_tests",
        "run_stress_new_tests_check_for_flakiness",
        "run_tests_extra",
        "run_unused_deps",
        "run_audit_deps",
        "run_audit_deps_external",
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
    # iota-framework-tests, iota-json, iota-json-rpc-tests use the Move examples directly as part of their tests.
    FILTERSET_TESTS_MOVE_EXAMPLES_RDEPS = [
        "rdeps(iota-test-transaction-builder)",
        "rdeps(iota-core)",
        "package(iota-framework-tests)",
        "(package(iota-json) and test(test_basic_args_linter_top_level))",
        "(package(iota-json-rpc-tests) and (test(try_get_past_object_deleted) or test(test_publish)))"
    ]
    
    # initialize the orchestrator with configuration
    def __init__(self, args=None):
        self.args = args
        self.setup_logging()
        self.root_dir = self._get_root_directory()
        self.config = self._load_config(os.environ)
        
        # Log configuration for debugging
        self.logger.info("Test orchestrating configuration loaded:")
        for key, value in self.config.items():
            # Don't log sensitive information like passwords
            if 'password' in key.lower():
                self.logger.info(f"  {key}: [REDACTED]")
            else:
                self.logger.info(f"  {key}: {value}")
        
        if self.args:
            print_args = {
                'run_tests',   
                'run_sim_tests',
                'run_stress_new_tests_check_for_flakiness',
                'run_tests_extra',
                'run_unused_deps',
                'run_audit_deps',
                'run_audit_deps_external',
                'tests_crates_workspace',
                'tests_crates_external',
                'tests_pg_integration',
                'tests_move_examples_rdeps',
                'filter_overwrite',
                'filter_overwrite_external',
            }
            
            self.logger.info("Additional command line arguments:")
            for attr in dir(self.args):
                if attr in print_args:
                    value = getattr(self.args, attr)
                    self.logger.info(f"  {attr}: {value}")
        else:
            self.logger.info("No command line arguments provided")

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
            
    # load and validate configuration from CLI arguments and environment
    def _load_config(self, env_source: Dict[str, str]) -> Dict[str, str]:
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

        def get_arg_with_default(attr_name: str, default_value):
            """Helper to get argument attribute with proper default handling"""
            if not self.args:
                return default_value
            
            value = getattr(self.args, attr_name, None)
            return value if value is not None else default_value

        def get_arg_given(attr_name: str) -> bool:
            """Helper to detect if an argument was passed to the script, regardless of its value"""
            if not self.args:
                return False
            value = getattr(self.args, attr_name, None)
            return value is not None
        
        return {
            # PostgreSQL configuration (infrastructure config - keep as env vars)
            'postgres_password': get_env_str('POSTGRES_PASSWORD', 'postgrespw'),
            'postgres_user': get_env_str('POSTGRES_USER', 'postgres'),
            'postgres_db': get_env_str('POSTGRES_DB', 'iota_indexer'),
            'postgres_host': get_env_str('POSTGRES_HOST', 'postgres'),
            'postgres_port': get_env_int('POSTGRES_PORT', 5432),
            # CI uses postgres provided via a github CI service. It needs to be able to not restart postgres.
            # Locally, this script restarts postgres by default. Override by passing RESTART_POSTGRES=false
            # only the tests that need postgres will automatically (re-)start it
            'restart_postgres': get_env_bool('RESTART_POSTGRES', True),
            
            # Test execution control

            # CI will only test crates that have changed in the PR
            # For local tests, tests all crates by default. Override with TEST_ONLY_CHANGED_CRATES=true
            'test_only_changed_crates': get_arg_with_default('test_only_changed_crates', False),
            
            # Changed crates lists
            'changed_crates': get_arg_with_default('changed_crates', ''),
            'changed_crates_given': get_arg_given('changed_crates'),
            'changed_crates_external': get_arg_with_default('changed_crates_external', ''),
            'changed_crates_external_given': get_arg_given('changed_crates_external'),
            
            # Test execution settings
            'manifest_path': get_arg_with_default('manifest_path', './Cargo.toml'),
            'manifest_path_external': get_arg_with_default('manifest_path_external', './external-crates/move/Cargo.toml'),
            'no_capture': get_arg_with_default('no_capture', False),
            'simtest_timeout': get_arg_with_default('simtest_timeout', 180000),
            'base_branch': get_arg_with_default('base_branch', 'origin/develop'),
            'dry_run': get_arg_with_default('dry_run', False),
            'no_fail_fast': get_arg_with_default('no_fail_fast', False),
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
    
    # find crates that have changed by comparing current branch with the specified base branch.
    # subfolder_filter: if specified, only look for changes in files that start with this path
    # crates_filter_file: path to the crates filter YAML file to use (default: .github/crates-filters.yml)
    def search_changed_crates(self, subfolder_filter: str = None, crates_filter_file: str = None) -> List[str]:
        try:
            base_branch = self.config['base_branch']
            # Log that we are using the fallback method to detect changed crates
            filter_msg = f" in {subfolder_filter}" if subfolder_filter else ""
            self.logger.info(f"Detecting changed crates{filter_msg} by comparing with {base_branch}...")

            # Get changed files
            result = subprocess.run(
                ["git", "diff", "--name-only", f"{base_branch}..HEAD"],
                capture_output=True,
                text=True,
                check=True,
                cwd=self.root_dir
            )
            changed_files = [f.strip() for f in result.stdout.split('\n') if f.strip()]
            
            # Filter changed files to subfolder if specified
            if subfolder_filter:
                changed_files = [f for f in changed_files if f.startswith(subfolder_filter)]
            
            # Load crate mappings
            if crates_filter_file is None:
                crates_filter_file = '.github/crates-filters.yml'
            crates_filters_path = self.root_dir / crates_filter_file
            crate_mappings = self.parse_crates_filters(crates_filters_path)
            
            # Find matching crates
            matching_crates = set()
            for crate_name, paths in crate_mappings.items():
                for path_prefix in paths:
                    for changed_file in changed_files:
                        # Ensure we match complete directory paths, not just prefixes
                        if changed_file.startswith(path_prefix+'/'):
                            matching_crates.add(crate_name)
                            break
            
            # Log detected changed crates
            if matching_crates:
                self.logger.info(f"Detected changed crates{filter_msg}: {', '.join(sorted(matching_crates))}")
            else:
                self.logger.info(f"No changed crates detected{filter_msg}.")
            
            return sorted(list(matching_crates))
            
        except subprocess.CalledProcessError as e:
            self.logger.error(f"Failed to get changed files from git: {e}")
            return []
        except Exception as e:
            self.logger.error(f"Error detecting changed crates: {e}")
            return []
    
    # print command and execute it, returning exit code
    def print_and_run_command(self, command: str, env: Optional[Dict[str, str]] = None) -> int:
        if self.config['dry_run']:
            self.logger.info(f"[DRY RUN]: {command}")
            if env:
                env_vars = ' '.join([f"{k}={v}" for k, v in env.items() if not k.lower().endswith('password')])
                if env_vars:
                    self.logger.info(f"[DRY RUN] With environment: {env_vars}")
            return 0
        
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
    # subfolder_filter: if specified, only look for changes in files that start with this path
    # crates_filter_file: path to the crates filter YAML file to use (default: .github/crates-filters.yml)
    def build_filterset_changed_crates(self, test_only_changed_crates: bool, 
                                     changed_crates: str, changed_crates_given: bool,
                                     subfolder_filter: str = None, crates_filter_file: str = None) -> str:
        if not test_only_changed_crates:
            # test all crates (return empty filter_set)
            return ""
            
        # detected changed crates if "changed_crates" variable is empty,
        # and the changed crates were not given.
        if not changed_crates and not changed_crates_given:
            detected_crates = self.search_changed_crates(subfolder_filter, crates_filter_file)
            changed_crates = " ".join(detected_crates)
        
        if changed_crates:
            crate_list = [c.strip() for c in changed_crates.split() if c.strip()]
            return self.build_filterset_included_rdeps(crate_list)
        
        # If no crates were changed, we should not run any tests
        return None

    # build_filterset_tests builds a combined filter set for tests based on the given conditions
    # tests_crates_workspace: run tests for rust crates
    # tests_pg_integration: run tests that depend on Postgres
    # tests_move_example_used_by_others: run tests that depend on the Move examples
    # test_only_changed_crates: run tests only for the crates that have changed
    # changed_crates: the list of changed crates for rust
    def build_filterset_tests(self, tests_crates_workspace: bool, tests_pg_integration: bool,
                            tests_move_example_used_by_others: bool, test_only_changed_crates: bool,
                            changed_crates: str, changed_crates_given: bool) -> str:
        filter_set = ""
        
        # we always exclude the following tests, because they need shared state and are incompatible with nextest.
        # they are run separately after the nextest tests via "cargo test"
        exclude_set = self.build_filterset_excluded(self.FILTERSET_TESTS_POSTGRES_SHARED_TEST_RUNTIME)
        
        tests_added = False

        if tests_crates_workspace:
            changed_crates_rust_filter = self.build_filterset_changed_crates(
                test_only_changed_crates, changed_crates, changed_crates_given
            )
            # If changed_crates_rust_filter is None, it means no workspace crates changed,
            # so we shouldn't add any workspace tests
            if changed_crates_rust_filter is not None:
                filter_set = self.append_filter_item_or(filter_set, changed_crates_rust_filter)
                tests_added = True
            else:
                self.logger.info("Skipping workspace tests - no workspace crates changed")
        
        if tests_pg_integration:
            postgres_tests_filter = self.build_filterset_included(self.FILTERSET_TESTS_POSTGRES_PG_INTEGRATION)
            filter_set = self.append_filter_item_or(filter_set, postgres_tests_filter)
            tests_added = True
        else:
            postgres_tests_exclude_filter = self.build_filterset_excluded(self.FILTERSET_TESTS_POSTGRES_PG_INTEGRATION)
            exclude_set = self.append_filter_item_and(exclude_set, postgres_tests_exclude_filter)
        
        if tests_move_example_used_by_others:
            move_examples_rdeps_tests_filter = self.build_filterset_included(self.FILTERSET_TESTS_MOVE_EXAMPLES_RDEPS)
            filter_set = self.append_filter_item_or(filter_set, move_examples_rdeps_tests_filter)
            tests_added = True
        
        return self.build_filterset_combined(filter_set, exclude_set), tests_added
    
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
        port = self.config['postgres_port']
        
        if self.config['dry_run']:
            self.logger.info(f"[DRY RUN] Would wait for postgres on port {port}...")
            return
            
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
            'POSTGRES_PASSWORD': self.config['postgres_password'],
            'POSTGRES_USER': self.config['postgres_user'],
            'POSTGRES_DB': self.config['postgres_db'],
            'POSTGRES_HOST': self.config['postgres_host'],
            'PGPASSWORD': self.config['postgres_password']
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
        db_name = self.config['postgres_db']
        user = self.config['postgres_user']
        
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
        
        if self.config['no_capture']:
            parts.append("--nocapture")
        
        if self.config.get('no_fail_fast'):
            parts.append("--no-fail-fast")
        
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
        
        if self.config['no_capture']:
            parts.append("--nocapture")
        
        if self.config.get('no_fail_fast'):
            parts.append("--no-fail-fast")
        
        # Set simtest timeout
        test_env = {
            'MSIM_WATCHDOG_TIMEOUT_MS': str(self.config['simtest_timeout'])
        }
        
        command = " ".join(parts)
        return self.print_and_run_command(command, test_env)
    
    # main test execution logic handling all test types
    def filter_and_run_tests(self, 
                             test_type: str,
                             tests_crates_workspace=False,
                             tests_crates_external=False,
                             tests_pg_integration=False,
                             tests_move_example_used_by_others=False,
                             filter_overwrite: str = None,
                             filter_overwrite_external: str = None,
                             ) -> int:
        
        if test_type not in [self.TEST_TYPE_NEXTEST, self.TEST_TYPE_SIMTEST]:
            self.logger.error(f"Invalid test type specified. Use 'nextest' or 'simtest'. Got: {test_type}")
            return 1
        
        # Use configuration from CLI arguments
        config = self.config
        
        # Override test type flags with method parameters
        test_only_changed_crates = config['test_only_changed_crates']
        changed_crates = config['changed_crates']
        changed_crates_given = config['changed_crates_given']
        changed_crates_external = config['changed_crates_external']
        changed_crates_external_given = config['changed_crates_external_given']
        restart_postgres = config['restart_postgres']
        
        # Early return if no conditions are set
        if not any([
            tests_crates_workspace,
            tests_crates_external,
            tests_pg_integration,
            tests_move_example_used_by_others,
            filter_overwrite,
            filter_overwrite_external
        ]):
            self.logger.error("No conditions are set to run tests. Exiting.")
            return 1
        
        no_fail_fast = self.config.get('no_fail_fast', False)
        first_failure = 0

        def handle_result(res: int) -> Optional[int]:
            """Returns the result to propagate immediately (fail-fast), or None to continue."""
            nonlocal first_failure
            if res != 0:
                if first_failure == 0:
                    first_failure = res
                if not no_fail_fast:
                    return res
            return None

        # check if external crates are set
        if tests_crates_external or filter_overwrite_external:
            external_filter = ""
            if filter_overwrite_external:
                self.logger.info(f"Using filter overwrite for external crates tests: \"{filter_overwrite_external}\"")
                external_filter = filter_overwrite_external
            else:
                external_filter = self.build_filterset_changed_crates(
                    test_only_changed_crates, changed_crates_external, changed_crates_external_given,
                    "external-crates/move/crates/", ".github/external-crates-filters.yml"
                )
            
            # If external_filter is None, it means no external crates changed,
            # so we shouldn't add any external tests
            if external_filter is not None:
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
                    if (propagate := handle_result(result)) is not None:
                        return propagate
            else:
                self.logger.info("Skipping external crates tests - no external crates changed")
        
        # check again if any of the other conditions are set, in case only external crates were set
        if not any([
            tests_crates_workspace,
            tests_pg_integration,
            tests_move_example_used_by_others,
            filter_overwrite,
        ]):
            return first_failure
        
        # Build main test filter set
        combined_set, tests_added = "", False

        if filter_overwrite:
            self.logger.info(f"Using filter overwrite for main tests: \"{filter_overwrite}\"")
            combined_set = filter_overwrite
            tests_added = True
        else:
            combined_set, tests_added = self.build_filterset_tests(
                tests_crates_workspace, tests_pg_integration, tests_move_example_used_by_others,
                test_only_changed_crates, changed_crates, changed_crates_given
            )
        
        if not tests_added:
            self.logger.error("No tests to run after building filter set. Exiting.")
            return 0
        
        # check if a restart of postgres is needed
        if tests_pg_integration and restart_postgres:
            self.restart_postgres_docker()
        
        # Run tests based on type
        if test_type == self.TEST_TYPE_NEXTEST:
            result = self.run_cargo_nextest(combined_set)
            if (propagate := handle_result(result)) is not None:
                return propagate
                
            # Run special postgres shared runtime tests with cargo test
            if tests_pg_integration:
                # Iota-indexer's RPC tests, which depend on a shared runtime, are incompatible with nextest due to its process-per-test execution model.
                # "cargo test", on the other hand, allows tests to share state and resources by default.
                #
                # Normally the following line can't be run with "all-features", because it would execute the "pg_integration" tests as well,
                # which rather should be run by "cargo nextest" and also not in parallel. "shared_test_runtime" feature flag should actually be used here,
                # but since we filter by "rpc-tests", there are no "shared_test_runtime" tests in the scope and it is fine to run with "all-features" here,
                # which reduces compilation time because we already run the nextest tests with "all-features" beforehand.
                rpc_test_cmd = "cargo test --profile simulator --package iota-indexer --test rpc-tests --all-features"
                if self.config['no_capture']:
                    rpc_test_cmd += " --nocapture"
                if self.config.get('no_fail_fast'):
                    rpc_test_cmd += " --no-fail-fast"
                result = self.print_and_run_command(rpc_test_cmd)
                if (propagate := handle_result(result)) is not None:
                    return propagate
                    
        elif test_type == self.TEST_TYPE_SIMTEST:
            result = self.run_cargo_simtest(combined_set)
            if (propagate := handle_result(result)) is not None:
                return propagate
        
        return first_failure
    
    ### Step execution methods

    # run tests with current configuration
    def run_tests(self, 
                  tests_crates_workspace=False,
                  tests_crates_external=False,
                  tests_pg_integration=False,
                  tests_move_example_used_by_others=False,
                  filter_overwrite: str = None,
                  filter_overwrite_external: str = None,
                  ) -> int:
        return self.filter_and_run_tests(
            self.TEST_TYPE_NEXTEST,
            tests_crates_workspace,
            tests_crates_external,
            tests_pg_integration,
            tests_move_example_used_by_others,
            filter_overwrite,
            filter_overwrite_external,
        )
    
    # run simtest with current configuration
    def run_sim_tests(self,
                     tests_crates_workspace=False,
                     tests_crates_external=False,
                     tests_pg_integration=False,
                     tests_move_example_used_by_others=False,
                     filter_overwrite: str = None,
                     filter_overwrite_external: str = None) -> int:
        return self.filter_and_run_tests(
            self.TEST_TYPE_SIMTEST,
            tests_crates_workspace,
            tests_crates_external,
            tests_pg_integration,
            tests_move_example_used_by_others,
            filter_overwrite,
            filter_overwrite_external,
        )
    
    # run stress tests for new tests to check for flakiness
    def run_stress_new_tests_check_for_flakiness(self) -> int:
        test_env = {
            'MSIM_WATCHDOG_TIMEOUT_MS': str(self.config['simtest_timeout'])
        }
        
        cmd = "scripts/simtest/stress-new-tests.sh"
        if self.config['no_capture']:
            cmd += " --nocapture"
            
        return self.print_and_run_command(cmd, test_env)
    
    # run extra tests like stresstest, doc tests, doc generation, changed files, etc.
    def run_tests_extra(self) -> int:
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
    def run_unused_deps(self) -> int:
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
    def run_audit_deps(self, manifest_path: str = None) -> int:
        if manifest_path is None:
            manifest_path = self.config.get('manifest_path', "./Cargo.toml")
        
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
    def run_audit_deps_external(self) -> int:
        manifest_path = self.config.get('manifest_path_external', "./external-crates/move/Cargo.toml")
        return self.run_audit_deps(manifest_path=manifest_path)

if __name__ == "__main__":    
    # Running all the tests will compile different sets of crates and take a lot of storage (>500GB)
    # If your machine has less storage, you can run only part of the tests (at a time),
    # use the individual flags to run specific test types.

    parser = argparse.ArgumentParser(
        description='Rust Test Orchestration Script',
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog="""
PostgreSQL Environment variables (infrastructure config):
  POSTGRES_PASSWORD:  PostgreSQL password (default: postgrespw)
  POSTGRES_USER:      PostgreSQL user (default: postgres)
  POSTGRES_DB:        PostgreSQL database (default: iota_indexer)
  POSTGRES_HOST:      PostgreSQL host (default: postgres)
  POSTGRES_PORT:      PostgreSQL port (default: 5432)
  RESTART_POSTGRES:   Whether to restart PostgreSQL before running tests that depend on it (default: true).
"""
    )
    
    # Individual test step flags
    parser.add_argument('--run-tests', action='store_true', help='Run rust tests via nextest and current configuration')
    parser.add_argument('--run-sim-tests', action='store_true', help='Run sim tests with current configuration')
    parser.add_argument('--run-stress-new-tests-check-for-flakiness', action='store_true', help='Run stress tests for new tests to check for flakiness')
    parser.add_argument('--run-tests-extra', action='store_true', help='Run extra tests like stresstest, doc tests, doc generation, changed files, etc.')
    parser.add_argument('--run-unused-deps', action='store_true', help='Check for unused dependencies with cargo-udeps')
    parser.add_argument('--run-audit-deps', action='store_true', help='Run dependency audit for security/license issues with cargo-deny')
    parser.add_argument('--run-audit-deps-external', action='store_true', help='Run dependency audit for external crates with cargo-deny')

    # Specific test type flags for "run-tests" and "run-sim-tests"
    parser.add_argument('--tests-crates-workspace', action='store_true', help='Run tests for internal Rust workspace crates (in combination with `--run-tests` or `--run-sim-tests`)')
    parser.add_argument('--tests-crates-external', action='store_true', help='Run tests for external/Move crates (in combination with `--run-tests` or `--run-sim-tests`)')
    parser.add_argument('--tests-pg-integration', action='store_true', help='Run PostgreSQL-dependent tests (in combination with `--run-tests` or `--run-sim-tests`)')
    parser.add_argument('--tests-move-examples-rdeps', action='store_true', help='Run tests for crates dependent on Move examples (in combination with `--run-tests` or `--run-sim-tests`)')
    
    # Filter overwrite flags for "run-tests" and "run-sim-tests"
    parser.add_argument('--filter-overwrite', type=str, help='Directly specify a filter set to overwrite the automatically built filter for main tests (in combination with `--run-tests` or `--run-sim-tests`). Example: --filter-overwrite "crate_a or rdeps(crate_b) or test(test_a)"')
    parser.add_argument('--filter-overwrite-external', type=str, help='Directly specify a filter set to overwrite the automatically built filter for external crates tests (in combination with `--run-tests` or `--run-sim-tests`). Example: --filter-overwrite-external "crate_c or rdeps(crate_d) or test(test_b)"')

    # Configuration arguments
    parser.add_argument('--test-only-changed-crates', action='store_true', help='Only test changed crates (default: test all crates)')
    parser.add_argument('--changed-crates', type=str, help='Space-separated list of changed crates to test')
    parser.add_argument('--changed-crates-external', type=str, help='Space-separated list of changed external crates to test')
    parser.add_argument('--manifest-path', type=str, help='Path to Cargo.toml manifest file (default: ./Cargo.toml)')
    parser.add_argument('--manifest-path-external', type=str, help='Path to Cargo.toml manifest file for external crates (default: ./Cargo.toml)')
    parser.add_argument('--no-capture', action='store_true', help='Disable test output capture (show all output)')
    parser.add_argument('--simtest-timeout', type=int, help='Timeout in milliseconds for simulation tests (default: 180000)')
    parser.add_argument('--base-branch', type=str, help='Base branch to compare for changed crates detection if no changed crates are given (default: origin/develop)')
    parser.add_argument('--dry-run', action='store_true', help='Print commands that would be executed without actually running them')
    parser.add_argument('--no-fail-fast', action='store_true', help='Continue running all test commands even if one fails, instead of stopping on the first failure. The script still exits with a non-zero code if any command failed.')

    parser.add_argument(
        '--verbose', '-v',
        action='store_true',
        help='Enable verbose logging'
    )
    
    args = parser.parse_args()
    
    # Set up logging level
    if args.verbose:
        logging.getLogger().setLevel(logging.DEBUG)
    
    # Verify if "run_tests" or "run_sim_tests" is requested, at least one of the specific test type flags should be set.
    if (args.run_tests or args.run_sim_tests) and not any([
        args.tests_crates_workspace, 
        args.tests_crates_external, 
        args.tests_pg_integration, 
        args.tests_move_examples_rdeps,
        args.filter_overwrite,
        args.filter_overwrite_external,
    ]):
        parser.error("When using --run-tests or --run-sim-tests, at least one of the specific test type flags must be set: --tests-crates-workspace, --tests-crates-external, --tests-pg-integration, --tests-move-examples-rdeps, --filter-overwrite, --filter-overwrite-external")

    # Verify if any specific test type flag is set without "run_tests" or "run_sim_tests"
    if any([
        args.tests_crates_workspace, 
        args.tests_crates_external, 
        args.tests_pg_integration, 
        args.tests_move_examples_rdeps,
        args.filter_overwrite,
        args.filter_overwrite_external,
    ]) and not (args.run_tests or args.run_sim_tests):
        parser.error("Specific test type flags cannot be used without --run-tests or --run-sim-tests. Please specify which test type to run with the specific test type flags.")

    # Create orchestrator and run
    orchestrator = RustTestOrchestrator(args)

    # Map argument flags to method names
    step_mapping = {
        'run_tests': args.run_tests,
        'run_sim_tests': args.run_sim_tests,
        'run_stress_new_tests_check_for_flakiness': args.run_stress_new_tests_check_for_flakiness,
        'run_tests_extra': args.run_tests_extra,
        'run_unused_deps': args.run_unused_deps,
        'run_audit_deps': args.run_audit_deps,
        'run_audit_deps_external': args.run_audit_deps_external,
    }
    
    # Collect enabled steps
    enabled_steps = [step for step, enabled in step_mapping.items() if enabled]
    
    # If no specific steps are requested, run all steps except run_tests and run_sim_tests
    # because they are called by other steps in the CI
    if not enabled_steps:
        enabled_steps = [step for step in RustTestOrchestrator.VALID_STEPS 
                        if step not in {"run_tests", "run_sim_tests"}]
    
    # Run the enabled steps
    for step in enabled_steps:
        orchestrator.logger.info(f"Running step: {step}")
    
        step_method = getattr(orchestrator, step, None)
        if step_method and callable(step_method):
            try:
                # if the step is "run_tests" or "run_sim_tests", 
                # we need to pass the specific test type flags based on the CLI arguments
                if step in {"run_tests", "run_sim_tests"}:
                    result = step_method(
                        tests_crates_workspace=args.tests_crates_workspace,
                        tests_crates_external=args.tests_crates_external,
                        tests_pg_integration=args.tests_pg_integration,
                        tests_move_example_used_by_others=args.tests_move_examples_rdeps,
                        filter_overwrite=args.filter_overwrite,
                        filter_overwrite_external=args.filter_overwrite_external,
                    )
                else:
                    result = step_method()
                
                if isinstance(result, int) and result != 0:
                    orchestrator.logger.error(f"Step '{step}' failed with exit code {result}")
                    sys.exit(result)
            except Exception as e:
                orchestrator.logger.error(f"Error running step '{step}': {e}")
                sys.exit(1)
        else:
            orchestrator.logger.error(f"Unknown step: {step}")
            sys.exit(1)

    if enabled_steps:
        orchestrator.logger.info("All steps completed successfully")
    else:
        orchestrator.logger.error("No steps to run")
        sys.exit(1)
