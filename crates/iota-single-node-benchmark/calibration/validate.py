#!/usr/bin/env python3
# Copyright (c) 2026 IOTA Stiftung
# SPDX-License-Identifier: Apache-2.0
"""Stage 3 validation: score a calibration artifact on data it was not
trained on, against the plan's acceptance criteria.

    # collect a mixed-workload dataset (workloads interleaved within one run)
    ./validate.py collect --out DIR [--spec mixed-default.json] [--runs 5]

    # score an artifact against any dataset(s)
    ./validate.py score --artifact artifact.json --data DIR [DIR ...]

Acceptance (from the plan): predicted cpu_time >= measured on >= 99% of
transactions, and the 95th-percentile overestimate <= ~2x. Scoring a
single-workload sweep dataset with a mixed-trained artifact is the
"single-workload commits" check; scoring mixed data with a sweep-trained
artifact is the reverse. Replay of real checkpoint ranges uses the same
scorer once replay capture exists.
"""

import argparse
import json
import statistics
import subprocess
import sys
import time
from pathlib import Path

from fit import BASE_PREDICTORS, load_dataset
from sweep import REPO_ROOT, machine_manifest

DEFAULT_BINARY = REPO_ROOT / "target/release/calibrate"
CALIBRATION_DIR = Path(__file__).resolve().parent
COVERAGE_TARGET = 0.99
P95_TARGET = 2.0


def predict(artifact, profile):
    w = artifact["coefficients_ns_per_unit"]
    total = artifact["c0_ns"]
    for c in BASE_PREDICTORS:
        if c in w:
            total += w[c] * profile.get(c, 0)
    gas_map = profile.get("native_gas_by_function") or profile.get("native_gas_by_module", {})
    for fn, gas in gas_map.items():
        total += w.get(f"native_gas[{fn}]", 0.0) * gas
    for fn, calls in profile.get("native_calls_by_function", {}).items():
        total += w.get(f"native_calls[{fn}]", 0.0) * calls
    return artifact["safety_multiplier"] * total


def collect(args):
    if not args.binary.exists():
        sys.exit(f"binary not found: {args.binary}")
    if getattr(args, "concurrency", 0):
        # keep every lane busy for several waves under contention
        args.tx_count = max(args.tx_count, 4 * args.concurrency)
    spec = args.spec if args.spec.exists() else CALIBRATION_DIR / args.spec
    point_dir = args.out / "mixed" / f"spec={spec.stem}"
    point_dir.mkdir(parents=True, exist_ok=True)
    manifest = machine_manifest(args.binary, sys.argv[1:])
    (args.out / "manifest.json").write_text(json.dumps(manifest, indent=2) + "\n")
    for i in range(args.runs):
        out_file = point_dir / f"run-{i}.jsonl"
        if out_file.exists() and out_file.stat().st_size > 0:
            continue
        print(f"[mixed] run {i} ({args.tx_count} txs)", flush=True)
        cmd = [str(args.binary), "--tx-count", str(args.tx_count),
               "--profile-output", str(out_file),
               *(["--concurrency", str(args.concurrency)] if args.concurrency else []),
               "mixed", "--spec-file", str(spec)]
        r = subprocess.run(cmd, capture_output=True, text=True)
        if r.returncode != 0:
            out_file.unlink(missing_ok=True)
            sys.exit(f"benchmark failed:\n{r.stdout[-2000:]}\n{r.stderr[-2000:]}")
        if args.cooldown:
            time.sleep(args.cooldown)
    print(f"mixed dataset written to {args.out}")


def score(args):
    artifact = json.loads(args.artifact.read_text())
    rows = []
    for d in args.data:
        r, _ = load_dataset(d)
        print(f"{d}: {len(r)} transactions")
        rows.extend(r)
    if not rows:
        sys.exit("no rows to score")

    ratios = []
    by_sweep = {}
    for row in rows:
        p = predict(artifact, row["profile"])
        ratio = p / max(row["measured_ns"], 1.0)
        ratios.append(ratio)
        by_sweep.setdefault(row["sweep"], []).append(ratio)

    covered = sum(1 for r in ratios if r >= 1.0) / len(ratios)
    over = sorted(ratios)
    p95_over = over[int(0.95 * (len(over) - 1))]
    median_over = statistics.median(ratios)

    coverage_pass = covered >= COVERAGE_TARGET
    p95_pass = p95_over <= P95_TARGET
    report = {
        "artifact": str(args.artifact),
        "datasets": [str(d) for d in args.data],
        "n_transactions": len(rows),
        "coverage": round(covered, 4),
        "coverage_target": COVERAGE_TARGET,
        "coverage_pass": coverage_pass,
        "p95_overestimate": round(p95_over, 3),
        "p95_target": P95_TARGET,
        "p95_pass": p95_pass,
        "median_overestimate": round(median_over, 3),
        "by_sweep": {
            s: {
                "n": len(v),
                "coverage": round(sum(1 for r in v if r >= 1.0) / len(v), 4),
                "median_overestimate": round(statistics.median(v), 3),
            }
            for s, v in sorted(by_sweep.items())
        },
    }
    if args.report:
        args.report.write_text(json.dumps(report, indent=2) + "\n")

    print(f"\ncoverage: {covered:.2%} (target >= {COVERAGE_TARGET:.0%}) "
          f"{'PASS' if coverage_pass else 'FAIL'}")
    print(f"p95 overestimate: x{p95_over:.2f} (target <= x{P95_TARGET}) "
          f"{'PASS' if p95_pass else 'FAIL'}")
    print(f"median overestimate: x{median_over:.2f}")
    print("\nper workload (coverage, median overestimate):")
    for s, v in report["by_sweep"].items():
        flag = "" if v["coverage"] >= COVERAGE_TARGET else "  <-- under-covered"
        print(f"  {s}: {v['coverage']:.2%}, x{v['median_overestimate']}{flag}")
    sys.exit(0 if coverage_pass and p95_pass else 1)


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    sub = ap.add_subparsers(dest="cmd", required=True)

    c = sub.add_parser("collect", help="collect a mixed-workload dataset")
    c.add_argument("--out", required=True, type=Path)
    c.add_argument("--spec", type=Path, default=Path("mixed-default.json"))
    c.add_argument("--runs", type=int, default=5)
    c.add_argument("--tx-count", type=int, default=200)
    c.add_argument("--cooldown", type=float, default=1.0)
    c.add_argument("--binary", type=Path, default=DEFAULT_BINARY)
    c.add_argument("--concurrency", type=int, default=0,
                   help="transactions executing at once (0 = one at a time); "
                        "nonzero for the contended mixed collection")
    c.set_defaults(func=collect)

    v = sub.add_parser("score", help="score an artifact against datasets")
    v.add_argument("--artifact", required=True, type=Path)
    v.add_argument("--data", nargs="+", type=Path, required=True)
    v.add_argument("--report", type=Path, help="write the full report JSON here")
    v.set_defaults(func=score)

    args = ap.parse_args()
    args.func(args)


if __name__ == "__main__":
    main()
