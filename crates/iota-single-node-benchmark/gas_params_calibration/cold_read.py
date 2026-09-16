#!/usr/bin/env python3
# Copyright (c) 2026 IOTA Stiftung
# SPDX-License-Identifier: Apache-2.0
"""Cold-read data collection for gas-metering calibration.

Drives the standalone `cold_read_bench` binary: per object size, populate a
store once, then run fresh-process measure passes (optionally dropping the OS
page cache in between). Each measure pass reads the same sample cold then
warm, so every run carries its own contrast.

The cold coefficient composes with the integrated sweeps: sweep.py measures
the warm in-execution read cost; this rig measures the cold-minus-warm fetch
delta underneath it.

    ./cold_read.py --out DIR --purge-cmd "sudo purge"      # macOS
    ./cold_read.py --out DIR --purge-cmd "sync && echo 3 | sudo tee /proc/sys/vm/drop_caches"  # Linux

Without --purge-cmd only the RocksDB block cache and the process are cold;
the summary flags when the cold/warm ratio suggests the page cache stayed
warm.
"""

import argparse
import json
import os
import statistics
import subprocess
import sys
import time
from pathlib import Path

from sweep import REPO_ROOT, machine_manifest

DEFAULT_BINARY = REPO_ROOT / "target/release/cold_read_bench"
OBJECT_SIZES = [256, 1024, 4096, 16384]
# Rough per-object store overhead used to size the object count for a target
# database size.
PER_OBJECT_OVERHEAD = 200


def run(cmd, block_cache_mb=None, **kw):
    env = os.environ.copy()
    if block_cache_mb is not None:
        # The objects table's block cache defaults to 5 GiB — larger than the
        # stores this rig builds. A cold read's cost does not depend on the
        # size of the cache it missed, so the cache is pinned small to keep
        # misses missing across the whole sample.
        env["OBJECTS_BLOCK_CACHE_MB"] = str(block_cache_mb)
    r = subprocess.run(cmd, capture_output=True, text=True, env=env, **kw)
    if r.returncode != 0:
        sys.exit(f"command failed: {' '.join(map(str, cmd))}\n{r.stdout[-2000:]}\n{r.stderr[-2000:]}")
    return r


def load_rows(path):
    rows = []
    with open(path) as f:
        for line in f:
            row = json.loads(line)
            if "meta" not in row:
                rows.append(row)
    return rows


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--out", required=True, type=Path)
    ap.add_argument("--sizes", default=",".join(map(str, OBJECT_SIZES)))
    ap.add_argument("--db-bytes", type=int, default=2 * 1024**3,
                    help="target store size per object-size point; must exceed "
                         "--block-cache-mb for cold reads to be real")
    ap.add_argument("--runs", type=int, default=3)
    ap.add_argument("--sample", type=int, default=1000)
    ap.add_argument("--seed", type=int, default=42)
    ap.add_argument("--purge-cmd", default=None,
                    help="shell command run before each measure pass to drop the OS page cache")
    ap.add_argument("--block-cache-mb", type=int, default=128,
                    help="objects block cache for the measure process (default 5120 in "
                         "production; pinned small here so the store exceeds it)")
    ap.add_argument("--binary", type=Path, default=DEFAULT_BINARY)
    ap.add_argument("--quick", action="store_true",
                    help="plumbing check: 2 sizes, 64 MiB stores, 1 run, 200 reads")
    ap.add_argument("--summarize-only", action="store_true")
    args = ap.parse_args()

    sizes = [int(s) for s in args.sizes.split(",")]
    if args.quick:
        sizes = sizes[:2]
        args.db_bytes, args.runs, args.sample = 64 * 1024**2, 1, 200

    if not args.binary.exists():
        sys.exit(f"binary not found: {args.binary}\n"
                 f"build it with: cargo build --release -p iota-single-node-benchmark --bin cold_read_bench")

    args.out.mkdir(parents=True, exist_ok=True)
    if not args.summarize_only:
        manifest = machine_manifest(args.binary, sys.argv[1:])
        manifest["purge_cmd"] = args.purge_cmd
        manifest["block_cache_mb"] = args.block_cache_mb
        (args.out / "manifest.json").write_text(json.dumps(manifest, indent=2) + "\n")
        if not args.purge_cmd:
            print("WARNING: no --purge-cmd; the OS page cache stays warm, so 'cold' "
                  "means only a fresh process and cold block cache", file=sys.stderr)

    summary = []
    for size in sizes:
        num_objects = args.db_bytes // (size + PER_OBJECT_OVERHEAD)
        db_path = args.out / "dbs" / f"size={size}"
        run_dir = args.out / f"size={size}"
        run_dir.mkdir(parents=True, exist_ok=True)
        with_packages = size == sizes[0]

        if not args.summarize_only and not db_path.exists():
            print(f"[size={size}] populating {num_objects} objects "
                  f"(~{args.db_bytes >> 20} MiB)", flush=True)
            cmd = [str(args.binary), "populate", "--db-path", str(db_path),
                   "--num-objects", str(num_objects), "--object-size", str(size),
                   "--seed", str(args.seed)]
            if with_packages:
                cmd.append("--with-framework-packages")
            run(cmd, block_cache_mb=args.block_cache_mb)

        run_files = []
        for i in range(args.runs):
            out_file = run_dir / f"run-{i}.jsonl"
            run_files.append(out_file)
            if args.summarize_only or (out_file.exists() and out_file.stat().st_size > 0):
                continue
            if args.purge_cmd:
                subprocess.run(args.purge_cmd, shell=True, check=True)
                time.sleep(1.0)
            print(f"[size={size}] measure run {i}", flush=True)
            cmd = [str(args.binary), "measure", "--db-path", str(db_path),
                   "--num-objects", str(num_objects), "--seed", str(args.seed),
                   "--sample", str(args.sample), "--out", str(out_file)]
            if with_packages:
                cmd.append("--packages")
            run(cmd, block_cache_mb=args.block_cache_mb)

        by_pass = {"cold": [], "warm": []}
        constructs = []
        pkg_rows = []
        for rf in run_files:
            if not rf.exists():
                continue
            for row in load_rows(rf):
                if row["kind"] == "object":
                    by_pass[row["pass"]].append(row["fetch_ns"])
                    if row["pass"] == "cold":
                        constructs.append(row["construct_ns"])
                else:
                    pkg_rows.append(row)
        if not by_pass["cold"]:
            continue
        cold, warm = statistics.median(by_pass["cold"]), statistics.median(by_pass["warm"])
        point = {
            "object_size": size,
            "n_reads": len(by_pass["cold"]),
            "cold_fetch_ns": cold,
            "warm_fetch_ns": warm,
            "cold_construct_ns": statistics.median(constructs),
            "cold_warm_ratio": round(cold / warm, 2) if warm else None,
        }
        summary.append(point)
        flag = "" if warm == 0 or cold / warm >= 2 else "  <-- page cache likely warm"
        print(f"[size={size}] cold {cold:.0f} ns vs warm {warm:.0f} ns "
              f"(x{point['cold_warm_ratio']}){flag}")
        for row in pkg_rows:
            if row["pass"] == "cold":
                print(f"[size={size}] package {row['object_id'][:10]}…: "
                      f"fetch {row['fetch_ns']} ns, deserialize "
                      f"{row['modules']} modules {row['deserialize_ns']} ns")

    # ns per byte across sizes, from cold medians
    if len(summary) >= 2:
        xs = [p["object_size"] for p in summary]
        ys = [p["cold_fetch_ns"] for p in summary]
        n, mx, my = len(xs), sum(xs) / len(xs), sum(ys) / len(ys)
        sxx = sum((x - mx) ** 2 for x in xs)
        slope = sum((x - mx) * (y - my) for x, y in zip(xs, ys)) / sxx
        print(f"cold fetch: {slope:.3f} ns/byte, intercept "
              f"{my - slope * mx:.0f} ns/object")

    (args.out / "summary.json").write_text(json.dumps(summary, indent=2) + "\n")
    print(f"dataset written to {args.out}")


if __name__ == "__main__":
    main()
