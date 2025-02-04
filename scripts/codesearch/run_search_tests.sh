#!/bin/bash
# OPTIONS:
#    --target                       # Target directory to search in.
#    --regex                        # Regex pattern to search for.
#    --output                       # Output file to save the results.
#    --verbose                      # Display detailed file path and line number for each occurrence.
#    --debug                        # Display the line where the occurrence was found.
source ../utils/python_venv_wrapper.sh

$PYTHON_CMD search_tests.py --regex "zklogin" --verbose --debug