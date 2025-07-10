#!/bin/bash

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


# Phase 1: validator-1 isolated from validator-3 and validator-4
block_between validator-1 validator-3
block_between validator-1 validator-4
sleep "$duration"
restore validator-1

# Phase 1a: block incoming only on validator-1 from validator-3 and validator-4
block_incoming_between validator-1 validator-3
block_incoming_between validator-1 validator-4
sleep "$duration"
restore validator-1

# Phase 1b: block outgoing only on validator-1 to validator-3 and validator-4
block_outgoing_between validator-1 validator-3
block_outgoing_between validator-1 validator-4
sleep "$duration"
restore validator-1

# Phase 2: validator-2 isolated from validator-3
block_between validator-2 validator-3
sleep "$duration"
restore validator-2

# Phase 3: validator-4 isolated from validator-2 and validator-3
block_between validator-4 validator-2
block_between validator-4 validator-3
sleep "$duration"
restore validator-4

echo "=== Experiment Completed ==="