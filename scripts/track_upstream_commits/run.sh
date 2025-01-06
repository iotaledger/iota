#!/bin/bash
# OPTIONS:
#   -h, --help            show this help message and exit
#   --since SINCE         Start commit hash for git log (e.g., "bb778828e36d53a7d91a27e55109f2f45621badc").
#   --until UNTIL         End commit hash for git log (e.g., "3ada97c109cc7ae1b451cb384a1f2cfae49c8d3e").
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