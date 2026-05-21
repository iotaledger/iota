#!/usr/bin/env bash
# End-to-end runner for the post-consensus load shedding stress experiment.
# This is the SOLE entry point for running the experiment. Do not call
# stress-pcool-load-shedding.sh directly — this script handles image
# staleness, per-validator config, and orchestration that the inner wrapper
# can't see on its own.
#
# Does, in order:
#   1. Rebuild the iota-node docker image (skipped with --no-rebuild)
#   2. Tear down any running private network, clean data directories
#   3. Re-bootstrap the private network with benchmark gas accounts
#   4. Apply per-validator YAML overrides (asymmetric delays) BEFORE bring-up
#      so containers load the right config on first start
#   5. Bring it up with white-flag flow enabled
#   6. Start (or refresh) the local Grafana / Prometheus stack
#   7. Wait for fullnode + Grafana to be reachable
#   8. Verify the running iota-node binary actually has the new
#      execution_delay_ms / overload_signal_* fields (catches stale images)
#   9. Open the post-consensus dashboard in the browser
#  10. Exec the stress wrapper, forwarding env vars and extra args
#
# Usage:
#   ./run-pcool-experiment.sh                                       # full pipeline, defaults
#   ./run-pcool-experiment.sh --no-rebuild                          # skip docker image rebuild
#   ./run-pcool-experiment.sh --no-browser                          # don't open the dashboard URL
#   ./run-pcool-experiment.sh --num-validators 8                    # larger committee
#   ./run-pcool-experiment.sh -- --benchmark-stats-path /tmp/x.json # forward extra args to stress.rs
#
# Tunable env vars (all pass through to the stress wrapper, plus a few
# orchestrator-only ones marked with *):
#   QPS                                 target offered load (default 2000)
#   DURATION                            stress duration (default 120s)
#   WORKERS                             stress.rs worker count (default 12)
#   IN_FLIGHT_RATIO                     per-account outstanding-tx cap (default 5)
#   NUM_TRANSFER_ACCOUNTS               2 is the sweet spot (default 2)
#   NUM_CLIENT_THREADS                  stress.rs client threads (default 4)
#   TRANSFER_OBJECT_PCT / SHARED_COUNTER_PCT  workload mix (default 100/0)
#   EXECUTION_DELAY_MS                  symmetric per-tx execution delay in ms
#                                       (default 100 — set 0 for baseline)
#   EXECUTION_DELAY_MS_PER_VALIDATOR    comma list per validator, e.g.
#                                       "0,0,0,200" — overrides the symmetric
#                                       default. Asymmetric runs go through here.
#   FULLNODE_RPC                        comma-separated fullnode JSON-RPC URLs
#                                       (default http://127.0.0.1:9000)
#   PROM_URL                            Prometheus base URL (default http://127.0.0.1:9090)
#   RUNS_DIR                            where artifacts land (default ./runs)
#
# Examples:
#   QPS=5000 DURATION=180s ./run-pcool-experiment.sh
#   EXECUTION_DELAY_MS_PER_VALIDATOR=0,0,0,200 ./run-pcool-experiment.sh
#   EXECUTION_DELAY_MS=0 ./run-pcool-experiment.sh --no-browser  # baseline run

set -euo pipefail

if ROOT="$(git rev-parse --show-toplevel 2>/dev/null)" && [[ -n "$ROOT" ]]; then
  :
else
  ROOT="$(cd "$(dirname "$0")" && pwd)"
fi
cd "$ROOT"

PRIVNET="$ROOT/dev-tools/iota-private-network"
GRAFANA="$ROOT/dev-tools/grafana-local"
DOCKER_NODE="$ROOT/docker/iota-node"
WRAPPER="$ROOT/stress-pcool-load-shedding.sh"
VALIDATOR_CONFIGS_DIR="$PRIVNET/configs/validators"
DASHBOARD_URL="http://localhost:3000/d/pcool-load-shedding-stress/post-consensus-load-shedding-stress-test?refresh=5s&from=now-15m&to=now"
PROM_URL="${PROM_URL:-http://127.0.0.1:9090}"

# Prometheus metric names the running validator MUST expose AT IDLE. Their
# absence on the /metrics endpoint at startup means the image was built from
# a tree that predated them, and the YAML overrides would be silently ignored
# at runtime.
#
# All entries here are scalar `IntGauge`/`IntCounter` or `*Vec` metrics whose
# label values are populated unconditionally on every overload-monitor tick
# (e.g. `authority_load_shedding_source` is `.set(...)`-ed for all three
# `source` labels). `authority_overload_notifications_received_total{from_authority}`
# is deliberately omitted from this list — the rust prometheus client only
# emits a `*Vec` metric on the wire once at least one labeled series has been
# observed, and that one only fires on receipt of a peer overload notification.
# It will appear in `/metrics` once the network actually goes into overload.
#
# We probe via `docker exec ... curl`, which works against the slim debian
# runtime image without needing any extra tooling installed inside the
# container.
EXPECTED_METRIC_NAMES=(
  authority_quorum_load_shedding_percentage
  post_consensus_load_shedding_dropped_transactions_total
  authority_load_shedding_source
  authority_overload_notifications_sent_total
  overload_signal_txn_ready_rate_tps
  overload_signal_execution_rate_tps
  overload_signal_cache_pending_count
)

NO_REBUILD=0
SKIP_BROWSER=0
NUM_VALIDATORS=4
PASS_THROUGH=()

while [[ $# -gt 0 ]]; do
  case "$1" in
    --no-rebuild)     NO_REBUILD=1; shift ;;
    --no-browser)     SKIP_BROWSER=1; shift ;;
    --num-validators) NUM_VALIDATORS="$2"; shift 2 ;;
    -h|--help)
      sed -n '2,60p' "$0" | sed 's/^# \{0,1\}//'
      exit 0 ;;
    --) shift; PASS_THROUGH=("$@"); break ;;
    *)  PASS_THROUGH=("$@"); break ;;
  esac
done

# --- platform helpers --------------------------------------------------------

is_macos() { [[ "$(uname -s)" == "Darwin" ]]; }

# Linux needs sudo for bootstrap.sh (chowns postgres data dir to 999:999).
# macOS Docker Desktop handles user-mapping internally — no sudo, and using sudo
# can leave files root-owned that subsequent runs can't clean up.
maybe_sudo() {
  if is_macos; then "$@"; else sudo "$@"; fi
}

open_url() {
  if is_macos; then
    open "$1"
  elif command -v xdg-open >/dev/null 2>&1; then
    xdg-open "$1" >/dev/null 2>&1 &
  else
    echo "    (no opener found — open manually: $1)"
  fi
}

# --- preflight ---------------------------------------------------------------

require() {
  command -v "$1" >/dev/null 2>&1 || { echo "ERROR: $1 not on PATH" >&2; exit 1; }
}
require docker
require curl
require bash
[[ -x "$WRAPPER" ]] || { echo "ERROR: $WRAPPER not executable" >&2; exit 1; }

rebuild_image() {
  echo "==> Rebuilding iota-node docker image..."
  if [[ ! -x "$DOCKER_NODE/build.sh" ]]; then
    echo "ERROR: $DOCKER_NODE/build.sh not found or not executable" >&2
    exit 1
  fi
  (cd "$DOCKER_NODE" && ./build.sh -t iota-node)
}

# --- 1. rebuild iota-node image ---------------------------------------------

if [[ $NO_REBUILD -eq 0 ]]; then
  echo "==> [1/10] Rebuilding iota-node docker image..."
  rebuild_image
else
  echo "==> [1/10] Skipping docker image rebuild (--no-rebuild)"
fi

# --- 2. teardown + clean ------------------------------------------------------

echo "==> [2/10] Tearing down existing private network..."
(cd "$PRIVNET" && docker compose down -v 2>&1 | tail -5) || true

echo "==> [3/10] Wiping ./data (host bind-mounted state)..."
# Try without sudo first (macOS / well-formed Linux); only escalate if that fails
# due to a legacy root-owned dir (left by a prior `sudo ./bootstrap.sh` on macOS).
if ! rm -rf "$PRIVNET/data" 2>/dev/null; then
  echo "    rm failed (likely root-owned residue from a previous sudo bootstrap); retrying with sudo..."
  sudo rm -rf "$PRIVNET/data"
fi

# --- 3. bootstrap (regenerates per-validator YAML) ---------------------------

echo "==> [4/10] Bootstrapping ($NUM_VALIDATORS validators, benchmark mode)..."
(cd "$PRIVNET" && maybe_sudo ./bootstrap.sh -b -n "$NUM_VALIDATORS")

# --- 4. per-validator YAML overrides (asymmetric delays) ---------------------
# Runs after bootstrap (YAML files exist) and before bring-up (containers
# haven't started yet), so containers come up with the correct config on
# first start — no restart dance required.

apply_per_validator_delays() {
  local list="${EXECUTION_DELAY_MS_PER_VALIDATOR:-}"
  if [[ -z "$list" ]]; then
    echo "==> [5/10] No per-validator delays set (EXECUTION_DELAY_MS_PER_VALIDATOR empty)."
    return 0
  fi
  require yq
  echo "==> [5/10] Applying per-validator execution-delay-ms overrides..."
  IFS=',' read -ra delays <<<"$list"
  local i=1
  for delay in "${delays[@]}"; do
    local cfg="$VALIDATOR_CONFIGS_DIR/validator-${i}-8080.yaml"
    if [[ ! -f "$cfg" ]]; then
      echo "    WARN: $cfg not found, skipping" >&2
      i=$((i+1))
      continue
    fi
    delay="$delay" yq -i '.authority-overload-config.execution-delay-ms = (env(delay) | tonumber)' "$cfg"
    echo "    validator-$i -> execution-delay-ms=$delay"
    i=$((i+1))
  done
}
apply_per_validator_delays

# --- 5. bring up validators + faucet -----------------------------------------

echo "==> [6/10] Starting private network with white-flag flow enabled..."
(cd "$PRIVNET" && \
  IOTA_PROTOCOL_CONFIG_OVERRIDE_ENABLE=1 \
  IOTA_PROTOCOL_CONFIG_FEATURE_FLAGS_OVERRIDE_ENABLE_WHITE_FLAG_FLOW=true \
  ./run.sh faucet)

# --- 6. grafana + prometheus stack -------------------------------------------

echo "==> [7/10] Starting Grafana + Prometheus stack..."
(cd "$GRAFANA" && docker compose up -d 2>&1 | tail -5)

# --- 7. wait for services to be reachable ------------------------------------

wait_for() {
  local name="$1" url="$2" attempts="${3:-60}"
  printf "    waiting for %s " "$name"
  for _ in $(seq 1 "$attempts"); do
    if curl -sS -o /dev/null -m 2 "$url" 2>/dev/null; then
      echo " ok."
      return 0
    fi
    printf "."
    sleep 1
  done
  echo " TIMEOUT after ${attempts}s."
  return 1
}

# JSON-RPC handshake returns 405 for GET, which still indicates listening; count
# any 2xx/4xx as "up".
wait_for_rpc() {
  printf "    waiting for fullnode JSON-RPC "
  for _ in $(seq 1 60); do
    code=$(curl -sS -o /dev/null -w "%{http_code}" -m 2 http://127.0.0.1:9000 2>/dev/null || true)
    case "$code" in
      2*|4*) echo " ok ($code)."; return 0 ;;
    esac
    printf "."
    sleep 1
  done
  echo " TIMEOUT."
  return 1
}

echo "==> [8/10] Waiting for services..."
wait_for_rpc                              || echo "    (fullnode not responding — check 'docker compose logs fullnode-1')"
wait_for "grafana"   http://127.0.0.1:3000/api/health 30 || echo "    (grafana not responding — check 'docker compose logs grafana' in $GRAFANA)"
wait_for "prometheus" http://127.0.0.1:9090/-/ready  30 || echo "    (prometheus not responding)"
sleep 5   # genesis-setup churn grace

# --- 8. staleness check via /metrics ----------------------------------------
# A running validator with a stale image silently ignores YAML keys it
# doesn't know about. This was the symptom that motivated routing all runs
# through this script: an asymmetric delay was written into the YAML but the
# running binary had no `execution_delay_ms` field, so the validator ran at
# full speed and we saw nothing on the dashboard. Catch it here by probing
# the /metrics endpoint for the metric names that only exist after the
# post-consensus changes landed.

probe_validator_metrics() {
  # Returns the validator-1 /metrics body on stdout, or empty on failure.
  # The runtime image ships curl but not wget or strings, so probe over curl.
  docker exec validator-1 sh -c \
    'curl -sS --max-time 5 http://localhost:9184/metrics' 2>/dev/null
}

check_image_staleness() {
  echo "==> [9/10] Verifying running iota-node exposes all expected metrics..."
  # Give the metrics endpoint a moment to bind in case startup is still in flight.
  local metrics=""
  for _ in $(seq 1 30); do
    metrics="$(probe_validator_metrics)"
    [[ -n "$metrics" ]] && break
    sleep 1
  done
  if [[ -z "$metrics" ]]; then
    echo "    /metrics endpoint did not respond — check 'docker compose logs validator-1' in $PRIVNET" >&2
    return 1
  fi

  local missing=()
  for name in "${EXPECTED_METRIC_NAMES[@]}"; do
    if ! grep -qE "^# (HELP|TYPE) ${name}( |\$)" <<<"$metrics"; then
      missing+=("$name")
    fi
  done

  if [[ ${#missing[@]} -gt 0 ]]; then
    echo "    STALE IMAGE — these metrics are missing on the running validator:" >&2
    printf '      %s\n' "${missing[@]}" >&2
    if [[ $NO_REBUILD -eq 1 ]]; then
      echo "    --no-rebuild was passed but the image is stale. Forcing a rebuild now." >&2
      rebuild_image
      echo "    Bouncing the network so the new image takes effect..."
      (cd "$PRIVNET" && docker compose down -v 2>&1 | tail -3) || true
      if ! rm -rf "$PRIVNET/data" 2>/dev/null; then sudo rm -rf "$PRIVNET/data"; fi
      (cd "$PRIVNET" && maybe_sudo ./bootstrap.sh -b -n "$NUM_VALIDATORS")
      apply_per_validator_delays
      (cd "$PRIVNET" && \
        IOTA_PROTOCOL_CONFIG_OVERRIDE_ENABLE=1 \
        IOTA_PROTOCOL_CONFIG_FEATURE_FLAGS_OVERRIDE_ENABLE_WHITE_FLAG_FLOW=true \
        ./run.sh faucet)
      wait_for_rpc || true
      sleep 5
      # Re-probe; if still missing, the rebuild is genuinely broken.
      metrics="$(probe_validator_metrics)"
      local still_missing=()
      for name in "${EXPECTED_METRIC_NAMES[@]}"; do
        grep -qE "^# (HELP|TYPE) ${name}( |\$)" <<<"$metrics" || still_missing+=("$name")
      done
      if [[ ${#still_missing[@]} -gt 0 ]]; then
        echo "    ERROR: rebuild ran but /metrics still missing:" >&2
        printf '      %s\n' "${still_missing[@]}" >&2
        echo "    Check that the relevant register_int_*_with_registry! calls exist in iota-core" >&2
        echo "    and that docker/iota-node/Dockerfile's COPY of crates/ is including your edits." >&2
        exit 1
      fi
      echo "    ok — all expected metrics present after rebuild."
    else
      echo "    ERROR: rebuild ran in step [1/10] but the image still doesn't expose these metrics." >&2
      echo "    Check that the relevant register_int_*_with_registry! calls exist in iota-core" >&2
      echo "    and that docker/iota-node/Dockerfile's COPY of crates/ is including your edits." >&2
      exit 1
    fi
  else
    echo "    ok — all expected metrics present."
  fi
}
check_image_staleness

# --- 9. open dashboard --------------------------------------------------------

echo "==> [10/10] Opening dashboard..."
if [[ $SKIP_BROWSER -eq 0 ]]; then
  open_url "$DASHBOARD_URL"
else
  echo "    Dashboard: $DASHBOARD_URL"
fi

# --- 10. run stress (env vars + any -- args pass through) --------------------

echo "==> Running stress test..."
echo "----------------------------------------------------------------"
# Use ${arr[@]+"${arr[@]}"} so an empty PASS_THROUGH array doesn't trip the
# bash 3.2 (macOS default) "unbound variable" check under `set -u`.
exec "$WRAPPER" ${PASS_THROUGH[@]+"${PASS_THROUGH[@]}"}
