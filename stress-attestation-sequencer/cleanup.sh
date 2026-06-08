#!/usr/bin/env bash
#
# cleanup.sh — tear down the local stress-test stack.
# Brings the monitoring stack (dev-tools/grafana-local) down FIRST, then runs
# <repo>/dev-tools/iota-private-network/cleanup.sh (docker compose down +
# removes the data dir), forwarding all args. Invokable from any directory.
#
# Note: must run as root/sudo (the network data dir is root-owned by the
# containers), e.g.:
#   sudo ./cleanup.sh
#
set -euo pipefail

# Resolve the repo root from this script's own location so it works from any cwd
# (and regardless of $HOME under sudo).
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
PRIVNET_DIR="${PRIVNET_DIR:-$REPO_ROOT/dev-tools/iota-private-network}"

if [[ ! -d "$PRIVNET_DIR" ]]; then
  echo "ERROR: PRIVNET_DIR not found: $PRIVNET_DIR" >&2
  exit 1
fi

# Bring monitoring (prometheus + grafana) down BEFORE the network.
GRAFANA_DIR="${GRAFANA_DIR:-$REPO_ROOT/dev-tools/grafana-local}"
if [[ -d "$GRAFANA_DIR" ]]; then
  echo "Stopping monitoring (prometheus + grafana)..."
  (cd "$GRAFANA_DIR" && docker compose down --remove-orphans) || true
  echo
fi

# Then bring the network down (docker compose down + rm -rf data; needs root).
cd "$PRIVNET_DIR"
echo "Stopping the network (validators, fullnode(s), faucet)..."
exec ./cleanup.sh "$@"
