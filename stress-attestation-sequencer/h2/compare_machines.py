#!/usr/bin/env python3
"""compare_machines.py — cross-machine calibration diff.

Joins two calibration-<machine>.csv files on (slow_n, slow_size) and reports, per point:
  - CU match: attested/actual CUs are protocol-defined gas metering, so they must
    be identical on every machine. A mismatch means the two runs are on different
    protocol/gas configs — a red flag, not a hardware difference.
  - exec-time ratio: internal Move-VM execution latency is machine-dependent
    (per-core speed). At low rate (unloaded) this is the intrinsic per-tx cost, so
    the ratio reflects raw single-thread performance. Reported mean ± sem each.

Usage:  compare_machines.py <baseline.csv> <other.csv> [baseline_name other_name]
Pure stdlib.
"""

import csv
import sys

a_path, b_path = sys.argv[1], sys.argv[2]
a_name = sys.argv[3] if len(sys.argv) > 3 else "A"
b_name = sys.argv[4] if len(sys.argv) > 4 else "B"


def load(path):
    rows = {}
    for r in csv.DictReader(open(path)):
        rows[(int(r["slow_n"]), int(r["slow_size"]))] = r
    return rows


A, B = load(a_path), load(b_path)
keys = sorted(set(A) & set(B), key=lambda k: (int(A[k]["product"]), k[0]))
only_a, only_b = sorted(set(A) - set(B)), sorted(set(B) - set(A))

print(f"{a_name} = {a_path}")
print(f"{b_name} = {b_path}")
if only_a:
    print(f"  (only in {a_name}: {only_a})")
if only_b:
    print(f"  (only in {b_name}: {only_b})")
print()

hdr = (
    f"{'n':>5} {'size':>5} {'product':>8} | {'CU':>12} {'CU ok':>6} | "
    f"{a_name + ' exec':>14} {b_name + ' exec':>14} {b_name + '/' + a_name:>8}"
)
print(hdr)
print("-" * len(hdr))

cu_mismatches = 0
for k in keys:
    ra, rb = A[k], B[k]
    ca, cb = float(ra["actual_cu"]), float(rb["actual_cu"])
    cu_ok = abs(ca - cb) <= 1e-6 * max(abs(ca), abs(cb), 1.0)
    cu_mismatches += not cu_ok
    ea, eb = float(ra["exec_mean_ms"]), float(rb["exec_mean_ms"])
    sa, sb = float(ra["exec_sem_ms"]), float(rb["exec_sem_ms"])
    cu_str = f"{ca:.1f}" if cu_ok else f"{ca:.1f}!={cb:.1f}"
    print(
        f"{k[0]:>5} {k[1]:>5} {int(ra['product']):>8} | {cu_str:>12} "
        f"{'yes' if cu_ok else 'NO':>6} | "
        f"{ea:>8.3f}±{sa:<5.3f} {eb:>8.3f}±{sb:<5.3f} {eb / ea:>8.2f}"
    )

print()
if cu_mismatches:
    print(
        f"WARNING: {cu_mismatches} CU mismatch(es) — machines differ in protocol/gas config."
    )
else:
    print(f"CUs identical on both machines ({len(keys)} points) — as expected.")
