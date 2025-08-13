#!/bin/bash
# Orchestrate: build images (optional) -> bootstrap -> run -> fuzz test
# Run from: iota/dev-tools/iota-private-network/experiments/

set -euo pipefail
set -x

# Defaults
NUM_VALIDATORS=4
PROTOCOL=""
BUILD=true   # pass: -b false  to skip image rebuilds

usage() {
  echo "Usage: $0 [-n num_validators] [-p protocol] [-b build_images(true|false)]"
  echo "Examples:"
  echo "  $0                       # 4 validators, no protocol override, build images"
  echo "  $0 -n 6 -p starfish      # 6 validators, Starfish, build images"
  echo "  $0 -p starfish -b false  # Starfish, skip building images"
}

while getopts ":n:p:b:h" opt; do
  case "$opt" in
    n) NUM_VALIDATORS="$OPTARG" ;;
    p) PROTOCOL="$OPTARG" ;;
    b)
      case "$OPTARG" in
        true|false) BUILD=$OPTARG ;;
        *)
          echo "Error: -b must be 'true' or 'false'"; usage; exit 2
          ;;
      esac
      ;;
    h) usage; exit 0 ;;
    \?) echo "Error: Invalid option -$OPTARG"; usage; exit 2 ;;
    :)  echo "Error: Option -$OPTARG requires an argument"; usage; exit 2 ;;
  esac
done
shift $((OPTIND-1))

# Ensure we are in the correct directory
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
if [[ "$(basename "$SCRIPT_DIR")" != "experiments" ]]; then
  echo "Error: Must be executed from iota/dev-tools/iota-private-network/experiments/"
  exit 1
fi

echo "=== SUMMARY ==="
echo "Validators     : $NUM_VALIDATORS"
echo "Protocol       : ${PROTOCOL:-<none>}"
echo "Build images   : $BUILD"
echo "===============."

# 1) Build images (optional)
if [ "$BUILD" = true ]; then
  # Build iota-node
  ( cd ../../../docker/iota-node && ./build.sh -t iota-node --no-cache )
  # Build iota-tools
  ( cd ../../../docker/iota-tools && ./build.sh -t iota-tools --no-cache )
else
  echo "Skipping image builds (per -b false)"
fi

# 2) Bootstrap network
if [ -n "$PROTOCOL" ]; then
  ( cd .. && ./bootstrap.sh -n "$NUM_VALIDATORS" -p "$PROTOCOL" )
else
  ( cd .. && ./bootstrap.sh -n "$NUM_VALIDATORS" )
fi

# 3) Bring up docker network
( cd .. && ./run.sh -n "$NUM_VALIDATORS")

# 4)

sleep 10

# 5) Launch fuzz test
./network-fuzz-test.sh -n "$NUM_VALIDATORS"

echo "All steps completed."