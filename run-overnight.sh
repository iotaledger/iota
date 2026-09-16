#!/usr/bin/env bash
# Runs regime sweep(s) overnight. Each regime targets ONE binary cap to
# engage cleanly while leaving the next-higher cap idle. INTERVAL is sized
# per regime so per-burst peak ≈ nominal (no accumulation across bursts).
#
# Varies:   BURST, INTERVAL, and WORKERS per regime. WORKERS=13 across
#           B-E (gives ~150-200 margin to next cap); WORKERS=14 for F
#           (intentional over-saturation).
# Drain:    ~1200 tx/s. No-accumulation rule: nominal ≤ drain × INTERVAL.
set -euo pipefail
cd "$(dirname "$0")"

sudo -v
(while true; do
  sudo -n true
  sleep 60
done) &
trap "kill $! 2>/dev/null || true" EXIT

wait_for_run() { while pgrep -f run_inner.sh >/dev/null 2>&1; do sleep 30; done; }

# Regime A — 50/50 (cap=1000) engages, 60/60 (cap=1200) idle.
#   nominal = 14×3×25 = 1050  |  INTERVAL=1s
# BURST=3 INTERVAL=1s    QPS_TOTAL=2000 WORKERS=14 DURATION=15s ITERS=2 ./run.sh
# sleep 10; wait_for_run

# Regime B — 60/60 (cap=1200) engages, 75/75 (cap=1500) idle.
#   nominal = 13×4×25 = 1300  |  INTERVAL=1.2s  |  200 margin to 1500 cap
# BURST=4 INTERVAL=1200ms QPS_TOTAL=2000 WORKERS=13 DURATION=15s ITERS=20 ./run.sh
# sleep 10
# wait_for_run

# Regime C — 75/75 (cap=1500) engages, 90/90 (cap=1800) idle.
#   nominal = 13×5×25 = 1625  |  INTERVAL=1.5s  |  175 margin to 1800 cap
# BURST=5 INTERVAL=1500ms QPS_TOTAL=2000 WORKERS=13 DURATION=15s ITERS=20 ./run.sh
# sleep 10
# wait_for_run

# Regime D — 90/90 (cap=1800) engages, 100/100 (cap=2000) idle.
#   nominal = 13×6×25 = 1950  |  INTERVAL=1.8s  |  50 margin to 2000 (tight — may
#   show some stray 100/100 engagement due to per-burst variance).
# BURST=6 INTERVAL=1800ms QPS_TOTAL=2000 WORKERS=13 DURATION=15s ITERS=20 ./run.sh
# sleep 10
# wait_for_run

# Regime E — 100/100 (cap=2000) engages — peak overshoots cap each burst,
# queue saturates briefly at max=2000 then drains between bursts (sawtooth).
# All caps engage; 100/100 sees ~275 r_max per burst × ~7 bursts/iter.
#   nominal = 13×7×25 = 2275  |  INTERVAL=2s  |  drain×INT = 2400 (full drain)
# BURST=7 INTERVAL=2000ms QPS_TOTAL=2000 WORKERS=13 DURATION=15s ITERS=20 ./run.sh
# sleep 10
# wait_for_run

# Regime F — sustained heavy spam. Queue pinned at max_pending=2000
# throughout the iter. All caps engage hard; 100/100 sees sustained
# reactive rejection (r_max). Tests "saturated cap" regime.
#   nominal = 14×9×25 = 3150  |  INTERVAL=1s  |  +1950/s accumulation →
#   queue saturates at max=2000 within first burst, stays there.
# BURST=9 INTERVAL=1s QPS_TOTAL=2000 WORKERS=14 DURATION=15s ITERS=20 ./run.sh
# sleep 10
# wait_for_run

# ============================================================================
# max=20000 regime sweep (Mode B — continuous arrival, no barrier bursts).
# Each regime: QPS sized so queue ramps linearly to engage ONE additional
# binary cap by end of iter while staying clear of the next higher cap.
# Drain ≈ 1180 tx/s, spammer delivery efficiency ≈ 87%.
# QPS formula: QPS ≈ (target_peak/15 + 1180) / 0.87
# ============================================================================

# Regime A — 50/50 (cap=10K) engages, 60/60 (cap=12K) idle.
#   target peak ~11K  |  QPS=2200  |  smoke-verified 60/60 idle, 50/50 r_sat~2100
QPS_TOTAL=2200 DURATION=15s WORKERS=14 ITERS=5 ./run.sh
sleep 10
wait_for_run

# Regime B — 60/60 (cap=12K) engages, 75/75 (cap=15K) idle.
#   target peak ~13.5K  |  QPS=2390  |  smoke-verified 75/75 idle, 60/60 r_sat~2750
QPS_TOTAL=2390 DURATION=15s WORKERS=14 ITERS=5 ./run.sh
sleep 10
wait_for_run

# Regime C — 75/75 (cap=15K) engages, 90/90 (cap=18K) idle.
#   target peak ~16.5K  |  QPS=2620  |  smoke-verified 90/90 idle, 75/75 r_sat~2787
QPS_TOTAL=2620 DURATION=15s WORKERS=14 ITERS=5 ./run.sh
sleep 10
wait_for_run

# Regime D — 90/90 (cap=18K) engages, 100/100 (cap=20K) idle.
#   target peak ~18.5K  |  QPS=2780  |  smoke-verified 100/100 idle (peak grazes 20K but
#   r_max=0), 90/90 r_sat~3470
QPS_TOTAL=2780 DURATION=15s WORKERS=14 ITERS=5 ./run.sh
sleep 10
wait_for_run

# Regime E — 100/100 (cap=20K) saturates briefly at end of iter — all caps
# engage. Queue pinned at max=20K from t~14s onward.
#   target peak >20K  |  QPS=3000  |  smoke-verified 100/100 r_react~4250
QPS_TOTAL=3000 DURATION=15s WORKERS=14 ITERS=5 ./run.sh
sleep 10
wait_for_run

# Regime F — sustained heavy spam. Queue pinned at max=20K throughout from
# t~7s onward. All caps saturated; 100/100 sees sustained reactive
# rejection at ~2200/s.
#   QPS=5000  |  smoke-verified 100/100 r_react~33500 (8× Regime E)
QPS_TOTAL=5000 DURATION=15s WORKERS=14 ITERS=5 ./run.sh
sleep 10
wait_for_run
