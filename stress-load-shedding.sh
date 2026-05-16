#!/bin/bash
# Stress test the pre-consensus graduated load-shedding path against the
# local 4-validator network bootstrapped with `dev-tools/iota-private-network/bootstrap.sh -b`.
#
# Override defaults via env: QPS=5000 DURATION=300s ./stress-load-shedding.sh
# Pass extra args directly to stress.rs:    ./stress-load-shedding.sh --benchmark-stats-path /tmp/run.json
#
# After the run, queries Prometheus over the run window and writes per-metric
# JSON, a flat CSV, and a human-readable summary to runs/<utc-timestamp>/.

set -euo pipefail

ulimit -n 65536 || echo "warning: could not raise FD limit (continuing with current ulimit -n=$(ulimit -n))" >&2

command -v jq >/dev/null || { echo "Error: jq required for metrics capture" >&2; exit 1; }
command -v curl >/dev/null || { echo "Error: curl required for metrics capture" >&2; exit 1; }

REPO_ROOT="$(git rev-parse --show-toplevel)"
cd "$REPO_ROOT"

QPS="${QPS:-2000}"
DURATION="${DURATION:-120s}"
WORKERS="${WORKERS:-12}"
IN_FLIGHT_RATIO="${IN_FLIGHT_RATIO:-5}"
BURST_SIZE="${BURST_SIZE:-1}"
NUM_TRANSFER_ACCOUNTS="${NUM_TRANSFER_ACCOUNTS:-10}"
NUM_CLIENT_THREADS="${NUM_CLIENT_THREADS:-4}"
# 0 = TD spams all validators (default amplification factor).
# 1 = pin all spam to one validator → 4× per-validator gate pressure.
# 1..committee_size = subset of validators by sorted display name.
NUM_VALIDATORS_TO_TARGET="${NUM_VALIDATORS_TO_TARGET:-0}"
TRANSFER_OBJECT_PCT="${TRANSFER_OBJECT_PCT:-100}"
SHARED_COUNTER_PCT="${SHARED_COUNTER_PCT:-0}"
FULLNODE_RPC="${FULLNODE_RPC:-http://127.0.0.1:9000}"
# Full comma-separated list of all fullnode URLs. When set (typically by
# stress-multi.sh), parallel gas-generation branches fan out across all of
# them round-robin instead of hammering a single fullnode. Defaults to just
# the primary FULLNODE_RPC if unset.
FULLNODE_RPC_ALL="${FULLNODE_RPC_ALL:-$FULLNODE_RPC}"
CLIENT_METRIC_PORT="${CLIENT_METRIC_PORT:-8081}"
READY_FILE="${READY_FILE:-}"
START_FILE="${START_FILE:-}"
BARRIER_PERIOD_MS="${BARRIER_PERIOD_MS:-0}"
GAS_CHUNK_SIZE="${GAS_CHUNK_SIZE:-500}"
GAS_POOL_CACHE_PATH="${GAS_POOL_CACHE_PATH:-}"
PRIMARY_GAS_OWNER="${PRIMARY_GAS_OWNER:-0xf479d29837d22943aba6afc401f518a36521b990874eca784886185bd26bf681}"
LOG_LEVEL="${RUST_LOG:-info}"
PROM_URL="${PROM_URL:-http://localhost:9090}"
RUNS_DIR="${RUNS_DIR:-runs}"

GENESIS_BLOB="dev-tools/iota-private-network/configs/genesis/genesis.blob"
KEYSTORE="dev-tools/iota-private-network/configs/genesis/benchmark.keystore"

if [[ ! -f "$GENESIS_BLOB" || ! -f "$KEYSTORE" ]]; then
    echo "Error: genesis.blob or benchmark.keystore missing." >&2
    echo "       Run: cd dev-tools/iota-private-network && sudo ./bootstrap.sh -b -n 4" >&2
    exit 1
fi

start_epoch=$(date +%s)
ts=$(date -u -d "@$start_epoch" +%Y-%m-%dT%H-%M-%SZ)
out="$RUNS_DIR/$ts"
mkdir -p "$out"

# Snapshot validator-side config so we know which limits + start_pct the run was using.
VALIDATOR_CFG="dev-tools/iota-private-network/configs/validators/validator-1-8080.yaml"
max_pending="?"
start_pct="?"
if [[ -f "$VALIDATOR_CFG" ]]; then
    max_pending=$(awk '/max-pending-transactions:/{print $2; exit}' "$VALIDATOR_CFG" || echo "?")
    start_pct=$(awk '/graduated-load-shedding-soft-limit-pct:/{print $2; exit}' "$VALIDATOR_CFG" || echo "?")
fi

# Detect white-flag flow (env on the running validator container).
white_flag="?"
if command -v docker >/dev/null 2>&1; then
    white_flag=$(docker exec validator-1 sh -c 'echo "${IOTA_PROTOCOL_CONFIG_FEATURE_FLAGS_OVERRIDE_ENABLE_WHITE_FLAG_FLOW:-unset}"' 2>/dev/null || echo "?")
fi

cat > "$out/params.txt" <<EOF
ts_utc=$ts
start_epoch=$start_epoch
QPS=$QPS
DURATION=$DURATION
WORKERS=$WORKERS
IN_FLIGHT_RATIO=$IN_FLIGHT_RATIO
BURST_SIZE=$BURST_SIZE
BARRIER_PERIOD_MS=$BARRIER_PERIOD_MS
GAS_CHUNK_SIZE=$GAS_CHUNK_SIZE
NUM_TRANSFER_ACCOUNTS=$NUM_TRANSFER_ACCOUNTS
NUM_CLIENT_THREADS=$NUM_CLIENT_THREADS
TRANSFER_OBJECT_PCT=$TRANSFER_OBJECT_PCT
SHARED_COUNTER_PCT=$SHARED_COUNTER_PCT
FULLNODE_RPC=$FULLNODE_RPC
FULLNODE_RPC_ALL=$FULLNODE_RPC_ALL
PROM_URL=$PROM_URL
max_pending_transactions=$max_pending
graduated_load_shed_start_pct=$start_pct
white_flag_flow=$white_flag
extra_args="$*"
EOF

echo "=> stress run: qps=$QPS duration=$DURATION workers=$WORKERS in_flight_ratio=$IN_FLIGHT_RATIO burst=$BURST_SIZE barrier_ms=$BARRIER_PERIOD_MS"
echo "=> output:    $out/"
echo "=> dashboard: http://localhost:3000/d/load-shedding-stress/load-shedding-stress-test?refresh=5s&from=now-5m&to=now"
echo

# `script -qe -c` allocates a PTY so cargo/stress.rs see a TTY on stdout and
# emit colored tracing output. The saved log keeps ANSI escapes — view with
# `less -R runs/<ts>/stress-stdout.log`, strip via
# `sed -r 's/\x1b\[[0-9;]*[mGKH]//g' < log`.
# Default to false: stress goes through the TransactionDriver/QuorumDriver
# (LocalValidatorAggregatorProxy → validators directly via gRPC), not the
# legacy JSON-RPC-through-fullnode path. Required setup:
#   1. docker-compose binds each validator's 8080 to 127.0.0.{11..14}:8080
#   2. /etc/hosts maps validator-N → 127.0.0.{1N} so the address
#      `/dns/validator-N/tcp/8080/http` from genesis resolves on the host
# This eliminates HTTP/1.1 EADDRNOTAVAIL port exhaustion (HTTP/2 multiplexes)
# and engages the white-flag pipeline at the validator. Set
# USE_FULLNODE_FOR_EXECUTION=true to compare against the old JSON-RPC path.
USE_FULLNODE_FOR_EXECUTION="${USE_FULLNODE_FOR_EXECUTION:-false}"
USE_FULLNODE_FOR_RECONFIG="${USE_FULLNODE_FOR_RECONFIG:-false}"

# Mirror the validator-side white-flag override on the client so
# LocalValidatorAggregatorProxy's auto-detect (which reads
# ProtocolConfig::get_for_version → checks `IOTA_PROTOCOL_CONFIG_*` env vars
# at iota-protocol-config/src/lib.rs:1733) picks TD instead of QD. Without
# these, the client's get_for_version returns the protocol-25 default
# (white_flag=false), stress dispatches to QD's handle_transaction, and
# validators reject with "handle_transaction is disabled when white flag
# flow is enabled. Use submit_tx (ValidatorV2 service) instead."
export IOTA_PROTOCOL_CONFIG_OVERRIDE_ENABLE="${IOTA_PROTOCOL_CONFIG_OVERRIDE_ENABLE:-1}"
export IOTA_PROTOCOL_CONFIG_FEATURE_FLAGS_OVERRIDE_ENABLE_WHITE_FLAG_FLOW="${IOTA_PROTOCOL_CONFIG_FEATURE_FLAGS_OVERRIDE_ENABLE_WHITE_FLAG_FLOW:-true}"
stress_args=(
    --local false
    --fullnode-rpc-addresses "$FULLNODE_RPC_ALL"
    --use-fullnode-for-execution "$USE_FULLNODE_FOR_EXECUTION"
    --use-fullnode-for-reconfig "$USE_FULLNODE_FOR_RECONFIG"
    --client-metric-port "$CLIENT_METRIC_PORT"
    --genesis-blob-path "$GENESIS_BLOB"
    --keystore-path "$KEYSTORE"
    --primary-gas-owner-id "$PRIMARY_GAS_OWNER"
    --num-client-threads "$NUM_CLIENT_THREADS"
    --num-transfer-accounts "$NUM_TRANSFER_ACCOUNTS"
    --num-validators-to-target "$NUM_VALIDATORS_TO_TARGET"
    --gas-request-chunk-size "$GAS_CHUNK_SIZE"
    --run-duration "$DURATION"
)
if [ -n "$READY_FILE" ] && [ -n "$START_FILE" ]; then
    stress_args+=(--ready-file "$READY_FILE" --start-file "$START_FILE")
fi
if [ "$BARRIER_PERIOD_MS" -gt 0 ] 2>/dev/null; then
    stress_args+=(--barrier-period-ms "$BARRIER_PERIOD_MS")
fi
if [ -n "$GAS_POOL_CACHE_PATH" ]; then
    stress_args+=(--gas-pool-cache-path "$GAS_POOL_CACHE_PATH")
fi
stress_args+=(
    bench
    --target-qps "$QPS"
    --in-flight-ratio "$IN_FLIGHT_RATIO"
    --num-workers "$WORKERS"
    --burst-size "$BURST_SIZE"
    --transfer-object "$TRANSFER_OBJECT_PCT"
    --shared-counter "$SHARED_COUNTER_PCT"
)

set +e
RUST_LOG="$LOG_LEVEL" script -qe -c \
    "cargo run --release -p iota-benchmark --bin stress -- ${stress_args[*]@Q} ${*@Q}" \
    "$out/stress-stdout.log"
stress_rc=$?
set -e

end_epoch=$(date +%s)
run_seconds=$((end_epoch - start_epoch))
echo "stress.rs exit=$stress_rc duration=${run_seconds}s" >> "$out/params.txt"

# Capture Prometheus metrics over the SPAM phase only, not the full run.
# Warmup (pay_iota setup) can take minutes and contains its own retry/rejection
# events that contaminate the load-shedding metrics if folded into the window.
# Source of truth for "spam started": mtime of the barrier `go` file (written
# by stress-multi.sh once all subprocesses are ready). Falls back to
# start_epoch if the start file is missing (e.g., single-proc invocations).
if [ -n "$START_FILE" ] && [ -f "$START_FILE" ]; then
    metrics_start_epoch=$(stat -c %Y "$START_FILE" 2>/dev/null || echo "$start_epoch")
else
    metrics_start_epoch=$start_epoch
fi
spam_seconds=$((end_epoch - metrics_start_epoch))
if [ "$spam_seconds" -lt 1 ]; then spam_seconds=1; fi
echo "spam phase: start_epoch=$metrics_start_epoch duration=${spam_seconds}s" >> "$out/params.txt"

# Fail-fast: if stress.rs crashed (panic = 134, OOM, etc.) or returned non-zero,
# skip metrics capture (Prometheus has nothing useful for a 3s aborted run) and
# print the actual error tail so the caller can diagnose immediately.
if [ "$stress_rc" -ne 0 ]; then
    echo "=> stress.rs FAILED with rc=$stress_rc after ${run_seconds}s — skipping metrics capture."
    echo "=> Last error/panic lines from stress-stdout.log:"
    sed -r 's/\x1b\[[0-9;]*[mGKH]//g' "$out/stress-stdout.log" \
        | grep -iE 'panic|insufficient|error|fail' \
        | grep -v 'compile\|^warning' \
        | tail -8
    exit "$stress_rc"
fi

# ----- capture from Prometheus -----

query_range() {
    local query=$1 outfile=$2
    curl -sfG "$PROM_URL/api/v1/query_range" \
        --data-urlencode "query=$query" \
        --data-urlencode "start=$metrics_start_epoch" \
        --data-urlencode "end=$end_epoch" \
        --data-urlencode "step=1s" \
        -o "$out/$outfile" || echo "warning: query failed: $query" >&2
}

query_instant() {
    local query=$1 outfile=$2
    curl -sfG "$PROM_URL/api/v1/query" \
        --data-urlencode "query=$query" \
        --data-urlencode "time=$end_epoch" \
        -o "$out/$outfile" || echo "warning: query failed: $query" >&2
}

echo
echo "=> capturing metrics from Prometheus over spam window [${metrics_start_epoch}..${end_epoch}] (${spam_seconds}s; total wall=${run_seconds}s)..."

query_range 'sum by (host) (sequencing_certificate_inflight{host=~"validator.*"})'              num_inflight.json
query_range 'sequencing_in_flight_submissions{host=~"validator.*"}'                             in_flight.json
query_range 'consensus_queue_load_shedding_percentage{host=~"validator.*"}'                     shed_pct.json
query_range 'rate(transaction_overload_sources[5s])'                                           overload_rate.json
query_range 'rate(validator_service_num_rejected_tx_during_overload[5s])'                      rejected_rate.json
query_range 'rate(total_transaction_effects{host=~"validator.*"}[5s])'                         useful_tps.json
query_instant "sum by (host, source) (increase(transaction_overload_sources[${spam_seconds}s]))" overload_total.json
query_instant "sum by (host, error)  (increase(validator_service_num_rejected_tx_during_overload[${spam_seconds}s]))" rejected_total.json

# ----- flat CSV: timestamp,metric,host,labels,value -----

{
    echo "timestamp,metric,host,labels,value"
    for spec in "num_inflight.json:sum_sequencing_certificate_inflight" \
                "in_flight.json:sequencing_in_flight_submissions" \
                "shed_pct.json:consensus_queue_load_shedding_percentage" \
                "overload_rate.json:rate_transaction_overload_sources" \
                "rejected_rate.json:rate_validator_service_num_rejected_tx_during_overload" \
                "useful_tps.json:rate_total_transaction_effects"; do
        file="${spec%%:*}"
        metric="${spec##*:}"
        [[ -f "$out/$file" ]] || continue
        jq -r --arg metric "$metric" '
            .data.result[]? as $r
            | ($r.metric.host // $r.metric.instance // "") as $host
            | ($r.metric | del(.host,.instance,.__name__,.job) | tostring) as $labels
            | $r.values[]?
            | [.[0], $metric, $host, $labels, .[1]] | @csv
        ' "$out/$file"
    done
} > "$out/metrics.csv"

# ----- human-readable summary -----

summarize_gauge() {
    local file=$1 label=$2
    [[ -f "$out/$file" ]] || { echo "  (no data)"; return; }
    jq -r --arg label "$label" '
        .data.result[]? as $r
        | ($r.metric.host // $r.metric.instance // "?") as $h
        | ($r.values | map(.[1] | tonumber)) as $v
        | "  \($h)\tpeak=\(if ($v|length)>0 then ($v|max) else 0 end)\tavg=\(if ($v|length)>0 then (($v|add)/($v|length)|.*100|floor/100) else 0 end)\tfinal=\(if ($v|length)>0 then ($v|last) else 0 end)"
    ' "$out/$file" | sort
}

summarize_total() {
    local file=$1
    [[ -f "$out/$file" ]] || { echo "  (no data)"; return; }
    jq -r '
        if (.data.result | length) == 0 then "  (no series — no rejections during run)"
        else
            ([.data.result[] | "  \(.metric.host // "?")\t\(.metric.source // .metric.error // "")\t\(.value[1] | tonumber | floor)"] | sort | join("\n")) +
            "\n  TOTAL\t\t\([.data.result[].value[1] | tonumber] | add | floor)"
        end
    ' "$out/$file"
}

{
    echo "=== Load Shedding Stress Run summary ==="
    echo "ts_utc:        $ts"
    echo "params:        QPS=$QPS  DURATION=$DURATION  WORKERS=$WORKERS  IN_FLIGHT_RATIO=$IN_FLIGHT_RATIO"
    echo "validator:     max_pending=$max_pending  start_pct=$start_pct  white_flag=$white_flag"
    echo "wall:          ${run_seconds}s (spam window: ${spam_seconds}s)   stress_rc=$stress_rc"
    echo
    echo "[gauge] sum(sequencing_certificate_inflight) — total in-flight per validator (shedding-input)"
    summarize_gauge num_inflight.json num_inflight
    echo
    echo "[gauge] sequencing_in_flight_submissions (post-permit; capped at submit_semaphore size)"
    summarize_gauge in_flight.json in_flight
    echo
    echo "[gauge] consensus_queue_load_shedding_percentage"
    summarize_gauge shed_pct.json shed_pct
    echo
    echo "[counter total over run] transaction_overload_sources by (host, source)"
    summarize_total overload_total.json
    echo
    echo "[counter total over run] validator_service_num_rejected_tx_during_overload by (host, error)"
    summarize_total rejected_total.json
    echo
    echo "[useful TPS] rate(total_transaction_effects[5s]) — txs/sec executed (effects produced)"
    summarize_gauge useful_tps.json useful_tps
    echo
    echo "Output dir: $out/"
    echo "Files: params.txt summary.txt metrics.csv {in_flight,shed_pct,overload_rate,rejected_rate}.json {overload,rejected}_total.json stress-stdout.log"
} | tee "$out/summary.txt"

exit "$stress_rc"
