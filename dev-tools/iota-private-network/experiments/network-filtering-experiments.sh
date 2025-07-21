#!/bin/bash

# Copyright (c) 2025 IOTA Stiftung
# SPDX-License-Identifier: Apache-2.0

# Duration for each filter phase (seconds)
duration=60

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


# Phase 1: validator-1 isolated from validator-2
block_between validator-1 validator-2
sleep "$duration"
restore validator-1

# Phase 2: validator-1 isolated from validator-2 and validator-3
block_between validator-1 validator-2
block_between validator-1 validator-3
sleep "$duration"
restore validator-1

# Phase 3: mixed isolation
#  - 1↔2, 1↔3, 3↔4, 4↔1
block_between validator-1 validator-2
block_between validator-2 validator-3
block_between validator-3 validator-4
block_between validator-4 validator-1
sleep "$duration"
# Restore all affected validators
restore validator-1
restore validator-2
restore validator-3
restore validator-4

echo "=== Experiment Completed ==="