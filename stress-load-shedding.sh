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
NUM_TRANSFER_ACCOUNTS="${NUM_TRANSFER_ACCOUNTS:-10}"
NUM_CLIENT_THREADS="${NUM_CLIENT_THREADS:-4}"
TRANSFER_OBJECT_PCT="${TRANSFER_OBJECT_PCT:-100}"
SHARED_COUNTER_PCT="${SHARED_COUNTER_PCT:-0}"
FULLNODE_RPC="${FULLNODE_RPC:-http://127.0.0.1:9000}"
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

cat > "$out/params.txt" <<EOF
ts_utc=$ts
start_epoch=$start_epoch
QPS=$QPS
DURATION=$DURATION
WORKERS=$WORKERS
IN_FLIGHT_RATIO=$IN_FLIGHT_RATIO
NUM_TRANSFER_ACCOUNTS=$NUM_TRANSFER_ACCOUNTS
NUM_CLIENT_THREADS=$NUM_CLIENT_THREADS
TRANSFER_OBJECT_PCT=$TRANSFER_OBJECT_PCT
SHARED_COUNTER_PCT=$SHARED_COUNTER_PCT
FULLNODE_RPC=$FULLNODE_RPC
PROM_URL=$PROM_URL
extra_args="$*"
EOF

echo "=> stress run: qps=$QPS duration=$DURATION workers=$WORKERS in_flight_ratio=$IN_FLIGHT_RATIO"
echo "=> output:    $out/"
echo "=> dashboard: http://localhost:3000/d/load-shedding-stress/load-shedding-stress-test?refresh=5s&from=now-5m&to=now"
echo

# `script -qe -c` allocates a PTY so cargo/stress.rs see a TTY on stdout and
# emit colored tracing output. The saved log keeps ANSI escapes — view with
# `less -R runs/<ts>/stress-stdout.log`, strip via
# `sed -r 's/\x1b\[[0-9;]*[mGKH]//g' < log`.
stress_args=(
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
    --in-flight-ratio "$IN_FLIGHT_RATIO"
    --num-workers "$WORKERS"
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

# ----- capture from Prometheus -----

query_range() {
    local query=$1 outfile=$2
    curl -sfG "$PROM_URL/api/v1/query_range" \
        --data-urlencode "query=$query" \
        --data-urlencode "start=$start_epoch" \
        --data-urlencode "end=$end_epoch" \
        --data-urlencode "step=5s" \
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
echo "=> capturing metrics from Prometheus over [${start_epoch}..${end_epoch}] (${run_seconds}s)..."

query_range 'sequencing_in_flight_submissions'                                                  in_flight.json
query_range 'consensus_queue_load_shedding_percentage{host=~"validator.*"}'                     shed_pct.json
query_range 'rate(transaction_overload_sources[15s])'                                           overload_rate.json
query_range 'rate(validator_service_num_rejected_tx_during_overload[15s])'                      rejected_rate.json
query_instant "sum by (host, source) (increase(transaction_overload_sources[${run_seconds}s]))" overload_total.json
query_instant "sum by (host, error)  (increase(validator_service_num_rejected_tx_during_overload[${run_seconds}s]))" rejected_total.json

# ----- flat CSV: timestamp,metric,host,labels,value -----

{
    echo "timestamp,metric,host,labels,value"
    for spec in "in_flight.json:sequencing_in_flight_submissions" \
                "shed_pct.json:consensus_queue_load_shedding_percentage" \
                "overload_rate.json:rate_transaction_overload_sources" \
                "rejected_rate.json:rate_validator_service_num_rejected_tx_during_overload"; do
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
    echo "wall:          ${run_seconds}s   stress_rc=$stress_rc"
    echo
    echo "[gauge] sequencing_in_flight_submissions"
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
    echo "Output dir: $out/"
    echo "Files: params.txt summary.txt metrics.csv {in_flight,shed_pct,overload_rate,rejected_rate}.json {overload,rejected}_total.json stress-stdout.log"
} | tee "$out/summary.txt"

exit "$stress_rc"
