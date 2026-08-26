#!/usr/bin/env bash
# Copyright (c) 2026 IOTA Stiftung
# SPDX-License-Identifier: Apache-2.0
#
# Put a Linux calibration machine into a repeatable state and record it.
# Needs root (or passwordless sudo) to change governor/boost; without it,
# the script only reports. Usage: machine_prep.sh OUT_DIR
set -euo pipefail
out="${1:?usage: machine_prep.sh OUT_DIR}"
mkdir -p "$out"
sudo_cmd=""
if [ "$(id -u)" -ne 0 ]; then
  if sudo -n true 2>/dev/null; then sudo_cmd="sudo"; else
    echo "no root/sudo: reporting only, not changing machine state" >&2
  fi
fi
write() { # write VALUE FILE (best effort)
  [ -w "$2" ] || [ -n "$sudo_cmd" ] || return 0
  [ -e "$2" ] && echo "$1" | $sudo_cmd tee "$2" >/dev/null 2>&1 || true
}
if [ -n "$sudo_cmd" ] || [ "$(id -u)" -eq 0 ]; then
  for g in /sys/devices/system/cpu/cpu*/cpufreq/scaling_governor; do write performance "$g"; done
  write 1 /sys/devices/system/cpu/intel_pstate/no_turbo     # Intel: disable turbo
  write 0 /sys/devices/system/cpu/cpufreq/boost              # AMD/other: disable boost
fi
{
  echo "date: $(date -u +%FT%TZ)"
  echo "kernel: $(uname -a)"
  echo "governors: $(cat /sys/devices/system/cpu/cpu*/cpufreq/scaling_governor 2>/dev/null | sort -u | tr '\n' ' ')"
  echo "smt: $(cat /sys/devices/system/cpu/smt/control 2>/dev/null || echo n/a)"
  echo "intel_no_turbo: $(cat /sys/devices/system/cpu/intel_pstate/no_turbo 2>/dev/null || echo n/a)"
  echo "cpufreq_boost: $(cat /sys/devices/system/cpu/cpufreq/boost 2>/dev/null || echo n/a)"
  echo "thp: $(cat /sys/kernel/mm/transparent_hugepage/enabled 2>/dev/null || echo n/a)"
  echo "load: $(cat /proc/loadavg)"
  echo "disk for $out: $(df -h "$out" | tail -1)"
  command -v fio >/dev/null && fio --name=baseline --filename="$out/.fio-baseline" --size=1G \
      --rw=randread --bs=4k --direct=1 --iodepth=16 --runtime=20 --time_based --output-format=terse \
      2>/dev/null | awk -F';' '{print "fio_randread_4k_iops: " $8}' && rm -f "$out/.fio-baseline"
} | tee "$out/machine-state.txt"
