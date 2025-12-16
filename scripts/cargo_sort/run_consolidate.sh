#!/bin/bash

source ./python_cmd.sh
$PYTHON_CMD cargo_sort.py --consolidate-deps --ignore external-crates --ignore nre "$@"
