#!/bin/bash
# Configuration file for build cache server scripts

# Build cache server configuration
export BUILD_CACHE_SERVER="localhost:8080"

# Basic authentication (set these if the server requires authentication)
export BUILD_CACHE_USER="${BUILD_CACHE_USER:-}"
export BUILD_CACHE_PASSWORD="${BUILD_CACHE_PASSWORD:-}"

# Default build parameters
export COMMIT="develop"     # Git commit/branch/tag to build
export CPU_TARGET="native"  # CPU target (native, skylake, etc.)

# Default binaries to build/check/download
export BINARIES="iota,iota-node,stress"

# Timeout settings (in seconds)
export BUILD_TIMEOUT=2700  # 45 minutes
export CHECK_INTERVAL=30   # 30 seconds

# Output directory for downloaded binaries
export OUTPUT_DIR="./binaries"