#!/bin/bash

# List of validators to test
validators=("validator-1")

# Initial delay in seconds
initial_delay=10

# Maximum delay in seconds
max_delay=60

# Step to increase the delay each round
step=10

echo "=== Starting validator downtime experiment ==="

# Phase 1: single validator offline with increasing delay
delay=$initial_delay
while [ $delay -le $max_delay ]; do
  echo "[Phase 1] Stopping ${validators[0]} for ${delay}s..."
  docker stop ${validators[0]}
  sleep $delay
  echo "[Phase 1] Starting ${validators[0]}..."
  docker start ${validators[0]}
  sleep 5
  delay=$((delay + step))
done

# Phase 2: two validators offline with increasing delay
validators=("validator-1" "validator-2")
delay=$initial_delay
while [ $delay -le $max_delay ]; do
  echo "[Phase 2] Stopping ${validators[*]} for ${delay}s..."
  docker stop "${validators[@]}"
  sleep $delay
  echo "[Phase 2] Starting ${validators[*]}..."
  docker start "${validators[@]}"
  sleep 5
  delay=$((delay + step))
done

echo "=== Experiment completed ==="