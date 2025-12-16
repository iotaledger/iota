#!/bin/bash

source ./python_cmd.sh
$PYTHON_CMD cargo_sort.py --consolidate-deps \
  --target ../../external-crates/move
