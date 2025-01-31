#!/bin/bash
# usage: track_upstream_commits.py [-h]
#                                  --since SINCE 
#                                  --until UNTIL 
#                                  [--codeowners CODEOWNERS [CODEOWNERS ...]] 
#                                  [--codeowners-file CODEOWNERS_FILE] 
#                                  [--folders FOLDERS [FOLDERS ...]]
#                                  [--repo-url REPO_URL] 
#                                  [--repo-tag REPO_TAG] 
#                                  [--target-folder TARGET_FOLDER] 
#                                  [--clone-source]
# 
# Track upstream commits for specified folders.
# 
# options:
#   -h, --help            show this help message and exit
#   --since SINCE         Start commit hash or the tag for git log (e.g., "bb778828e36d53a7d91a27e55109f2f45621badc", "mainnet-v1.32.2"), it is EXCLUDED from the results.
#   --until UNTIL         End commit hash or the tag for git log (e.g., "3ada97c109cc7ae1b451cb384a1f2cfae49c8d3e", "mainnet-v1.36.2"), it is INCLUDED in the results.
#   --codeowners CODEOWNERS [CODEOWNERS ...]
#                         Code owners of the folders (e.g., "@iotaledger/node @iotaledger/consensus").
#   --codeowners-file CODEOWNERS_FILE
#                         The path to the code owners file.
#   --folders FOLDERS [FOLDERS ...]
#                         List of folders relative to the project root to track (e.g., "crates/iota-core crates/iota-node").
#   --repo-url REPO_URL   The URL to the repository. Can also be a local folder.
#   --repo-tag REPO_TAG   The tag to checkout in the repository.
#   --target-folder TARGET_FOLDER
#                         The path to the target folder.
#   --clone-source        Clone the upstream repository.
source ../utils/python_venv_wrapper.sh

$PYTHON_CMD track_upstream_commits.py \
    --repo-tag "mainnet-v1.41.1" \
    --target-folder result \
    --clone-source \
    "$@"
