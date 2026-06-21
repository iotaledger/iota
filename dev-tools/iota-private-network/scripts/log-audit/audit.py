#!/usr/bin/env python3
"""Double-spend safety audit for IOTA white-flag conflict resolution.

Usage:
    python3 audit.py <logs_dir> [--include-fullnode] [--json out.json]

Auto-discovers files in <logs_dir> by name:
    validator-*.log         → validator parser
    fullnode-*.log          → fullnode parser (skipped unless --include-fullnode
                              or the dir contains no stress-benchmark.log)
    stress-benchmark.log    → stress parser

Exit codes:
    0 = PASS         — checks ran on real signal and found no safety violation
    1 = FAIL         — a safety violation (e.g. a loser that also executed)
    2 = INCONCLUSIVE — coverage check failed: the parser matched none of a
                       signal that must be present, so nothing was verified
                       (commonly the node log format drifted from the parsers)
"""

from __future__ import annotations

import argparse
import glob
import json
import os
import sys
import threading
import time
from dataclasses import asdict
from typing import List

import checks
import parsers


class Watchdog:
    """Background thread that prints HANG warnings when no progress has been
    reported for `timeout_s` seconds. Call `tick()` from the producer's
    progress callback to mark progress.
    """

    def __init__(self, timeout_s: float, label: str = ""):
        self.timeout_s = timeout_s
        self.label = label
        self.last_tick = time.time()
        self.stop_event = threading.Event()
        self.lock = threading.Lock()
        self.thread = threading.Thread(target=self._run, daemon=True)

    def __enter__(self):
        self.last_tick = time.time()
        self.thread.start()
        return self

    def __exit__(self, *exc):
        self.stop_event.set()

    def tick(self):
        with self.lock:
            self.last_tick = time.time()

    def _run(self):
        check_interval = max(self.timeout_s / 4.0, 1.0)
        while not self.stop_event.wait(check_interval):
            with self.lock:
                idle = time.time() - self.last_tick
            if idle > self.timeout_s:
                print(
                    f"  !! WATCHDOG: no progress on {self.label} for "
                    f"{idle:.0f}s — parser may be hung",
                    flush=True,
                )


def _discover(logs_dir: str):
    validators = sorted(glob.glob(os.path.join(logs_dir, "validator-*.log")))
    fullnodes = sorted(glob.glob(os.path.join(logs_dir, "fullnode-*.log")))
    stress = sorted(glob.glob(os.path.join(logs_dir, "stress*.log")))
    return validators, fullnodes, stress


def _validator_name(path: str) -> str:
    return os.path.basename(path).removesuffix(".log")


def _human(n: int) -> str:
    if n >= 1_000_000:
        return f"{n / 1_000_000:.1f}M"
    if n >= 1_000:
        return f"{n / 1_000:.1f}k"
    return str(n)


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("logs_dir", help="Directory containing the log files")
    ap.add_argument(
        "--include-fullnode",
        action="store_true",
        help="Parse fullnode-*.log files (large; opt-in)",
    )
    ap.add_argument(
        "--json",
        dest="json_out",
        default=None,
        help="Write machine-readable report to this path",
    )
    ap.add_argument(
        "--watchdog",
        type=float,
        default=30.0,
        help="Seconds without progress before printing a HANG warning",
    )
    ap.add_argument(
        "--max-fullnode-lines",
        type=int,
        default=0,
        help="Stop fullnode parsing after N lines (0 = no limit). For debugging.",
    )
    args = ap.parse_args()

    validator_paths, fullnode_paths, stress_paths = _discover(args.logs_dir)

    if not validator_paths:
        print(f"no validator-*.log files in {args.logs_dir}", file=sys.stderr)
        return 2

    print(f"=== Double-Spend Conflict Audit ===")
    print(f"logs dir: {args.logs_dir}")
    print(f"validators: {len(validator_paths)}")
    print(f"fullnodes:  {len(fullnode_paths)} "
          f"({'parsing' if args.include_fullnode else 'skipped (use --include-fullnode)'})")
    print(f"stress:     {len(stress_paths)}")
    print()

    # ---- Validator pass --------------------------------------------------
    validator_events: list = []
    for path in validator_paths:
        v = _validator_name(path)
        t0 = time.time()
        before = len(validator_events)

        with Watchdog(args.watchdog, label=v) as wd:
            def _v_progress(line_no, _t0=t0, _v=v, _wd=wd):
                _wd.tick()
                rate = line_no / max(time.time() - _t0, 1e-3)
                print(
                    f"    {_v}: {_human(line_no)} lines ({_human(int(rate))}/s)",
                    flush=True,
                )

            for ev in parsers.parse_validator_log(
                path, v, progress_cb=_v_progress
            ):
                validator_events.append(ev)

        elapsed = time.time() - t0
        print(
            f"  {v}: {_human(len(validator_events) - before)} events "
            f"in {elapsed:.1f}s"
        )

    # ---- Fullnode pass (optional) ----------------------------------------
    fn_submissions: set = set()
    fn_final_failures: dict = {}
    fn_executed: dict = {}

    if args.include_fullnode and fullnode_paths:
        for path in fullnode_paths:
            t0 = time.time()
            n_sub = n_fail = n_exec = 0
            name = os.path.basename(path)

            with Watchdog(args.watchdog, label=name) as wd:
                def _fn_progress(
                    line_no, ns, nf, ne, _t0=t0, _name=name, _wd=wd
                ):
                    _wd.tick()
                    elapsed = time.time() - _t0
                    rate = line_no / max(elapsed, 1e-3)
                    print(
                        f"    {_name}: {_human(line_no)} lines "
                        f"({_human(int(rate))}/s, {elapsed:.0f}s elapsed) "
                        f"sub={_human(ns)} fail={_human(nf)} exec={_human(ne)}",
                        flush=True,
                    )

                for ev in parsers.parse_fullnode_log(
                    path,
                    progress_cb=_fn_progress,
                    max_lines=args.max_fullnode_lines,
                ):
                    if isinstance(ev, parsers.FnSubmissionSeen):
                        fn_submissions.add(ev.digest)
                        n_sub += 1
                    elif isinstance(ev, parsers.FnFinalFailure):
                        fn_final_failures[ev.digest] = ev.reason
                        n_fail += 1
                    elif isinstance(ev, parsers.FnEffectsExecuted):
                        fn_executed[ev.digest] = ev.effects_digest
                        n_exec += 1

            elapsed = time.time() - t0
            print(
                f"  {name}: "
                f"submissions={_human(n_sub)} "
                f"final_failures={_human(n_fail)} "
                f"executed={_human(n_exec)} "
                f"in {elapsed:.1f}s"
            )

    # ---- Stress pass (optional, informational only) ----------------------
    stress_gave_up = 0
    stress_expected_failures = 0
    double_spend_attempts: list = []
    # Digests the stress client saw rejected at submission (pre-consensus), so
    # their absence from validator post-consensus logs is expected (Check F).
    pre_consensus_rejected: set = set()
    for path in stress_paths:
        t0 = time.time()
        n_attempts = 0
        n_ds_submits = 0
        n_gave_up = 0
        n_expected = 0
        name = os.path.basename(path)

        with Watchdog(args.watchdog, label=name) as wd:
            def _s_progress(line_no, _t0=t0, _name=name, _wd=wd):
                _wd.tick()
                elapsed = time.time() - _t0
                rate = line_no / max(elapsed, 1e-3)
                print(
                    f"    {_name}: {_human(line_no)} lines "
                    f"({_human(int(rate))}/s, {elapsed:.0f}s elapsed)",
                    flush=True,
                )

            for ev in parsers.parse_stress_log(path, progress_cb=_s_progress):
                if isinstance(ev, parsers.StressAttempt):
                    n_attempts += 1
                    if checks.is_pre_consensus_rejection(ev.err):
                        pre_consensus_rejected.add(ev.digest)
                elif isinstance(ev, parsers.StressGaveUp):
                    n_gave_up += 1
                elif isinstance(ev, parsers.StressExpectedFailure):
                    n_expected += 1
                elif isinstance(ev, parsers.DoubleSpendAttempt):
                    double_spend_attempts.append(ev)
                    n_ds_submits += 1

        stress_gave_up += n_gave_up
        stress_expected_failures += n_expected
        elapsed = time.time() - t0
        print(
            f"  {name}: "
            f"attempts={_human(n_attempts)} "
            f"gave_up={n_gave_up} "
            f"expected_failures={n_expected} "
            f"double_spend_submits={_human(n_ds_submits)} "
            f"in {elapsed:.1f}s"
        )

    print()

    # ---- Run checks ------------------------------------------------------
    results: List[checks.CheckResult] = []
    coverage = checks.check_coverage(
        validator_events,
        num_validator_logs=len(validator_paths),
        num_stress_logs=len(stress_paths),
        num_double_spend_attempts=len(double_spend_attempts),
        fullnode_enabled=bool(args.include_fullnode and fullnode_paths),
        num_fn_submissions=len(fn_submissions),
    )
    results.append(coverage)
    results.append(checks.check_single_winner_per_input(validator_events))
    results.append(checks.check_cross_validator_agreement(validator_events))
    results.append(checks.check_losers_never_executed(validator_events))
    results.append(checks.check_batch_counts(validator_events))
    if args.include_fullnode and fullnode_paths:
        results.append(
            checks.check_stress_consistency(
                validator_events,
                fn_submissions,
                fn_final_failures,
                fn_executed,
            )
        )
    if double_spend_attempts:
        # The fullnode's terminal-failure reason is a second source of
        # pre-consensus rejections (e.g. stale input object) when available.
        for digest, reason in fn_final_failures.items():
            if checks.is_pre_consensus_rejection(reason):
                pre_consensus_rejected.add(digest)
        results.append(
            checks.check_double_spend_pairs(
                double_spend_attempts,
                validator_events,
                pre_consensus_rejected,
            )
        )

    # ---- Render summary --------------------------------------------------
    print("Check results")
    print("-------------")
    for r in results:
        n_fail = sum(1 for a in r.anomalies if a.severity == "FAIL")
        n_warn = sum(1 for a in r.anomalies if a.severity == "WARN")
        if n_fail:
            status = "FAIL"
        elif n_warn:
            status = "WARN"
        else:
            status = "PASS"
        print(
            f"  [{r.name}] {r.description:<40s} "
            f"{status} ({_human(r.items_checked)} items, "
            f"{n_fail} fail / {n_warn} warn)"
        )

    # The coverage check (name "0") failing means the parser extracted none of
    # a signal that must be present — we verified nothing, so we must NOT report
    # PASS. Distinguish that (INCONCLUSIVE, exit 2) from a genuine safety
    # violation in A/B/C/E (FAIL, exit 1).
    coverage_failed = not coverage.passed
    safety_fail = any(
        a.severity == "FAIL"
        for r in results
        if r is not coverage
        for a in r.anomalies
    )

    print()
    if coverage_failed:
        print(
            "OVERALL: INCONCLUSIVE — parser coverage check FAILED; cannot "
            "certify safety (see check [0] below)"
        )
    elif safety_fail:
        print("OVERALL: FAIL — anomalies detail below")
    else:
        print("OVERALL: PASS — no double-spend leaked")

    print()

    # ---- Detail any anomalies --------------------------------------------
    any_anomaly = False
    for r in results:
        if not r.anomalies:
            continue
        any_anomaly = True
        print(f"-- Check {r.name}: {r.description} ({len(r.anomalies)} anomalies) --")
        MAX_SHOW = 20
        for a in r.anomalies[:MAX_SHOW]:
            print(f"  [{a.severity}] {a.message}")
            for k, v in a.evidence.items():
                if isinstance(v, list) and len(v) > 6:
                    v = v[:6] + [f"... ({len(v)} total)"]
                print(f"        {k}: {v}")
        if len(r.anomalies) > MAX_SHOW:
            print(f"  ... ({len(r.anomalies) - MAX_SHOW} more; see JSON for full list)")
        print()

    if not any_anomaly:
        print("(no anomalies)")

    # ---- Machine-readable output -----------------------------------------
    if args.json_out:
        json_doc = {
            "logs_dir": args.logs_dir,
            "validators": [_validator_name(p) for p in validator_paths],
            "include_fullnode": args.include_fullnode,
            "fn_submissions": len(fn_submissions),
            "fn_final_failures": len(fn_final_failures),
            "fn_executed": len(fn_executed),
            "double_spend_submits": len(double_spend_attempts),
            "overall_status": (
                "INCONCLUSIVE"
                if coverage_failed
                else "FAIL"
                if safety_fail
                else "PASS"
            ),
            "overall_pass": not (coverage_failed or safety_fail),
            "checks": [
                {
                    "name": r.name,
                    "description": r.description,
                    "items_checked": r.items_checked,
                    "passed": r.passed,
                    "anomalies": [asdict(a) for a in r.anomalies],
                }
                for r in results
            ],
        }
        with open(args.json_out, "w") as f:
            json.dump(json_doc, f, indent=2, default=str)
        print(f"\nwrote {args.json_out}")

    if coverage_failed:
        return 2
    return 1 if safety_fail else 0


if __name__ == "__main__":
    sys.exit(main())
