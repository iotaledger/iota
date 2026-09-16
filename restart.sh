#!/usr/bin/env bash
set -euo pipefail

# Restart both grafana-local and iota-private-network. Run from the repo root.
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

# 1. Tear down grafana-local.
cd "$ROOT/dev-tools/grafana-local"
docker compose down -v

# 2. Reset and re-bootstrap the private network.
cd "$ROOT/dev-tools/iota-private-network"
sudo ./cleanup.sh
sudo ./bootstrap.sh -b -n 4

# 3. Start validators + fullnode + faucet. Protocol config env vars enable the
#    white-flag flow so iota-spammer's TransactionDriver path (`-V`) works.
IOTA_PROTOCOL_CONFIG_OVERRIDE_ENABLE=1 \
IOTA_PROTOCOL_CONFIG_FEATURE_FLAGS_OVERRIDE_ENABLE_WHITE_FLAG_FLOW=true \
  ./run.sh -n 4 faucet

# 4. Bring grafana-local back up.
cd "$ROOT/dev-tools/grafana-local"
docker compose up -d
