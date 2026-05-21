#!/usr/bin/env bash
# Inner stress wrapper for post-consensus load shedding.
#
# DO NOT INVOKE THIS DIRECTLY. Use ./run-pcool-experiment.sh — it handles
# image rebuilds, network teardown/bootstrap, per-validator config overrides,
# and binary-staleness checks that this script cannot see on its own. Running
# this wrapper directly will silently miss those, which has burned us before.
#
# What this wrapper does: assumes a freshly-bootstrapped network is up,
# captures the deployed config + a Prometheus pre-flight snapshot, runs
# stress.rs for the configured duration, then dumps Prometheus series + a
# summary into runs/<utc-ts>/. The orchestrator script invokes it as the
# final step.

set -euo pipefail

ROOT="$(git rev-parse --show-toplevel 2>/dev/null || realpath "$(dirname "$0")")"
cd "$ROOT"

# ---- knobs (override via env) -----------------------------------------------
QPS="${QPS:-2000}"
DURATION="${DURATION:-120s}"
WORKERS="${WORKERS:-12}"
IN_FLIGHT_RATIO="${IN_FLIGHT_RATIO:-5}"
NUM_TRANSFER_ACCOUNTS="${NUM_TRANSFER_ACCOUNTS:-2}"
NUM_CLIENT_THREADS="${NUM_CLIENT_THREADS:-4}"
TRANSFER_OBJECT_PCT="${TRANSFER_OBJECT_PCT:-100}"
SHARED_COUNTER_PCT="${SHARED_COUNTER_PCT:-0}"

EXECUTION_DELAY_MS="${EXECUTION_DELAY_MS:-100}"
EXECUTION_DELAY_MS_PER_VALIDATOR="${EXECUTION_DELAY_MS_PER_VALIDATOR:-}"

FULLNODE_RPC="${FULLNODE_RPC:-http://127.0.0.1:9000}"
PROM_URL="${PROM_URL:-http://127.0.0.1:9090}"
RUNS_DIR="${RUNS_DIR:-$ROOT/runs}"

PRIVNET_DIR="$ROOT/dev-tools/iota-private-network"
GENESIS_BLOB="${GENESIS_BLOB:-$PRIVNET_DIR/configs/genesis/genesis.blob}"
KEYSTORE="${KEYSTORE:-$PRIVNET_DIR/configs/genesis/benchmark.keystore}"
PRIMARY_GAS_OWNER="${PRIMARY_GAS_OWNER:-0xf479d29837d22943aba6afc401f518a36521b990874eca784886185bd26bf681}"
VALIDATOR_CONFIGS_DIR="${VALIDATOR_CONFIGS_DIR:-$PRIVNET_DIR/configs/validators}"

# ---- snapshot deployed config ----------------------------------------------
TS="$(date -u +%Y%m%dT%H%M%SZ)"
RUN_DIR="$RUNS_DIR/$TS"
mkdir -p "$RUN_DIR"

snapshot_validator_configs() {
  for cfg in "$VALIDATOR_CONFIGS_DIR"/validator-*-8080.yaml; do
    [[ -f "$cfg" ]] || continue
    local name
    name="$(basename "$cfg")"
    {
      echo "# $name"
      yq '.authority-overload-config' "$cfg" 2>/dev/null || echo "(no authority-overload-config block)"
      echo
    } >>"$RUN_DIR/deployed-config.txt"
  done
}

if command -v yq >/dev/null 2>&1; then
  snapshot_validator_configs
fi

# Pre-flight: warn if the network looks like it's still grinding through a
# pre-existing backlog from a previous run. With Semaphore::new(1) and a
# non-zero execution_delay_ms the validators throttle to ~10 TPS, so any
# pre-existing pending-cert count > a few hundred means stress traffic will
# spend the run sitting behind that backlog. Cleared with:
#   docker compose down -v && sudo rm -rf data/ && sudo ./bootstrap.sh -b -n 4
preflight_check() {
  local q
  q=$(curl -sG "$PROM_URL/api/v1/query" --data-urlencode \
        'query=max(transaction_manager_num_pending_certificates)' 2>/dev/null \
      | python3 -c 'import json,sys
try: r=json.load(sys.stdin); print(int(float(r["data"]["result"][0]["value"][1])))
except Exception: print(0)' 2>/dev/null)
  q=${q:-0}
  if [[ "$q" -gt 500 ]]; then
    echo "WARN: transaction_manager_num_pending_certificates=$q before stress starts." >&2
    echo "      Stale backlog from a previous run will block your stress traffic." >&2
    echo "      Recommended:" >&2
    echo "        cd dev-tools/iota-private-network && docker compose down -v && sudo rm -rf data/" >&2
    echo "        sudo ./bootstrap.sh -b -n 4 && ./run.sh faucet" >&2
    echo "      Continuing anyway." >&2
  fi
}
preflight_check

START_EPOCH="$(date +%s)"
{
  echo "ts=$TS"
  echo "qps=$QPS"
  echo "duration=$DURATION"
  echo "workers=$WORKERS"
  echo "in_flight_ratio=$IN_FLIGHT_RATIO"
  echo "num_transfer_accounts=$NUM_TRANSFER_ACCOUNTS"
  echo "num_client_threads=$NUM_CLIENT_THREADS"
  echo "transfer_object_pct=$TRANSFER_OBJECT_PCT"
  echo "shared_counter_pct=$SHARED_COUNTER_PCT"
  echo "execution_delay_ms=$EXECUTION_DELAY_MS"
  echo "execution_delay_ms_per_validator=$EXECUTION_DELAY_MS_PER_VALIDATOR"
  echo "fullnode_rpc=$FULLNODE_RPC"
  echo "primary_gas_owner=$PRIMARY_GAS_OWNER"
  echo "start_epoch=$START_EPOCH"
} >"$RUN_DIR/params.txt"

# ---- run stress -------------------------------------------------------------
ulimit -n 65536 || true

STRESS_CMD=(
  cargo run --release -p iota-benchmark --bin stress --
    --local false
    --fullnode-rpc-addresses "$FULLNODE_RPC"
    --use-fullnode-for-execution true
    --use-fullnode-for-reconfig true
    --genesis-blob-path "$GENESIS_BLOB"
    --keystore-path "$KEYSTORE"
    --primary-gas-owner-id "$PRIMARY_GAS_OWNER"
    --num-client-threads "$NUM_CLIENT_THREADS"
    --num-transfer-accounts "$NUM_TRANSFER_ACCOUNTS"
    --run-duration "$DURATION"
    bench
    --target-qps "$QPS"
    --num-workers "$WORKERS"
    --in-flight-ratio "$IN_FLIGHT_RATIO"
    --transfer-object "$TRANSFER_OBJECT_PCT"
    --shared-counter "$SHARED_COUNTER_PCT"
    "$@"
)

echo "Stress run started at $TS"
echo "Writing artifacts to $RUN_DIR"

# Pre-build so a stale cargo error doesn't end up as an empty stdout log under tee.
# `cargo run` would rebuild silently and the failure mode is invisible if stderr is
# buffered. Build first; abort if the build itself fails.
echo "Building stress binary..."
if ! cargo build --release -p iota-benchmark --bin stress 2>&1 | tee "$RUN_DIR/build.log"; then
  echo "ERROR: cargo build failed. See $RUN_DIR/build.log" >&2
  echo "exit_code=2" >>"$RUN_DIR/params.txt"
  exit 2
fi

set +e
# Plain pipe with tee — portable across macOS (BSD) and Linux (GNU).
# stress.rs disables colors when not on a TTY, which is fine for log archiving.
RUST_LOG=warn "${STRESS_CMD[@]}" 2>&1 | tee "$RUN_DIR/stress-stdout.log"
exit_code=${PIPESTATUS[0]}
set -e
END_EPOCH="$(date +%s)"
echo "end_epoch=$END_EPOCH" >>"$RUN_DIR/params.txt"
echo "exit_code=$exit_code" >>"$RUN_DIR/params.txt"

# ---- prometheus capture -----------------------------------------------------
# Pad the window by a few seconds on each side so we catch the ramp and the
# recovery tail. Step of 1s matches prometheus.yaml's scrape_interval.
FROM=$((START_EPOCH - 5))
TO=$((END_EPOCH + 30))
STEP=1
RATE_WINDOW=30s

query_range() {
  local name="$1" q="$2"
  curl -sG "$PROM_URL/api/v1/query_range" \
    --data-urlencode "query=$q" \
    --data-urlencode "start=$FROM" \
    --data-urlencode "end=$TO" \
    --data-urlencode "step=$STEP" \
    >"$RUN_DIR/$name.json"
}

query_instant_total() {
  local name="$1" q="$2"
  curl -sG "$PROM_URL/api/v1/query" \
    --data-urlencode "query=$q" \
    --data-urlencode "time=$TO" \
    >"$RUN_DIR/$name.json"
}

echo "Capturing prometheus data from $PROM_URL ..."

query_range local_pct          'authority_load_shedding_percentage'
query_range source_pct         'authority_load_shedding_source'
query_range sent_rate          "rate(authority_overload_notifications_sent_total[$RATE_WINDOW])"
query_range received_rate      "sum by (host, from_authority) (rate(authority_overload_notifications_received_total[$RATE_WINDOW]))"
query_range last_recv_pct      'authority_overload_notification_last_received_percentage'
query_range quorum_pct         'authority_quorum_load_shedding_percentage'
query_range dropped_rate       "rate(post_consensus_load_shedding_dropped_transactions_total[$RATE_WINDOW])"
query_range useful_tps         "rate(total_transaction_effects[$RATE_WINDOW])"
query_range queue_p99          "histogram_quantile(0.99, sum by (host, le) (rate(execution_queueing_delay_s_bucket[$RATE_WINDOW])))"

query_instant_total sent_total      "increase(authority_overload_notifications_sent_total[${TO}s] @ ${TO})"
query_instant_total dropped_total   "increase(post_consensus_load_shedding_dropped_transactions_total[${TO}s] @ ${TO})"

# ---- summary ---------------------------------------------------------------
write_summary() {
  local out="$RUN_DIR/summary.txt"
  : >"$out"

  python3 - "$RUN_DIR" >>"$out" <<'PY'
import json, os, sys, statistics

run_dir = sys.argv[1]

def load(name):
    path = os.path.join(run_dir, name + ".json")
    if not os.path.exists(path):
        return None
    with open(path) as f:
        return json.load(f)

def by_host(data):
    out = {}
    if not data or data.get("status") != "success":
        return out
    for s in data.get("data", {}).get("result", []):
        host = s["metric"].get("host", "?")
        labels = {k: v for k, v in s["metric"].items() if k != "host"}
        vals = [float(v[1]) for v in s.get("values", [])]
        out.setdefault(host, []).append((labels, vals))
    return out

def reduce_series(values_list, fn):
    flat = [v for _, vals in values_list for v in vals if not (v != v)]
    return fn(flat) if flat else float("nan")

def fmt(x):
    return f"{x:.3f}" if isinstance(x, float) else str(x)

print("=== Per-host summary ===")
for metric_name, fn_max, fn_avg, fn_final in [
    ("local_pct",     "peak",  "avg",  "final"),
    ("quorum_pct",    "peak",  "avg",  "final"),
    ("useful_tps",    "peak",  "avg",  "final"),
    ("sent_rate",     "peak",  "avg",  "final"),
    ("dropped_rate",  "peak",  "avg",  "final"),
    ("queue_p99",     "peak",  "avg",  "final"),
]:
    data = load(metric_name)
    if data is None:
        continue
    hosts = by_host(data)
    print(f"\n-- {metric_name} --")
    for host, series in sorted(hosts.items()):
        peak = reduce_series(series, max)
        avg = reduce_series(series, lambda vs: sum(vs)/len(vs))
        final = next((vals[-1] for _, vals in series if vals), float("nan"))
        print(f"  {host:>15s}  peak={fmt(peak):>8s}  avg={fmt(avg):>8s}  final={fmt(final):>8s}")

print("\n=== Counter totals over run window ===")
for name in ("sent_total", "dropped_total"):
    data = load(name)
    if data is None:
        continue
    print(f"\n-- {name} --")
    for s in data.get("data", {}).get("result", []):
        host = s["metric"].get("host", "?")
        val = s.get("value", ["", "nan"])[1]
        print(f"  {host:>15s}  total={val}")

print("\n=== Source breakdown (final percentage per host x source) ===")
data = load("source_pct")
if data and data.get("status") == "success":
    rows = {}
    for s in data.get("data", {}).get("result", []):
        host = s["metric"].get("host", "?")
        source = s["metric"].get("source", "?")
        vals = [float(v[1]) for v in s.get("values", [])]
        if vals:
            rows[(host, source)] = vals[-1]
    for (host, source), v in sorted(rows.items()):
        print(f"  {host:>15s}  {source:<22s} final={v:.1f}")
PY
}

if command -v python3 >/dev/null 2>&1; then
  write_summary
fi

echo ""
echo "Done. Artifacts in $RUN_DIR"
echo "  Summary:    $RUN_DIR/summary.txt"
echo "  Params:     $RUN_DIR/params.txt"
echo "  Stress log: $RUN_DIR/stress-stdout.log  (render with: less -R)"
echo ""
echo "Dashboard: http://localhost:3000/d/pcool-load-shedding-stress/post-consensus-load-shedding-stress-test?refresh=5s&from=now-15m&to=now"

exit "$exit_code"
