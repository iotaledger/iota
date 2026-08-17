#!/usr/bin/env python3
# Copyright (c) 2026 IOTA Stiftung
# SPDX-License-Identifier: Apache-2.0
"""Stage 2 write-side data collection: sustained runs against the real store.

Drives the benchmark's sustained mode (rounds of a write-heavy workload,
committed through the real store with write stalls enabled) and reads the
response variables back out of the per-round statistics and the RocksDB LOG:

  - sustained commit throughput and its trend as the store grows
  - write stalls / stops (the signal `B` is defined against)
  - cumulative compaction bytes vs. user bytes -> write amplification

    ./write_side.py --out DIR --duration 3600 --num-mints 8 --nft-size 1024

Short runs only measure the burst the memtables absorb; compaction steady
state needs hours (the summary reports how much compaction actually ran, so
an undersized run is visible rather than silent).
"""

import argparse
import json
import re
import statistics
import subprocess
import sys
from pathlib import Path

from sweep import REPO_ROOT, machine_manifest

DEFAULT_BINARY = REPO_ROOT / "target/release/calibrate"


def parse_rocksdb_log(log_path: Path):
    """Last cumulative stats + stall events from a RocksDB LOG file."""
    out = {"stall_events": 0, "stop_events": 0}
    if not log_path.exists():
        return out
    cum_write = re.compile(r"Cumulative compaction: ([\d.]+) (KB|MB|GB|TB) write")
    cum_ingest = re.compile(r"Cumulative writes: .* ingest: ([\d.]+) (KB|MB|GB|TB)")
    unit = {"KB": 2**10, "MB": 2**20, "GB": 2**30, "TB": 2**40}
    with open(log_path, errors="replace") as f:
        for line in f:
            m = cum_write.search(line)
            if m:
                out["compaction_write_bytes"] = int(float(m.group(1)) * unit[m.group(2)])
            m = cum_ingest.search(line)
            if m:
                out["ingest_bytes"] = int(float(m.group(1)) * unit[m.group(2)])
            if "Stalling writes" in line:
                out["stall_events"] += 1
            if "Stopping writes" in line:
                out["stop_events"] += 1
    return out


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--out", required=True, type=Path)
    ap.add_argument("--duration", type=int, default=3600,
                    help="seconds of sustained load (steady state needs hours)")
    ap.add_argument("--tx-count", type=int, default=100, help="transactions per round")
    ap.add_argument("--num-mints", type=int, default=8)
    ap.add_argument("--nft-size", type=int, default=1024)
    ap.add_argument("--binary", type=Path, default=DEFAULT_BINARY)
    ap.add_argument("--quick", action="store_true", help="plumbing check: 30 seconds")
    ap.add_argument("--summarize-only", action="store_true")
    args = ap.parse_args()

    if args.quick:
        args.duration = 30
    if not args.binary.exists():
        sys.exit(f"binary not found: {args.binary}\n"
                 f"build it with: cargo build --release -p iota-single-node-benchmark --bin calibrate")

    args.out.mkdir(parents=True, exist_ok=True)
    db_path = args.out / "db"
    stats_path = args.out / "rounds.jsonl"

    if not args.summarize_only:
        if db_path.exists():
            sys.exit(f"refusing to reuse existing store at {db_path}")
        manifest = machine_manifest(args.binary, sys.argv[1:])
        (args.out / "manifest.json").write_text(json.dumps(manifest, indent=2) + "\n")
        cmd = [str(args.binary),
               "--tx-count", str(args.tx_count),
               "--duration-secs", str(args.duration),
               "--db-path", str(db_path),
               "--enable-write-stall",
               "--stats-output", str(stats_path),
               "ptb", "--num-mints", str(args.num_mints), "--nft-size", str(args.nft_size)]
        print(f"running {args.duration}s of sustained load "
              f"({args.tx_count} txs x {args.num_mints} mints x {args.nft_size} B per round)",
              flush=True)
        r = subprocess.run(cmd, capture_output=True, text=True)
        if r.returncode != 0:
            sys.exit(f"benchmark failed:\n{r.stdout[-2000:]}\n{r.stderr[-2000:]}")

    rounds = [json.loads(l) for l in open(stats_path)]
    if not rounds:
        sys.exit("no rounds recorded")

    # User payload bytes per round, from the workload shape.
    user_bytes_per_round = args.tx_count * args.num_mints * args.nft_size
    total_user_bytes = user_bytes_per_round * len(rounds)

    def window(rs):
        secs = rs[-1]["elapsed_secs"] - rs[0]["elapsed_secs"] or 1
        return {
            "rounds": len(rs),
            "user_bytes_per_sec": user_bytes_per_round * len(rs) / secs,
            "commit_ms_median": statistics.median(r["commit_ms"] for r in rs),
            "commit_ms_p95": sorted(r["commit_ms"] for r in rs)[int(0.95 * (len(rs) - 1))],
            "execute_ms_median": statistics.median(r["execute_ms"] for r in rs),
        }

    third = max(len(rounds) // 3, 1)
    log_stats = parse_rocksdb_log(db_path / "store" / "perpetual" / "LOG")
    wa = None
    if log_stats.get("compaction_write_bytes") and total_user_bytes:
        # Total device writes = ingest (WAL+memtable flush) + compaction
        # rewrites; amplification is device writes per user byte.
        wa = (log_stats.get("ingest_bytes", 0) + log_stats["compaction_write_bytes"]) \
            / total_user_bytes

    summary = {
        "duration_secs": rounds[-1]["elapsed_secs"],
        "rounds": len(rounds),
        "txs": sum(r["txs"] for r in rounds),
        "user_bytes_total": total_user_bytes,
        "db_bytes_final": rounds[-1]["db_bytes"],
        "first_third": window(rounds[:third]),
        "last_third": window(rounds[-third:]),
        "rocksdb": log_stats,
        "write_amplification": wa,
    }
    (args.out / "summary.json").write_text(json.dumps(summary, indent=2) + "\n")

    print(json.dumps(summary, indent=2))
    if log_stats["stall_events"] == 0 and log_stats["stop_events"] == 0:
        print("\nNOTE: no write stalls observed — the sustained rate is below the "
              "stall onset at this duration/state size. `B` needs runs long "
              "enough (or rates high enough) to find the onset.", file=sys.stderr)
    if not log_stats.get("compaction_write_bytes"):
        print("NOTE: no cumulative compaction stats in LOG yet — run longer for "
              "write amplification.", file=sys.stderr)


if __name__ == "__main__":
    main()
