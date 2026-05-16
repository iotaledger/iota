#!/usr/bin/env bash
# Dump host hardware + OS specs. Useful for comparing benchmark results across
# machines (workstation vs server) — see how CPU/RAM/NUMA affect peak ratio.
#
# Usage: ./hw-specs.sh
#        ./hw-specs.sh > server-A.txt   # save for comparison

echo "=== CPU ==="
lscpu | grep -E '^Model name|^Architecture|^CPU\(s\):|^Thread|^Core|MHz|L[123].*cache|^Vendor|^Hypervisor|^Virtualization'
echo

echo "=== Memory ==="
free -h
echo

echo "=== Storage ==="
df -hT / 2>/dev/null
lsblk -o NAME,SIZE,TYPE,ROTA,MODEL 2>/dev/null | grep -v loop | head -10
echo

echo "=== Kernel / OS ==="
uname -srm
if [ -f /etc/os-release ]; then
    . /etc/os-release
    echo "$PRETTY_NAME"
fi
echo

echo "=== NUMA ==="
numactl --hardware 2>/dev/null | head -5 || echo "(no numactl)"
echo

echo "=== ulimits ==="
echo "open files (soft):     $(ulimit -n)"
echo "max user procs:        $(ulimit -u)"
