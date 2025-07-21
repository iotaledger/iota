#!/bin/bash

# Copyright (c) 2025 IOTA Stiftung
# SPDX-License-Identifier: Apache-2.0

# Initial delay in seconds
initial_delay=10

# Maximum delay in seconds
max_delay=90

# Step to increase the delay each round
step=30

echo "=== Starting validator downtime experiment ==="


# Phase 1: stop/start validator-1
delay=$initial_delay
while [ $delay -le $max_delay ]; do
  echo "[Phase 1] Stopping validator-1 for ${delay}s..."
  docker stop validator-1
  sleep $delay
  echo "[Phase 1] Starting validator-1..."
  docker start validator-1
  sleep $delay
  delay=$((delay + step))
done

# Phase 2: pause/unpause validator-1
delay=$initial_delay
while [ $delay -le $max_delay ]; do
  echo "[Phase 2] Pausing validator-1 for ${delay}s..."
  docker pause validator-1
  sleep $delay
  echo "[Phase 2] Unpausing validator-1..."
  docker unpause validator-1
  sleep $delay
  delay=$((delay + step))
done

# Phase 3: disconnect/connect validator-1 from network
# Detect actual network name containing validator-1
network_name=$(docker inspect validator-1 --format '{{range $k, $v := .NetworkSettings.Networks}}{{$k}}{{end}}')
delay=$initial_delay
while [ $delay -le $max_delay ]; do
  echo "[Phase 3] Disconnecting validator-1 from $network_name for ${delay}s..."
  docker network disconnect "$network_name" validator-1
  sleep $delay
  echo "[Phase 3] Reconnecting validator-1 to $network_name..."
  docker network connect "$network_name" validator-1
  sleep 5
  delay=$((delay + step))
done

echo "=== Experiment completed ==="