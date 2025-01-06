#!/bin/bash
# OPTIONS:
#   -h, --help            show this help message and exit
#   --since SINCE         Start date for git log (e.g., "2024-09-05").
#   --until UNTIL         End date for git log (e.g., "2024-10-26").
#   --codeowner CODEOWNER
#                         code owner of the crates (e.g., "core-node)
#   --repo-url REPO_URL   The URL to the repository. Can also be a local folder.
#   --repo-tag REPO_TAG   The tag to checkout in the repository.
#   --version VERSION     The semantic version to filter overwrites/patches if not found in the repo-tag.
#   --target-folder TARGET_FOLDER
#                         The path to the target folder.
#   --clone-source        Clone the upstream repository.
#   --compare-source-folder COMPARE_SOURCE_FOLDER
#                         The path to the source folder for comparison.
source python_venv_wrapper.sh

$PYTHON_CMD track_upstream_commits.py \
    --repo-tag "mainnet-v1.36.2" \
    --target-folder result \
    --clone-source \
    "$@"