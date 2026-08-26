#!/usr/bin/env bash
# Copyright (c) 2026 IOTA Stiftung
# SPDX-License-Identifier: Apache-2.0
#
# Put a Linux calibration machine into a repeatable state and record it.
# Needs root (or passwordless sudo) to change governor/boost; without it,
# the script only reports.
#   machine_prep.sh OUT_DIR [--turbo on|off]     (default: off, the fitting protocol)
set -euo pipefail
out="${1:?usage: machine_prep.sh OUT_DIR [--turbo on|off]}"; shift
turbo="off"
while [ $# -gt 0 ]; do
  case "$1" in
    --turbo) turbo="$2"; shift 2;;
    *) echo "unknown arg $1" >&2; exit 2;;
  esac
done
case "$turbo" in on|off) ;; *) echo "--turbo must be on or off" >&2; exit 2;; esac
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
  if [ "$turbo" = "off" ]; then
    write 1 /sys/devices/system/cpu/intel_pstate/no_turbo   # Intel: disable turbo
    write 0 /sys/devices/system/cpu/cpufreq/boost            # AMD/other: disable boost
  else
    write 0 /sys/devices/system/cpu/intel_pstate/no_turbo   # Intel: enable turbo
    write 1 /sys/devices/system/cpu/cpufreq/boost            # AMD/other: enable boost
  fi
fi
{
  echo "date: $(date -u +%FT%TZ)"
  echo "turbo policy requested: $turbo"
  echo "kernel: $(uname -a)"
  echo "governors: $(cat /sys/devices/system/cpu/cpu*/cpufreq/scaling_governor 2>/dev/null | sort -u | tr '\n' ' ')"
  echo "smt: $(cat /sys/devices/system/cpu/smt/control 2>/dev/null || echo n/a)"
  echo "intel_no_turbo: $(cat /sys/devices/system/cpu/intel_pstate/no_turbo 2>/dev/null || echo n/a)"
  echo "cpufreq_boost: $(cat /sys/devices/system/cpu/cpufreq/boost 2>/dev/null || echo n/a)"
  echo "thp: $(cat /sys/kernel/mm/transparent_hugepage/enabled 2>/dev/null || echo n/a)"
  echo "load: $(cat /proc/loadavg 2>/dev/null || echo n/a)"
  echo "disk for $out: $(df -h "$out" | tail -1)"
  # Device topology behind the store: the cold-read constants describe this device.
  command -v lsblk >/dev/null 2>&1 && echo "block devices:" && lsblk -o NAME,TYPE,SIZE,MODEL,ROTA 2>/dev/null | sed 's/^/  /'
  [ -r /proc/mdstat ] && echo "mdstat: $(grep -E '^md' /proc/mdstat | tr '\n' ';')"
  # Disk baseline is optional: a missing or failing fio must not abort the run.
  if command -v fio >/dev/null 2>&1; then
    if fio --name=baseline --filename="$out/.fio-baseline" --size=1G --rw=randread --bs=4k \
        --direct=1 --iodepth=16 --runtime=20 --time_based --output-format=terse 2>/dev/null \
        | awk -F';' '{print "fio_randread_4k_iops: " $8}'; then :; else
      echo "fio_randread_4k_iops: n/a (fio failed)"
    fi
    rm -f "$out/.fio-baseline"
  else
    echo "fio_randread_4k_iops: n/a (fio not installed)"
  fi
} | tee "$out/machine-state.txt"
