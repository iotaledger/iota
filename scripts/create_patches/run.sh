#!/bin/bash
# usage: create_patches.py [-h]
#                          --source-folder SOURCE_FOLDER
#                          --target-folder TARGET_FOLDER
#                          --output-folder OUTPUT_FOLDER
#                          [--folders FOLDERS [FOLDERS ...]]
#                          [--codeowners CODEOWNERS [CODEOWNERS ...]]
#                          [--codeowners-file CODEOWNERS_FILE]
#                          [--verbose]
# 
# Generate patch files for differing files in specified subfolders.
# 
# options:
#   -h, --help            show this help message and exit
#   --source-folder SOURCE_FOLDER
#                         The path to the source folder for comparison.
#   --target-folder TARGET_FOLDER
#                         The path to the target folder for comparison.
#   --folders FOLDERS [FOLDERS ...]
#                         Optionally filter by a list of folders relative to the project root (e.g., "crates/iota-core crates/iota-node").
#   --codeowners CODEOWNERS [CODEOWNERS ...]
#                         Optionally filter folders by code owners (e.g., "@iotaledger/node @iotaledger/consensus").
#   --codeowners-file CODEOWNERS_FILE
#                         The path to the code owners file.
#   --output-folder OUTPUT_FOLDER
#                         The path to the output folder to save patch files.
#   --verbose             Print verbose output.
source ../utils/python_venv_wrapper.sh

$PYTHON_CMD create_patches.py \
    --source-folder ../slipstream/results/mainnet-v1.32.2/main \
    --target-folder ../slipstream/results/mainnet-v1.41.1/main \
    --output-folder patches \
    --codeowners @iotaledger/node \
    --verbose \
    "$@"