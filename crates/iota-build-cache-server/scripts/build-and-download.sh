#!/bin/bash

# Get script directory and source config
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/config.sh"
source "$SCRIPT_DIR/utils.sh"

print_config
check_availability
AVAILABILITY_CHECK=$?
set -e  # Enable exit on error

if [ $AVAILABILITY_CHECK -ne 0 ]; then
    build
    wait
fi

download
