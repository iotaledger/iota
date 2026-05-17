#!/usr/bin/env bash
# 15-run burst sweep: 5 iterations × 3 burst sizes, fresh network per run.
# Records ratio + peak inflight per run to a CSV.
#
# Usage: ./burst-sweep.sh
# Watch progress: tail -f burst-sweep.log
# CSV output:     burst-sweep.csv
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
ITERS=20
PRIVNET=/home/roman/IOTA/iotaledger/iota/dev-tools/iota-private-network
REPO=/home/roman/IOTA/iotaledger/iota

# CSV header (only if new file)
[ -f "$OUT_CSV" ] || echo "iso_time,burst,bar_ms,iter,peak_inflight,ratio,exit_codes_ok" > "$OUT_CSV"

exec >> "$OUT_LOG" 2>&1

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
    rm -f ~/.stress-gas-pool/owner-*.json
    # 20s gives Mysticeti time to form quorum, validators to bind gRPC,
    # and the first /etc/hosts lookups to resolve. 5s was too short and
    # caused initial pay_iota tx to time out → fail-fast.
    sleep 20

    # Run
    cd "$REPO"
    # IFR=20: per-worker pool = (40000/24) × 20 / 16 = 2082 payloads.
    # BURST ≤ 2080 fits without truncation; BURST=2050 leaves only ~32
    # payloads of margin per worker (intentional — we want the burst to
    # nearly drain the pool to maximize racing arrivals at the gate).
    NUM_VALIDATORS_TO_TARGET=1 NUM_PROCS=24 QPS_TOTAL=40000 DURATION=15s \
      WORKERS=16 IN_FLIGHT_RATIO=20 BURST_SIZE=$burst BARRIER_PERIOD_MS=$bar \
      GAS_CHUNK_SIZE=500 ./stress-multi.sh 2>&1 | tail -50 \
      | tee /tmp/burst-sweep-iter.log

    # Extract result from latest summary.txt
    latest=$(ls -td "$REPO"/runs/multi-*/ | head -1)
    if [ -f "$latest/summary.txt" ]; then
      peak=$(grep '^peak inflight:' "$latest/summary.txt" | awk -F: '{print $2}' | xargs)
      ratio=$(grep '^ratio:' "$latest/summary.txt" | awk -F: '{print $2}' | xargs | sed 's/×//')
      exits=$(grep '^exit codes:' "$latest/summary.txt" | awk -F: '{print $2}' | xargs)
      # Treat empty/zero as FAIL too
      [ -z "$peak" ] && peak=0
      [ -z "$ratio" ] && ratio=0
      ok=$(echo "$exits" | awk '{for(i=1;i<=NF;i++) if($i!="0"){print 0; exit} print 1}')
      iso=$(basename "$latest" | sed 's/multi-//')
      echo "$iso,$burst,$bar,$i,$peak,$ratio,$ok" >> "$OUT_CSV"
      echo ">>> RESULT: burst=$burst bar=$bar iter=$i peak=$peak ratio=${ratio}× ok=$ok"
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
      echo "$iso,$burst,$bar,$i,FAIL,FAIL,0" >> "$OUT_CSV"
      echo ">>> RESULT: burst=$burst bar=$bar iter=$i FAILED"
    fi
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
