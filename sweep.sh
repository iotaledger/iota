#!/usr/bin/env bash
# sweep.sh — unified validator load-shedding experiment runner. Per iter
# captures BOTH cap-policy data (peak overshoot, latency, throughput) and
# fairness data (per-pool first-pass accept rate) so a single sweep
# produces all the data needed for the graduated-vs-binary story.
#
# Merges the former cap-policy-sweep.sh (spammer-only) and fairness-sweep.sh
# (spammer + honest pool). The honest pool is on by default
# (HONEST_PROC_COUNT=1) and adds negligible (~0.1%) load to the 40k QPS
# spammer, so cap-policy metrics are unaffected.
#
# At start_pct=100 the policy is binary (reactive only — 100% drop at
# max_pending). At start_pct<100 it's graduated (RED-style): a soft zone
# between (start_pct × max_pending) and (sat_pct × max_pending) where
# submissions are probabilistically dropped, plus a 100% shed band above
# sat_pct, plus a reactive cap at max_pending.
#
# Per-iter the script captures:
#   - peak_inflight / max_pending      (overshoot ratio, lower = safer)
#   - useful_tps                       (throughput)
#   - permit_wait/hold p50/p99         (stage B / C latency)
#   - reject counters by category      (preventive / saturated / reactive)
#   - spammer/honest first-pass rate   (fairness)
#   - inflight, shed_pct, rejection rate TIME-SERIES at 100ms / 1s grid
#     (enables sawtooth viz + degradation curves without re-running)
#
# Usage:
#   ./sweep.sh                                   # use current yaml policy
#   START_PCT=50 SAT_PCT=95 ./sweep.sh           # patch yaml first
#   ITERS=20 ./sweep.sh                          # n=20 (default 5)
#   HONEST_PROC_COUNT=0 ./sweep.sh               # disable honest pool
#
# Output:
#   sweep.jsonl   — one JSON record per iter (appends, nested)
#   sweep.log     — full sweep log
#
# pandas analysis:
#   df = pd.read_json("sweep.jsonl", lines=True)
#   df = pd.json_normalize(df.to_dict(orient="records"))
#
# Note: if you see AddrNotAvailable / TCP port exhaustion errors in the
# subprocess logs (rare since the TransactionDriver switch — long-lived
# gRPC channels replace per-call short-lived sockets), run
# `sudo ./tune-sysctl.sh` once per boot session.
set -uo pipefail

# CRITICAL: the validator-side white-flag override needs these env vars at
# `docker compose up` time. They must be set before run.sh starts validators.
export IOTA_PROTOCOL_CONFIG_OVERRIDE_ENABLE=1
export IOTA_PROTOCOL_CONFIG_FEATURE_FLAGS_OVERRIDE_ENABLE_WHITE_FLAG_FLOW=true

ITERS="${ITERS:-1}"

# Validator-side load-shedding policy knobs. Each is OPTIONAL — if set,
# the script patches the corresponding field in validator-common.yaml
# before the sweep. If unset, whatever the yaml currently has is used.
#
# START_PCT       → graduated-load-shedding-soft-limit-pct (0-100)
# SAT_PCT         → graduated-load-shedding-saturation-pct (0-100, must be
#                   >= START_PCT). Where the curve reaches 100% shedding.
#                   Default 100 = saturate at max_pending (legacy). Lower
#                   gives preventive headroom: at SAT_PCT=90 the curve
#                   reaches 100% at 90% of max_pending, preventing overshoot.
# MAX_PENDING     → max-pending-transactions (any positive int)
# SEMAPHORE_CAP   → max-pending-local-submissions (any positive int)
# SEM_SHEDDING    → semaphore-shedding-enabled (true|false). When false,
#                   the upfront "no permits available" reject is disabled
#                   while submit_semaphore still bounds submit_inner
#                   concurrency. Default leaves yaml as-is (= true).
#
# Typical sweep at fixed max_pending=1000, sem=2000:
#   for p in 100 50 25 10; do
#     MAX_PENDING=1000 SEMAPHORE_CAP=2000 START_PCT=$p \
#       ITERS=60 ./cap-policy-sweep.sh
#   done
START_PCT="${START_PCT:-}"
SAT_PCT="${SAT_PCT:-}"
MAX_PENDING="${MAX_PENDING:-}"
SEMAPHORE_CAP="${SEMAPHORE_CAP:-}"
SEM_SHEDDING="${SEM_SHEDDING:-}"

# Pool config. Total NUM_PROCS = spammer + honest + honest_cl. Default
# matches burst-sweep.sh's 24-spammer baseline so spam pressure stays
# comparable (the +2 honest procs take 2 slots, leaving 24 spammers).
NUM_PROCS="${NUM_PROCS:-26}"

# Honest pools: low-rate steady submitters that double as the fairness
# probe. Two pools by default — one open-loop (the canonical fairness
# probe) and one closed-loop (real-client effective throughput). Both
# share QPS/burst/IFR/workers config — only the loop type differs.
# 50 QPS × 2 procs = 100 QPS total ≈ 0.25% of 40k spammer load,
# negligible impact on cap-policy metrics. Set HONEST_PROC_COUNT=0 and
# HONEST_CL_PROC_COUNT=0 to disable the honest experiment entirely.
HONEST_PROC_COUNT="${HONEST_PROC_COUNT:-1}"
HONEST_CL_PROC_COUNT="${HONEST_CL_PROC_COUNT:-1}"
HONEST_QPS_PER_PROC="${HONEST_QPS_PER_PROC:-50}"
HONEST_BURST_SIZE="${HONEST_BURST_SIZE:-1}"
HONEST_BARRIER_PERIOD_MS="${HONEST_BARRIER_PERIOD_MS:-0}"
HONEST_IFR="${HONEST_IFR:-4}"
HONEST_WORKERS="${HONEST_WORKERS:-4}"
# When true, the HONEST_PROC_COUNT pool fires at fixed QPS regardless
# of inflight — first_pass_pct measures pure validator admission. The
# HONEST_CL_PROC_COUNT pool is ALWAYS closed-loop (real-client model);
# this flag does not affect it.
HONEST_OPEN_LOOP="${HONEST_OPEN_LOOP:-true}"

# Spammer pool config (canonical settings from burst-sweep.sh).
QPS_TOTAL="${QPS_TOTAL:-40000}"
DURATION="${DURATION:-15s}"
WORKERS="${WORKERS:-16}"
IN_FLIGHT_RATIO="${IN_FLIGHT_RATIO:-20}"
BURST_SIZE="${BURST_SIZE:-1800}"
# OPEN_LOOP=true makes the spammer pool fire submissions at target_qps
# regardless of in-flight count. Removes the closed-loop per-worker
# round-trip ceiling that pins per-proc submission rate well below
# nominal QPS under heavy validator contention. Use for cap-safety and
# goodput experiments where sustained validator-gate pressure matters
# more than per-tx correctness. Honest pool is always closed-loop.
OPEN_LOOP="${OPEN_LOOP:-true}"
BARRIER_PERIOD_MS="${BARRIER_PERIOD_MS:-500}"
GAS_CHUNK_SIZE="${GAS_CHUNK_SIZE:-500}"
NUM_VALIDATORS_TO_TARGET="${NUM_VALIDATORS_TO_TARGET:-1}"

# Time-series capture via Prometheus query_range. Disable with
# CAPTURE_TIMESERIES=false if you want smaller JSONL records.
CAPTURE_TIMESERIES="${CAPTURE_TIMESERIES:-true}"
PROM_URL="${PROM_URL:-http://localhost:9090}"

# Skip script-level network bring-up + teardown. The iter loop ALWAYS
# does a full network reset on every iter regardless of starting state,
# so the script-level setup is functionally redundant when the script
# is called repeatedly from a wrapper (e.g. run.sh's interleaved mode).
# Default false preserves standalone-./sweep.sh behaviour.
SKIP_NETWORK_LIFECYCLE="${SKIP_NETWORK_LIFECYCLE:-false}"

# FAST_MODE: skip per-iter network teardown + bootstrap. Validators stay
# up across iters; stress.rs reuses the gas-pool cache. Applies new
# policy via `bootstrap + docker compose restart` (not down -v + up).
# On detected iter failure (non-zero stress.rs exit code) exits with
# non-zero so the caller (run.sh) can do a full reset before retrying.
#
# NOTE: empirically slower than slow-mode despite skipping the between-
# iter docker reset — stress-multi.sh's per-iter runtime grew from
# ~84s to ~130s (likely gas-cache reuse causes stress.rs to do extra
# validation against mutated validator state, or validator state
# accumulates slowly degrading consensus speed). Keep FAST_MODE=false
# until that's understood. Set FAST_MODE=true to opt in if you want
# to experiment.
FAST_MODE="${FAST_MODE:-false}"

OUT_JSONL="sweep.jsonl"
OUT_LOG="sweep.log"
PRIVNET=/home/roman/IOTA/iotaledger/iota/dev-tools/iota-private-network
REPO=/home/roman/IOTA/iotaledger/iota
YAML_CFG="$PRIVNET/configs/validator-common.yaml"

# Derived
DURATION_SECS=$(echo "$DURATION" | sed 's/s$//')
N_SPAMMER=$((NUM_PROCS - HONEST_PROC_COUNT - HONEST_CL_PROC_COUNT))
QPS_PER_SPAMMER=$((QPS_TOTAL / N_SPAMMER))
# Spammer "offered" includes the burst at t=0 plus QPS-paced submissions
# over DURATION_SECS. Both honest pools are QPS-paced from start. All
# ignore retries (which inflate stress.rs's view but not what the client
# originally intended to submit).
SPAMMER_OFFERED=$((BURST_SIZE * N_SPAMMER + QPS_PER_SPAMMER * DURATION_SECS * N_SPAMMER))
HONEST_OFFERED=$((HONEST_QPS_PER_PROC * DURATION_SECS * HONEST_PROC_COUNT))
HONEST_CL_OFFERED=$((HONEST_QPS_PER_PROC * DURATION_SECS * HONEST_CL_PROC_COUNT))

# Host + git context captured ONCE at sweep start; embedded into every
# JSONL record so post-hoc analysis can group / filter by host or commit.
HOST_NAME="$(hostname)"
HOST_NPROC="$(nproc 2>/dev/null || echo 0)"
HOST_KERNEL="$(uname -r)"
HOST_MEM_GIB="$(awk '/MemTotal/ {printf "%.0f", $2/1024/1024}' /proc/meminfo 2>/dev/null || echo 0)"
IOTA_GIT_COMMIT="$(git -C "$REPO" rev-parse --short HEAD 2>/dev/null || echo unknown)"

# JSONL output is append-only; no header required (each line is self-describing).

exec >> "$OUT_LOG" 2>&1

echo "================ sweep $(date -u) ================"
echo "config: NUM_PROCS=$NUM_PROCS  (spammer=$N_SPAMMER honest=$HONEST_PROC_COUNT honest_cl=$HONEST_CL_PROC_COUNT)"
echo "        spammer:   N=$N_SPAMMER QPS_per=$QPS_PER_SPAMMER BURST=$BURST_SIZE BAR=${BARRIER_PERIOD_MS}ms OPEN_LOOP=$OPEN_LOOP"
if [ "$HONEST_PROC_COUNT" -gt 0 ]; then
  echo "        honest:    N=$HONEST_PROC_COUNT QPS_per=$HONEST_QPS_PER_PROC OPEN_LOOP=$HONEST_OPEN_LOOP"
fi
if [ "$HONEST_CL_PROC_COUNT" -gt 0 ]; then
  echo "        honest_cl: N=$HONEST_CL_PROC_COUNT QPS_per=$HONEST_QPS_PER_PROC OPEN_LOOP=false (closed-loop)"
fi
echo "        offered per iter: spammer=$SPAMMER_OFFERED  honest=$HONEST_OFFERED  honest_cl=$HONEST_CL_OFFERED"
echo "        capture_timeseries=$CAPTURE_TIMESERIES  prom=$PROM_URL"
echo

# Patch a `key: int_value` line in validator-common.yaml. Used to set
# load-shedding knobs (graduated-soft-limit-pct, max-pending-transactions,
# max-pending-local-submissions) before the per-iter network reset
# regenerates each validator's config from this overlay yaml.
#
# Args: $1 = yaml key, $2 = new integer value
# If $2 is empty, no-op (leave the yaml as-is). Verifies the patch
# actually landed by re-reading the yaml — sed silently does nothing
# if the key is misspelled, and that's a foot-gun that wastes hours.
patch_yaml_int() {
  local key="$1" value="$2"
  [ -z "$value" ] && return 0
  if ! [[ "$value" =~ ^[0-9]+$ ]]; then
    echo "Error: $key must be a positive integer, got '$value'" >&2
    exit 1
  fi
  sed -i -E "s/^([[:space:]]*${key}:[[:space:]]*).*/\1${value}/" "$YAML_CFG"
  local actual=$(grep -E "^[[:space:]]*${key}:" "$YAML_CFG" | awk -F: '{print $2}' | xargs)
  if [ "$actual" != "$value" ]; then
    echo "Error: yaml patch did not stick for $key (asked for $value, found '$actual')" >&2
    exit 1
  fi
  echo "=> Patched $YAML_CFG: ${key} = $value"
}

# Apply patches in order. graduated-load-shedding-soft-limit-pct gets
# extra validation (must be 0-100).
if [ -n "$START_PCT" ]; then
  if [ "$START_PCT" -gt 100 ] 2>/dev/null; then
    echo "Error: START_PCT must be in [0, 100], got '$START_PCT'" >&2
    exit 1
  fi
fi
patch_yaml_bool() {
  local key="$1" value="$2"
  [ -z "$value" ] && return 0
  if [ "$value" != "true" ] && [ "$value" != "false" ]; then
    echo "Error: $key must be 'true' or 'false', got '$value'" >&2
    exit 1
  fi
  sed -i -E "s/^([[:space:]]*${key}:[[:space:]]*).*/\1${value}/" "$YAML_CFG"
  local actual=$(grep -E "^[[:space:]]*${key}:" "$YAML_CFG" | awk -F: '{print $2}' | xargs)
  if [ "$actual" != "$value" ]; then
    echo "Error: yaml patch did not stick for $key (asked for $value, found '$actual')" >&2
    exit 1
  fi
  echo "=> Patched $YAML_CFG: ${key} = $value"
}

patch_yaml_int "max-pending-transactions"               "$MAX_PENDING"
patch_yaml_int "max-pending-local-submissions"          "$SEMAPHORE_CAP"
patch_yaml_int "graduated-load-shedding-soft-limit-pct" "$START_PCT"
patch_yaml_int "graduated-load-shedding-saturation-pct" "$SAT_PCT"
patch_yaml_bool "semaphore-shedding-enabled"            "$SEM_SHEDDING"

# FAST_MODE: apply the patched yaml WITHOUT destroying validator volumes.
# bootstrap.sh regenerates per-validator configs from the patched yaml;
# `docker compose restart` bounces containers so they re-read the new
# configs. Validators boot up with new policy, preserving their authority
# DB + gas-pool state from previous iters. Saves ~50s per iter vs the
# full down-v + up cycle (which forces stress.rs to re-init the gas pool).
apply_policy_via_restart() {
  echo
  echo "=== FAST_MODE: applying patched yaml via bootstrap + restart ==="
  cd "$PRIVNET"
  sudo ./bootstrap.sh -b -n 4 2>&1 | tail -3
  # --force-recreate guarantees containers actually restart with the new
  # configs (plain `restart` can be quirky on already-running containers).
  # Volumes are preserved, so validator DB + gas-pool state carry over.
  docker compose up -d --force-recreate 2>&1 | tail -5
  cd "$REPO"

  # TWO-step readiness check:
  #   1. Prometheus reports up{host=validator.*}=1 for all 4 validators
  #      (validator metric server is responding to scrapes)
  #   2. fullnode-1 RPC actually responds to a system-state query
  #      (this is what stress.rs needs — without it stress.rs panics
  #      with "Failed to get latest committee").
  # 60s cap (was 30s) since validator startup + consensus quorum can
  # legitimately take 20-40s after a container recreate.
  echo -n "  waiting for validator metric scrapes..."
  for attempt in $(seq 1 60); do
    up_count=$(curl -sG --max-time 2 'http://localhost:9090/api/v1/query' \
      --data-urlencode 'query=count(up{host=~"validator.*"}==1)' \
      2>/dev/null | jq -r '.data.result[0].value[1] // "0"')
    if [ "${up_count:-0}" -ge 4 ] 2>/dev/null; then
      echo " 4/4 up after ${attempt}s"
      break
    fi
    sleep 1
    echo -n "."
  done
  echo -n "  waiting for RPC ready (fullnode-1 chain_id)..."
  for attempt in $(seq 1 60); do
    if curl -sf --max-time 2 -X POST -H "Content-Type: application/json" \
      -d '{"jsonrpc":"2.0","id":1,"method":"iota_getChainIdentifier","params":[]}' \
      http://localhost:9000 >/dev/null 2>&1; then
      echo " ready after ${attempt}s"
      break
    fi
    sleep 1
    echo -n "."
  done
  sleep 3  # grace for consensus quorum to fully form
}

# Bump per-process file descriptor limit to the hard cap. Each stress
# subprocess opens many gRPC connections; the default soft limit of 1024
# is too tight at NUM_PROCS=96. Hard limit is whatever the system permits;
# raising the soft up to it doesn't require root.
HARD_NOFILE=$(ulimit -Hn)
if [ "$(ulimit -n)" -lt "$HARD_NOFILE" ]; then
  ulimit -n "$HARD_NOFILE" 2>/dev/null \
    || echo "  (warning: could not raise file descriptor soft limit to $HARD_NOFILE)"
fi
echo "  ulimit -n  ✓ soft=$(ulimit -n) hard=$HARD_NOFILE"

# -------- Pre-flight (matches burst-sweep.sh) ---------
PREFLIGHT_OK=1
echo "=== pre-flight checks ==="

if sudo -n true 2>/dev/null; then
  echo "  sudo cache  ✓"
else
  echo "  sudo cache  ✗ — run \`sudo -v\` first"
  PREFLIGHT_OK=0
fi

if [ "$EUID" -eq 0 ]; then
  echo "  target/ own ✓ skipped (running as root)"
else
  FOREIGN=$(find "$REPO/target" ! -uid "$(id -u)" 2>/dev/null | head -1)
  if [ -z "$FOREIGN" ]; then
    echo "  target/ own ✓ user-owned"
  else
    echo "  target/ own ✗ found foreign-owned file: $FOREIGN"
    echo "    fix: sudo chown -R \$USER:\$USER $REPO/target"
    PREFLIGHT_OK=0
  fi
fi

if grep -q '^127\.0\.0\.11.*validator-1' /etc/hosts; then
  echo "  /etc/hosts  ✓ validator-1..4 aliased"
else
  echo "  /etc/hosts  ✗ missing validator-N → 127.0.0.{11..14} entries"
  PREFLIGHT_OK=0
fi

if [ "$PREFLIGHT_OK" -eq 0 ]; then
  echo
  echo "=== ABORTING: pre-flight checks failed — fix the issues above and re-run ==="
  exit 1
fi

# -------- Initial bring-up: iota network first, then grafana ---------
# Skipped in interleaved-runner mode — the iter loop's full reset
# (down -v + bootstrap + up) handles cold starts equally well.
if [ "$SKIP_NETWORK_LIFECYCLE" != "true" ]; then
  echo
  echo "=== bringing up network (validators + fullnode-1 + faucet) ==="
  cd "$PRIVNET"
  if [ ! -f "$PRIVNET/configs/genesis/genesis.blob" ]; then
    echo "  genesis.blob missing — running bootstrap.sh -b -n 4"
    sudo ./bootstrap.sh -b -n 4 2>&1 | tail -3
  fi
  ./run.sh -n 4 faucet 2>&1 | tail -2
  sleep 5

  echo
  echo "=== bringing up Prometheus + Grafana ==="
  cd "$REPO/dev-tools/grafana-local"
  docker compose up -d 2>&1 | tail -3

  echo -n "  waiting for prometheus..."
  PROM_READY=0
  for attempt in $(seq 1 30); do
    if curl -sf --max-time 2 'http://localhost:9090/api/v1/query?query=up' >/dev/null 2>&1; then
      PROM_READY=1
      echo " ready after ${attempt}s"
      break
    fi
    sleep 1
    echo -n "."
  done
  if [ "$PROM_READY" -eq 0 ]; then
    echo
    echo "  prometheus  ✗ still unreachable after 30s — aborting"
    exit 1
  fi
  cd "$REPO"
else
  echo
  echo "=== SKIP_NETWORK_LIFECYCLE=true — skipping initial bring-up (iter loop will reset) ==="
fi

echo
echo "=== starting sweep (ITERS=$ITERS  FAST_MODE=$FAST_MODE) ==="
echo

# FAST_MODE: apply the patched yaml ONCE via restart (no destroy). The
# iter loop below then skips the network reset between iters and just
# does pkill + drain. Validators keep running, gas-pool cache stays
# valid, stress.rs starts ~50s faster per iter.
if [ "$FAST_MODE" = "true" ]; then
  apply_policy_via_restart
fi

# -------- Iteration loop ---------
fail_streak=0
MAX_FAIL_STREAK=3
for i in $(seq 1 $ITERS); do
  echo
  echo "=================================================="
  echo "[sweep iter=$i/$ITERS  pct=${START_PCT:-(current yaml)}  fast=$FAST_MODE]  $(date -u +%H:%M:%S)"
  echo "=================================================="

  # Kill any leftover stress.rs processes from a previous iter that may
  # still be holding metric ports (8081 + i). The metric server panics
  # with "Address already in use" if a stale binary hasn't released its
  # port yet — stress-multi.sh waits on the bash wrapper PID, but
  # setsid puts the stress.rs grandchild in its own process group, so
  # `wait` may return before the grandchild fully cleans up.
  pkill -9 -f "target/release/stress " 2>/dev/null || true
  sleep 1

  if [ "$FAST_MODE" = "true" ]; then
    # Fast iter: validators + grafana stay up from previous iter (or from
    # apply_policy_via_restart above). Wait for the validator queue to
    # actually drain (poll Prometheus inflight gauge) rather than blind
    # sleep — adapts to depth: ~1s when already empty, up to ~8s if a
    # large queue lingers from the prev iter's spam window.
    for drain_attempt in $(seq 1 8); do
      drain_inflight=$(curl -sG --max-time 2 "$PROM_URL/api/v1/query" \
        --data-urlencode 'query=max(sum by (host) (sequencing_certificate_inflight{host=~"validator.*"}))' \
        2>/dev/null | jq -r '.data.result[0].value[1] // "0"')
      # Strip decimal portion (Prometheus may return "5.2"); threshold 10
      # allows a tiny noise margin since the gauge can briefly tick non-zero.
      drain_int=${drain_inflight%.*}
      if [ "${drain_int:-0}" -le 10 ] 2>/dev/null; then
        echo "  drained after ${drain_attempt}s (inflight=${drain_inflight})"
        break
      fi
      sleep 1
    done
    sleep 1  # small grace after drain
  else
  # Reset both stacks per iter. Grafana must come down BEFORE the iota
  # network: grafana-local containers attach to `iota-network` (for
  # validator scraping), so the network can't be removed cleanly while
  # they're up. All Prometheus queries happen at the end of each iter
  # window inside stress-multi.sh — there is no cross-iter analysis,
  # so bouncing the grafana stack per iter costs nothing and gives
  # each iter a fresh Prometheus DB.
  (cd "$REPO/dev-tools/grafana-local" && docker compose down 2>&1 | tail -1) || true
  # `docker compose down` returns before the OS fully releases host
  # ports bound by the removed containers (e.g. tempo's 3200, grafana's
  # 3000, prom's 9090). The next `compose up` then races and panics
  # with "address already in use". 2s is enough on every test we've
  # seen so far.
  sleep 2
  cd "$PRIVNET"
  docker compose down -v 2>&1 | tail -1 || true
  sudo ./bootstrap.sh -b -n 4 2>&1 | tail -3
  ./run.sh -n 4 faucet 2>&1 | tail -1
  rm -f "$REPO"/runs/.stress-gas-pool/owner-*.json
  (cd "$REPO/dev-tools/grafana-local" && docker compose up -d 2>&1 | tail -1)
  # Wait for Prometheus to be ready before the spam window starts.
  for attempt in $(seq 1 30); do
    if curl -sf --max-time 2 'http://localhost:9090/api/v1/query?query=up' >/dev/null 2>&1; then
      break
    fi
    sleep 1
  done
  # Wait for ALL 4 validators to expose the consensus inflight metric —
  # signal that they've finished startup and are ready to accept txs.
  # Replaces the legacy "sleep 20" which over-waited unnecessarily.
  # Cap at 30s as a safety fallback; typical wait is 3-8s.
  for attempt in $(seq 1 30); do
    ready_count=$(curl -sG --max-time 2 'http://localhost:9090/api/v1/query' \
      --data-urlencode 'query=count(consensus_max_pending_inflight_txs{host=~"validator.*"})' \
      2>/dev/null | jq -r '.data.result[0].value[1] // "0"')
    if [ "${ready_count:-0}" -ge 4 ] 2>/dev/null; then
      echo "  validators ready after ${attempt}s"
      break
    fi
    sleep 1
  done
  # Tiny grace period for consensus quorum formation after metric appears.
  sleep 2

  cd "$REPO"
  fi  # end !FAST_MODE per-iter reset

  # Capture iter start timestamp BEFORE stress-multi.sh launches so the
  # Prometheus time-series query window covers the spam window cleanly.
  ITER_START_EPOCH=$(date +%s)

  # Launch stress-multi.sh with both spammer and honest pool (set
  # HONEST_PROC_COUNT=0 to disable honest pool).
  NUM_PROCS="$NUM_PROCS" \
  HONEST_PROC_COUNT="$HONEST_PROC_COUNT" \
  HONEST_CL_PROC_COUNT="$HONEST_CL_PROC_COUNT" \
  HONEST_QPS_PER_PROC="$HONEST_QPS_PER_PROC" \
  HONEST_BURST_SIZE="$HONEST_BURST_SIZE" \
  HONEST_BARRIER_PERIOD_MS="$HONEST_BARRIER_PERIOD_MS" \
  HONEST_IFR="$HONEST_IFR" \
  HONEST_WORKERS="$HONEST_WORKERS" \
  HONEST_OPEN_LOOP="$HONEST_OPEN_LOOP" \
  NUM_VALIDATORS_TO_TARGET="$NUM_VALIDATORS_TO_TARGET" \
  QPS_TOTAL="$QPS_TOTAL" \
  DURATION="$DURATION" \
  WORKERS="$WORKERS" \
  IN_FLIGHT_RATIO="$IN_FLIGHT_RATIO" \
  BURST_SIZE="$BURST_SIZE" \
  OPEN_LOOP="$OPEN_LOOP" \
  BARRIER_PERIOD_MS="$BARRIER_PERIOD_MS" \
  GAS_CHUNK_SIZE="$GAS_CHUNK_SIZE" \
  ./stress-multi.sh 2>&1 | tail -50 | tee "$REPO/runs/sweep-iter.log"

  ITER_END_EPOCH=$(date +%s)

  # Parse per-iter summary + emit one JSONL record.
  latest=$(ls -td "$REPO"/runs/multi-*/ | head -1)

  # Read validator config from the yaml at THIS moment so each record
  # captures what was actually deployed (in case the yaml was patched
  # between sweeps or by hand).
  read_yaml_int() {
    grep -E "^[[:space:]]*$1:" "$YAML_CFG" 2>/dev/null | awk -F: '{print $2}' | xargs
  }
  val_max_pending=$(read_yaml_int "max-pending-transactions")
  val_sem_cap=$(read_yaml_int "max-pending-local-submissions")
  val_start_pct=$(read_yaml_int "graduated-load-shedding-soft-limit-pct")
  val_sat_pct=$(read_yaml_int "graduated-load-shedding-saturation-pct")
  : "${val_sat_pct:=100}"
  val_sem_shedding=$(read_yaml_int "semaphore-shedding-enabled")
  # Default to "true" if the yaml line is somehow missing (matches Rust default).
  : "${val_sem_shedding:=true}"

  if [ -f "$latest/summary.txt" ]; then
    peak=$(grep '^peak inflight:' "$latest/summary.txt" | awk -F: '{print $2}' | xargs)
    ratio=$(grep '^ratio:' "$latest/summary.txt" | awk -F: '{print $2}' | xargs | sed 's/×//')
    exits=$(grep '^exit codes:' "$latest/summary.txt" | awk -F: '{print $2}' | xargs)
    r_prev=$(grep '^reject_grad_preventive:' "$latest/summary.txt" | awk -F: '{print $2}' | xargs)
    r_grad_react=$(grep '^reject_grad_reactive:' "$latest/summary.txt" | awk -F: '{print $2}' | xargs)
    r_grad_sat=$(grep '^reject_grad_saturated:' "$latest/summary.txt" | awk -F: '{print $2}' | xargs)
    r_max=$(grep '^reject_max_pending:' "$latest/summary.txt" | awk -F: '{print $2}' | xargs)
    r_sem=$(grep '^reject_semaphore:' "$latest/summary.txt" | awk -F: '{print $2}' | xargs)
    useful_tps=$(grep '^useful_tps:' "$latest/summary.txt" | awk -F: '{print $2}' | xargs)
    admit_p50=$(grep '^admit_lat_p50:' "$latest/summary.txt" | awk -F: '{print $2}' | xargs)
    admit_p99=$(grep '^admit_lat_p99:' "$latest/summary.txt" | awk -F: '{print $2}' | xargs)
    permit_hold_p50=$(grep '^permit_hold_p50:' "$latest/summary.txt" | awk -F: '{print $2}' | xargs)
    permit_hold_p90=$(grep '^permit_hold_p90:' "$latest/summary.txt" | awk -F: '{print $2}' | xargs)
    permit_hold_p95=$(grep '^permit_hold_p95:' "$latest/summary.txt" | awk -F: '{print $2}' | xargs)
    permit_hold_p99=$(grep '^permit_hold_p99:' "$latest/summary.txt" | awk -F: '{print $2}' | xargs)
    permit_hold_p999=$(grep '^permit_hold_p999:' "$latest/summary.txt" | awk -F: '{print $2}' | xargs)
    permit_wait_p50=$(grep '^permit_wait_p50:' "$latest/summary.txt" | awk -F: '{print $2}' | xargs)
    permit_wait_p90=$(grep '^permit_wait_p90:' "$latest/summary.txt" | awk -F: '{print $2}' | xargs)
    permit_wait_p95=$(grep '^permit_wait_p95:' "$latest/summary.txt" | awk -F: '{print $2}' | xargs)
    permit_wait_p99=$(grep '^permit_wait_p99:' "$latest/summary.txt" | awk -F: '{print $2}' | xargs)
    permit_wait_p999=$(grep '^permit_wait_p999:' "$latest/summary.txt" | awk -F: '{print $2}' | xargs)
    pre_acquire_p50=$(grep '^pre_acquire_p50:' "$latest/summary.txt" | awk -F: '{print $2}' | xargs)
    pre_acquire_p99=$(grep '^pre_acquire_p99:' "$latest/summary.txt" | awk -F: '{print $2}' | xargs)
    shed_pct_avg=$(grep '^shed_pct_avg:' "$latest/summary.txt" | awk -F: '{print $2}' | xargs)
    shed_pct_max=$(grep '^shed_pct_max:' "$latest/summary.txt" | awk -F: '{print $2}' | xargs)
    inflight_stddev=$(grep '^inflight_stddev:' "$latest/summary.txt" | awk -F: '{print $2}' | xargs)
    inflight_mean=$(grep '^inflight_mean:' "$latest/summary.txt" | awk -F: '{print $2}' | xargs)
    saturation_75pct=$(grep '^saturation_75pct:' "$latest/summary.txt" | awk -F: '{print $2}' | xargs)
    consensus_lat_p50=$(grep '^consensus_lat_p50:' "$latest/summary.txt" | awk -F: '{print $2}' | xargs)
    consensus_lat_p90=$(grep '^consensus_lat_p90:' "$latest/summary.txt" | awk -F: '{print $2}' | xargs)
    consensus_lat_p95=$(grep '^consensus_lat_p95:' "$latest/summary.txt" | awk -F: '{print $2}' | xargs)
    consensus_lat_p99=$(grep '^consensus_lat_p99:' "$latest/summary.txt" | awk -F: '{print $2}' | xargs)
    consensus_lat_p999=$(grep '^consensus_lat_p999:' "$latest/summary.txt" | awk -F: '{print $2}' | xargs)
    spam_start_epoch=$(grep '^spam_start_epoch:' "$latest/summary.txt" | awk -F: '{print $2}' | xargs)
    spam_end_epoch=$(grep '^spam_end_epoch:' "$latest/summary.txt" | awk -F: '{print $2}' | xargs)
    spammer_success=$(grep '^spammer_success:' "$latest/summary.txt" | awk -F: '{print $2}' | xargs)
    honest_success=$(grep '^honest_success:' "$latest/summary.txt" | awk -F: '{print $2}' | xargs)
    honest_cl_success=$(grep '^honest_cl_success:' "$latest/summary.txt" | awk -F: '{print $2}' | xargs)

    : "${peak:=0}"; : "${ratio:=0}"; : "${r_prev:=0}"; : "${r_grad_react:=0}"
    : "${r_max:=0}"; : "${r_sem:=0}"; : "${r_grad_sat:=0}"; : "${useful_tps:=0}"; : "${admit_p50:=0}"; : "${admit_p99:=0}"
    : "${permit_hold_p50:=0}"; : "${permit_hold_p90:=0}"; : "${permit_hold_p95:=0}"
    : "${permit_hold_p99:=0}"; : "${permit_hold_p999:=0}"
    : "${permit_wait_p50:=0}"; : "${permit_wait_p90:=0}"; : "${permit_wait_p95:=0}"
    : "${permit_wait_p99:=0}"; : "${permit_wait_p999:=0}"
    : "${pre_acquire_p50:=0}"; : "${pre_acquire_p99:=0}"
    : "${shed_pct_avg:=0}"; : "${shed_pct_max:=0}"
    : "${inflight_stddev:=0}"; : "${inflight_mean:=0}"; : "${saturation_75pct:=0}"
    : "${consensus_lat_p50:=0}"; : "${consensus_lat_p90:=0}"; : "${consensus_lat_p95:=0}"
    : "${consensus_lat_p99:=0}"; : "${consensus_lat_p999:=0}"
    : "${spam_start_epoch:=0}"; : "${spam_end_epoch:=0}"
    : "${spammer_success:=0}"; : "${honest_success:=0}"; : "${honest_cl_success:=0}"

    ok=$(echo "$exits" | awk '{for(j=1;j<=NF;j++) if($j!="0"){print 0; exit} print 1}')
    spammer_fp=$(awk -v s="$spammer_success" -v o="$SPAMMER_OFFERED" \
      'BEGIN{if(o>0) printf "%.4f", 100.0*s/o; else print 0}')
    honest_fp=$(awk -v s="$honest_success" -v o="$HONEST_OFFERED" \
      'BEGIN{if(o>0) printf "%.4f", 100.0*s/o; else print 0}')
    honest_cl_fp=$(awk -v s="$honest_cl_success" -v o="$HONEST_CL_OFFERED" \
      'BEGIN{if(o>0) printf "%.4f", 100.0*s/o; else print 0}')
    iso=$(basename "$latest" | sed 's/multi-//')
    failed=0
  else
    iso=$(basename "$latest" 2>/dev/null | sed 's/multi-//' || echo "?")
    peak=0; ratio=0; r_prev=0; r_grad_react=0; r_grad_sat=0; r_max=0; r_sem=0
    useful_tps=0; admit_p50=0; admit_p99=0
    permit_hold_p50=0; permit_hold_p90=0; permit_hold_p95=0; permit_hold_p99=0; permit_hold_p999=0
    permit_wait_p50=0; permit_wait_p90=0; permit_wait_p95=0; permit_wait_p99=0; permit_wait_p999=0
    pre_acquire_p50=0; pre_acquire_p99=0
    shed_pct_avg=0; shed_pct_max=0
    inflight_stddev=0; inflight_mean=0; saturation_75pct=0
    consensus_lat_p50=0; consensus_lat_p90=0; consensus_lat_p95=0; consensus_lat_p99=0; consensus_lat_p999=0
    spam_start_epoch=0; spam_end_epoch=0
    spammer_success=0; spammer_fp=0; honest_success=0; honest_fp=0
    honest_cl_success=0; honest_cl_fp=0; ok=0; exits=""
    failed=1
  fi

  # Per-pool client-side latency stats. Each stress process emits
  # benchmark_stats.json with a full latency_ms array — we aggregate
  # across the spammer pool (procs 0..N_SPAMMER-1) and the honest pool
  # (procs N_SPAMMER..total-1) and emit percentiles. This is the only
  # per-pool latency data available (validator histograms aggregate all
  # sources together), so without this the fairness story can only
  # claim admission-rate fairness, not latency fairness.
  PER_POOL_STATS='{}'
  if [ "$failed" -eq 0 ]; then
    PER_POOL_STATS=$(LATEST="$latest" N_SPAMMER="$N_SPAMMER" N_HONEST="$HONEST_PROC_COUNT" N_HONEST_CL="$HONEST_CL_PROC_COUNT" python3 -c '
import json, glob, os
latest = os.environ["LATEST"].rstrip("/")
n_spam = int(os.environ["N_SPAMMER"])
n_honest = int(os.environ["N_HONEST"])
n_honest_cl = int(os.environ["N_HONEST_CL"])

def collect(indices):
    lats = []
    success = 0
    errors = 0
    for i in indices:
        files = glob.glob(f"{latest}/process-{i}/*/benchmark_stats.json")
        if not files:
            continue
        try:
            with open(files[0]) as f:
                d = json.load(f)
        except (OSError, json.JSONDecodeError):
            continue
        lats.extend(d.get("latency_ms", []))
        success += d.get("num_success_txes", 0) or 0
        errors += d.get("num_error_txes", 0) or 0
    return lats, success, errors

def percentiles(arr):
    if not arr:
        return {}
    s = sorted(arr)
    out = {}
    for p in [50, 75, 90, 95, 99, 99.9]:
        key = f"p{p if p != int(p) else int(p)}"
        idx = min(int(p / 100 * len(s)), len(s) - 1)
        out[key] = s[idx]
    return out

spam_lats, spam_succ, spam_err = collect(range(n_spam))
hon_lats, hon_succ, hon_err = collect(range(n_spam, n_spam + n_honest))
cl_lats, cl_succ, cl_err = collect(range(n_spam + n_honest, n_spam + n_honest + n_honest_cl))

print(json.dumps({
    "spammer": {
        "success": spam_succ,
        "errors": spam_err,
        "n_lat": len(spam_lats),
        "lat_ms": percentiles(spam_lats),
    },
    "honest": {
        "success": hon_succ,
        "errors": hon_err,
        "n_lat": len(hon_lats),
        "lat_ms": percentiles(hon_lats),
    },
    "honest_cl": {
        "success": cl_succ,
        "errors": cl_err,
        "n_lat": len(cl_lats),
        "lat_ms": percentiles(cl_lats),
    },
}))
' 2>/dev/null || echo '{}')
  fi

  # Time-series scrape via Prometheus query_range. Each query returns a
  # JSON array of [timestamp, value] pairs at the requested step. All series
  # are aggregated to a single number across validators per timestamp
  # (sum-by-host then max, mirroring stress-multi.sh's scalar queries).
  TS_INFLIGHT="null"
  TS_SHED_PCT="null"
  TS_REJ_REACTIVE="null"
  TS_REJ_SATURATED="null"
  TS_REJ_PREVENTIVE="null"
  TS_PERMIT_WAIT_P99="null"
  TS_PERMIT_HOLD_P99="null"
  TS_PRE_ACQUIRE_P99="null"
  TS_CONSENSUS_LAT_P99="null"
  TS_TPS="null"
  if [ "$CAPTURE_TIMESERIES" = "true" ] && [ "$failed" -eq 0 ]; then
    scrape_range() {
      local query="$1" step="${2:-0.1}"
      curl -sG --max-time 5 "$PROM_URL/api/v1/query_range" \
        --data-urlencode "query=$query" \
        --data-urlencode "start=$ITER_START_EPOCH" \
        --data-urlencode "end=$ITER_END_EPOCH" \
        --data-urlencode "step=$step" \
        2>/dev/null \
      | jq -c '.data.result[0].values // []' 2>/dev/null \
      || echo "null"
    }
    TS_INFLIGHT=$(scrape_range 'max(sum by (host) (sequencing_certificate_inflight{host=~"validator.*"}))' 0.1)
    TS_SHED_PCT=$(scrape_range 'max(consensus_queue_load_shedding_percentage{host=~"validator.*"})' 0.1)
    # Rejections live on transaction_overload_sources as label values, not
    # separate metrics. Lookback [5s] smooths sparse counter updates.
    TS_REJ_REACTIVE=$(scrape_range 'sum(rate(transaction_overload_sources{host=~"validator.*", source="consensus_graduated_reactive"}[5s]))' 1)
    TS_REJ_SATURATED=$(scrape_range 'sum(rate(transaction_overload_sources{host=~"validator.*", source="consensus_graduated_saturated"}[5s]))' 1)
    TS_REJ_PREVENTIVE=$(scrape_range 'sum(rate(transaction_overload_sources{host=~"validator.*", source="consensus_graduated_preventive"}[5s]))' 1)
    # Histogram quantiles need wider lookback to accumulate enough bucket
    # samples for a stable percentile. [1s] returns mostly NaN.
    TS_PERMIT_WAIT_P99=$(scrape_range 'histogram_quantile(0.99, sum by (le) (rate(sequencing_submit_permit_wait_duration_bucket{host=~"validator.*"}[5s])))' 1)
    # Stage C (permit_hold), stage A (pre_acquire), e2e (consensus_lat)
    # at p99 over time — completes the per-stage latency picture so we
    # can see WHEN each stage misbehaves during the spam window.
    TS_PERMIT_HOLD_P99=$(scrape_range 'histogram_quantile(0.99, sum by (le) (rate(sequencing_submit_permit_hold_duration_bucket{host=~"validator.*"}[5s])))' 1)
    TS_PRE_ACQUIRE_P99=$(scrape_range 'histogram_quantile(0.99, sum by (le) (rate(sequencing_submit_pre_acquire_duration_bucket{host=~"validator.*"}[5s])))' 1)
    TS_CONSENSUS_LAT_P99=$(scrape_range 'histogram_quantile(0.99, sum by (le) (rate(sequencing_certificate_latency_bucket{host=~"validator.*"}[5s])))' 1)
    # TPS = total_transaction_effects counter (matches stress-multi.sh useful_tps).
    TS_TPS=$(scrape_range 'sum(rate(total_transaction_effects{host=~"validator.*"}[5s]))' 1)
  fi

  # Emit one JSONL record. Values passed via env to avoid shell-quoting
  # hell inside the python one-liner. Time-series JSON blobs pass
  # through verbatim and are parsed inside the python helper.
  ISO="$iso" ITER="$i" FAILED="$failed" \
  ITER_START_EPOCH="$ITER_START_EPOCH" ITER_END_EPOCH="$ITER_END_EPOCH" \
  HOST_NAME="$HOST_NAME" HOST_NPROC="$HOST_NPROC" HOST_KERNEL="$HOST_KERNEL" HOST_MEM_GIB="$HOST_MEM_GIB" \
  IOTA_GIT_COMMIT="$IOTA_GIT_COMMIT" \
  SPAM_NUM_PROCS="$N_SPAMMER" SPAM_QPS_PER="$QPS_PER_SPAMMER" SPAM_QPS_TOTAL="$QPS_TOTAL" \
  SPAM_BURST="$BURST_SIZE" SPAM_BAR_MS="$BARRIER_PERIOD_MS" SPAM_DURATION="$DURATION" SPAM_DURATION_SECS="$DURATION_SECS" \
  SPAM_WORKERS="$WORKERS" SPAM_IFR="$IN_FLIGHT_RATIO" SPAM_OPEN_LOOP="$OPEN_LOOP" \
  SPAM_GAS_CHUNK="$GAS_CHUNK_SIZE" SPAM_NUM_VALIDATORS="$NUM_VALIDATORS_TO_TARGET" \
  SPAM_OFFERED="$SPAMMER_OFFERED" \
  HON_PROC_COUNT="$HONEST_PROC_COUNT" HON_QPS_PER="$HONEST_QPS_PER_PROC" HON_BURST="$HONEST_BURST_SIZE" \
  HON_BAR_MS="$HONEST_BARRIER_PERIOD_MS" HON_IFR="$HONEST_IFR" HON_WORKERS="$HONEST_WORKERS" \
  HON_OPEN_LOOP="$HONEST_OPEN_LOOP" \
  HON_OFFERED="$HONEST_OFFERED" HON_SUCCESS="$honest_success" HON_FP="$honest_fp" \
  HON_CL_PROC_COUNT="$HONEST_CL_PROC_COUNT" \
  HON_CL_OFFERED="$HONEST_CL_OFFERED" HON_CL_SUCCESS="$honest_cl_success" HON_CL_FP="$honest_cl_fp" \
  VAL_MAX_PENDING="$val_max_pending" VAL_SEM_CAP="$val_sem_cap" VAL_START_PCT="$val_start_pct" \
  VAL_SAT_PCT="$val_sat_pct" \
  VAL_SEM_SHEDDING="$val_sem_shedding" \
  R_PEAK="$peak" R_RATIO="$ratio" R_USEFUL_TPS="$useful_tps" \
  R_ADMIT_LAT_P50="$admit_p50" R_ADMIT_LAT_P99="$admit_p99" \
  R_PERMIT_HOLD_P50="$permit_hold_p50" R_PERMIT_HOLD_P90="$permit_hold_p90" R_PERMIT_HOLD_P95="$permit_hold_p95" R_PERMIT_HOLD_P99="$permit_hold_p99" R_PERMIT_HOLD_P999="$permit_hold_p999" \
  R_PERMIT_WAIT_P50="$permit_wait_p50" R_PERMIT_WAIT_P90="$permit_wait_p90" R_PERMIT_WAIT_P95="$permit_wait_p95" R_PERMIT_WAIT_P99="$permit_wait_p99" R_PERMIT_WAIT_P999="$permit_wait_p999" \
  R_PRE_ACQUIRE_P50="$pre_acquire_p50" R_PRE_ACQUIRE_P99="$pre_acquire_p99" \
  R_SHED_PCT_AVG="$shed_pct_avg" R_SHED_PCT_MAX="$shed_pct_max" \
  R_INFLIGHT_STDDEV="$inflight_stddev" R_INFLIGHT_MEAN="$inflight_mean" \
  R_SATURATION_75PCT="$saturation_75pct" \
  R_CONSENSUS_LAT_P50="$consensus_lat_p50" R_CONSENSUS_LAT_P90="$consensus_lat_p90" R_CONSENSUS_LAT_P95="$consensus_lat_p95" R_CONSENSUS_LAT_P99="$consensus_lat_p99" R_CONSENSUS_LAT_P999="$consensus_lat_p999" \
  R_SPAM_START_EPOCH="$spam_start_epoch" R_SPAM_END_EPOCH="$spam_end_epoch" \
  R_SPAMMER_SUCCESS="$spammer_success" R_SPAMMER_FP="$spammer_fp" \
  R_REJ_PREV="$r_prev" R_REJ_REACT="$r_grad_react" R_REJ_SAT="$r_grad_sat" R_REJ_MAX="$r_max" R_REJ_SEM="$r_sem" \
  R_EXIT_CODES="$exits" R_OK="$ok" \
  TS_INFLIGHT="$TS_INFLIGHT" TS_SHED_PCT="$TS_SHED_PCT" \
  TS_REJ_REACTIVE="$TS_REJ_REACTIVE" TS_REJ_SATURATED="$TS_REJ_SATURATED" TS_REJ_PREVENTIVE="$TS_REJ_PREVENTIVE" \
  TS_PERMIT_WAIT_P99="$TS_PERMIT_WAIT_P99" TS_PERMIT_HOLD_P99="$TS_PERMIT_HOLD_P99" \
  TS_PRE_ACQUIRE_P99="$TS_PRE_ACQUIRE_P99" TS_CONSENSUS_LAT_P99="$TS_CONSENSUS_LAT_P99" \
  TS_TPS="$TS_TPS" \
  PER_POOL_STATS="$PER_POOL_STATS" \
  python3 -c '
import json, os
def s(k, d=""): return os.environ.get(k, d)
def i(k):
    v=os.environ.get(k,"")
    try: return int(v)
    except: return None
def f(k):
    v=os.environ.get(k,"")
    try: return float(v)
    except: return None
def b(k): return os.environ.get(k,"").lower()=="true"
def ints(k):
    return [int(x) for x in s(k).split() if x.lstrip("-").isdigit()]
def ts(k):
    """Parse a time-series JSON blob like [[ts,\"v\"], ...] from env.
    Returns a list of [float, float] pairs, or None on parse failure /
    explicit null sentinel from the shell scrape."""
    raw = os.environ.get(k, "null")
    if not raw or raw == "null":
        return None
    try:
        arr = json.loads(raw)
        if not isinstance(arr, list):
            return None
        out = []
        for pair in arr:
            if not isinstance(pair, list) or len(pair) != 2:
                continue
            try:
                t = float(pair[0])
                v = float(pair[1])
                out.append([t, v])
            except (TypeError, ValueError):
                continue
        return out if out else None
    except (json.JSONDecodeError, ValueError):
        return None

# Per-pool client-side latency aggregated from benchmark_stats.json files
# (emitted by the upstream PER_POOL_STATS bash block). Shape:
#   {"spammer": {"success": int, "errors": int, "n_lat": int,
#                "lat_ms": {"p50": int, "p75": int, ..., "p99.9": int}},
#    "honest":  {same}}
# Falls back to empty {} on parse failure.
try:
    per_pool = json.loads(os.environ.get("PER_POOL_STATS", "{}") or "{}")
except (json.JSONDecodeError, ValueError):
    per_pool = {}
spam_pool = per_pool.get("spammer", {}) or {}
hon_pool = per_pool.get("honest", {}) or {}
hon_cl_pool = per_pool.get("honest_cl", {}) or {}

def effective_qps(pool, n_procs_env, qps_per_env):
    """Per-pool effective QPS = lat_ms_n / duration_secs.

    Uses lat_ms_n (count of txs that received a validator response) rather
    than num_success_txes because stress.rs does not increment success/error
    counters for open-loop pools — only the latency array is populated.
    lat_ms_n is therefore the universally correct measured-attempt count:
      open-loop:    fired QPS, recorded latency for each acked response.
                    lat_ms_n / dur = actual response rate.
      closed-loop:  wait-then-fire; latency recorded for each completed
                    attempt. lat_ms_n / dur = effective submission rate.

    `target_qps = qps_per_proc * proc_count` is the configured offered rate.
    `throughput_ratio = effective_qps / target_qps`:
      1.0  → pool kept up with target (validator admitted ~all)
      <1.0 → either validator rejected (open-loop) or pool self-throttled
              (closed-loop). For honest_cl this proxies retry-storm pressure.
    """
    n_lat = pool.get("n_lat") or 0
    dur = i("SPAM_DURATION_SECS") or 0
    n = i(n_procs_env) or 0
    qp = i(qps_per_env) or 0
    target = n * qp
    eff = n_lat / dur if dur else 0
    ratio = eff / target if target else None
    return {
        "effective_qps": round(eff, 2),
        "target_qps": target if target else None,
        "throughput_ratio": round(ratio, 4) if ratio is not None else None,
    }

spam_eff = effective_qps(spam_pool, "SPAM_NUM_PROCS", "SPAM_QPS_PER")
hon_eff = effective_qps(hon_pool, "HON_PROC_COUNT", "HON_QPS_PER")
hon_cl_eff = effective_qps(hon_cl_pool, "HON_CL_PROC_COUNT", "HON_QPS_PER")

rec = {
  "iso_time": s("ISO"),
  "iter": i("ITER"),
  "failed": i("FAILED")==1,
  "iter_window": {
    # ITER_START/END span the entire stress-multi.sh invocation (~80s
    # including setup + cooldown). spam_start/end pinpoint the actual
    # 15s spam window — use these to slice time-series in analysis.
    "start_epoch": i("ITER_START_EPOCH"),
    "end_epoch": i("ITER_END_EPOCH"),
    "spam_start_epoch": i("R_SPAM_START_EPOCH"),
    "spam_end_epoch": i("R_SPAM_END_EPOCH"),
  },
  "host": {
    "hostname": s("HOST_NAME"),
    "nproc": i("HOST_NPROC"),
    "kernel": s("HOST_KERNEL"),
    "total_mem_gib": i("HOST_MEM_GIB"),
  },
  "git": {"iota_commit": s("IOTA_GIT_COMMIT")},
  "spammer": {
    "num_procs": i("SPAM_NUM_PROCS"),
    "qps_per_proc": i("SPAM_QPS_PER"),
    "qps_total": i("SPAM_QPS_TOTAL"),
    "burst_size": i("SPAM_BURST"),
    "barrier_period_ms": i("SPAM_BAR_MS"),
    "duration": s("SPAM_DURATION"),
    "duration_secs": i("SPAM_DURATION_SECS"),
    "workers": i("SPAM_WORKERS"),
    "in_flight_ratio": i("SPAM_IFR"),
    "open_loop": b("SPAM_OPEN_LOOP"),
    "gas_chunk_size": i("SPAM_GAS_CHUNK"),
    "num_validators_to_target": i("SPAM_NUM_VALIDATORS"),
    "offered": i("SPAM_OFFERED"),
    "success": i("R_SPAMMER_SUCCESS"),
    "first_pass_pct": f("R_SPAMMER_FP"),
    # Per-pool client-side latency (ms) from benchmark_stats.json. The
    # success/errors fields here come from the same source and may differ
    # from R_SPAMMER_SUCCESS (which goes through stress-multi.sh
    # aggregation and has known post-retry counter artifacts).
    "bench_success": spam_pool.get("success"),
    "bench_errors": spam_pool.get("errors"),
    "lat_ms_n": spam_pool.get("n_lat"),
    "lat_ms": spam_pool.get("lat_ms") or None,
    "effective_qps": spam_eff["effective_qps"],
    "target_qps": spam_eff["target_qps"],
    "throughput_ratio": spam_eff["throughput_ratio"],
  },
  "honest": {
    "proc_count": i("HON_PROC_COUNT"),
    "qps_per_proc": i("HON_QPS_PER"),
    "burst_size": i("HON_BURST"),
    "barrier_period_ms": i("HON_BAR_MS"),
    "in_flight_ratio": i("HON_IFR"),
    "workers": i("HON_WORKERS"),
    "open_loop": b("HON_OPEN_LOOP"),
    "offered": i("HON_OFFERED"),
    "success": i("HON_SUCCESS"),
    "first_pass_pct": f("HON_FP"),
    "bench_success": hon_pool.get("success"),
    "bench_errors": hon_pool.get("errors"),
    "lat_ms_n": hon_pool.get("n_lat"),
    "lat_ms": hon_pool.get("lat_ms") or None,
    "effective_qps": hon_eff["effective_qps"],
    "target_qps": hon_eff["target_qps"],
    "throughput_ratio": hon_eff["throughput_ratio"],
  },
  "honest_cl": {
    "proc_count": i("HON_CL_PROC_COUNT"),
    # honest_cl shares all per-proc config with the honest pool above
    # (same QPS, burst, IFR, workers) — only open_loop differs (always
    # false). Comparing honest.first_pass_pct vs honest_cl.first_pass_pct
    # quantifies the closed-loop self-throttling bias.
    "qps_per_proc": i("HON_QPS_PER"),
    "burst_size": i("HON_BURST"),
    "barrier_period_ms": i("HON_BAR_MS"),
    "in_flight_ratio": i("HON_IFR"),
    "workers": i("HON_WORKERS"),
    "open_loop": False,
    "offered": i("HON_CL_OFFERED"),
    "success": i("HON_CL_SUCCESS"),
    "first_pass_pct": f("HON_CL_FP"),
    "bench_success": hon_cl_pool.get("success"),
    "bench_errors": hon_cl_pool.get("errors"),
    "lat_ms_n": hon_cl_pool.get("n_lat"),
    "lat_ms": hon_cl_pool.get("lat_ms") or None,
    "effective_qps": hon_cl_eff["effective_qps"],
    "target_qps": hon_cl_eff["target_qps"],
    "throughput_ratio": hon_cl_eff["throughput_ratio"],
  },
  "validator": {
    "max_pending_transactions": i("VAL_MAX_PENDING"),
    "max_pending_local_submissions": i("VAL_SEM_CAP"),
    "graduated_load_shedding_soft_limit_pct": i("VAL_START_PCT"),
    "graduated_load_shedding_saturation_pct": i("VAL_SAT_PCT"),
    "semaphore_shedding_enabled": b("VAL_SEM_SHEDDING"),
  },
  "results": {
    "peak_inflight": i("R_PEAK"),
    "ratio_peak_over_sem": f("R_RATIO"),
    "useful_tps": f("R_USEFUL_TPS"),
    "admit_lat_p50": f("R_ADMIT_LAT_P50"),
    "admit_lat_p99": f("R_ADMIT_LAT_P99"),
    "permit_hold_p50": f("R_PERMIT_HOLD_P50"),
    "permit_hold_p90": f("R_PERMIT_HOLD_P90"),
    "permit_hold_p95": f("R_PERMIT_HOLD_P95"),
    "permit_hold_p99": f("R_PERMIT_HOLD_P99"),
    "permit_hold_p999": f("R_PERMIT_HOLD_P999"),
    "permit_wait_p50": f("R_PERMIT_WAIT_P50"),
    "permit_wait_p90": f("R_PERMIT_WAIT_P90"),
    "permit_wait_p95": f("R_PERMIT_WAIT_P95"),
    "permit_wait_p99": f("R_PERMIT_WAIT_P99"),
    "permit_wait_p999": f("R_PERMIT_WAIT_P999"),
    "pre_acquire_p50": f("R_PRE_ACQUIRE_P50"),
    "pre_acquire_p99": f("R_PRE_ACQUIRE_P99"),
    "shed_pct_avg": f("R_SHED_PCT_AVG"),
    "shed_pct_max": f("R_SHED_PCT_MAX"),
    "inflight_stddev": f("R_INFLIGHT_STDDEV"),
    "inflight_mean": f("R_INFLIGHT_MEAN"),
    "saturation_75pct": f("R_SATURATION_75PCT"),
    "consensus_lat_p50": f("R_CONSENSUS_LAT_P50"),
    "consensus_lat_p90": f("R_CONSENSUS_LAT_P90"),
    "consensus_lat_p95": f("R_CONSENSUS_LAT_P95"),
    "consensus_lat_p99": f("R_CONSENSUS_LAT_P99"),
    "consensus_lat_p999": f("R_CONSENSUS_LAT_P999"),
    # Per-pool success / first-pass also lifted into spammer/honest
    # objects above; kept here for backwards-compat with older plot.py
    # that looks at results.spammer_success / results.spammer_first_pass_pct.
    "spammer_success": i("R_SPAMMER_SUCCESS"),
    "spammer_first_pass_pct": f("R_SPAMMER_FP"),
    "honest_success": i("HON_SUCCESS"),
    "honest_first_pass_pct": f("HON_FP"),
    "reject_grad_preventive": i("R_REJ_PREV"),
    "reject_grad_saturated": i("R_REJ_SAT"),
    "reject_grad_reactive": i("R_REJ_REACT"),
    "reject_max_pending": i("R_REJ_MAX"),
    "reject_semaphore": i("R_REJ_SEM"),
    "exit_codes": ints("R_EXIT_CODES"),
    "exit_codes_ok": i("R_OK")==1,
  },
  "timeseries": {
    # [timestamp_epoch, value] pairs. inflight + shed_pct at 100ms step;
    # rejection rates / per-stage latency p99 / TPS at 1s step. None if
    # the scrape failed or was disabled.
    "inflight": ts("TS_INFLIGHT"),
    "shed_pct": ts("TS_SHED_PCT"),
    "reject_reactive_rate": ts("TS_REJ_REACTIVE"),
    "reject_saturated_rate": ts("TS_REJ_SATURATED"),
    "reject_preventive_rate": ts("TS_REJ_PREVENTIVE"),
    # Per-stage p99 latency time-series — pre_acquire=stage A, permit_wait
    # =stage B, permit_hold=stage C, consensus_lat=e2e. Together these
    # show WHICH stage misbehaves WHEN during the spam window.
    "pre_acquire_p99": ts("TS_PRE_ACQUIRE_P99"),
    "permit_wait_p99": ts("TS_PERMIT_WAIT_P99"),
    "permit_hold_p99": ts("TS_PERMIT_HOLD_P99"),
    "consensus_lat_p99": ts("TS_CONSENSUS_LAT_P99"),
    "tps": ts("TS_TPS"),
  },
}
# Derived convenience field: ratio_peak_over_max_pending — the
# experimentally meaningful safety metric for this study. Computed here
# so analysis code does not have to re-derive it.
peak = rec["results"]["peak_inflight"]
maxp = rec["validator"]["max_pending_transactions"]
rec["results"]["ratio_peak_over_max_pending"] = (
    round(peak / maxp, 4) if (peak is not None and maxp) else None
)
print(json.dumps(rec))
' >> "$OUT_JSONL"

  if [ "$failed" -eq 1 ]; then
    echo ">>> RESULT: iter=$i pct=$val_start_pct FAILED"
  else
    echo ">>> RESULT: iter=$i pct=$val_start_pct sat=$val_sat_pct max=$val_max_pending sem=$val_sem_cap sem_shed=$val_sem_shedding"
    echo "    spammer:   offered=$SPAMMER_OFFERED success=$spammer_success first_pass=${spammer_fp}%"
    if [ "$HONEST_PROC_COUNT" -gt 0 ]; then
      echo "    honest:    offered=$HONEST_OFFERED success=$honest_success first_pass=${honest_fp}% (open_loop=$HONEST_OPEN_LOOP)"
    fi
    if [ "$HONEST_CL_PROC_COUNT" -gt 0 ]; then
      echo "    honest_cl: offered=$HONEST_CL_OFFERED success=$honest_cl_success first_pass=${honest_cl_fp}% (closed-loop)"
    fi
    echo "    peak=$peak  ratio=${ratio}×  tps=$useful_tps  rej[prev=$r_prev,sat=$r_grad_sat,react=$r_grad_react,max=$r_max,sem=$r_sem]  hold[p50=$permit_hold_p50,p99=$permit_hold_p99]  wait[p50=$permit_wait_p50,p99=$permit_wait_p99]  pre_acq[p50=$pre_acquire_p50,p99=$pre_acquire_p99]  shed[avg=$shed_pct_avg,max=$shed_pct_max]"
  fi

  # FAST_MODE failure tracking: any non-zero stress.rs exit, or zero
  # peak_inflight (no txs reached validator at all) → state likely broken
  # (gas pool exhausted, validators wedged, etc).
  #
  # In FAST_MODE, ANY iter failure exits sweep.sh with non-zero so the
  # caller (run.sh) can do a full reset before the next sweep.sh
  # invocation. Without this, ITERS=1 callers never see failures (since
  # MAX_FAIL_STREAK would never trigger within a single-iter sweep).
  iter_bad=0
  if [ "$failed" -eq 1 ] || [ "$ok" -ne 1 ] || [ "${peak:-0}" -le 0 ]; then
    iter_bad=1
  fi
  if [ "$iter_bad" -eq 1 ]; then
    fail_streak=$((fail_streak + 1))
    echo "    [fail_streak=$fail_streak/$MAX_FAIL_STREAK]"
    if [ "$FAST_MODE" = "true" ]; then
      # Try to capture validator state for diagnosis before bailing.
      echo "=== FAST_MODE: iter failed — capturing validator-1 logs (last 30 lines) ==="
      (cd "$PRIVNET" && docker compose logs --tail 30 validator-1 2>&1 | tail -35) || true
      if [ "$fail_streak" -ge "$MAX_FAIL_STREAK" ]; then
        echo "=== FAST_MODE: $MAX_FAIL_STREAK consecutive failures — aborting sweep.sh ==="
      else
        echo "=== FAST_MODE: exiting non-zero so caller can reset before next iter ==="
      fi
      ls -dt "$REPO"/runs/multi-* 2>/dev/null | tail -n +3 | xargs -r rm -rf
      exit 1
    fi
  else
    fail_streak=0
  fi

  # Per-iter cleanup: keep last 2 multi-* dirs, drop older ones.
  ls -dt "$REPO"/runs/multi-* 2>/dev/null | tail -n +3 | xargs -r rm -rf
done

echo
echo "================ DONE $(date -u) ================"
echo "Results: $OUT_JSONL"

# Headline summary grouped by (max_pending, sem_cap, start_pct). Reads
# ALL records in the JSONL (across sweeps and across hosts), so the
# table accumulates as you re-run more arms.
#
# Headline metrics:
#   peak_inflight             (raw queue depth — useful but absolute)
#   ratio_peak_over_max_pending (safety — lower is better)
#   useful_tps                (throughput — higher is better)
echo
echo "=== Per-policy summary from $OUT_JSONL ==="
P="$OUT_JSONL" python3 -c '
import json, os, sys
from collections import defaultdict
from statistics import median, mean

path = os.environ["P"]
groups = defaultdict(list)
with open(path) as f:
  for line in f:
    line = line.strip()
    if not line: continue
    try: r = json.loads(line)
    except: continue
    if r["results"].get("failed", False) or r.get("failed", False):
      continue
    peak = r["results"].get("peak_inflight")
    if peak is None or peak <= 0: continue
    v = r["validator"]
    key = (
      r["host"]["hostname"],
      v.get("max_pending_transactions", "?"),
      v.get("max_pending_local_submissions", "?"),
      v.get("graduated_load_shedding_soft_limit_pct", "?"),
      v.get("graduated_load_shedding_saturation_pct", "?"),
    )
    groups[key].append({
      "peak": peak,
      "ratio_cap": r["results"].get("ratio_peak_over_max_pending"),
      "tps": r["results"].get("useful_tps") or 0,
    })

print("  %-14s %9s %5s %4s %4s %3s %9s %9s %10s %10s %8s %8s" % ("host", "max_pend", "sem", "pct", "sat", "n", "peak_med", "peak_max", "ratio_med", "ratio_max", "tps_med", "tps_max"))
print("  " + "-" * 115)
for key in sorted(groups):
  rows = groups[key]
  peaks = [r["peak"] for r in rows]
  ratios = [r["ratio_cap"] for r in rows if r["ratio_cap"] is not None]
  tps = [r["tps"] for r in rows]
  print(f"  {key[0]:<14} {str(key[1]):>9} {str(key[2]):>5} {str(key[3]):>4} {str(key[4]):>4} {len(rows):>3} {median(peaks):>9.0f} {max(peaks):>9} {median(ratios):>10.3f} {max(ratios):>10.3f} {median(tps):>8.1f} {max(tps):>8.1f}")
'

# -------- Teardown ---------
# Skipped in interleaved-runner mode — the wrapper (e.g. run.sh) handles
# a single final teardown after all rounds complete.
if [ "$SKIP_NETWORK_LIFECYCLE" != "true" ]; then
  echo
  echo "=== tearing down stacks ==="
  echo "  stopping grafana + prometheus..."
  (cd "$REPO/dev-tools/grafana-local" && docker compose down 2>&1 | tail -3) || true
  echo "  stopping iota private network..."
  (cd "$PRIVNET" && docker compose down -v 2>&1 | tail -3) || true
  echo "  done."
else
  echo
  echo "=== SKIP_NETWORK_LIFECYCLE=true — leaving network up for next invocation ==="
fi
