#!/usr/bin/env bash

set -euo pipefail

# Make sure relative paths to cap-policy-sweep.sh resolve regardless
# of where the user invokes this script from.
cd "$(dirname "$0")"

nohup bash -c '
  START_PCT=100 MAX_PENDING=1000 SEMAPHORE_CAP=20 SEM_SHEDDING=false ./cap-policy-sweep.sh
  START_PCT=75  MAX_PENDING=1000 SEMAPHORE_CAP=20 SEM_SHEDDING=false ./cap-policy-sweep.sh
  START_PCT=50  MAX_PENDING=1000 SEMAPHORE_CAP=20 SEM_SHEDDING=false ./cap-policy-sweep.sh
  START_PCT=25  MAX_PENDING=1000 SEMAPHORE_CAP=20 SEM_SHEDDING=false ./cap-policy-sweep.sh
  START_PCT=100 MAX_PENDING=1000 SEMAPHORE_CAP=20 SEM_SHEDDING=true  ./cap-policy-sweep.sh
  START_PCT=50  MAX_PENDING=1000 SEMAPHORE_CAP=20 SEM_SHEDDING=true  ./cap-policy-sweep.sh
  START_PCT=100 MAX_PENDING=500  SEMAPHORE_CAP=20 SEM_SHEDDING=false ./cap-policy-sweep.sh
  START_PCT=100 MAX_PENDING=900  SEMAPHORE_CAP=20 SEM_SHEDDING=false ./cap-policy-sweep.sh
' >/dev/null 2>&1 &

disown
echo "PID: $!"

# After launching
#
# Check it's alive:
# pgrep -af cap-policy-sweep
#
# Watch progress:
# tail -f cap-policy-sweep.log | grep --line-buffered -E '^\[cap-policy|^>>> RESULT|^===.*===$'
#
# Or count completed iters across all configs (~50 per config, expect 350 total):
# wc -l cap-policy-sweep.jsonl
#
# To kill it later
#
# # Find the orchestrator + current sweep
# pgrep -af "bash -c|cap-policy-sweep|stress-multi"
#
# # Clean kill
# pkill -INT -f cap-policy-sweep.sh
# sleep 10
# pkill -TERM -f cap-policy-sweep.sh   # if still alive
#
# Then run the docker teardown lines from the previous answer to clean up containers.
