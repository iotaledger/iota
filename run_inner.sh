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
    # Validator RocksDB lives in bind-mounted ./data/validator-* dirs,
    # untouched by `docker compose down -v`. If the prior run forked
    # (panic at checkpoints/mod.rs:545 or :1299), the poisoned state
    # replays on the next `compose up` — wipe before bringing up.
    sudo rm -rf data/validator-* data/fullnode-* data/faucet-* data/primary data/replica 2>/dev/null || true
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
# run.sh: SEM_CAP=500, SAT_PCT=95). Binary policies (pct=100) always
# use sat=100 — sat<100 only makes sense in the graduated zone.
POLICIES=(
  # 3 hard binary policies at different hard limit (max pending)
  "MAX_PENDING=20000 START_PCT=100      SAT_PCT=100      SEM_SHEDDING=false"
  # "MAX_PENDING=20000 START_PCT=$SAT_PCT SAT_PCT=$SAT_PCT SEM_SHEDDING=false"
  # "MAX_PENDING=20000 START_PCT=50       SAT_PCT=50       SEM_SHEDDING=false"
  # 1 graduated policy with 100% saturation
  # "MAX_PENDING=20000 START_PCT=50       SAT_PCT=100      SEM_SHEDDING=false"
  # 3 graduated policies with 95% saturation and different soft limit
  # "MAX_PENDING=20000 START_PCT=75       SAT_PCT=$SAT_PCT SEM_SHEDDING=false"
  # "MAX_PENDING=20000 START_PCT=50       SAT_PCT=$SAT_PCT SEM_SHEDDING=false"
  # "MAX_PENDING=20000 START_PCT=25       SAT_PCT=$SAT_PCT SEM_SHEDDING=false"
  # 1 hard binary policy for production config
  # "MAX_PENDING=20000 START_PCT=100 SAT_PCT=100      SEM_SHEDDING=true"
  # 1 graduated policy (proposed) for production config
  # "MAX_PENDING=20000 START_PCT=50  SAT_PCT=$SAT_PCT SEM_SHEDDING=true"
)

P_TOTAL=${#POLICIES[@]}
# Across-policy consecutive-failure escape hatch. sweep.sh tracks its own
# fail_streak internally but resets to 0 on every fresh invocation, so a
# pre-flight-level failure (e.g. sudo cache expired) used to infinite-loop:
# sweep.sh aborts -> full_reset -> sweep.sh aborts -> ... Cap it here.
MAX_CONSEC_FAILURES="${MAX_CONSEC_FAILURES:-3}"
consec_failures=0
for ((round = 1; round <= ITERS; round++)); do
  echo "=== run.sh round=$round/$ITERS  $(date -u +%H:%M:%S) ==="
  P_IDX=0
  for policy in "${POLICIES[@]}"; do
    P_IDX=$((P_IDX + 1))
    # If a previous iter signalled fatal failure, do a full reset
    # before attempting the next policy.
    if [ "${need_reset:-0}" -eq 1 ]; then
      full_reset
      need_reset=0
    fi
    if ! env $policy ITERS=1 POLICY_IDX=$P_IDX POLICY_TOTAL=$P_TOTAL ./sweep.sh; then
      echo "=== run.sh: sweep.sh exited non-zero — marking for reset ==="
      need_reset=1
      consec_failures=$((consec_failures + 1))
      if [ "$consec_failures" -ge "$MAX_CONSEC_FAILURES" ]; then
        echo "=== run.sh: ABORT — $consec_failures consecutive policy failures (cap=$MAX_CONSEC_FAILURES). Check sudo cache, validator state, recent script edits. ==="
        # Skip to teardown.
        break 2
      fi
    else
      consec_failures=0
    fi
  done
done

# ---------- final teardown ----------
echo "=== run.sh: final teardown $(date -u +%H:%M:%S) ==="
(cd "$GRAFANA" && docker compose down 2>&1 | tail -3) || true
(cd "$PRIVNET" && docker compose down -v 2>&1 | tail -3) || true
