#!/bin/bash
# OPTIONS:
#    --target-folder                    # The path to the target folder.
#    --skip-dev-dependencies            # Whether or not to include the `dev-dependencies`.
source ../utils/python_venv_wrapper.sh

$PYTHON_CMD dependency_graphs.py --skip-dev-dependencies