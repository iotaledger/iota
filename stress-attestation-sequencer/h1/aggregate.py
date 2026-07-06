#!/usr/bin/env python3
"""Aggregate H1 raw timeseries across multiple runs into a V1-vs-V2 summary.

Percentiles are computed by POOLING the raw histogram buckets across runs — the
statistically correct way to combine percentiles (you cannot average per-run
quantiles). Throughput and CPU are averaged across runs. With a single run per
group, the result matches the per-run scalar summary.

Usage: h1-aggregate.py <results_dir> [out.md]
  Scans <results_dir>/*/run-a-v1-timeseries.json   (V1, attestation OFF)
    and <results_dir>/*/run-b-v2-timeseries.json   (V2, attestation ON)
"""

import glob
import json
import math
import os
import sys

# (raw histogram base name, display label)
LATENCY_METRICS = [
    ("validator_attestation_latency", "validator_attestation_latency (s)"),
    (
        "transaction_driver_settlement_finality_latency",
        "settlement_finality_latency (s)",
    ),
    ("transaction_driver_submit_transaction_latency", "submit_transaction_latency (s)"),
    (
        "validator_transaction_execution_latency",
        "validator_transaction_execution_latency (s)",
    ),
    (
        "authority_state_internal_execution_latency",
        "internal_execution_latency — real VM (s)",
    ),
    (
        "checkpoint_creation_latency",
        "checkpoint_creation_latency — commit→built (s)",
    ),
]

# Safety counters that MUST stay 0 — the H4 (safety) pass/fail signals. Any
# non-zero one fails H4. Not plotted; summary.md's H4 section lists the offenders.
# (metric, display label)
SAFETY_COUNTERS = [
    ("validator_attestation_task_panics", "attestation task panics"),
    ("split_brain_checkpoint_forks", "split-brain checkpoint forks"),
    ("remote_checkpoint_forks", "remote checkpoint forks"),
    ("global_state_hash_inconsistent_state", "inconsistent state hash"),
    ("total_client_double_spend_attempts_detected", "double-spend attempts detected"),
    (
        "validator_service_num_rejected_tx_soft_lock_conflict",
        "soft-lock conflicts (equivocation)",
    ),
]


def delta(values):
    """Increment of a cumulative counter series over its window (last - first)."""
    if not values:
        return 0.0
    first, last = float(values[0][1]), float(values[-1][1])
    d = last - first
    return d if d >= 0 else last  # counter reset within the window: fall back to last


def series_max(series_per_run, metric):
    """Max value of a metric across all series and runs — a robust 'ever non-zero'
    test for safety counters (counters: final count; gauges: transient flip to 1)."""
    m = 0.0
    for s in series_per_run:
        for x in s.get(metric, []):
            for _, v in x.get("values", []):
                try:
                    m = max(m, float(v))
                except (TypeError, ValueError):
                    pass
    return m


def pooled_buckets(series_per_run, base):
    """Sum per-`le` bucket increments across hosts AND runs -> {le: count}.

    This is the pooled histogram: equivalent to PromQL `sum by (le) (...)` but
    combined over every run, so the quantile is taken on the union of samples.
    """
    acc = {}
    for series in series_per_run:
        for s in series.get(f"{base}_bucket", []):
            le = s.get("metric", {}).get("le")
            if le is None:
                continue
            acc[le] = acc.get(le, 0.0) + delta(s.get("values", []))
    return acc


def hquantile(q, buckets):
    """Prometheus-style histogram_quantile over cumulative {le: count}."""
    if not buckets:
        return None
    pts = sorted(
        (math.inf if le in ("+Inf", "Inf", "inf") else float(le), c)
        for le, c in buckets.items()
    )
    total = pts[-1][1]  # +Inf cumulative == total count
    if total <= 0:
        return None
    rank = q * total
    prev_le, prev_c = 0.0, 0.0
    for le, c in pts:
        if c >= rank:
            if math.isinf(le):
                return prev_le if prev_le > 0 else None
            if c == prev_c:
                return le
            return prev_le + (le - prev_le) * (rank - prev_c) / (c - prev_c)
        prev_le, prev_c = le, c
    return pts[-1][0]


def mean(xs):
    xs = [x for x in xs if x is not None]
    return sum(xs) / len(xs) if xs else None


def load(group_glob):
    runs = []
    for path in sorted(glob.glob(group_glob)):
        try:
            runs.append(json.load(open(path)))
        except Exception as e:  # noqa: BLE001
            print(f"WARN: skipping {path}: {e}", file=sys.stderr)
    return runs


def aggregate(runs):
    """metric_base -> {p50,p95,p99} (pooled), plus mean _tps and _cpu."""
    series_per_run = [r.get("series", {}) for r in runs]
    out = {}
    for base, _ in LATENCY_METRICS:
        bk = pooled_buckets(series_per_run, base)
        out[base] = {
            p: hquantile(qv, bk)
            for p, qv in (("p50", 0.5), ("p95", 0.95), ("p99", 0.99))
        }
    tps_per_run, cpu_per_run = [], []
    for r in runs:
        win = max(1, int(r.get("end_epoch", 0)) - int(r.get("start_epoch", 0)))
        s = r.get("series", {})
        tps_rates = [
            delta(x.get("values", [])) / win
            for x in s.get("transactions_included_in_checkpoint", [])
        ]
        tps_per_run.append(max(tps_rates) if tps_rates else None)
        cpu_rates = [
            delta(x.get("values", [])) / win
            for x in s.get("container_cpu_usage_seconds_total", [])
            if x.get("metric", {}).get("name", "").startswith("validator-")
        ]
        cpu_per_run.append(mean(cpu_rates))
    out["_tps"] = mean(tps_per_run)
    out["_cpu"] = mean(cpu_per_run)
    # Safety counters (must stay 0): max value seen across validators and runs.
    out["_safety"] = {m: series_max(series_per_run, m) for m, _ in SAFETY_COUNTERS}
    return out


def configs(runs):
    """Distinct config dicts across the pooled runs (to flag mixed pools)."""
    seen = []
    for r in runs:
        c = r.get("config", {})
        if c and c not in seen:
            seen.append(c)
    return seen


def fmt(x):
    return "—" if x is None else f"{x:.6g}"


def dlt(a, b):
    return "—" if (a is None or b is None) else f"{b - a:+.6g}"


def crash_incidents(results_dir):
    """Scan per-iteration _state.log for a validator that crashed, restarted, or was
    OOM-killed — an H4 failure the timeseries counters don't capture. run.sh writes
    one line per node:
      /validator-1 status=running restarts=0 oom=false exit=0
    BOTH runs are scanned: Run A (attestation OFF) in run-a-node-logs/, Run B (ON) in
    node-logs/. Returns human-readable strings for any non-clean node, tagged V1/V2 —
    so an attestation-only fork (V2 only, V1 clean) is visible at a glance."""
    incidents = []
    for subdir, run in (("run-a-node-logs", "V1"), ("node-logs", "V2")):
        for sp in sorted(
            glob.glob(os.path.join(results_dir, "*", subdir, "_state.log"))
        ):
            itr = sp.split(os.sep)[-3]  # results/<LABEL>/<iter-NNN>/<subdir>/_state.log
            try:
                lines = open(sp).read().splitlines()
            except Exception as e:  # noqa: BLE001
                print(f"WARN: cannot read {sp}: {e}", file=sys.stderr)
                continue
            for line in lines:
                toks = line.split()
                if not toks:
                    continue
                kv = dict(t.split("=", 1) for t in toks if "=" in t)
                restarts = int(kv.get("restarts", "0") or 0)
                oom = kv.get("oom", "false") == "true"
                status = kv.get("status", "")
                if restarts > 0 or oom or status not in ("running", ""):
                    incidents.append(
                        f"[{run}] {itr} {toks[0]}: status={status} restarts={restarts} oom={oom}"
                    )
    return incidents


def main():
    results_dir = sys.argv[1] if len(sys.argv) > 1 else "."
    out = sys.argv[2] if len(sys.argv) > 2 else os.path.join(results_dir, "summary.md")
    v1 = load(os.path.join(results_dir, "*", "run-a-v1-timeseries.json"))
    v2 = load(os.path.join(results_dir, "*", "run-b-v2-timeseries.json"))
    if not v1 and not v2:
        print(f"No run-*-v*-timeseries.json found under {results_dir}", file=sys.stderr)
        sys.exit(1)
    a, b = aggregate(v1), aggregate(v2)

    L = [
        "# H1 — attestation overhead: aggregated results\n",
        f"- V1 (attestation off): pooled over **{len(v1)}** run(s)",
        f"- V2 (attestation on): pooled over **{len(v2)}** run(s)",
        "- Percentiles pool raw histogram buckets across runs (correct",
        "  cross-run aggregation, not an average of per-run quantiles).",
        "- Throughput / CPU are means across runs.\n",
    ]
    cfgs = configs(v1 + v2)
    if len(cfgs) > 1:
        L.append("> [!WARNING]")
        L.append("> Pooled runs span more than one config; results may be")
        L.append("> misleading:")
        for i, c in enumerate(cfgs, 1):
            L.append(f"> - config {i}:")
            L += [f">   - {k}: {v}" for k, v in sorted(c.items())]
        L.append("")
    elif cfgs:
        L.append("- Config:")
        L += [f"  - {k}: {v}" for k, v in sorted(cfgs[0].items())]
        L.append("")

    for pct in ("p50", "p95", "p99"):
        L += [
            f"## {pct}\n",
            "| metric | V1 | V2 | V2−V1 |",
            "| --- | --- | --- | --- |",
        ]
        for base, name in LATENCY_METRICS:
            va, vb = a.get(base, {}).get(pct), b.get(base, {}).get(pct)
            L.append(f"| {name} | {fmt(va)} | {fmt(vb)} | {dlt(va, vb)} |")
        L.append("")
    L += [
        "## throughput / CPU (mean across runs)\n",
        "| metric | V1 | V2 | V2−V1 |",
        "| --- | --- | --- | --- |",
        f"| finalized TPS | {fmt(a['_tps'])} | {fmt(b['_tps'])} | {dlt(a['_tps'], b['_tps'])} |",
        f"| per-validator CPU (busy cores) | {fmt(a['_cpu'])} | {fmt(b['_cpu'])} | {dlt(a['_cpu'], b['_cpu'])} |",
    ]
    # --- H4 (safety): the ONLY pass/fail hypothesis. FAILS if any safety counter is
    # non-zero (fork / inconsistent state / double-spend / attestor panic / soft-lock
    # equivocation) OR any validator crashed/restarted/OOM'd. Watched on every run. ---
    nonzero = [
        (label, a["_safety"][m], b["_safety"][m])
        for m, label in SAFETY_COUNTERS
        if (a["_safety"][m] or 0) > 0 or (b["_safety"][m] or 0) > 0
    ]
    incidents = crash_incidents(results_dir)
    failed = bool(nonzero or incidents)
    L += [
        "",
        "## H4 — safety (pass/fail)\n",
        f"**H4: {'FAIL ✗' if failed else 'PASS ✓'}**",
        "",
    ]
    if not failed:
        L.append(
            "All safety counters zero (checkpoint forks, inconsistent state, "
            "double-spend, attestor panics, soft-lock equivocation) and no validator "
            "crash / restart / OOM across the pooled runs."
        )
    else:
        L += [
            "> [!CAUTION]",
            "> H4 FAILED — a safety violation occurred. Treat any H1/H2/H3 numbers from",
            "> these runs as suspect until investigated (run-a-node-logs/ = V1,",
            "> node-logs/ = V2, plus each dir's _crash.log).",
        ]
        if nonzero:
            L += [
                "",
                "Non-zero safety counters (max across pooled runs):",
                "",
                "| safety counter | V1 | V2 |",
                "| --- | --- | --- |",
            ]
            L += [f"| {label} | {fmt(va)} | {fmt(vb)} |" for label, va, vb in nonzero]
        if incidents:
            L += ["", "Validator crash / restart / OOM (V1=run-a-node-logs, V2=node-logs):", ""]
            L += [f"- {x}" for x in incidents]
    with open(out, "w") as f:
        f.write("\n".join(L) + "\n")
    # Echo the verdict to stderr too, so a FAIL is visible in the run log without
    # opening summary.md (non-fatal: the run still completes and plots render).
    if failed:
        print(
            f"H4: FAIL — {len(nonzero)} non-zero safety counter(s), "
            f"{len(incidents)} validator incident(s) in {results_dir}",
            file=sys.stderr,
        )


if __name__ == "__main__":
    main()
