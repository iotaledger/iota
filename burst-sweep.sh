#!/usr/bin/env bash
# 15-run burst sweep: 5 iterations × 3 burst sizes, fresh network per run.
# Records ratio + peak inflight per run to a CSV.
#
# Usage: ./burst-sweep.sh
# Watch progress: tail -f burst-sweep.log
# CSV output:     burst-sweep.csv
#
# Note: if you see AddrNotAvailable / TCP port exhaustion errors in the
# subprocess logs (rare since the TransactionDriver switch — long-lived
# gRPC channels replace per-call short-lived sockets), run
# `sudo ./tune-sysctl.sh` once per boot session.
set -uo pipefail

OUT_CSV="burst-sweep.csv"
OUT_LOG="burst-sweep.log"

# CRITICAL: the validator-side white-flag override needs these env vars at
# `docker compose up` time, otherwise iota-node boots in QD/cert mode and
# rejects all submit_tx_v2 calls with "White flag flow is not enabled in
# this protocol version" — every iter fails with 0 inflight. These must
# match the client-side overrides exported by stress-load-shedding.sh.
export IOTA_PROTOCOL_CONFIG_OVERRIDE_ENABLE=1
export IOTA_PROTOCOL_CONFIG_FEATURE_FLAGS_OVERRIDE_ENABLE_WHITE_FLAG_FLOW=true

# Graduated-shedding A/B baseline arm: the canonical config that reliably
# triggers the race-up-to-max_pending pathology (~21% hit rate for ≥50×
# across all prior runs). Single-cell, large N to characterize the
# bimodal distribution under hard-binary load shedding (no graduated):
#   - 21% rolls overshoot to ratio ~50× (capped by max_pending=1000)
#   - 79% rolls stay in 5-10× steady-state regime
#   - ~0% rolls in the 11-49× middle band
#
# This same config will be re-run with graduated shedding enabled to
# show the bimodal collapses to ~1-2× — the cleanest A/B demonstration
# of graduated vs hard-binary defense.
#
# Other params (held constant for both arms):
#   NUM_PROCS=24, IFR=20, TARGET=1, QPS=40000, WORKERS=16,
#   DURATION=15s, GAS_CHUNK_SIZE=500
BURSTS=(1800)
BARS=(500)
ITERS="${ITERS:-30}"
PRIVNET=/home/roman/IOTA/iotaledger/iota/dev-tools/iota-private-network
REPO=/home/roman/IOTA/iotaledger/iota

START_PCT="${START_PCT:-}"
# OPEN_LOOP=true makes the spammer pool fire submissions at target_qps
# regardless of in-flight count (recycles payloads immediately after
# submission). Removes the per-worker closed-loop ceiling that pins
# actual submission rate below nominal QPS under heavy validator
# contention. Use for cap-safety / goodput experiments where sustained
# validator-gate pressure matters more than per-tx correctness;
# duplicate submissions are likely, but the load-shedding gate runs
# before deduplication so they still count toward queue depth.
OPEN_LOOP="${OPEN_LOOP:-false}"
YAML_CFG="$PRIVNET/configs/validator-common.yaml"

# CSV header (only if new file). `start_pct` is graduated-load-shedding-soft-limit-pct
# from validator-common.yaml at run time — embedded per row so cross-pct CSV
# concatenation stays self-describing.
[ -f "$OUT_CSV" ] || echo "iso_time,burst,bar_ms,iter,start_pct,peak_inflight,ratio,exit_codes_ok,reject_grad_preventive,reject_grad_reactive,reject_max_pending,reject_semaphore,useful_tps,queue_p50,queue_p75,queue_p99,reject_rate_max,reject_rate_mean,admit_lat_p50,admit_lat_p99,permit_hold_p50,permit_hold_p99,permit_wait_p50,permit_wait_p99,pre_acquire_p50,pre_acquire_p99,inflight_stddev,inflight_mean,saturation_75pct,consensus_lat_p50,consensus_lat_p99" > "$OUT_CSV"

exec >> "$OUT_LOG" 2>&1

# Optional: override graduated-load-shedding-soft-limit-pct in
# validator-common.yaml for this sweep. If unset, whatever is already in the
# yaml is used. The per-iter teardown + bootstrap.sh -b regenerates each
# validator config from the (updated) overlay, so containers start with the
# new value automatically.
if [ -n "$START_PCT" ]; then
  if ! [[ "$START_PCT" =~ ^[0-9]+$ ]] || [ "$START_PCT" -gt 100 ]; then
    echo "Error: START_PCT must be an integer in [0, 100], got '$START_PCT'" >&2
    exit 1
  fi
  sed -i -E "s/^([[:space:]]*graduated-load-shedding-soft-limit-pct:[[:space:]]*).*/\1${START_PCT}/" "$YAML_CFG"
  # Verify the patch landed — if the field name was misspelled in the yaml
  # the sed silently changes nothing and serde would fall back to the default.
  ACTUAL_PCT=$(grep -E '^[[:space:]]*graduated-load-shedding-soft-limit-pct:' \
    "$YAML_CFG" | awk -F: '{print $2}' | xargs)
  if [ "$ACTUAL_PCT" != "$START_PCT" ]; then
    echo "Error: yaml patch did not stick (asked for $START_PCT, found '$ACTUAL_PCT' in $YAML_CFG)" >&2
    exit 1
  fi
  echo "=> Patched $YAML_CFG: graduated-load-shedding-soft-limit-pct = $START_PCT"
fi

echo "================ burst-sweep $(date -u) ================"

# -------- Pre-flight: bail fast on common foot-guns ---------
PREFLIGHT_OK=1
echo "=== pre-flight checks ==="

# 1. sudo cached? bootstrap.sh needs it. -n fails immediately if not cached.
if sudo -n true 2>/dev/null; then
  echo "  sudo cache  ✓"
else
  echo "  sudo cache  ✗ — run \`sudo -v\` first"
  PREFLIGHT_OK=0
fi

# 2. target/ writable by current user? cargo build fails if mixed-ownership.
# On the workstation we run as roman → check no root-owned artifacts leaked in
# from a prior `sudo ./burst-sweep.sh`. On the EPYC server we run as root →
# everything is uid 0 by design, and the workstation-style check would always
# trip. So: when running as root, skip the check entirely. When not root,
# look for files NOT owned by the current user (catches root-leaked artifacts
# AND any other foreign-uid contamination).
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

# 3. /etc/hosts has validator-N aliases (required for TD path).
if grep -q '^127\.0\.0\.11.*validator-1' /etc/hosts; then
  echo "  /etc/hosts  ✓ validator-1..4 aliased"
else
  echo "  /etc/hosts  ✗ missing validator-N → 127.0.0.{11..14} entries"
  echo "    fix: sudo tee -a /etc/hosts <<EOF"
  echo "127.0.0.11 validator-1"
  echo "127.0.0.12 validator-2"
  echo "127.0.0.13 validator-3"
  echo "127.0.0.14 validator-4"
  echo "EOF"
  PREFLIGHT_OK=0
fi

if [ "$PREFLIGHT_OK" -eq 0 ]; then
  echo
  echo "=== ABORTING: pre-flight checks failed — fix the issues above and re-run ==="
  exit 1
fi

# -------- Initial bring-up: network first, then grafana ---------
# The iter loop tears down + recreates the iota network each iteration, but
# we still want grafana up FROM THE START so Prometheus has been scraping
# the validators since iter 1. Running network first means Prometheus's
# scrape targets resolve immediately when grafana starts — no flapping.
echo
echo "=== bringing up network (validators + fullnode-1 + faucet) ==="
cd "$PRIVNET"
# bootstrap.sh -b builds the genesis if missing. Idempotent — skips if
# both genesis-template-4.yaml and genesis.blob exist.
if [ ! -f "$PRIVNET/configs/genesis/genesis.blob" ]; then
  echo "  genesis.blob missing — running bootstrap.sh -b -n 4"
  sudo ./bootstrap.sh -b -n 4 2>&1 | tail -3
fi
./run.sh -n 4 faucet 2>&1 | tail -2
# Give validators a moment to bind their gRPC ports before Prometheus scrapes
sleep 5

echo
echo "=== bringing up Prometheus + Grafana ==="
cd "$REPO/dev-tools/grafana-local"
docker compose up -d 2>&1 | tail -3

# Prometheus takes ~5-15s to fully bind its HTTP endpoint after the
# container starts. Retry up to 30s before giving up.
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
if [ "$PROM_READY" -eq 1 ]; then
  echo "  prometheus  ✓ http://localhost:9090"
else
  echo
  echo "  prometheus  ✗ http://localhost:9090 still unreachable after 30s"
  echo "    check: docker compose -f $REPO/dev-tools/grafana-local/docker-compose.yaml logs"
  exit 1
fi
cd "$REPO"

echo
echo "=== all systems up — starting sweep ==="
echo

for burst in "${BURSTS[@]}"; do
 for bar in "${BARS[@]}"; do
  for i in $(seq 1 $ITERS); do
    echo
    echo "=================================================="
    echo "[burst=$burst bar=$bar iter=$i/$ITERS] $(date -u +%H:%M:%S)"
    echo "=================================================="

    # Reset network
    cd "$PRIVNET"
    docker compose down -v 2>&1 | tail -1 || true
    sudo ./bootstrap.sh -b -n 4 2>&1 | tail -3
    ./run.sh -n 4 faucet 2>&1 | tail -1
    rm -f "$REPO"/runs/.stress-gas-pool/owner-*.json
    # 20s gives Mysticeti time to form quorum, validators to bind gRPC,
    # and the first /etc/hosts lookups to resolve. 5s was too short and
    # caused initial pay_iota tx to time out → fail-fast.
    sleep 20

    # Run
    cd "$REPO"
    # IFR=20: per-worker pool = (QPS_TOTAL/NUM_PROCS) × 20 / 16 payloads.
    # At the defaults (NUM_PROCS=24, QPS_TOTAL=40000) the pool is 2082
    # payloads per worker; BURST=1800 leaves headroom; BURST=2050 nearly
    # drains the pool to maximize racing arrivals at the gate.
    #
    # NUM_PROCS / QPS_TOTAL overridable so we can scale concurrent
    # validator-gate pressure. Keep per-proc QPS ≈ 1666 (validated init-coin
    # sizing): QPS_TOTAL = NUM_PROCS × 1666, e.g.
    #   NUM_PROCS=32 QPS_TOTAL=52000
    #   NUM_PROCS=48 QPS_TOTAL=78000
    #   NUM_PROCS=72 QPS_TOTAL=118000
    NUM_VALIDATORS_TO_TARGET=1 \
      NUM_PROCS="${NUM_PROCS:-24}" QPS_TOTAL="${QPS_TOTAL:-40000}" \
      DURATION=15s \
      WORKERS=16 IN_FLIGHT_RATIO=20 BURST_SIZE=$burst BARRIER_PERIOD_MS=$bar \
      OPEN_LOOP="$OPEN_LOOP" \
      GAS_CHUNK_SIZE=500 ./stress-multi.sh 2>&1 | tail -50 \
      | tee "$REPO/runs/burst-sweep-iter.log"

    # Extract result from latest summary.txt
    latest=$(ls -td "$REPO"/runs/multi-*/ | head -1)
    if [ -f "$latest/summary.txt" ]; then
      peak=$(grep '^peak inflight:' "$latest/summary.txt" | awk -F: '{print $2}' | xargs)
      ratio=$(grep '^ratio:' "$latest/summary.txt" | awk -F: '{print $2}' | xargs | sed 's/×//')
      exits=$(grep '^exit codes:' "$latest/summary.txt" | awk -F: '{print $2}' | xargs)
      # Per-source rejection counts (added after the authority.rs 4-label split)
      r_prev=$(grep '^reject_grad_preventive:' "$latest/summary.txt" | awk -F: '{print $2}' | xargs)
      r_grad_react=$(grep '^reject_grad_reactive:' "$latest/summary.txt" | awk -F: '{print $2}' | xargs)
      r_max=$(grep '^reject_max_pending:' "$latest/summary.txt" | awk -F: '{print $2}' | xargs)
      r_sem=$(grep '^reject_semaphore:' "$latest/summary.txt" | awk -F: '{print $2}' | xargs)
      # Throughput, queue distribution, latency, rejection-rate scalars
      useful_tps=$(grep '^useful_tps:' "$latest/summary.txt" | awk -F: '{print $2}' | xargs)
      q_p50=$(grep '^queue_depth_p50:' "$latest/summary.txt" | awk -F: '{print $2}' | xargs)
      q_p75=$(grep '^queue_depth_p75:' "$latest/summary.txt" | awk -F: '{print $2}' | xargs)
      q_p99=$(grep '^queue_depth_p99:' "$latest/summary.txt" | awk -F: '{print $2}' | xargs)
      rej_rate_max=$(grep '^reject_rate_max:' "$latest/summary.txt" | awk -F: '{print $2}' | xargs)
      rej_rate_mean=$(grep '^reject_rate_mean:' "$latest/summary.txt" | awk -F: '{print $2}' | xargs)
      admit_p50=$(grep '^admit_lat_p50:' "$latest/summary.txt" | awk -F: '{print $2}' | xargs)
      admit_p99=$(grep '^admit_lat_p99:' "$latest/summary.txt" | awk -F: '{print $2}' | xargs)
      permit_hold_p50=$(grep '^permit_hold_p50:' "$latest/summary.txt" | awk -F: '{print $2}' | xargs)
      permit_hold_p99=$(grep '^permit_hold_p99:' "$latest/summary.txt" | awk -F: '{print $2}' | xargs)
      permit_wait_p50=$(grep '^permit_wait_p50:' "$latest/summary.txt" | awk -F: '{print $2}' | xargs)
      permit_wait_p99=$(grep '^permit_wait_p99:' "$latest/summary.txt" | awk -F: '{print $2}' | xargs)
      pre_acquire_p50=$(grep '^pre_acquire_p50:' "$latest/summary.txt" | awk -F: '{print $2}' | xargs)
      pre_acquire_p99=$(grep '^pre_acquire_p99:' "$latest/summary.txt" | awk -F: '{print $2}' | xargs)
      inflight_stddev=$(grep '^inflight_stddev:' "$latest/summary.txt" | awk -F: '{print $2}' | xargs)
      inflight_mean=$(grep '^inflight_mean:' "$latest/summary.txt" | awk -F: '{print $2}' | xargs)
      saturation_75pct=$(grep '^saturation_75pct:' "$latest/summary.txt" | awk -F: '{print $2}' | xargs)
      consensus_lat_p50=$(grep '^consensus_lat_p50:' "$latest/summary.txt" | awk -F: '{print $2}' | xargs)
      consensus_lat_p99=$(grep '^consensus_lat_p99:' "$latest/summary.txt" | awk -F: '{print $2}' | xargs)
      # Soft-limit-pct as currently deployed via validator-common.yaml. Read
      # at the same point we extract the result so each row records the
      # config it ran against, not what the global may have moved to.
      start_pct=$(grep -E 'graduated-load-shedding-soft-limit-pct:' \
        "$PRIVNET/configs/validator-common.yaml" 2>/dev/null \
        | awk -F: '{print $2}' | xargs)
      # Treat empty/zero as FAIL too
      [ -z "$peak" ] && peak=0
      [ -z "$ratio" ] && ratio=0
      [ -z "$r_prev" ] && r_prev=0
      [ -z "$r_grad_react" ] && r_grad_react=0
      [ -z "$r_max" ] && r_max=0
      [ -z "$r_sem" ] && r_sem=0
      [ -z "$useful_tps" ] && useful_tps=0
      [ -z "$q_p50" ] && q_p50=0
      [ -z "$q_p75" ] && q_p75=0
      [ -z "$q_p99" ] && q_p99=0
      [ -z "$rej_rate_max" ] && rej_rate_max=0
      [ -z "$rej_rate_mean" ] && rej_rate_mean=0
      [ -z "$admit_p50" ] && admit_p50=0
      [ -z "$admit_p99" ] && admit_p99=0
      [ -z "$permit_hold_p50" ] && permit_hold_p50=0
      [ -z "$permit_hold_p99" ] && permit_hold_p99=0
      [ -z "$permit_wait_p50" ] && permit_wait_p50=0
      [ -z "$permit_wait_p99" ] && permit_wait_p99=0
      [ -z "$pre_acquire_p50" ] && pre_acquire_p50=0
      [ -z "$pre_acquire_p99" ] && pre_acquire_p99=0
      [ -z "$inflight_stddev" ] && inflight_stddev=0
      [ -z "$inflight_mean" ] && inflight_mean=0
      [ -z "$saturation_75pct" ] && saturation_75pct=0
      [ -z "$consensus_lat_p50" ] && consensus_lat_p50=0
      [ -z "$consensus_lat_p99" ] && consensus_lat_p99=0
      [ -z "$start_pct" ] && start_pct="?"
      ok=$(echo "$exits" | awk '{for(i=1;i<=NF;i++) if($i!="0"){print 0; exit} print 1}')
      iso=$(basename "$latest" | sed 's/multi-//')
      echo "$iso,$burst,$bar,$i,$start_pct,$peak,$ratio,$ok,$r_prev,$r_grad_react,$r_max,$r_sem,$useful_tps,$q_p50,$q_p75,$q_p99,$rej_rate_max,$rej_rate_mean,$admit_p50,$admit_p99,$permit_hold_p50,$permit_hold_p99,$permit_wait_p50,$permit_wait_p99,$pre_acquire_p50,$pre_acquire_p99,$inflight_stddev,$inflight_mean,$saturation_75pct,$consensus_lat_p50,$consensus_lat_p99" >> "$OUT_CSV"
      echo ">>> RESULT: burst=$burst bar=$bar iter=$i pct=$start_pct peak=$peak ratio=${ratio}× ok=$ok  tps=$useful_tps  q[p50=$q_p50,p99=$q_p99]  rej[prev=$r_prev,grad_reactive=$r_grad_react,max=$r_max,sem=$r_sem]  rej_rate[max=$rej_rate_max,mean=$rej_rate_mean]  admit_p99=$admit_p99 hold[p50=$permit_hold_p50,p99=$permit_hold_p99]  wait[p50=$permit_wait_p50,p99=$permit_wait_p99]  pre_acq[p50=$pre_acquire_p50,p99=$pre_acquire_p99]  inflight[mean=$inflight_mean,std=$inflight_stddev] sat75=$saturation_75pct cons_p99=$consensus_lat_p99"
      # Early-exit safety: if first 2 iters give peak=0, something's broken
      # network-side (e.g. white-flag misconfig). Don't burn 75 min on it.
      if [ "$i" -le 2 ] && [ "${peak:-0}" -eq 0 ] 2>/dev/null; then
        early_fail=$((${early_fail:-0} + 1))
        if [ "$early_fail" -ge 2 ]; then
          echo ">>> ABORTING: 2 consecutive peak=0 results — fix the network setup before continuing"
          exit 2
        fi
      else
        early_fail=0
      fi
    else
      iso=$(basename "$latest" | sed 's/multi-//' 2>/dev/null || echo "?")
      start_pct=$(grep -E 'graduated-load-shedding-soft-limit-pct:' \
        "$PRIVNET/configs/validator-common.yaml" 2>/dev/null \
        | awk -F: '{print $2}' | xargs)
      [ -z "$start_pct" ] && start_pct="?"
      echo "$iso,$burst,$bar,$i,$start_pct,FAIL,FAIL,0,FAIL,FAIL,FAIL,FAIL,FAIL,FAIL,FAIL,FAIL,FAIL,FAIL,FAIL" >> "$OUT_CSV"
      echo ">>> RESULT: burst=$burst bar=$bar iter=$i pct=$start_pct FAILED"
    fi

    # Disk-leak prevention: runs/multi-* dirs accumulate 24 process logs each
    # (~80 MB per process → ~2 GB per iter). A full sweep at ITERS=30 is ~60 GB;
    # back-to-back sweep runs were filling the EPYC's 1.8T root
    # within hours. The CSV already holds the result we care about, so we
    # only need to keep the most recent N run dirs for forensic debugging.
    # Keep last 2 (current iter + previous, in case the current one is the
    # one we want to inspect post-mortem).
    ls -dt "$REPO"/runs/multi-* 2>/dev/null | tail -n +3 | xargs -r rm -rf
  done
 done
done

echo
echo "================ DONE $(date -u) ================"
echo "Results: $OUT_CSV"

# Per-(burst,bar) stats — grouped on column 2 (burst) AND column 3 (bar).
echo
echo "=== Per-(burst,bar) max + median ==="
awk -F, 'NR>1 && $6 != "FAIL" && $6+0 > 0 {
  k=$2","$3
  arr[k]=arr[k] " " $6
  n[k]++
  if($6+0 > max[k]) max[k]=$6+0
} END {
  for (k in n) {
    split(arr[k], a, " ")
    asort(a)
    med = a[int((n[k]+1)/2)]
    printf "  burst=%-5s bar=%-5s n=%d  median=%.2fx  max=%.2fx\n", \
      substr(k, 1, index(k,",")-1), substr(k, index(k,",")+1), n[k], med, max[k]
  }
}' "$OUT_CSV" | sort

# -------- Teardown: stop grafana + iota network so nothing hogs ports/CPU ---------
echo
echo "=== tearing down stacks ==="
echo "  stopping grafana + prometheus..."
(cd "$REPO/dev-tools/grafana-local" && docker compose down 2>&1 | tail -3) || true
echo "  stopping iota private network..."
(cd "$PRIVNET" && docker compose down -v 2>&1 | tail -3) || true
echo "  done."
