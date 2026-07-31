#!/usr/bin/env bash
# Pull the H2 probe calibration CSV from the EPYC box (probe results only —
# everything else in h2/results/ is produced locally).
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")"

rsync -avz --partial --info=progress2 \
  iota-private-network:"$IOTA"/stress-attestation-sequencer/h2/results/calibration-epyc-9454p.csv \
  results/
