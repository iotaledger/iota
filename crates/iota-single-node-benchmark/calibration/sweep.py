#!/usr/bin/env python3
# Copyright (c) 2026 IOTA Stiftung
# SPDX-License-Identifier: Apache-2.0
"""Stage 1 calibration sweeps for the multidimensional gas metering work.

Runs iota-single-node-benchmark over one workload knob at a time, several
runs per point, and collects the per-transaction JSONL profiles written by
--profile-output into a dataset directory:

    <out>/manifest.json                      machine + commit + invocation record
    <out>/<sweep>/<knob>=<value>/run-<i>.jsonl   raw per-transaction rows
    <out>/summary.jsonl                      one row per sweep point (medians)
    <out>/slopes.json                        least-squares slope per sweep

Stdlib only, so it runs unchanged on this development machine and on the
reference machine. Timing data is only meaningful from a release build.
"""

import argparse
import hashlib
import json
import platform
import statistics
import subprocess
import sys
import time
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[3]
DEFAULT_BINARY = REPO_ROOT / "target/release/calibrate"

# One entry per Stage 1 row reachable with today's knobs. `x_field` is the
# profile counter the swept knob is expected to drive; the slope of median
# measured_ns on that counter is the quick-look coefficient estimate.
SWEEPS = {
    "interpreter": {
        "knob": "--computation",
        "values": [0, 25, 50, 100, 150, 200, 250],
        "fixed": [],
        "x_field": "instructions_executed",
    },
    "reads-runtime": {
        "knob": "--num-dynamic-fields",
        "values": [0, 4, 8, 16, 32, 64],
        "fixed": [],
        "x_field": "child_object_reads",
    },
    "reads-input": {
        "knob": "--num-transfers",
        "values": [0, 2, 4, 8, 16, 32],
        "fixed": [],
        "x_field": "input_object_count",
    },
    "writes-count": {
        "knob": "--num-mints",
        "values": [0, 4, 8, 16, 32],
        "fixed": [],
        "x_field": "written_object_count",
    },
    "writes-bytes": {
        "knob": "--nft-size",
        "values": [64, 256, 1024, 4096, 16384],
        "fixed": ["--num-mints", "8"],
        "x_field": "written_bytes",
    },
    # interpreter cost components, de-correlated
    "interpreter-scalar": {
        "knob": "--scalar-ops",
        "values": [1000, 4000, 16000, 64000, 256000],
        "fixed": [],
        "x_field": "instructions_executed",
    },
    "interpreter-push-pop": {
        "knob": "--push-pop-ops",
        "values": [1000, 4000, 16000, 64000, 256000],
        "fixed": [],
        "x_field": "interp_stack_height_flow",
    },
    "interpreter-vector-move": {
        "knob": "--vector-move-ops",
        "values": [64, 256, 1024, 4096],
        "fixed": ["--vector-move-size", "8192"],
        "x_field": "interp_stack_size_flow",
    },
    # working-memory workloads. Building a flat vector byte-by-byte through the
    # interpreter is gas-capped below what process-level memory readings can
    # resolve, so the flat-vector sweep keeps time as its response and the
    # resident-memory slope comes from the struct tree: nodes are cheap to
    # build (a few dozen instructions each) but heavy in real memory, and
    # they live in locals, driving the same counter.
    "memory-locals": {
        "knob": "--locals-bytes",
        "values": [4096, 16384, 65536, 262144],
        "fixed": [],
        "x_field": "locals_size_high_water_mark",
    },
    "memory-tree-width": {
        "knob": "--tree-width",
        "values": [16, 32, 48, 64],
        "fixed": ["--tree-depth", "3"],
        "x_field": "locals_size_high_water_mark",
        "y_field": "rss_delta_bytes",
        "tx_count": 5,
    },
    "child-bytes": {
        "knob": "--dynamic-field-size",
        "values": [0, 256, 1024, 4096],
        "fixed": ["--num-dynamic-fields", "16"],
        "x_field": "child_object_read_bytes",
    },
    # native families (per-call cost; input-size variants sweep the per-byte share)
    "hash-sha2-256": {
        "knob": "--num-hashes",
        "values": [0, 64, 256, 1024],
        "fixed": ["--hash-family", "sha2-256"],
        "x_field": "num_native_calls",
    },
    "hash-blake2b256": {
        "knob": "--num-hashes",
        "values": [0, 64, 256, 1024],
        "fixed": ["--hash-family", "blake2b256"],
        "x_field": "num_native_calls",
    },
    "hash-input-size": {
        "knob": "--hash-input-size",
        "values": [64, 512, 4096, 32768],
        "fixed": ["--num-hashes", "256", "--hash-family", "sha2-256"],
        "x_field": "interp_stack_size_flow",
    },
    "sig-ed25519": {
        "knob": "--num-sig-verifies",
        "values": [0, 8, 32, 128],
        "fixed": ["--sig-scheme", "ed25519"],
        "x_field": "num_native_calls",
        "tx_count": 50,
    },
    "sig-bls-min-sig": {
        "knob": "--num-sig-verifies",
        "values": [0, 4, 8, 16],
        "fixed": ["--sig-scheme", "bls-min-sig"],
        "x_field": "num_native_calls",
        "tx_count": 20,
    },
    "sig-bls-min-pk": {
        "knob": "--num-sig-verifies",
        "values": [0, 4, 8, 16],
        "fixed": ["--sig-scheme", "bls-min-pk"],
        "x_field": "num_native_calls",
        "tx_count": 20,
    },
    "sig-secp256k1": {
        "knob": "--num-sig-verifies",
        "values": [0, 8, 32, 128],
        "fixed": ["--sig-scheme", "secp256k1"],
        "x_field": "num_native_calls",
        "tx_count": 50,
    },
    "sig-secp256r1": {
        "knob": "--num-sig-verifies",
        "values": [0, 8, 32, 128],
        "fixed": ["--sig-scheme", "secp256r1"],
        "x_field": "num_native_calls",
        "tx_count": 50,
    },
    "ecvrf": {
        "knob": "--num-ecvrf-verifies",
        "values": [0, 8, 32, 128],
        "fixed": [],
        "x_field": "num_native_calls",
        "tx_count": 50,
    },
    # The alpha string is passed by reference into the native, so its size
    # shows up in the module's per-byte native gas, not in stack flow.
    "ecvrf-alpha-size": {
        "knob": "--ecvrf-alpha-size",
        "values": [64, 512, 4096, 32768],
        "fixed": ["--num-ecvrf-verifies", "32"],
        "x_field": "native_gas",
        "tx_count": 50,
    },
    "groth16-bls12381": {
        "knob": "--num-groth16-verifies",
        "values": [0, 2, 4, 8],
        "fixed": ["--groth16-curve", "bls12381"],
        "x_field": "num_native_calls",
        "tx_count": 20,
    },
    "groth16-bn254": {
        "knob": "--num-groth16-verifies",
        "values": [0, 2, 4, 8],
        "fixed": ["--groth16-curve", "bn254"],
        "x_field": "num_native_calls",
        "tx_count": 20,
    },
    # poseidon is behind a feature flag enabled on the benchmark's chain
    # (Chain::Unknown) but not on mainnet/testnet today; the coefficient is
    # calibrated ready for the flag flip.
    "poseidon": {
        "knob": "--num-poseidon-hashes",
        "values": [0, 64, 256, 1024],
        "fixed": ["--poseidon-input-count", "4"],
        "x_field": "num_native_calls",
    },
    # the underlying poseidon implementation supports at most 16 inputs
    "poseidon-inputs": {
        "knob": "--poseidon-input-count",
        "values": [1, 4, 8, 16],
        "fixed": ["--num-poseidon-hashes", "128"],
        "x_field": "interp_stack_size_flow",
    },
    # BLS12-381 group operations (the 0x2::group_ops native tag), spanning
    # the per-op cost range: add < scalar mul < hash-to-curve < pairing.
    "group-g1-add": {
        "knob": "--num-group-ops",
        "values": [0, 64, 256, 1024],
        "fixed": ["--group-op", "g1-add"],
        "x_field": "num_native_calls",
    },
    "group-g1-mul": {
        "knob": "--num-group-ops",
        "values": [0, 32, 128, 512],
        "fixed": ["--group-op", "g1-mul"],
        "x_field": "num_native_calls",
        "tx_count": 50,
    },
    "group-hash-to-g1": {
        "knob": "--num-group-ops",
        "values": [0, 32, 128, 512],
        "fixed": ["--group-op", "hash-to-g1"],
        "x_field": "num_native_calls",
        "tx_count": 50,
    },
    "group-pairing": {
        "knob": "--num-group-ops",
        "values": [0, 4, 16, 64],
        "fixed": ["--group-op", "pairing"],
        "x_field": "num_native_calls",
        "tx_count": 20,
    },
    # events and deletions
    "events-count": {
        "knob": "--num-events",
        "values": [0, 32, 128, 512],
        "fixed": ["--event-size", "64"],
        "x_field": "event_count",
    },
    "events-bytes": {
        "knob": "--event-size",
        "values": [32, 256, 1024, 4096],
        "fixed": ["--num-events", "64"],
        "x_field": "event_bytes",
    },
    "deletes": {
        "knob": "--num-deletes",
        "values": [0, 8, 32, 64],
        "fixed": ["--num-dynamic-fields", "64"],
        "x_field": "deleted_object_count",
    },
    # tie-breaking sweeps 
    "mutations": {
        "knob": "--num-mutations",
        "values": [0, 4, 16, 32],
        "fixed": [],
        "x_field": "written_object_count",
    },
    "burns": {
        "knob": "--num-burns",
        "values": [0, 4, 16, 32],
        "fixed": [],
        "x_field": "deleted_object_count",
    },
    "reads-input-native": {
        "knob": "--num-transfers",
        "values": [0, 2, 8, 32],
        "fixed": ["--use-native-transfer"],
        "x_field": "input_object_count",
    },
    "packages-count": {
        "knob": "--num-packages-called",
        "values": [1, 2, 4, 8],
        "fixed": ["--generated-package-count", "8", "--generated-package-bytes", "4096"],
        "x_field": "packages_loaded",
    },
    "packages-bytes": {
        "knob": "--generated-package-bytes",
        "values": [512, 2048, 8192, 32768],
        "fixed": ["--num-packages-called", "4", "--generated-package-count", "4"],
        "x_field": "package_bytes_loaded",
    },
    # separability sweeps: pushes-per-instruction 2.85 vs 0.60
    "push-high": {
        "knob": "--high-push-ops",
        "values": [1000, 4000, 16000, 64000],
        "fixed": [],
        "x_field": "interp_stack_height_flow",
    },
    "push-low": {
        "knob": "--low-push-ops",
        "values": [1000, 4000, 16000, 64000],
        "fixed": [],
        "x_field": "interp_instruction_count",
    },
}

# Every numeric profile field is summarized per sweep point (median over
# transactions); non-numeric fields (e.g. the per-module native gas map) are
# skipped.


def run_cmd(cmd, **kwargs):
    return subprocess.run(cmd, capture_output=True, text=True, **kwargs)


def machine_manifest(binary: Path, argv):
    uname = platform.uname()
    manifest = {
        "unix_time_secs": int(time.time()),
        "argv": argv,
        "platform": {
            "system": uname.system,
            "release": uname.release,
            "version": uname.version,
            "machine": uname.machine,
            "python": platform.python_version(),
        },
    }
    if uname.system == "Darwin":
        for key in ("machdep.cpu.brand_string", "hw.memsize", "hw.ncpu"):
            r = run_cmd(["sysctl", "-n", key])
            if r.returncode == 0:
                manifest["platform"][key] = r.stdout.strip()
    elif uname.system == "Linux":
        try:
            cpuinfo = Path("/proc/cpuinfo").read_text()
            model = [l for l in cpuinfo.splitlines() if l.startswith("model name")]
            if model:
                manifest["platform"]["cpu_model"] = model[0].split(":", 1)[1].strip()
            meminfo = Path("/proc/meminfo").read_text().splitlines()[0]
            manifest["platform"]["mem_total"] = meminfo.split(":", 1)[1].strip()
            manifest["platform"]["nproc"] = len(
                [l for l in cpuinfo.splitlines() if l.startswith("processor")]
            )
            # The reference-machine controls the plan requires (fixed governor,
            # turbo/boost state, SMT) are recorded rather than assumed.
            state_files = {
                "scaling_governors": "/sys/devices/system/cpu/cpu*/cpufreq/scaling_governor",
                "smt_control": "/sys/devices/system/cpu/smt/control",
                "intel_no_turbo": "/sys/devices/system/cpu/intel_pstate/no_turbo",
                "cpufreq_boost": "/sys/devices/system/cpu/cpufreq/boost",
                "transparent_hugepages": "/sys/kernel/mm/transparent_hugepage/enabled",
            }
            import glob
            for key, pattern in state_files.items():
                values = sorted({Path(f).read_text().strip() for f in glob.glob(pattern)})
                if values:
                    manifest["platform"][key] = values if len(values) > 1 else values[0]
        except OSError:
            pass

    git = {}
    for name, cmd in [
        ("commit", ["git", "rev-parse", "HEAD"]),
        ("branch", ["git", "rev-parse", "--abbrev-ref", "HEAD"]),
        ("status", ["git", "status", "--porcelain"]),
    ]:
        r = run_cmd(cmd, cwd=REPO_ROOT)
        if r.returncode == 0:
            git[name] = r.stdout.strip()
    git["dirty"] = bool(git.get("status"))
    git.pop("status", None)
    manifest["git"] = git

    r = run_cmd(["rustc", "--version"])
    if r.returncode == 0:
        manifest["rustc"] = r.stdout.strip()

    manifest["binary"] = {
        "path": str(binary),
        "sha256_16": hashlib.sha256(binary.read_bytes()).hexdigest()[:16],
        "is_release_path": "release" in binary.parts,
    }
    return manifest


def run_point(binary, args, sweep_name, spec, value, out_dir):
    point_dir = out_dir / sweep_name / f"{spec['knob'].lstrip('-')}={value}"
    point_dir.mkdir(parents=True, exist_ok=True)
    run_files = []
    for i in range(args.runs):
        run_file = point_dir / f"run-{i}.jsonl"
        run_files.append(run_file)
        if run_file.exists() and run_file.stat().st_size > 0:
            continue  # resume support
        cmd = [
            str(binary),
            "--tx-count", str(spec.get("tx_count", args.tx_count)),
            "--profile-output", str(run_file),
            "--rss-output", str(run_file.with_suffix(".rss.json")),
            "ptb",
            spec["knob"], str(value),
            *spec["fixed"],
        ]
        r = run_cmd(cmd)
        if r.returncode != 0:
            run_file.unlink(missing_ok=True)
            sys.exit(
                f"benchmark failed for {sweep_name} {spec['knob']}={value} run {i}:\n"
                f"{r.stdout[-2000:]}\n{r.stderr[-2000:]}"
            )
        if args.cooldown > 0:
            time.sleep(args.cooldown)
    return run_files


def load_rows(run_file):
    rows = []
    with open(run_file) as f:
        for line in f:
            row = json.loads(line)
            if "meta" in row:
                continue
            rows.append(row)
    return rows


def summarize_point(sweep_name, spec, value, run_files):
    per_run_medians = []
    pooled_ns = []
    field_values = {}
    n_txs = 0
    for rf in run_files:
        rows = load_rows(rf)
        if not rows:
            continue
        n_txs += len(rows)
        ns = [r["measured_ns"] for r in rows]
        pooled_ns.extend(ns)
        per_run_medians.append(statistics.median(ns))
        for r in rows:
            for f, v in r["profile"].items():
                if isinstance(v, (int, float)) and not isinstance(v, bool):
                    field_values.setdefault(f, []).append(v)
    if not per_run_medians:
        return None
    rss_deltas = []
    rss_peak_before_phase = 0
    for rf in run_files:
        rss_file = rf.with_suffix(".rss.json")
        if rss_file.exists():
            rss = json.loads(rss_file.read_text())
            rss_deltas.append(rss["delta_bytes"])
            rss_peak_before_phase += bool(rss.get("peak_before_phase"))
    pooled_ns.sort()
    summary = {
        "sweep": sweep_name,
        "knob": spec["knob"].lstrip("-"),
        "value": value,
        "n_runs": len(run_files),
        "n_txs": n_txs,
        # the point estimate: median of per-run medians
        "measured_ns": statistics.median(per_run_medians),
        "measured_ns_p10": pooled_ns[int(0.10 * (len(pooled_ns) - 1))],
        "measured_ns_p90": pooled_ns[int(0.90 * (len(pooled_ns) - 1))],
    }
    if rss_deltas:
        summary["rss_delta_bytes"] = statistics.median(rss_deltas)
        # A peak that predates the measured phase means setup dominated the
        # footprint: raise the workload's memory knob.
        summary["rss_peak_before_phase_runs"] = rss_peak_before_phase
    for f, vals in field_values.items():
        if vals:
            summary[f] = statistics.median(vals)
    return summary


def fit_slope(points, x_field, y_field="measured_ns"):
    """Least-squares slope of y_field on x_field over sweep points."""
    xy = [
        (p.get(x_field), p[y_field])
        for p in points
        if p.get(x_field) is not None and p.get(y_field) is not None
    ]
    if len(xy) < 2:
        return None
    xs, ys = zip(*xy)
    n = len(xs)
    mx, my = sum(xs) / n, sum(ys) / n
    sxx = sum((x - mx) ** 2 for x in xs)
    if sxx == 0:
        return None
    slope = sum((x - mx) * (y - my) for x, y in xy) / sxx
    intercept = my - slope * mx
    ss_res = sum((y - (intercept + slope * x)) ** 2 for x, y in xy)
    ss_tot = sum((y - my) ** 2 for y in ys)
    r2 = 1.0 - ss_res / ss_tot if ss_tot > 0 else None
    return {
        "x_field": x_field,
        "ns_per_unit": slope,
        "intercept_ns": intercept,
        "r_squared": r2,
        "n_points": n,
    }


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--out", required=True, type=Path, help="dataset directory")
    ap.add_argument("--sweeps", default=",".join(SWEEPS),
                    help=f"comma-separated subset of: {', '.join(SWEEPS)}")
    ap.add_argument("--runs", type=int, default=5, help="runs per sweep point")
    ap.add_argument("--tx-count", type=int, default=100, help="transactions per run")
    ap.add_argument("--binary", type=Path, default=DEFAULT_BINARY)
    ap.add_argument("--cooldown", type=float, default=1.0,
                    help="seconds to sleep between runs (thermal settling)")
    ap.add_argument("--quick", action="store_true",
                    help="plumbing check: 3 values, 2 runs, 20 txs")
    ap.add_argument("--summarize-only", action="store_true",
                    help="recompute summary/slopes from existing run files")
    args = ap.parse_args()

    if args.quick:
        args.runs, args.tx_count = 2, 20

    if not args.binary.exists():
        sys.exit(f"benchmark binary not found: {args.binary}\n"
                 f"build it with: cargo build --release -p iota-single-node-benchmark --bin calibrate")

    selected = [s.strip() for s in args.sweeps.split(",") if s.strip()]
    unknown = [s for s in selected if s not in SWEEPS]
    if unknown:
        sys.exit(f"unknown sweep(s): {unknown}; available: {list(SWEEPS)}")

    args.out.mkdir(parents=True, exist_ok=True)
    if not args.summarize_only:
        manifest = machine_manifest(args.binary, sys.argv[1:])
        (args.out / "manifest.json").write_text(json.dumps(manifest, indent=2) + "\n")
        if not manifest["binary"]["is_release_path"]:
            print("WARNING: not a release-path binary; timings are not usable "
                  "for calibration", file=sys.stderr)

    summaries = []
    slopes = {}
    for sweep_name in selected:
        spec = SWEEPS[sweep_name]
        values = spec["values"][:3] if args.quick else spec["values"]
        points = []
        for value in values:
            if args.summarize_only:
                point_dir = args.out / sweep_name / f"{spec['knob'].lstrip('-')}={value}"
                run_files = sorted(point_dir.glob("run-*.jsonl"))
            else:
                print(f"[{sweep_name}] {spec['knob']}={value} "
                      f"({args.runs} runs x {args.tx_count} txs)", flush=True)
                run_files = run_point(args.binary, args, sweep_name, spec, value, args.out)
            point = summarize_point(sweep_name, spec, value, run_files)
            if point:
                points.append(point)
        summaries.extend(points)
        y_field = spec.get("y_field", "measured_ns")
        slope = fit_slope(points, spec["x_field"], y_field)
        if slope:
            slope["y_field"] = y_field
            slopes[sweep_name] = slope
            unit = "ns" if y_field == "measured_ns" else y_field.replace("_bytes", " bytes")
            print(f"[{sweep_name}] slope: {slope['ns_per_unit']:.3f} {unit} per "
                  f"{spec['x_field']} (r²={slope['r_squared']:.4f})")

    with open(args.out / "summary.jsonl", "w") as f:
        for s in summaries:
            f.write(json.dumps(s) + "\n")
    (args.out / "slopes.json").write_text(json.dumps(slopes, indent=2) + "\n")
    print(f"dataset written to {args.out}")


if __name__ == "__main__":
    main()
