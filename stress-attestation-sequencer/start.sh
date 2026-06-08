#!/usr/bin/env bash
#
# start.sh — bring up the local iota-private-network configured for the
# validator-attestation / sequencer (TotalComputationCost) stress test.
#
# It sets the protocol-config overrides this test needs, then calls the
# network's own run.sh (forwarding any args). Every knob is an env var, so you
# can change the congestion mode / limits between runs WITHOUT rebuilding the
# Rust code or docker images:
#
#   ./start.sh                                   # 4 validators + faucet, attested, TotalComputationCost
#   ./start.sh -n 10 faucet                      # args forwarded straight to run.sh
#   MODE=TotalGasBudget ./start.sh               # baseline congestion mode
#   ATTEST=false ./start.sh                      # disable validator attestation
#   PCOOL=false ./start.sh                       # disable white-flag-flow
#   MAX_ACCUMULATED_TXN_COST=2999999 ./start.sh
#
# run.sh args: [-n NUM_VALIDATORS] [faucet | backup | indexer | indexer-cluster | all ...]
#
set -euo pipefail

# Resolve the repo root from this script's own location so it can be invoked
# from any directory. This script lives at <repo>/stress-attestation-sequencer/.
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

# iota-private-network tooling inside the monorepo (override with PRIVNET_DIR=...).
PRIVNET_DIR="${PRIVNET_DIR:-$REPO_ROOT/dev-tools/iota-private-network}"

# --- Protocol-config overrides (docker-compose forwards these to every node) --
# Attestation + white-flag-flow ON; congestion mode = TotalComputationCost.
export IOTA_PROTOCOL_CONFIG_OVERRIDE_ENABLE=1
export IOTA_PROTOCOL_CONFIG_FEATURE_FLAGS_OVERRIDE_ENABLE_WHITE_FLAG_FLOW="${PCOOL:-true}"
export IOTA_PROTOCOL_CONFIG_FEATURE_FLAGS_OVERRIDE_ENABLE_VALIDATOR_ATTESTATION="${ATTEST:-true}"
export IOTA_PROTOCOL_CONFIG_FEATURE_FLAGS_OVERRIDE_PER_OBJECT_CONGESTION_CONTROL_MODE="${MODE:-TotalComputationCost}"

# Optional numeric limits: only exported when provided, so the protocol
# version's own value stands otherwise (avoids pinning a baseline by accident).
if [[ -n "${MAX_ACCUMULATED_TXN_COST:-}" ]]; then
  export IOTA_PROTOCOL_CONFIG_OVERRIDE_MAX_ACCUMULATED_TXN_COST_PER_OBJECT_IN_MYSTICETI_COMMIT="$MAX_ACCUMULATED_TXN_COST"
fi
if [[ -n "${MAX_CONGESTION_OVERSHOOT:-}" ]]; then
  export IOTA_PROTOCOL_CONFIG_OVERRIDE_MAX_CONGESTION_LIMIT_OVERSHOOT_PER_COMMIT="$MAX_CONGESTION_OVERSHOOT"
fi
if [[ -n "${MAX_DEFERRAL_ROUNDS:-}" ]]; then
  export IOTA_PROTOCOL_CONFIG_OVERRIDE_MAX_DEFERRAL_ROUNDS_FOR_CONGESTION_CONTROL="$MAX_DEFERRAL_ROUNDS"
fi

# --- Sanity checks ------------------------------------------------------------
if [[ ! -d "$PRIVNET_DIR" ]]; then
  echo "ERROR: PRIVNET_DIR not found: $PRIVNET_DIR" >&2
  exit 1
fi
if [[ ! -f "$PRIVNET_DIR/configs/genesis/genesis.blob" ]]; then
  echo "ERROR: network not bootstrapped (missing configs/genesis/genesis.blob)." >&2
  echo "       Bootstrap first, e.g.: (cd '$PRIVNET_DIR' && ./bootstrap.sh -b)" >&2
  exit 1
fi

# --- Forward to run.sh --------------------------------------------------------
run_args=("$@")
if [[ ${#run_args[@]} -eq 0 ]]; then
  run_args=(faucet)
fi

# Colors (auto-disabled when stdout is not a terminal).
if [[ -t 1 ]]; then
  GREEN=$'\033[0;32m'
  RED=$'\033[0;31m'
  BLUE=$'\033[0;34m'
  YELLOW=$'\033[0;33m'
  CYAN=$'\033[0;36m'
  MAGENTA=$'\033[0;35m'
  RESET=$'\033[0m'
else
  GREEN=''
  RED=''
  BLUE=''
  YELLOW=''
  CYAN=''
  MAGENTA=''
  RESET=''
fi

echo "${BLUE}iota-private-network @ $PRIVNET_DIR${RESET}"
echo "${CYAN}  - enable_white_flag_flow             = $IOTA_PROTOCOL_CONFIG_FEATURE_FLAGS_OVERRIDE_ENABLE_WHITE_FLAG_FLOW${RESET}"
echo "${CYAN}  - enable_validator_attestation       = $IOTA_PROTOCOL_CONFIG_FEATURE_FLAGS_OVERRIDE_ENABLE_VALIDATOR_ATTESTATION${RESET}"
echo "${CYAN}  - per_object_congestion_control_mode = $IOTA_PROTOCOL_CONFIG_FEATURE_FLAGS_OVERRIDE_PER_OBJECT_CONGESTION_CONTROL_MODE${RESET}"
echo "${YELLOW}run.sh ${run_args[*]}${RESET}"
echo

cd "$PRIVNET_DIR"
./run.sh "${run_args[@]}"

# --- Wait for validators and verify they applied the overrides we set ---------
# We only check the protocol-config overrides this script actually exported
# (the 3 flags above, plus any numeric limit you set). The override macro logs
# each applied field as: ProtocolConfig field "<f>" has been overridden with
# the value: <v>  — so we poll each validator's log until those lines match.
READY_TIMEOUT="${READY_TIMEOUT:-120}"

declare -A expected
while IFS='=' read -r _name _value; do
  case "$_name" in
  IOTA_PROTOCOL_CONFIG_OVERRIDE_ENABLE) continue ;; # the gate, not a config field
  IOTA_PROTOCOL_CONFIG_FEATURE_FLAGS_OVERRIDE_*) _field="${_name#IOTA_PROTOCOL_CONFIG_FEATURE_FLAGS_OVERRIDE_}" ;;
  IOTA_PROTOCOL_CONFIG_OVERRIDE_*) _field="${_name#IOTA_PROTOCOL_CONFIG_OVERRIDE_}" ;;
  *) continue ;;
  esac
  expected["${_field,,}"]="$_value"
done < <(env | grep -E '^IOTA_PROTOCOL_CONFIG_(FEATURE_FLAGS_)?OVERRIDE_')

list_validators() { docker compose ps --services 2>/dev/null | grep -E '^validator-[0-9]+$' | sort; }
mismatches() { # $1 = service ; prints "field expected got" for each missing/wrong override
  local svc="$1" logs field
  logs="$(docker compose logs "$svc" 2>&1)"
  for field in "${!expected[@]}"; do
    grep -qF "ProtocolConfig field \"$field\" has been overridden with the value: ${expected[$field]}" <<<"$logs" ||
      echo "  [$svc] $field (want '${expected[$field]}')"
  done
}

echo
echo "${YELLOW}Verifying validators applied:${RESET}"
for _f in "${!expected[@]}"; do echo "${CYAN}  - $_f = ${expected[$_f]}${RESET}"; done
ok=false
deadline=$((SECONDS + READY_TIMEOUT))
while ((SECONDS < deadline)); do
  mapfile -t vals < <(list_validators)
  if ((${#vals[@]} > 0)); then
    all_ok=true
    for v in "${vals[@]}"; do
      if [[ -n "$(mismatches "$v")" ]]; then all_ok=false; fi
    done
    if $all_ok; then
      ok=true
      break
    fi
  fi
  sleep 3
done

if ! $ok; then
  echo "${RED}ERROR: validators did not apply the expected overrides within ${READY_TIMEOUT}s:${RESET}" >&2
  for v in $(list_validators); do
    while IFS= read -r _line; do echo "${RED}${_line}${RESET}" >&2; done < <(mismatches "$v")
  done
  echo "Tearing down (docker compose down --remove-orphans)..." >&2
  docker compose down --remove-orphans || true
  exit 1
fi

echo "${GREEN}OK: all validators applied the requested overrides.${RESET}"

# --- Bring up monitoring (prometheus + grafana), now that the network is up ---
GRAFANA_DIR="${GRAFANA_DIR:-$REPO_ROOT/dev-tools/grafana-local}"
if [[ -d "$GRAFANA_DIR" ]]; then
  echo
  echo "${BLUE}grafana-local @ $GRAFANA_DIR${RESET}"
  (cd "$GRAFANA_DIR" && docker compose up -d)
  echo "${GREEN}Monitoring up - [Grafana](http://localhost:3000), [Prometheus](http://localhost:9090)${RESET}"
else
  echo "${YELLOW}WARN: monitoring dir not found, skipping: $GRAFANA_DIR${RESET}" >&2
fi
