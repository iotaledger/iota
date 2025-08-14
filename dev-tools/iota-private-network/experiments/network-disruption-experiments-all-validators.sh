#!/bin/bash

# Copyright (c) 2025 IOTA Stiftung
# SPDX-License-Identifier: Apache-2.0

# === CONFIGURATION ===

# Duration settings (in seconds)
disrupt_duration=60
recover_duration=60

echo "=== Starting full network disruption experiment using direct tc ==="

# === HELPER FUNCTIONS ===

add_netem_loss() {
  local loss_percent=$1
  for i in {1..4}; do
    echo "[NETEM] Adding $loss_percent% packet loss to validator-$i"
    docker run --rm --privileged --net container:validator-$i gaiadocker/iproute2 qdisc add dev eth0 root netem loss ${loss_percent}%
  done
}

remove_netem_loss() {
  for i in {1..4}; do
    echo "[NETEM] Removing traffic disruption from validator-$i"
    docker run --rm --privileged --net container:validator-$i gaiadocker/iproute2 qdisc del dev eth0 root
  done
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