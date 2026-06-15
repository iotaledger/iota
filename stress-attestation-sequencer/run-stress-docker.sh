#!/usr/bin/env bash
#
# run-stress-docker.sh — run the `stress` benchmark INSIDE the private-network
# docker network so it can submit DIRECTLY to validators (bypassing the
# fullnode). The proxy then auto-selects the driver the same way the fullnode
# orchestrator does: TransactionDriver when white-flag flow is on, QuorumDriver
# when off (detected from the fullnode's protocol config over RPC).
#
# Why in-docker instead of exposing validator ports: validators publish no host
# ports and advertise docker-internal DNS addresses (validator-N:8080) in
# genesis, so a host-side client can't reach them. Running here needs ZERO
# genesis/compose changes and leaves the faucet/fullnode paths intact.
#
# Runner image: the network's own `iotaledger/iota-tools`, which already ships
# `/usr/local/bin/stress` (built from source by docker/iota-tools/Dockerfile).
# Using the same image keeps the ABI consistent — no foreign image, no glibc
# matching, no host binary to mount.
#
# IMPORTANT: the image's `stress` reflects whatever code was in the image build.
# To exercise UNCOMMITTED changes (e.g. the direct-to-validator TD work), rebuild
# the iota-tools image from this branch first, then run this script.
#
# Prereq: the network is up (start.sh) so the genesis files and docker network exist.
#
# Tunables (env): RUN_DURATION, TARGET_QPS, NUM_WORKERS, NUM_CLIENT_THREADS,
#                 NUM_TRANSFER_ACCOUNTS, IN_FLIGHT_RATIO, PRIMARY_GAS_OWNER,
#                 RUNNER_IMAGE, DOCKER_NETWORK, FULLNODE_RPC,
#                 USE_FULLNODE_FOR_EXECUTION.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
GENESIS_DIR="$REPO_ROOT/dev-tools/iota-private-network/configs/genesis"

# Workload knobs (mirror h1-attestation-overhead.sh).
RUN_DURATION="${RUN_DURATION:-30s}"
TARGET_QPS="${TARGET_QPS:-2000}"
NUM_WORKERS="${NUM_WORKERS:-24}"
NUM_CLIENT_THREADS="${NUM_CLIENT_THREADS:-12}"
NUM_TRANSFER_ACCOUNTS="${NUM_TRANSFER_ACCOUNTS:-4}"
IN_FLIGHT_RATIO="${IN_FLIGHT_RATIO:-2}"
PRIMARY_GAS_OWNER="${PRIMARY_GAS_OWNER:-0xf479d29837d22943aba6afc401f518a36521b990874eca784886185bd26bf681}"

# Runner: the network's own image (already contains `stress`).
RUNNER_IMAGE="${RUNNER_IMAGE:-iotaledger/iota-tools:latest}"
DOCKER_NETWORK="${DOCKER_NETWORK:-iota-private-network_iota-network}"
# In-network DNS (NOT 127.0.0.1): used for reconfig + white-flag-flow detection.
FULLNODE_RPC="${FULLNODE_RPC:-http://fullnode-1:9000}"
# false => direct-to-validator (the point of this runner); true => via fullnode.
USE_FULLNODE_FOR_EXECUTION="${USE_FULLNODE_FOR_EXECUTION:-false}"

if [[ ! -f "$GENESIS_DIR/genesis.blob" ]]; then
  echo "ERROR: $GENESIS_DIR/genesis.blob not found (bootstrap the network first)." >&2
  exit 1
fi
if ! docker network inspect "$DOCKER_NETWORK" >/dev/null 2>&1; then
  echo "ERROR: docker network '$DOCKER_NETWORK' not found. Start the network first (start.sh)." >&2
  exit 1
fi

echo "Running stress IN-DOCKER on '$DOCKER_NETWORK' (image: $RUNNER_IMAGE)"
echo "  fullnode RPC (reconfig + WFF detect): $FULLNODE_RPC"
echo "  use-fullnode-for-execution: $USE_FULLNODE_FOR_EXECUTION (false => direct-to-validator)"

# Mount the host-generated genesis read-only; attach to the validators' network.
# `stress` itself comes from the image (--entrypoint), not the host.
exec docker run --rm \
  --network "$DOCKER_NETWORK" \
  --ulimit nofile=524288:524288 \
  -v "$GENESIS_DIR":/genesis:ro \
  --entrypoint /usr/local/bin/stress \
  "$RUNNER_IMAGE" \
  --local false \
  --fullnode-rpc-addresses "$FULLNODE_RPC" \
  --use-fullnode-for-execution "$USE_FULLNODE_FOR_EXECUTION" \
  --use-fullnode-for-reconfig true \
  --genesis-blob-path /genesis/genesis.blob \
  --keystore-path /genesis/benchmark.keystore \
  --primary-gas-owner-id "$PRIMARY_GAS_OWNER" \
  --num-client-threads "$NUM_CLIENT_THREADS" \
  --num-transfer-accounts "$NUM_TRANSFER_ACCOUNTS" \
  --run-duration "$RUN_DURATION" \
  bench --target-qps "$TARGET_QPS" \
  --in-flight-ratio "$IN_FLIGHT_RATIO" \
  --num-workers "$NUM_WORKERS" \
  --transfer-object 100 \
  --shared-counter 0
