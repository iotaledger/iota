#!/bin/bash

# === CONFIGURATION ===

# Docker validator container names
validators=("validator-1" "validator-2" "validator-3" "validator-4")

# Duration settings (in seconds)
initial_duration=100
max_duration=120
step=30

echo "=== Starting network disruption experiments using Pumba ==="

# Ensure Pumba is available
if ! docker image inspect gaiaadm/pumba:latest >/dev/null 2>&1; then
  echo "Pulling Pumba image..."
  docker pull gaiaadm/pumba
fi

# === HELPER FUNCTION ===
run_pumba() {
  local validator=$1
  local duration=$2
  local pumba_args=$3

  echo "[PUMBA] Running on $validator with: $pumba_args for $duration seconds"
  docker run -d --rm \
    --name "pumba_${validator}_$(date +%s)" \
    -v /var/run/docker.sock:/var/run/docker.sock \
    gaiaadm/pumba \
    netem --tc-image gaiadocker/iproute2 \
    --duration "${duration}s" \
    $pumba_args "$validator"
}


echo "=== PHASE 1: Full isolation ==="
# === PHASE 1: Full isolation ===
for duration in $(seq $initial_duration $step $max_duration); do
  run_pumba "${validators[0]}" "$duration" "loss --percent 100"
  sleep $((duration + 10))
done

echo "=== PHASE 2: Outgoing blocked ==="
# === PHASE 2: Outgoing blocked ===
for duration in $(seq $initial_duration $step $max_duration); do
  run_pumba "${validators[1]}" "$duration" "loss --percent 100 --direction outbound"
  sleep $((duration + 10))
done

echo  "=== PHASE 3: Incoming blocked ==="
# === PHASE 3: Incoming blocked ===
for duration in $(seq $initial_duration $step $max_duration); do
  run_pumba "${validators[2]}" "$duration" "loss --percent 100 --direction inbound"
  sleep $((duration + 10))
done

echo "# === PHASE 4: Only allow connection to validator-3 ==="
# === PHASE 4: Only allow connection to validator-3 ===
validator_3_ip=$(docker inspect -f '{{range.NetworkSettings.Networks}}{{.IPAddress}}{{end}}' validator-3)
for duration in $(seq $initial_duration $step $max_duration); do
  run_pumba "${validators[3]}" "$duration" "loss --percent 100 --exclude-dst $validator_3_ip"
  sleep $((duration + 10))
done

echo "=== All experiments completed ==="