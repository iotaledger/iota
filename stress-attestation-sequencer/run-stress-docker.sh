#!/usr/bin/env bash
#
# run-stress-docker.sh — run the `stress` benchmark INSIDE the private-network
# docker network so it can submit DIRECTLY to validators (bypassing the
# fullnode). The proxy then auto-selects the driver the same way the fullnode
# orchestrator does: TransactionDriver when P-COOL flow is on, QuorumDriver
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
# In-network DNS (NOT 127.0.0.1): used for reconfig + P-COOL flow detection.
FULLNODE_RPC="${FULLNODE_RPC:-http://fullnode-1:9000}"
# false => direct-to-validator (the point of this runner); true => via fullnode.
USE_FULLNODE_FOR_EXECUTION="${USE_FULLNODE_FOR_EXECUTION:-false}"
# Pin submission/attestation to the first N validators (validator-1..N) on the
# direct TD path. Empty/unset => all validators. No effect via the fullnode.
NUM_TARGET_VALIDATORS="${NUM_TARGET_VALIDATORS:-}"
# Workload: owned (transfer) | shared (shared-counter) | slow (slow::slow).
# NOTE: shared/slow publish a Move package at runtime (compiled from repo sources
# that depend on the iota-framework). The iota-tools Dockerfile bakes those
# sources in (examples/move + iota-benchmark workload data + iota-framework/
# packages), so they work in this image — rebuild it after pulling that change.
WORKLOAD="${WORKLOAD:-owned}"
NUM_SHARED_COUNTERS="${NUM_SHARED_COUNTERS:-}" # WORKLOAD=shared: fewer => more congestion
SLOW_N="${SLOW_N:-}"                           # WORKLOAD=slow: slow::slow(n,size) vector count
SLOW_SIZE="${SLOW_SIZE:-}"                      # WORKLOAD=slow: each vector size in bytes
SLOW_SHARED="${SLOW_SHARED:-}"                  # WORKLOAD=slow: false => owned-only (no shared object / congestion)
case "$WORKLOAD" in
owned) WORKLOAD_ARGS=(--transfer-object 100 --shared-counter 0) ;;
shared)
  WORKLOAD_ARGS=(--transfer-object 0 --shared-counter 100)
  [[ -n "$NUM_SHARED_COUNTERS" ]] && WORKLOAD_ARGS+=(--num-shared-counters "$NUM_SHARED_COUNTERS")
  ;;
slow)
  WORKLOAD_ARGS=(--transfer-object 0 --slow 100)
  [[ -n "$SLOW_N" ]] && WORKLOAD_ARGS+=(--slow-n "$SLOW_N")
  [[ -n "$SLOW_SIZE" ]] && WORKLOAD_ARGS+=(--slow-size "$SLOW_SIZE")
  [[ -n "$SLOW_SHARED" ]] && WORKLOAD_ARGS+=(--slow-shared "$SLOW_SHARED")
  ;;
*) echo "ERROR: unknown WORKLOAD='$WORKLOAD' (owned | shared | slow)" >&2; exit 1 ;;
esac

# ANSI palette (auto-disabled when stdout is not a terminal), matching the H1 script.
if [[ -t 1 ]]; then
  RED=$'\033[0;31m'
  GREEN=$'\033[0;32m'
  YELLOW=$'\033[0;33m'
  BLUE=$'\033[0;34m'
  MAGENTA=$'\033[0;35m'
  CYAN=$'\033[0;36m'
  RESET=$'\033[0m'
else
  RED=''
  GREEN=''
  YELLOW=''
  BLUE=''
  MAGENTA=''
  CYAN=''
  RESET=''
fi

if [[ ! -f "$GENESIS_DIR/genesis.blob" ]]; then
  echo "${RED}ERROR: $GENESIS_DIR/genesis.blob not found (bootstrap the network first).${RESET}" >&2
  exit 1
fi
if ! docker network inspect "$DOCKER_NETWORK" >/dev/null 2>&1; then
  echo "${RED}ERROR: docker network '$DOCKER_NETWORK' not found. Start the network first (start.sh).${RESET}" >&2
  exit 1
fi


echo "${BLUE}Running stress IN-DOCKER on '$DOCKER_NETWORK' (image: $RUNNER_IMAGE)...${RESET}"
echo "  - fullnode RPC (reconfig + WFF detect): $FULLNODE_RPC"
echo "  - use-fullnode-for-execution: $USE_FULLNODE_FOR_EXECUTION (false => direct-to-validator)"
echo "  - workload: $WORKLOAD"

# Mount the host-generated genesis read-only; attach to the validators' network.
# `stress` itself comes from the image (--entrypoint), not the host.
# Only pass --num-target-validators when set (unset => all validators).
target_args=()
if [[ -n "$NUM_TARGET_VALIDATORS" ]]; then
  target_args=(--num-target-validators "$NUM_TARGET_VALIDATORS")
  echo "  - pinning submission/attestation to first $NUM_TARGET_VALIDATORS validator(s)"
fi
echo

exec docker run --rm \
  --network "$DOCKER_NETWORK" \
  --ulimit nofile=524288:524288 \
  -v "$GENESIS_DIR":/genesis:ro \
  -e MOVE_EXAMPLES_DIR=/iota/examples/move \
  -e BENCHMARK_MOVE_BASE_DIR=/iota/crates/iota-benchmark \
  --entrypoint /usr/local/bin/stress \
  "$RUNNER_IMAGE" \
  --local false \
  --fullnode-rpc-addresses "$FULLNODE_RPC" \
  --use-fullnode-for-execution "$USE_FULLNODE_FOR_EXECUTION" \
  --use-fullnode-for-reconfig true \
  "${target_args[@]}" \
  --genesis-blob-path /genesis/genesis.blob \
  --keystore-path /genesis/benchmark.keystore \
  --primary-gas-owner-id "$PRIMARY_GAS_OWNER" \
  --num-client-threads "$NUM_CLIENT_THREADS" \
  --num-transfer-accounts "$NUM_TRANSFER_ACCOUNTS" \
  --run-duration "$RUN_DURATION" \
  bench --target-qps "$TARGET_QPS" \
  --in-flight-ratio "$IN_FLIGHT_RATIO" \
  --num-workers "$NUM_WORKERS" \
  "${WORKLOAD_ARGS[@]}"
