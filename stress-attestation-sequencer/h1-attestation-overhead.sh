#!/usr/bin/env bash
#
# h1-attestation-overhead.sh — run the H1 experiment (attestation overhead,
# W4: V1 vs V2) end to end and capture the metrics:
#   0. build      pre-build the stress binary (so timestamps bracket only spam)
#   1. cleanup    tear down anything already running (best-effort)
#   2. bootstrap  -b, regenerate genesis with benchmark gas accounts
#   3. Run A — V1 attestation OFF (control), owned-object load, TotalTxCount
#   4. Run B — V2 attestation ON, same load (FULL reset between runs — cleanup +
#                 re-bootstrap, incl. a fresh Prometheus — so Run B cold-starts
#                 exactly like Run A; Run A is scraped to JSON beforehand)
#   then        scrape Prometheus for each run's window (Run A is scraped before
#               its reset), write JSON + summary.md, stop + clean the network,
#               and prompt whether to also stop + clear monitoring (Grafana).
#
# Run as a NORMAL user (cargo must not run as root); `sudo` is used internally
# only for cleanup/bootstrap.
#
# Tunables (env): N, RUN_DURATION, TARGET_QPS, NUM_WORKERS,
#                 SLEEP_BETWEEN_RUNS_S, PRE_SPAM_WAIT_S, PRE_STOP_WAIT_S, PROM.
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
RUN_DURATION="${RUN_DURATION:-30s}"
TARGET_QPS="${TARGET_QPS:-200}"
NUM_WORKERS="${NUM_WORKERS:-8}"
SLEEP_BETWEEN_RUNS_S="${SLEEP_BETWEEN_RUNS_S:-5}" # idle gap (s) to separate A/B on the timeline
PRE_SPAM_WAIT_S="${PRE_SPAM_WAIT_S:-0}"       # let the network settle this long after it's up, before the spam
PRE_STOP_WAIT_S="${PRE_STOP_WAIT_S:-2}"       # keep Run A's network up this long after scraping, before stopping
PROM="${PROM:-http://localhost:9090}"
PRIMARY_GAS_OWNER="0xf479d29837d22943aba6afc401f518a36521b990874eca784886185bd26bf681"
STRESS_BIN="$REPO_ROOT/target/release/stress"
RESULTS_DIR="$SCRIPT_DIR/results/h1-$(date +%Y%m%d-%H%M%S)"
mkdir -p "$RESULTS_DIR"

# Magenta phase banners / yellow prompts (auto-disabled when not a terminal).
if [[ -t 1 ]]; then
  MAGENTA=$'\033[0;35m'
  YELLOW=$'\033[0;33m'
  RESET=$'\033[0m'
else
  MAGENTA=''
  YELLOW=''
  RESET=''
fi
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

# Reset the network between runs so Run B's startup path is IDENTICAL to the
# initial setup before Run A — that full symmetry is what keeps the pre-spam
# warmup the same. We tear EVERYTHING down (incl. the monitoring stack) and
# re-bootstrap a brand-new genesis (current timestamp) with an empty DB, exactly
# like Run A's [1/5] cleanup + [2/5] bootstrap. Leaving Prometheus up across the
# reset appears to give Run B a longer warmup, so cleanup.sh brings it down and
# start.sh brings a fresh one back up for Run B.
#
# Run A is already scraped to run-a-v1.json before this point, so tearing
# Prometheus down here only drops Run A's live Grafana history (summary.md,
# built from the JSON, is unaffected).
reset_network() {
  echo "Tearing everything down (incl. Prometheus) and re-bootstrapping a fresh genesis for Run B..."
  sudo "$SCRIPT_DIR/cleanup.sh" || true
  sudo "$SCRIPT_DIR/bootstrap.sh" -b -n "$N"
}

# Evaluate a PromQL instant query at a given epoch; echo a JSON number or null.
prom_scalar() {
  curl -s --max-time 10 -G "$PROM/api/v1/query" \
    --data-urlencode "query=$1" --data-urlencode "time=$2" |
    python3 -c '
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
  local win=$((end - start))
  ((win < 1)) && win=1
  echo "Scraping Prometheus over ${win}s window -> $out"

  local fn='host=~"fullnode-.*",ping="false"' # fullnode, real txs only
  local vn='name=~"validator-.*"'             # cadvisor validator containers
  local q
  declare -A v
  for pct in 0.5 0.95 0.99; do
    v[att_$pct]=$(prom_scalar "histogram_quantile($pct, sum by (le) (rate(validator_attestation_latency_bucket[${win}s])))" "$end")
    v[set_$pct]=$(prom_scalar "histogram_quantile($pct, sum by (le) (rate(transaction_driver_settlement_finality_latency_bucket{${fn}}[${win}s])))" "$end")
    v[sub_$pct]=$(prom_scalar "histogram_quantile($pct, sum by (le) (rate(transaction_driver_submit_transaction_latency_bucket{${fn}}[${win}s])))" "$end")
    v[exe_$pct]=$(prom_scalar "histogram_quantile($pct, sum by (le) (rate(validator_transaction_execution_latency_bucket[${win}s])))" "$end")
    v[int_$pct]=$(prom_scalar "histogram_quantile($pct, sum by (le) (rate(authority_state_internal_execution_latency_bucket[${win}s])))" "$end")
  done
  local tps cpu
  tps=$(prom_scalar "max(rate(transactions_included_in_checkpoint[${win}s]))" "$end")
  cpu=$(prom_scalar "avg(sum by (name) (rate(container_cpu_usage_seconds_total{${vn}}[${win}s])))" "$end")

  cat >"$out" <<JSON
{
  "label": "$label",
  "start_epoch": $start,
  "end_epoch": $end,
  "window_seconds": $win,
  "validator_attestation_latency_s": { "p50": ${v[att_0.5]}, "p95": ${v[att_0.95]}, "p99": ${v[att_0.99]} },
  "settlement_finality_latency_s": { "p50": ${v[set_0.5]}, "p95": ${v[set_0.95]}, "p99": ${v[set_0.99]} },
  "submit_transaction_latency_s": { "p50": ${v[sub_0.5]}, "p95": ${v[sub_0.95]}, "p99": ${v[sub_0.99]} },
  "validator_transaction_execution_latency_s": { "p50": ${v[exe_0.5]}, "p95": ${v[exe_0.95]}, "p99": ${v[exe_0.99]} },
  "internal_execution_latency_s": { "p50": ${v[int_0.5]}, "p95": ${v[int_0.95]}, "p99": ${v[int_0.99]} },
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
       ("validator_transaction_execution_latency (s)", "validator_transaction_execution_latency_s"),
       ("internal_execution_latency — real VM (s)", "internal_execution_latency_s")]
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
  # Let the network settle before measuring, so the window has a clean idle
  # baseline and the validators are stable after (re)start.
  echo "Letting the network settle ${PRE_SPAM_WAIT_S}s before the spam..."
  sleep "$PRE_SPAM_WAIT_S"
  start=$(date +%s)
  (cd "$REPO_ROOT" && "$STRESS_BIN" \
    --local false \
    --fullnode-rpc-addresses http://127.0.0.1:9000 \
    --use-fullnode-for-execution true \
    --use-fullnode-for-reconfig true \
    --genesis-blob-path "$GENESIS_DIR/genesis.blob" \
    --keystore-path "$GENESIS_DIR/benchmark.keystore" \
    --primary-gas-owner-id "$PRIMARY_GAS_OWNER" \
    --num-client-threads 4 --num-transfer-accounts 10 --run-duration "$RUN_DURATION" \
    bench --target-qps "$TARGET_QPS" --in-flight-ratio 5 --num-workers "$NUM_WORKERS" \
    --transfer-object 100 --shared-counter 0)
  end=$(date +%s)
  scrape_metrics "$label" "$start" "$end" "$json_out"
}

# Cache sudo credentials up front so prompts don't interrupt mid-run.
sudo -v

banner "== H1 [0/5] build stress binary =="
(cd "$REPO_ROOT" && cargo build --release -p iota-benchmark --bin stress)

banner "== H1 [1/5] cleanup (in case something is running) =="
sudo "$SCRIPT_DIR/cleanup.sh" || true
# Start this invocation with an EMPTY Prometheus TSDB. The data volume is
# persistent (so it survives the between-runs teardown and both runs stay
# visible), but here — once, at the very start — we drop it with `down -v` so
# stale series from previous invocations don't linger. reset_network between
# Run A and Run B deliberately does NOT pass -v, so Run A's data is kept.
(cd "$REPO_ROOT/dev-tools/grafana-local" && docker compose down -v --remove-orphans) >/dev/null 2>&1 || true

banner "== H1 [2/5] bootstrap (-b, $N validators) =="
sudo "$SCRIPT_DIR/bootstrap.sh" -b -n "$N"

banner "== H1 [3/5] Run A — V1 (attestation OFF, control) =="
MODE=TotalTxCount ATTEST=false "$SCRIPT_DIR/start.sh" -n "$N" faucet
run_stress "Run A — V1 (attestation off)" "$RESULTS_DIR/run-a-v1.json"

# Run A is scraped; let the network run a moment so the post-run tail is
# captured, then fully reset (incl. Prometheus) and re-bootstrap a fresh genesis
# for Run B, idling so the runs are cleanly separated (targets go DOWN in the
# gap). Run A's metrics are already in run-a-v1.json, so dropping Prometheus
# here is fine.
echo "Letting the network run ${PRE_STOP_WAIT_S}s after the scrape before resetting..."
sleep "$PRE_STOP_WAIT_S"
reset_network
echo "Idle gap (${SLEEP_BETWEEN_RUNS_S}s) to separate Run A/B on the timeline..."
sleep "$SLEEP_BETWEEN_RUNS_S"

banner "== H1 [4/5] Run B — V2 (attestation ON) — fresh genesis, empty DB =="
# start.sh boots the validators from the freshly re-bootstrapped genesis with an
# empty data dir and brings up a fresh monitoring stack (reset_network tore the
# old one down), so Run B cold-starts exactly like Run A — only attestation
# differs. Run A's metrics live in run-a-v1.json (already scraped).
MODE=TotalTxCount "$SCRIPT_DIR/start.sh" -n "$N" faucet
run_stress "Run B — V2 (attestation on)" "$RESULTS_DIR/run-b-v2.json"

# Symmetric with the end of Run A: let the network run the same moment after the
# scrape so Run B's post-run tail lands on the dashboard too, before teardown.
echo "Letting the network run ${PRE_STOP_WAIT_S}s after the scrape before tearing down..."
sleep "$PRE_STOP_WAIT_S"

banner "== H1 [5/5] both runs complete — stopping network =="
write_summary "$RESULTS_DIR/run-a-v1.json" "$RESULTS_DIR/run-b-v2.json" "$RESULTS_DIR/summary.md"
echo "${YELLOW}Results saved to: $RESULTS_DIR${RESET}"
echo "${YELLOW}  run-a-v1.json, run-b-v2.json, summary.md${RESET}"

# Always stop + clean the network (down + wipe data) via the privnet's OWN
# cleanup, which leaves the monitoring stack up so both runs stay visible in
# Grafana. (cd in first: it runs `docker compose down` against the cwd.)
(cd "$REPO_ROOT/dev-tools/iota-private-network" && sudo ./cleanup.sh)
echo "${YELLOW}Network stopped and cleaned. Monitoring is still up — both runs visible:${RESET}"
echo "${YELLOW}  Grafana: http://localhost:3000/d/attestation-sequencer-stress${RESET}"

# Monitoring teardown is opt-in: `down -v` also removes the prometheus-data
# volume, fully clearing both runs' series.
read -r -p "${YELLOW}Also stop and CLEAR monitoring (Prometheus data)? [y/N] ${RESET}" ans
if [[ "$ans" == "y" || "$ans" == "Y" ]]; then
  (cd "$REPO_ROOT/dev-tools/grafana-local" && docker compose down -v --remove-orphans) || true
  echo "${YELLOW}Monitoring stopped and Prometheus data cleared.${RESET}"
else
  echo "${YELLOW}Monitoring left running. Stop + clear it later with:${RESET}"
  echo "${YELLOW}  (cd $REPO_ROOT/dev-tools/grafana-local && docker compose down -v)${RESET}"
fi
