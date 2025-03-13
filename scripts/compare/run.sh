#!/bin/bash
# usage: compare.py [-h]
#                   --source-folder SOURCE_FOLDER
#                   --target-folder TARGET_FOLDER
#                   [--codeowners CODEOWNERS [CODEOWNERS ...]]
#                   [--codeowners-file CODEOWNERS_FILE]
#                   [--compare-tool-binary COMPARE_TOOL_BINARY]
#                   [--compare-tool-arguments COMPARE_TOOL_ARGUMENTS]
#                   [--ignore-folders IGNORE_FOLDERS [IGNORE_FOLDERS ...]]
# 
# Repository comparison tool with optional code ownership filter.
# 
# options:
#   -h, --help            show this help message and exit
#   --source-folder SOURCE_FOLDER
#                         The path to the source folder for comparison.
#   --target-folder TARGET_FOLDER
#                         The path to the target folder for comparison.
#   --codeowners CODEOWNERS [CODEOWNERS ...]
#                         Optionally filter folders by code owners (e.g., "@iotaledger/node @iotaledger/consensus").
#   --codeowners-file CODEOWNERS_FILE
#                         The path to the code owners file.
#   --compare-tool-binary COMPARE_TOOL_BINARY
#                         The binary to use for comparison.
#   --compare-tool-arguments COMPARE_TOOL_ARGUMENTS
#                         The arguments to use for comparison.
#   --ignore-folders IGNORE_FOLDERS [IGNORE_FOLDERS ...]
#                         Optionally ignore folders (e.g., "node_modules").
source ../utils/python_venv_wrapper.sh

$PYTHON_CMD compare.py \
    --source-folder ../../ \
    --target-folder ../slipstream/results/mainnet-v1.44.3/main \
    --codeowners @iotaledger/node @iotaledger/core-protocol \
    --ignore-folders bridge crates/anemo-benchmark crates/x dashboards dev-tools doc docker nre scripts setups \
    "$@"