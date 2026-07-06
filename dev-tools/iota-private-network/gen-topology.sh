#!/usr/bin/env bash
#
# gen-topology.sh — regenerate the per-validator sections of the local network
# for a given validator count N:
#   - the validator service blocks in this dir's docker-compose.yaml
#   - the Validator_* scrape jobs in ../grafana-local/prometheus.yaml
#
# Only the text between the BEGIN/END markers in each file is rewritten; the
# hand-maintained rest (fullnodes, indexers, postgres, other scrape jobs) is
# left untouched. Idempotent — re-running with a different N just rewrites the
# marked regions. bootstrap.sh calls this automatically with its -n value, so N
# stays in lockstep with the generated genesis template; run it standalone to
# retarget without a full bootstrap.
#
# Usage: ./gen-topology.sh -n NUM_VALIDATORS
#
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
COMPOSE_FILE="${COMPOSE_FILE:-$SCRIPT_DIR/docker-compose.yaml}"
PROM_FILE="${PROM_FILE:-$SCRIPT_DIR/../grafana-local/prometheus.yaml}"

# Validators are statically addressed 10.0.0.(IP_BASE+i); the fixed infra
# (fullnodes, indexers, faucet, postgres) lives at 10.0.0.201+, so validators
# never collide with it up to N = MAX_VALIDATOR_IP - IP_BASE. Keep IP_BASE and
# the infra IPs in docker-compose.yaml in sync.
IP_BASE=10
MAX_VALIDATOR_IP=200

NUM_VALIDATORS=4
while getopts "n:" opt; do
  case "$opt" in
  n) NUM_VALIDATORS="$OPTARG" ;;
  *)
    echo "Usage: $0 -n NUM_VALIDATORS" >&2
    exit 1
    ;;
  esac
done

if ! [[ "$NUM_VALIDATORS" =~ ^[0-9]+$ ]] || ((NUM_VALIDATORS < 1)); then
  echo "ERROR: -n must be a positive integer (got: '$NUM_VALIDATORS')" >&2
  exit 1
fi
if ((IP_BASE + NUM_VALIDATORS > MAX_VALIDATOR_IP)); then
  echo "ERROR: N=$NUM_VALIDATORS exceeds the addressable validator range (max $((MAX_VALIDATOR_IP - IP_BASE)) with the current IP layout)." >&2
  exit 1
fi

# Replace everything between "$begin" and "$end" (both kept) in "$file" with the
# contents of the file "$repl". Markers are matched as substrings so leading
# indentation in the file doesn't matter.
splice() {
  local file="$1" begin="$2" end="$3" repl="$4"
  if ! grep -qF -- "$begin" "$file" || ! grep -qF -- "$end" "$file"; then
    echo "ERROR: markers not found in $file" >&2
    echo "       expected '$begin' and '$end'" >&2
    exit 1
  fi
  local tmp
  tmp="$(mktemp)"
  awk -v begin="$begin" -v end="$end" -v repl="$repl" '
    index($0, begin) { print; while ((getline line < repl) > 0) print line; close(repl); skip=1; next }
    index($0, end)   { skip=0 }
    !skip            { print }
  ' "$file" >"$tmp"
  mv "$tmp" "$file"
}

compose_block="$(mktemp)"
prom_block="$(mktemp)"
trap 'rm -f "$compose_block" "$prom_block"' EXIT

for ((i = 1; i <= NUM_VALIDATORS; i++)); do
  cat >>"$compose_block" <<EOF
  validator-$i:
    <<: *common-validator
    container_name: validator-$i
    hostname: validator-$i
    networks:
      iota-network:
        ipv4_address: 10.0.0.$((IP_BASE + i))
    volumes:
      - ./configs/validators/validator-$i-8080.yaml:/opt/iota/config/validator.yaml:ro
      - ./configs/genesis/genesis.blob:/opt/iota/config/genesis.blob:ro
      - ./data/validator-$i:/opt/iota/db:rw

EOF

  cat >>"$prom_block" <<EOF
  - job_name: "Validator_$i"
    static_configs:
      - targets: ["validator-$i:9184"]
        labels:
          host: validator-$i
          network: local
EOF
done

splice "$COMPOSE_FILE" "# BEGIN generated validators" "# END generated validators" "$compose_block"
splice "$PROM_FILE" "# BEGIN generated validator scrape jobs" "# END generated validator scrape jobs" "$prom_block"

echo "gen-topology: wrote $NUM_VALIDATORS validator(s)"
echo "  - $COMPOSE_FILE"
echo "  - $PROM_FILE"
