#!/bin/bash

source ./python_cmd.sh

# Run consolidate for external crates first
$PYTHON_CMD cargo_sort.py --consolidate-deps \
  --target ../../external-crates/move

# Then run consolidate for the rest, ignoring external-crates and nre
$PYTHON_CMD cargo_sort.py --consolidate-deps --ignore external-crates --ignore nre "$@"
