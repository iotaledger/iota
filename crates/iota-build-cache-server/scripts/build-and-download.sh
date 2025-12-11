#!/bin/bash

# Get script directory and source config
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/config.sh"
source "$SCRIPT_DIR/utils.sh"

print_config
check_availability
AVAILABILITY_CHECK=$?
set -e  # Enable exit on error

# If build is not available, start build and wait
if [ $AVAILABILITY_CHECK -ne 0 ]; then
    # Build will resolve branch/tag to commit and update COMMIT variable
    log_info "Starting build for commit/branch/tag: $COMMIT"
    build
    
    log_info "Waiting for build to complete for commit: $COMMIT"
    wait
fi

# Download uses the resolved commit hash
log_info "Downloading artifacts for commit: $COMMIT"
download
