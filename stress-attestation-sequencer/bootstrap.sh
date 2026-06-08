#!/usr/bin/env bash
#
# bootstrap.sh — thin wrapper around the iota-private-network bootstrap.
# Forwards all args to <repo>/dev-tools/iota-private-network/bootstrap.sh and
# can be invoked from any directory.
#
# Args (forwarded as-is):
#   -n NUM_VALIDATORS     number of validators (default 4)
#   -e EPOCH_DURATION_MS  epoch duration in ms (default 1200000 = 20 min)
#   -b                    benchmark mode (deterministic gas accounts + benchmark.keystore)
#
# For the stress test you typically want -b, e.g.:
#   ./bootstrap.sh -b -n 4
#
set -euo pipefail

# Resolve the repo root from this script's own location so it works from any cwd.
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
PRIVNET_DIR="${PRIVNET_DIR:-$REPO_ROOT/dev-tools/iota-private-network}"

if [[ ! -d "$PRIVNET_DIR" ]]; then
  echo "ERROR: PRIVNET_DIR not found: $PRIVNET_DIR" >&2
  exit 1
fi

cd "$PRIVNET_DIR"
exec ./bootstrap.sh "$@"
