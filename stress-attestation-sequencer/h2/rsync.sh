#!/usr/bin/env bash
# Pull fresh H2 results from the EPYC box: the probe calibration CSV and the
# matrix run data. Skips the bulky raw logs (node logs, compressed or not, and
# the stress client's stderr) but keeps the per-iteration _crash.log/_state.log
# scan artifacts, the client benchmark reports and all timeseries JSONs.
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")"

rsync -avz --bwlimit=6M --partial --info=progress2 \
  --exclude='validator-*.log*' --exclude='fullnode-*.log*' \
  --exclude='run-*-stress.log' \
  iota-private-network:"$IOTA"/stress-attestation-sequencer/h2/results/ \
  results/
