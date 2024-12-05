#!/bin/bash

# Check if either "python" or "python3" exists and use it
if command -v python3 &>/dev/null; then
    PYTHON_CMD="python3"
elif command -v python &>/dev/null; then
    PYTHON_CMD="python"
else
    echo "Neither python nor python3 binary is installed. Please install Python."
    exit 1
fi

$PYTHON_CMD track_upstream_commits.py \
    --repo-tag "mainnet-v1.36.2" \
    --target-folder result \
    --clone-source \
    "$@"