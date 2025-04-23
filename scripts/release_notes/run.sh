#!/bin/bash

source ../utils/python_venv_wrapper.sh

$PYTHON_CMD release_notes.py "$@"
