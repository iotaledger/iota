#!/bin/bash

# Copyright (c) 2025 IOTA Stiftung
# SPDX-License-Identifier: Apache-2.0

# Array of durations for each run (seconds)
durations=(30 60 90 120 240)
# Cool-down between runs
cooldown=30

# Helper: block traffic between container A and container B (applies on A)
block_between() {
  local A=$1 B=$2
  # Get B's IP
  IP_B=$(docker inspect -f '{{range .NetworkSettings.Networks}}{{.IPAddress}}{{end}}' "$B")
  echo "=== Blocking $A <-> $B (dropping packets on $A) ==="
  docker run --rm --privileged --net container:"$A" -e IP_B="$IP_B" nicolaka/netshoot sh -c "
    iptables -F &&
    iptables -A OUTPUT -d \$IP_B -j DROP &&
    iptables -A INPUT  -s \$IP_B -j DROP &&
    echo '  $A now isolated from $B'
  "
}

# Helper: block only incoming traffic between container A and container B (applies on A)
block_incoming_between() {
  local A=$1 B=$2
  IP_B=$(docker inspect -f '{{range .NetworkSettings.Networks}}{{.IPAddress}}{{end}}' "$B")
  echo "=== Blocking incoming on $A from $B ==="
  docker run --rm --privileged --net container:"$A" -e IP_B="$IP_B" nicolaka/netshoot sh -c "
    iptables -F &&
    iptables -A INPUT -s \$IP_B -j DROP &&
    echo '  $A now blocks incoming from $B'
  "
}

# Helper: block only outgoing traffic from container A to container B
block_outgoing_between() {
  local A=$1 B=$2
  IP_B=$(docker inspect -f '{{range .NetworkSettings.Networks}}{{.IPAddress}}{{end}}' "$B")
  echo "=== Blocking outgoing from $A to $B ==="
  docker run --rm --privileged --net container:"$A" -e IP_B="$IP_B" nicolaka/netshoot sh -c "
    iptables -F &&
    iptables -A OUTPUT -d \$IP_B -j DROP &&
    echo '  $A now blocks outgoing to $B'
  "
}

# Helper: restore connectivity on container A
restore() {
  local A=$1
  echo "=== Restoring connectivity on $A ==="
  docker run --rm --privileged --net container:"$A" nicolaka/netshoot sh -c "
    iptables -F &&
    echo '  $A is fully connected'
  "
}

echo "=== Network Filtering Experiment ==="

echo "=== Phase 1 - blocking incoming ==="

for duration in "${durations[@]}"; do
  echo "=== Run with duration: ${duration}s ==="

  #
  # Test 1:
  #
  # validator-14 only accepts incoming from validator-15
  for i in $(seq 1 19); do
    if [ "$i" -ne 15 ]; then
      block_incoming_between validator-14 validator-"$i"
    fi
  done

  # validator-15 only accepts incoming from validator-16
  for i in $(seq 1 19); do
    if [ "$i" -ne 16 ]; then
      block_incoming_between validator-15 validator-"$i"
    fi
  done


  # validator-16 only accepts incoming from 17, 18, 19
  for i in $(seq 1 16); do
    if ! echo "17 18 19" | grep -qw "$i"; then
      block_incoming_between validator-16 validator-"$i"
    fi
  done


  #  validator-17 only accepts incoming from 18, 19
  for i in $(seq 1 17); do
    if ! echo "18 19" | grep -qw "$i"; then
      block_incoming_between validator-17 validator-"$i"
    fi
  done


  # validator-18 only accepts incoming from 19
  for i in $(seq 1 18); do
    if [ "$i" -ne 19 ]; then
      block_incoming_between validator-18 validator-"$i"
    fi
  done


  #  validator-19 accepts no incoming
  for i in $(seq 1 19); do
    block_incoming_between validator-19 validator-"$i"
  done

  ## Run the experiment
  sleep "$duration"

  #  restore validators
  for i in $(seq 14 19); do
    restore validator-"$i"
  done

  ## Cool down
  echo "=== Cooling down for ${cooldown}s ==="
  sleep "${cooldown}"
done

echo "=== Experiment Completed ==="


echo "=== Phase 2 - blocking outgoing ==="

for duration in "${durations[@]}"; do
  echo "=== Run with duration: ${duration}s ==="

  #
  # Test 1:
  #
  # validator-14 only accepts outgoing from validator-15
  for i in $(seq 1 19); do
    if [ "$i" -ne 15 ]; then
      block_outgoing_between validator-14 validator-"$i"
    fi
  done

  # validator-15 only accepts outgoing from validator-16
  for i in $(seq 1 19); do
    if [ "$i" -ne 16 ]; then
      block_outgoing_between validator-15 validator-"$i"
    fi
  done


  # validator-16 only accepts outgoing from 17, 18, 19
  for i in $(seq 1 16); do
    if ! echo "17 18 19" | grep -qw "$i"; then
      block_outgoing_between validator-16 validator-"$i"
    fi
  done


  #  validator-17 only accepts outgoing from 18, 19
  for i in $(seq 1 17); do
    if ! echo "18 19" | grep -qw "$i"; then
      block_outgoing_between validator-17 validator-"$i"
    fi
  done


  # validator-18 only accepts outgoing from 19
  for i in $(seq 1 18); do
    if [ "$i" -ne 19 ]; then
      block_outgoing_between validator-18 validator-"$i"
    fi
  done


  #  validator-19 accepts no outgoing
  for i in $(seq 1 19); do
    block_outgoing_between validator-19 validator-"$i"
  done

  ## Run the experiment
  sleep "$duration"

  #  restore validators
  for i in $(seq 14 19); do
    restore validator-"$i"
  done

  ## Cool down
  echo "=== Cooling down for ${cooldown}s ==="
  sleep "${cooldown}"
done

echo "=== Experiment Completed ==="