#!/bin/bash

# Copyright (c) 2025 IOTA Stiftung
# SPDX-License-Identifier: Apache-2.0

# === CONFIGURATION ===

# Docker validator container name
validator="validator-1"

# Duration settings (in seconds)
disrupt_duration=60
recover_duration=60

echo "=== Starting custom network disruption experiment using direct tc ==="

# === HELPER FUNCTIONS ===

add_netem_loss() {
  local loss_percent=$1
  echo "[NETEM] Adding $loss_percent% packet loss to $validator"
  docker run --rm --privileged --net container:$validator gaiadocker/iproute2 qdisc add dev eth0 root netem loss ${loss_percent}%
}

remove_netem_loss() {
  echo "[NETEM] Removing traffic disruption from $validator"
  docker run --rm --privileged --net container:$validator gaiadocker/iproute2 qdisc del dev eth0 root
}

# === DISRUPTION CYCLE ===

for loss in 20 40 60 80 100; do
  echo "=== Disruption phase with $loss% loss ==="
  add_netem_loss $loss
  sleep $disrupt_duration

  echo "=== Recovery phase ==="
  remove_netem_loss
  sleep $recover_duration
done

echo "=== Experiment completed ==="