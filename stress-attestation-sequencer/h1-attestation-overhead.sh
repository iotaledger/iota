#!/usr/bin/env bash
#
# h1-attestation-overhead.sh — run the H1 experiment (attestation overhead,
# W4: V1 vs V2) end to end and capture the metrics:
#   0. build      pre-build the stress binary (so timestamps bracket only spam)
#   1. cleanup    tear down anything already running (best-effort)
#   2. bootstrap  -b, regenerate genesis with benchmark gas accounts
#   3. Run A — V1 attestation OFF (control), owned-object load, TotalTxCount
#   4. Run B — V2 attestation ON, same load (validators restart in place; NO
#                 cleanup between runs, so Prometheus keeps Run A's data)
#   then        scrape Prometheus for each run's window, write JSON + summary.md,
#               and offer to tear down (decline to keep Grafana/Prometheus up)
#
# Run as a NORMAL user (cargo must not run as root); `sudo` is used internally
# only for cleanup/bootstrap.
#
# Tunables (env): N, RUN_DURATION, TARGET_QPS, NUM_WORKERS, PROM.
set -euo pipefail

if [[ ${EUID:-$(id -u)} -eq 0 ]]; then
  echo "ERROR: run as a normal user, not root (cargo would build as root)." >&2
  echo "       sudo is invoked internally for cleanup/bootstrap." >&2
  exit 1
fi

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
GENESIS_DIR="$REPO_ROOT/dev-tools/iota-private-network/configs/genesis"

N="${N:-4}"
RUN_DURATION="${RUN_DURATION:-300s}"
TARGET_QPS="${TARGET_QPS:-200}"
NUM_WORKERS="${NUM_WORKERS:-8}"
PROM="${PROM:-http://localhost:9090}"
PRIMARY_GAS_OWNER="0xf479d29837d22943aba6afc401f518a36521b990874eca784886185bd26bf681"
STRESS_BIN="$REPO_ROOT/target/release/stress"
RESULTS_DIR="$SCRIPT_DIR/results/h1-$(date +%Y%m%d-%H%M%S)"
mkdir -p "$RESULTS_DIR"

# Magenta phase banners / yellow prompts (auto-disabled when not a terminal).
if [[ -t 1 ]]; then MAGENTA=$'\033[0;35m'; YELLOW=$'\033[0;33m'; RESET=$'\033[0m'; else MAGENTA=''; YELLOW=''; RESET=''; fi
RULE="$(printf '%80s' '' | tr ' ' '*')"
banner() {
  echo
  echo "${MAGENTA}${RULE}${RESET}"
  echo "${MAGENTA}$*${RESET}"
  echo "${MAGENTA}${RULE}${RESET}"
}

# Block until the fullnode JSON-RPC at :9000 accepts connections (start.sh
# verifies validators, not the fullnode, so it can lag behind).
wait_for_fullnode() {
  echo "Waiting for fullnode RPC at 127.0.0.1:9000 ..."
  for _ in $(seq 1 60); do
    if curl -s -o /dev/null --max-time 2 \
        -X POST -H 'Content-Type: application/json' \
        --data '{"jsonrpc":"2.0","id":1,"method":"iota_getChainIdentifier","params":[]}' \
        http://127.0.0.1:9000; then
      echo "Fullnode RPC is up."
      return 0
    fi
    sleep 2
  done
  echo "ERROR: fullnode RPC at 127.0.0.1:9000 not ready in time." >&2
  exit 1
}

# Evaluate a PromQL instant query at a given epoch; echo a JSON number or null.
prom_scalar() {
  curl -s --max-time 10 -G "$PROM/api/v1/query" \
      --data-urlencode "query=$1" --data-urlencode "time=$2" \
    | python3 -c '
import sys, json, math
try:
    r = json.load(sys.stdin).get("data", {}).get("result", [])
    if not r:
        print("null"); sys.exit()
    v = float(r[0]["value"][1])
    print("null" if not math.isfinite(v) else repr(v))
except Exception:
    print("null")
'
}

# Scrape the H1 metrics over a run window and write a JSON file.
# $1 label  $2 start_epoch  $3 end_epoch  $4 out.json
scrape_metrics() {
  local label="$1" start="$2" end="$3" out="$4"
  local win=$(( end - start )); (( win < 1 )) && win=1
  echo "Scraping Prometheus over ${win}s window -> $out"

  local fn='host=~"fullnode-.*",ping="false"'   # fullnode, real txs only
  local vn='name=~"validator-.*"'               # cadvisor validator containers
  local q
  declare -A v
  for pct in 0.5 0.95 0.99; do
    v[att_$pct]=$(prom_scalar "histogram_quantile($pct, sum by (le) (rate(validator_attestation_latency_bucket[${win}s])))" "$end")
    v[set_$pct]=$(prom_scalar "histogram_quantile($pct, sum by (le) (rate(transaction_driver_settlement_finality_latency_bucket{${fn}}[${win}s])))" "$end")
    v[sub_$pct]=$(prom_scalar "histogram_quantile($pct, sum by (le) (rate(transaction_driver_submit_transaction_latency_bucket{${fn}}[${win}s])))" "$end")
    v[exe_$pct]=$(prom_scalar "histogram_quantile($pct, sum by (le) (rate(validator_transaction_execution_latency_bucket[${win}s])))" "$end")
  done
  local tps cpu
  tps=$(prom_scalar "max(rate(transactions_included_in_checkpoint[${win}s]))" "$end")
  cpu=$(prom_scalar "avg(sum by (name) (rate(container_cpu_usage_seconds_total{${vn}}[${win}s])))" "$end")

  cat > "$out" <<JSON
{
  "label": "$label",
  "start_epoch": $start,
  "end_epoch": $end,
  "window_seconds": $win,
  "validator_attestation_latency_s": { "p50": ${v[att_0.5]}, "p95": ${v[att_0.95]}, "p99": ${v[att_0.99]} },
  "settlement_finality_latency_s": { "p50": ${v[set_0.5]}, "p95": ${v[set_0.95]}, "p99": ${v[set_0.99]} },
  "submit_transaction_latency_s": { "p50": ${v[sub_0.5]}, "p95": ${v[sub_0.95]}, "p99": ${v[sub_0.99]} },
  "validator_transaction_execution_latency_s": { "p50": ${v[exe_0.5]}, "p95": ${v[exe_0.95]}, "p99": ${v[exe_0.99]} },
  "finalized_tps": $tps,
  "per_validator_cpu_busy_cores": $cpu
}
JSON
}

# Build a markdown summary (V1 | V2 | V2-V1) from the two run JSONs.
# $1 run-a (V1)  $2 run-b (V2)  $3 out.md
write_summary() {
  python3 - "$1" "$2" "$3" <<'PY'
import sys, json
a = json.load(open(sys.argv[1])); b = json.load(open(sys.argv[2])); out = sys.argv[3]
def fmt(x): return "—" if x is None else f"{x:.6g}"
def dlt(x, y): return "—" if (x is None or y is None) else f"{y - x:+.6g}"
L = ["# H1 — attestation overhead: results\n",
     f"- Run A (V1, attestation off): {a['window_seconds']}s, epochs {a['start_epoch']}–{a['end_epoch']}",
     f"- Run B (V2, attestation on): {b['window_seconds']}s, epochs {b['start_epoch']}–{b['end_epoch']}\n"]
lat = [("validator_attestation_latency (s)", "validator_attestation_latency_s"),
       ("settlement_finality_latency (s)", "settlement_finality_latency_s"),
       ("submit_transaction_latency (s)", "submit_transaction_latency_s"),
       ("validator_transaction_execution_latency (s)", "validator_transaction_execution_latency_s")]
for pct in ("p50", "p95", "p99"):
    L += [f"## {pct}\n", "| metric | V1 | V2 | V2−V1 |", "| --- | --- | --- | --- |"]
    for name, key in lat:
        va, vb = a.get(key, {}).get(pct), b.get(key, {}).get(pct)
        L.append(f"| {name} | {fmt(va)} | {fmt(vb)} | {dlt(va, vb)} |")
    L.append("")
L += ["## throughput / CPU\n", "| metric | V1 | V2 | V2−V1 |", "| --- | --- | --- | --- |"]
for name, key in [("finalized TPS", "finalized_tps"), ("per-validator CPU (busy cores)", "per_validator_cpu_busy_cores")]:
    va, vb = a.get(key), b.get(key)
    L.append(f"| {name} | {fmt(va)} | {fmt(vb)} | {dlt(va, vb)} |")
open(out, "w").write("\n".join(L) + "\n")
print("wrote", out)
PY
}

# Identical owned-object (transfer) load for both runs, through the fullnode so
# it takes the attesting submit_tx path. TotalTxCount + owned objects => no
# shared-object sequencing, so the V1<->V2 delta is pure attestation overhead.
# $1 label  $2 out.json
run_stress() {
  local label="$1" json_out="$2" start end
  banner ">>> stress: $label"
  wait_for_fullnode
  start=$(date +%s)
  ( cd "$REPO_ROOT" && "$STRESS_BIN" \
      --local false \
      --fullnode-rpc-addresses http://127.0.0.1:9000 \
      --use-fullnode-for-execution true \
      --use-fullnode-for-reconfig true \
      --genesis-blob-path "$GENESIS_DIR/genesis.blob" \
      --keystore-path "$GENESIS_DIR/benchmark.keystore" \
      --primary-gas-owner-id "$PRIMARY_GAS_OWNER" \
      --num-client-threads 4 --num-transfer-accounts 10 --run-duration "$RUN_DURATION" \
      bench --target-qps "$TARGET_QPS" --in-flight-ratio 5 --num-workers "$NUM_WORKERS" \
      --transfer-object 100 --shared-counter 0 )
  end=$(date +%s)
  scrape_metrics "$label" "$start" "$end" "$json_out"
}

# Cache sudo credentials up front so prompts don't interrupt mid-run.
sudo -v

banner "== H1 [0/5] build stress binary =="
( cd "$REPO_ROOT" && cargo build --release -p iota-benchmark --bin stress )

banner "== H1 [1/5] cleanup (in case something is running) =="
sudo "$SCRIPT_DIR/cleanup.sh" || true

banner "== H1 [2/5] bootstrap (-b, $N validators) =="
sudo "$SCRIPT_DIR/bootstrap.sh" -b -n "$N"

banner "== H1 [3/5] Run A — V1 (attestation OFF, control) =="
MODE=TotalTxCount ATTEST=false "$SCRIPT_DIR/start.sh" -n "$N" faucet
run_stress "Run A — V1 (attestation off)" "$RESULTS_DIR/run-a-v1.json"

banner "== H1 [4/5] Run B — V2 (attestation ON) — restart validators in place =="
# No cleanup: re-running start.sh recreates the validators with attestation on
# while Prometheus/Grafana keep running, so Run A's series are preserved.
MODE=TotalTxCount "$SCRIPT_DIR/start.sh" -n "$N" faucet
run_stress "Run B — V2 (attestation on)" "$RESULTS_DIR/run-b-v2.json"

banner "== H1 [5/5] both runs complete =="
write_summary "$RESULTS_DIR/run-a-v1.json" "$RESULTS_DIR/run-b-v2.json" "$RESULTS_DIR/summary.md"
echo "${YELLOW}Results saved to: $RESULTS_DIR${RESET}"
echo "${YELLOW}  run-a-v1.json, run-b-v2.json, summary.md${RESET}"
echo "${YELLOW}Grafana (visual): http://localhost:3000/d/attestation-sequencer-stress${RESET}"
echo "${YELLOW}  Run A (V1) is the earlier ~${RUN_DURATION} window, Run B (V2) the later one.${RESET}"
read -r -p "${YELLOW}Tear down now (cleanup, removes Grafana/Prometheus)? [y/N] ${RESET}" ans
if [[ "$ans" == "y" || "$ans" == "Y" ]]; then
  sudo "$SCRIPT_DIR/cleanup.sh" || true
  echo "Torn down."
else
  echo "Left running. Tear down later with: sudo $SCRIPT_DIR/cleanup.sh"
fi
