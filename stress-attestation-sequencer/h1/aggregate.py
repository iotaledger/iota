#!/usr/bin/env python3
"""Aggregate H1 raw timeseries across multiple runs into a V1-vs-V2 summary.

Percentiles are computed by POOLING the raw histogram buckets across runs — the
statistically correct way to combine percentiles (you cannot average per-run
quantiles). Throughput and CPU are averaged across runs. With a single run per
group, the result matches the per-run scalar summary.

Usage: h1-aggregate.py <results_dir> [out.md]
  Scans <results_dir>/*/run-a-v1-timeseries.json   (V1, attestation OFF)
    and <results_dir>/*/run-b-v2-timeseries.json   (V2, attestation ON)

The experiment-agnostic machinery (counter deltas, histogram pooling, the
quantile, the crash scan) is shared with h2 in ../aggregate.py; this file
keeps only the H1 metric set and report layout.
"""

import os
import sys

# The parent dir goes at sys.path[0], AHEAD of this script's own directory, so
# `aggregate` resolves to the shared ../aggregate.py and not to this file.
sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
from aggregate import (  # noqa: E402
    crash_incidents,
    configs,
    delta,
    dlt,
    fmt,
    hquantile,
    load,
    mean,
    pooled_buckets,
    series_list,
    series_max,
    source_total,
)

# (raw histogram base name, display label)
LATENCY_METRICS = [
    ("validator_attestation_latency", "attestation latency — full (s)"),
    ("validator_attestation_queue_wait", "attestation latency — pool wait (s)"),
    (
        "validator_attestation_execution_latency",
        "attestation latency — dry-run exec (s)",
    ),
    (
        "validator_attestation_async_resume_latency",
        "attestation latency — async resume (s)",
    ),
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


# Pre-consensus admission-control rejections by source (cumulative counters:
# report the total increment over the window). The `source` label lives on the
# raw transaction_overload_sources series, which dump_timeseries stores under the
# `_by_source` key (the plain key is source-summed with no label).
OVERLOAD_SOURCES_KEY = "transaction_overload_sources_by_source"
OVERLOAD_SOURCES = [
    ("consensus_graduated", "overload rejections — graduated (total)"),
    ("consensus_max_pending", "overload rejections — max_pending (total)"),
    ("consensus_semaphore", "overload rejections — semaphore (total)"),
]
# Consensus queue depth + shedding levels (gauges: report the peak over the
# window). num_inflight is the value graduated/max_pending shedding gates on.
SHED_GAUGES = [
    ("sequencing_certificate_inflight", "num_inflight — peak (vs max_pending)"),
    ("consensus_queue_load_shedding_percentage", "consensus-queue shed % — peak"),
    (
        "consensus_handler_load_shedding_percentage",
        "post-consensus shed % — quorum, peak",
    ),
    ("authority_load_shedding_percentage", "post-consensus shed % — local, peak"),
]


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
            for x in series_list(s, "transactions_included_in_checkpoint")
        ]
        tps_per_run.append(max(tps_rates) if tps_rates else None)
        cpu_rates = [
            delta(x.get("values", [])) / win
            for x in series_list(s, "container_cpu_usage_seconds_total")
            if x.get("metric", {}).get("name", "").startswith("validator-")
        ]
        cpu_per_run.append(mean(cpu_rates))
    out["_tps"] = mean(tps_per_run)
    out["_cpu"] = mean(cpu_per_run)
    # Load shedding: per-source rejection totals (counters) + queue-depth/shed
    # peaks (gauges). Both flows shed, so these are a real V1-vs-V2 comparison.
    out["_overload"] = {
        src: source_total(series_per_run, OVERLOAD_SOURCES_KEY, src)
        for src, _ in OVERLOAD_SOURCES
    }
    out["_shed"] = {m: series_max(series_per_run, m) for m, _ in SHED_GAUGES}
    # Safety counters (must stay 0): max value seen across validators and runs.
    out["_safety"] = {m: series_max(series_per_run, m) for m, _ in SAFETY_COUNTERS}
    return out


# Where each run's node logs live (V1 = run-a-node-logs/, V2 = node-logs/) —
# so an attestation-only fork (V2 only, V1 clean) is visible at a glance.
CRASH_RUN_DIRS = (("run-a-node-logs", "V1"), ("node-logs", "V2"))


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
    # --- load shedding (pre + post consensus): overload rejection totals by source
    # + queue-depth/shed peaks. 0 across the board = no shedding at this config. ---
    L += [
        "",
        "## load shedding (pre + post consensus)\n",
        "| metric | V1 | V2 | V2−V1 |",
        "| --- | --- | --- | --- |",
    ]
    for src, name in OVERLOAD_SOURCES:
        va, vb = a["_overload"][src], b["_overload"][src]
        L.append(f"| {name} | {fmt(va)} | {fmt(vb)} | {dlt(va, vb)} |")
    for m, name in SHED_GAUGES:
        va, vb = a["_shed"][m], b["_shed"][m]
        L.append(f"| {name} | {fmt(va)} | {fmt(vb)} | {dlt(va, vb)} |")
    # --- H4 (safety): the ONLY pass/fail hypothesis. FAILS if any safety counter is
    # non-zero (fork / inconsistent state / double-spend / attestor panic / soft-lock
    # equivocation) OR any validator crashed/restarted/OOM'd. Watched on every run. ---
    nonzero = [
        (label, a["_safety"][m], b["_safety"][m])
        for m, label in SAFETY_COUNTERS
        if (a["_safety"][m] or 0) > 0 or (b["_safety"][m] or 0) > 0
    ]
    incidents = crash_incidents(results_dir, CRASH_RUN_DIRS)
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
