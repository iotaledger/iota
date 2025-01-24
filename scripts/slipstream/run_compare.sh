#!/bin/bash
# OPTIONS:
#    --config FILE_PATH                        # The path to the configuration file.
#    --verbose                                 # Print verbose output.
#    --repo-url REPO_URL_OR_LOCAL_FOLDER       # The URL to the repository. Can also be a local folder.
#    --repo-tag TAG                            # The tag to checkout in the repository.
#    --version                                 # The semantic version to filter overwrites/patches if not found in the repo-tag.
#    --target-folder FOLDER                    # The path to the target folder.
#    --main-repository-folder-name FOLDER      # The name of the main repository folder (subfolder of target-folder).
#    --target-branch BRANCH                    # The branch to create and checkout in the target folder.
#    --commit-between-steps                    # Create a commit between each step.
#    --panic-on-linter-errors                  # Panic on linter errors (typos, cargo fmt, dprint, pnpm lint, cargo clippy).
#    --clone-source                            # Clone the upstream repository.
#    --clone-history                           # Clone the complete history of the upstream repository.
#    --create-branch                           # Create a new branch in the target folder.
#    --delete                                  # Delete files or folders based on the rules in the config.
#    --apply-path-renames                      # Apply path renames based on the rules in the config.
#    --apply-code-renames                      # Apply code renames based on the rules in the config.
#    --copy-overwrites                         # Copy and overwrite files listed in the config.
#    --apply-patches                           # Apply git patches from the patches folder.
#    --run-fix-typos                           # Run script to fix typos.
#    --run-cargo-fmt                           # Run cargo fmt.
#    --run-dprint-fmt                          # Run dprint fmt.
#    --run-pnpm-prettier-fix                   # Run pnpm prettier:fix.
#    --run-pnpm-lint-fix                       # Run pnpm lint:fix.
#    --run-shell-commands                      # Run shell commands listed in the config.
#    --run-cargo-clippy                        # Run cargo clippy.
#    --recompile-framework-packages            # Recompile the framework system packages and bytecode snapshots.
#    --compare-results                         # Open tool for comparison.
#    --compare-source-folder FOLDER            # The path to the source folder for comparison.
#    --compare-tool-binary BINARY              # The binary to use for comparison.
#    --compare-tool-arguments ARGUMENTS        # The arguments to use for comparison.
source python_venv_wrapper.sh

$PYTHON_CMD slipstream.py \
    --config config_slipstream.json \
    --target-folder results/mainnet-v1.40.3/main \
    --compare-results \
    --compare-source-folder results/mainnet-v1.32.2/main