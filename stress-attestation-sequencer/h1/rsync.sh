#!/usr/bin/env bash
# Pull fresh H1 results from the EPYC box. Skips the bulky raw logs
# (compressed node logs and stress-client output) but keeps the per-iteration
# _crash.log/_state.log scan artifacts and all timeseries JSONs.
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")"

rsync -avz --bwlimit=6M --partial --info=progress2 \
  --exclude='validator-*.log*' --exclude='fullnode-*.log*' \
  --exclude='run-*-stress.log' \
  iota-private-network:"$IOTA"/stress-attestation-sequencer/h1/results/ \
  results/
