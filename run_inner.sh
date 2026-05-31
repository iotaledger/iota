#!/usr/bin/env bash
# run_inner.sh — the orchestrator body of run.sh, extracted into its own
# file for editability. Not meant to be invoked directly by users — run.sh
# launches it via nohup. Reads ITERS from environment.
#
# What this script does:
#   1. ONE initial bring-up (validators + grafana + Prometheus + ready poll)
#   2. Interleaved sweep over POLICIES × ITERS rounds, calling sweep.sh in
#      FAST_MODE (skip per-iter network destroy; bootstrap+restart only)
#   3. Full reset on consecutive iter failures (sweep.sh exit non-zero)
#   4. ONE final teardown
#
# kill.sh identifies this process by matching "run_inner.sh" in pgrep -af.

set -uo pipefail
cd "$(dirname "$0")"

PRIVNET=./dev-tools/iota-private-network
GRAFANA=./dev-tools/grafana-local

# SKIP_NETWORK_LIFECYCLE=true tells sweep.sh to skip its script-level
# bring-up + teardown — run.sh does ONE initial bring-up and ONE
# final teardown here instead.
#
# FAST_MODE was an experiment to also skip the per-iter docker reset
# (preserving validator state across iters via bootstrap + restart).
# In practice it made iters SLOWER (stress-multi.sh's per-iter runtime
# grew from 84s to 130s — likely from stale gas-pool cache + validator
# state accumulation). Reverted to default (per-iter full reset inside
# sweep.sh's iter loop, just like before FAST_MODE existed).
export SKIP_NETWORK_LIFECYCLE=true

# ---------- initial bring-up (one time for the whole sweep) ----------
initial_bringup() {
  echo "=== run.sh: initial bring-up $(date -u +%H:%M:%S) ==="
  (
    cd "$PRIVNET"
    if [ ! -f configs/genesis/genesis.blob ]; then
      sudo ./bootstrap.sh -b -n 4 2>&1 | tail -3
    fi
    ./run.sh -n 4 faucet 2>&1 | tail -2
  )
  (cd "$GRAFANA" && docker compose up -d 2>&1 | tail -3)
  # Wait for Prometheus
  for attempt in $(seq 1 30); do
    curl -sf --max-time 2 "http://localhost:9090/api/v1/query?query=up" >/dev/null 2>&1 && break
    sleep 1
  done
  # Wait for validators
  for attempt in $(seq 1 30); do
    ready=$(curl -sG --max-time 2 "http://localhost:9090/api/v1/query" \
      --data-urlencode 'query=count(consensus_max_pending_inflight_txs{host=~"validator.*"})' \
      2>/dev/null | jq -r '.data.result[0].value[1] // "0"')
    [ "${ready:-0}" -ge 4 ] 2>/dev/null && break
    sleep 1
  done
  sleep 2
  echo "=== run.sh: validators ready ==="
}

# Full reset (called when sweep.sh signals consecutive iter failures).
full_reset() {
  echo "=== run.sh: full network reset $(date -u +%H:%M:%S) ==="
  pkill -9 -f "target/release/stress " 2>/dev/null || true
  sleep 1
  (cd "$GRAFANA" && docker compose down 2>&1 | tail -1) || true
  sleep 2
  (cd "$PRIVNET" && docker compose down -v 2>&1 | tail -1) || true
  rm -f runs/.stress-gas-pool/owner-*.json
  initial_bringup
}

initial_bringup

# ---------- interleaved sweep ----------
# Policy list. SEM_CAP and SAT_PCT come from env vars (defaults set in
# run.sh: SEM_CAP=750, SAT_PCT=95). Binary policies (pct=100) always
# use sat=100 — sat<100 only makes sense in the graduated zone.
POLICIES=(
  "START_PCT=100 SAT_PCT=100       MAX_PENDING=1000 SEMAPHORE_CAP=$SEM_CAP SEM_SHEDDING=false"
  "START_PCT=75  SAT_PCT=$SAT_PCT  MAX_PENDING=1000 SEMAPHORE_CAP=$SEM_CAP SEM_SHEDDING=false"
  "START_PCT=50  SAT_PCT=100       MAX_PENDING=1000 SEMAPHORE_CAP=$SEM_CAP SEM_SHEDDING=false"
  "START_PCT=50  SAT_PCT=$SAT_PCT  MAX_PENDING=1000 SEMAPHORE_CAP=$SEM_CAP SEM_SHEDDING=false"
  "START_PCT=25  SAT_PCT=$SAT_PCT  MAX_PENDING=1000 SEMAPHORE_CAP=$SEM_CAP SEM_SHEDDING=false"
  "START_PCT=100 SAT_PCT=100       MAX_PENDING=900  SEMAPHORE_CAP=$SEM_CAP SEM_SHEDDING=false"
  "START_PCT=100 SAT_PCT=100       MAX_PENDING=500  SEMAPHORE_CAP=$SEM_CAP SEM_SHEDDING=false"
)
# START_PCT=100 SAT_PCT=100 MAX_PENDING=1000  SEMAPHORE_CAP=20  SEM_SHEDDING=true
# START_PCT=50  SAT_PCT=90  MAX_PENDING=1000  SEMAPHORE_CAP=20  SEM_SHEDDING=true
# START_PCT=100 SAT_PCT=100 MAX_PENDING=20000 SEMAPHORE_CAP=400 SEM_SHEDDING=true
# START_PCT=50  SAT_PCT=90  MAX_PENDING=20000 SEMAPHORE_CAP=400 SEM_SHEDDING=true


for ((round=1; round<=ITERS; round++)); do
  echo "=== run.sh round=$round/$ITERS  $(date -u +%H:%M:%S) ==="
  for policy in "${POLICIES[@]}"; do
    # If a previous iter signalled fatal failure, do a full reset
    # before attempting the next policy.
    if [ "${need_reset:-0}" -eq 1 ]; then
      full_reset
      need_reset=0
    fi
    if ! env $policy ITERS=1 ./sweep.sh; then
      echo "=== run.sh: sweep.sh exited non-zero — marking for reset ==="
      need_reset=1
    fi
  done
done

# ---------- final teardown ----------
echo "=== run.sh: final teardown $(date -u +%H:%M:%S) ==="
(cd "$GRAFANA" && docker compose down 2>&1 | tail -3) || true
(cd "$PRIVNET" && docker compose down -v 2>&1 | tail -3) || true
