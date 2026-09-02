#!/usr/bin/env python3
"""Concurrency contrast runs for the CPU calibration.

Re-runs a curated set of workloads at a range of worker counts (the
`--concurrency` cap on the calibrate binary) and reports, per workload, how
much per-transaction wall-clock inflates as workers are added. The inflation
ratio (median at N workers / median at 1 worker) is the measurement the
makespan model needs: it tests the assumption that a transaction's lane-time
is independent of what the other lanes run.

Expectation the runs check:
  - compute-bound workloads (signatures, tight arithmetic) stay near x1;
  - memory-bandwidth-bound workloads (held-memory trees, large moves, big
    reads/writes) inflate above it, from shared memory bandwidth, last-level
    cache, and the allocator.

The summary also reports each point's aggregate moved-bytes rate — the bytes
moved through the memory/store path per second, summed over all lanes — whose
plateau across worker counts is the bandwidth ceiling `B_mem` the
memory-bandwidth dimension needs (the per-lane inflation knee and the moved-bytes
plateau should coincide).

Layout:
  <out>/concurrency/<workload>/concurrency=<N>/run-*.jsonl
  <out>/concurrency/manifest.json                machine, commit, binary, cpus
  <out>/concurrency/summary.jsonl                one row per (workload, N)
  <out>/concurrency/inflation.json               per-workload inflation and moved-bytes rate vs N

Clock policy is part of this experiment (see the plan): fit-time collections
run turbo off, but these contrast runs are taken under both turbo states so the
1-worker -> N-worker inflation includes the all-core clock descent along with
cache and bandwidth contention. Set the turbo state with machine_prep.sh before
running; it is recorded in the machine state, not by this script.
"""

import argparse
import json
import multiprocessing
import statistics
import subprocess
import sys
from pathlib import Path

# Reuse the dataset provenance and summary helpers so a concurrency dataset
# carries the same machine/commit/binary manifest as a sweep dataset.
import sweep

# Each workload names its knob(s) and a class. The class is not consumed by the
# script; it documents the expected inflation regime and groups the report.
WORKLOADS = {
    # compute-bound: near x1 expected
    "sig-bls-min-pk": {
        "class": "compute",
        "args": ["--num-sig-verifies", "4", "--sig-scheme", "bls-min-pk"],
        "tx_count": 48,
    },
    "interpreter-scalar": {
        "class": "compute",
        "args": ["--scalar-ops", "64000"],
        "tx_count": 48,
    },
    "hash-sha2-256": {
        "class": "compute",
        "args": ["--num-hashes", "256", "--hash-family", "sha2-256"],
        "tx_count": 48,
    },
    # memory-bandwidth-bound: inflation above x1 expected
    "memory-tree-width": {
        "class": "memory",
        "args": ["--tree-width", "32", "--tree-depth", "3"],
        "tx_count": 48,
    },
    "interpreter-vector-move": {
        "class": "memory",
        "args": ["--vector-move-ops", "64000", "--vector-move-size", "8192"],
        "tx_count": 48,
    },
    "writes-bytes": {
        "class": "memory",
        "args": ["--num-mints", "8", "--nft-size", "16384"],
        "tx_count": 48,
    },
    "reads-input": {
        "class": "memory",
        "args": ["--num-transfers", "16"],
        "tx_count": 48,
    },
    # the real transaction mix admission must survive
    "mixed": {
        "class": "mixed",
        "subcommand": "mixed",
        "args": ["--spec-file", str(Path(__file__).parent / "mixed-default.json")],
        "tx_count": 100,
    },
}

DEFAULT_LEVELS = [1, 2, 4, 8]

# Bytes a transaction moves through the shared memory/store path: reads,
# working-set growth (the held-bytes peaks), and hashing-native input. The
# per-commit sum of these is what the memory-bandwidth dimension caps.
MOVED_BYTES_FIELDS = [
    "input_object_bytes",
    "child_object_read_bytes",
    "stack_size_high_water_mark",
    "locals_size_high_water_mark",
    "object_runtime_cached_bytes",
    "hash_input_bytes",
]


def run_point(binary, workload, spec, level, runs, out_dir, cooldown):
    # Keep every lane busy for several waves at high worker counts; the
    # workload tx_counts were sized for the 1..8 levels.
    tx_count = max(spec.get("tx_count", 48), 4 * level)
    point_dir = out_dir / workload / f"concurrency={level}"
    point_dir.mkdir(parents=True, exist_ok=True)
    run_files = []
    for i in range(runs):
        rf = point_dir / f"run-{i}.jsonl"
        run_files.append(rf)
        if rf.exists() and rf.stat().st_size > 0:
            continue  # resume
        cmd = [
            str(binary),
            "--tx-count", str(tx_count),
            "--concurrency", str(level),
            "--profile-output", str(rf),
            "--rss-output", str(rf.with_suffix(".rss.json")),
            spec.get("subcommand", "ptb"),
            *spec["args"],
        ]
        r = sweep.run_cmd(cmd)
        if r.returncode != 0:
            rf.unlink(missing_ok=True)
            sys.exit(
                f"benchmark failed for {workload} concurrency={level} run {i}:\n"
                f"{r.stdout[-2000:]}\n{r.stderr[-2000:]}"
            )
        if cooldown > 0:
            import time
            time.sleep(cooldown)
    return run_files


def summarize_point(workload, spec, level, run_files):
    pooled = []
    per_run_medians = []
    n_txs = 0
    sum_ns = 0
    sum_moved = 0
    moved_per_tx = []
    for rf in run_files:
        rows = sweep.load_rows(rf)
        if not rows:
            continue
        ns = [r["measured_ns"] for r in rows]
        pooled.extend(ns)
        per_run_medians.append(statistics.median(ns))
        n_txs += len(ns)
        sum_ns += sum(ns)
        for r in rows:
            t = sum(r["profile"].get(f, 0) for f in MOVED_BYTES_FIELDS)
            moved_per_tx.append(t)
            sum_moved += t
    if not per_run_medians:
        return None
    pooled.sort()
    # With all lanes busy, phase wall-clock ~= total lane-time / worker count,
    # so the aggregate rate the memory/store path sustained is:
    aggregate_bps = sum_moved / (sum_ns / 1e9 / max(level, 1)) if sum_ns else 0
    return {
        "workload": workload,
        "class": spec["class"],
        "concurrency": level,
        "n_runs": len([m for m in per_run_medians]),
        "n_txs": n_txs,
        "measured_ns": statistics.median(per_run_medians),
        "measured_ns_p50_pooled": pooled[len(pooled) // 2],
        "measured_ns_p95_pooled": pooled[int(0.95 * (len(pooled) - 1))],
        "moved_bytes_per_tx_median": statistics.median(moved_per_tx),
        "aggregate_moved_bytes_per_sec": round(aggregate_bps),
    }


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--out", required=True, type=Path, help="dataset directory")
    ap.add_argument("--binary", type=Path, default=sweep.DEFAULT_BINARY)
    ap.add_argument("--runs", type=int, default=5, help="runs per point")
    ap.add_argument("--cooldown", type=float, default=1.0)
    ap.add_argument(
        "--levels",
        default=",".join(str(x) for x in DEFAULT_LEVELS),
        help="comma-separated worker counts; capped at the machine's cpu count",
    )
    ap.add_argument(
        "--workloads",
        default=",".join(WORKLOADS),
        help=f"comma-separated subset of: {', '.join(WORKLOADS)}",
    )
    ap.add_argument("--summarize-only", action="store_true")
    args = ap.parse_args()

    cpus = multiprocessing.cpu_count()
    levels = sorted({int(x) for x in args.levels.split(",")})
    dropped = [x for x in levels if x > cpus]
    levels = [x for x in levels if x <= cpus]
    if 1 not in levels:
        # the 1-worker point is the baseline every inflation ratio divides by
        levels = [1] + levels
    selected = [w for w in args.workloads.split(",") if w in WORKLOADS]
    if not selected:
        sys.exit("no known workloads selected")

    out = args.out / "concurrency"
    out.mkdir(parents=True, exist_ok=True)

    if not args.summarize_only:
        manifest = sweep.machine_manifest(
            args.binary,
            {"levels": levels, "cpus": cpus, "workloads": selected,
             "dropped_levels_above_cpus": dropped},
        )
        manifest["cpu_count"] = cpus
        (out / "manifest.json").write_text(json.dumps(manifest, indent=2))
        if dropped:
            print(f"note: dropped levels above cpu count {cpus}: {dropped}")
        for w in selected:
            spec = WORKLOADS[w]
            for level in levels:
                print(f"  {w} (class {spec['class']}) concurrency={level} ...", flush=True)
                run_point(args.binary, w, spec, level, args.runs, out, args.cooldown)

    # summarize + inflation ratios
    summary_rows = []
    inflation = {}
    for w in selected:
        spec = WORKLOADS[w]
        base = None
        per_level = {}
        moved_by_level = {}
        for level in levels:
            run_files = [
                out / w / f"concurrency={level}" / f"run-{i}.jsonl"
                for i in range(args.runs)
            ]
            run_files = [rf for rf in run_files if rf.exists()]
            s = summarize_point(w, spec, level, run_files)
            if not s:
                continue
            summary_rows.append(s)
            per_level[level] = s["measured_ns"]
            moved_by_level[level] = s["aggregate_moved_bytes_per_sec"]
            if level == 1:
                base = s["measured_ns"]
        if base:
            inflation[w] = {
                "class": spec["class"],
                "baseline_ns_at_1": base,
                "ratio_by_workers": {
                    str(l): round(v / base, 4) for l, v in sorted(per_level.items())
                },
                "moved_mb_per_sec_by_workers": {
                    str(l): round(v / 1e6, 1) for l, v in sorted(moved_by_level.items())
                },
            }

    with open(out / "summary.jsonl", "w") as f:
        for row in summary_rows:
            f.write(json.dumps(row) + "\n")
    (out / "inflation.json").write_text(json.dumps(inflation, indent=2))

    print("\nInflation vs 1 worker (median per-tx wall-clock):")
    width = max(len(w) for w in inflation) if inflation else 10
    hdr_levels = [l for l in levels]
    print(f"  {'workload':{width}} {'class':7} " + " ".join(f"x{l:<6}" for l in hdr_levels))
    for w, d in inflation.items():
        cells = " ".join(
            f"{d['ratio_by_workers'].get(str(l), float('nan')):<7.3f}" for l in hdr_levels
        )
        print(f"  {w:{width}} {d['class']:7} {cells}")
    print("\nAggregate moved-bytes rate, MB/s (plateau across workers = the bandwidth ceiling B_mem):")
    print(f"  {'workload':{width}} {'class':7} " + " ".join(f"@{l:<7}" for l in hdr_levels))
    for w, d in inflation.items():
        cells = " ".join(
            f"{d['moved_mb_per_sec_by_workers'].get(str(l), float('nan')):<8.1f}"
            for l in hdr_levels
        )
        print(f"  {w:{width}} {d['class']:7} {cells}")
    print(f"\nwrote {out}/summary.jsonl, {out}/inflation.json")


if __name__ == "__main__":
    main()
