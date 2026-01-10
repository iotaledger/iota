#!/bin/bash

source ./python_cmd.sh
$PYTHON_CMD cargo_sort.py --consolidate-deps \
  --strict \
  --target ../../external-crates/move
