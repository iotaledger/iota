#!/bin/bash
# usage: compare.py [-h] 
#                   --source-folder SOURCE_FOLDER
#                   --target-folder TARGET_FOLDER
#                   [--codeowners CODEOWNERS [CODEOWNERS ...]]
#                   [--codeowners-file CODEOWNERS_FILE]
#                   [--compare-tool-binary COMPARE_TOOL_BINARY]
#                   [--compare-tool-arguments COMPARE_TOOL_ARGUMENTS]
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
source ../utils/python_venv_wrapper.sh

$PYTHON_CMD compare.py \
    --source-folder ../slipstream/results/mainnet-v1.32.2/main \
    --target-folder ../slipstream/results/mainnet-v1.41.1/main \
    --codeowners @iotaledger/node \
    "$@"