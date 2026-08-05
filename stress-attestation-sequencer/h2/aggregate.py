#!/usr/bin/env python3
"""Aggregate H2 raw timeseries into a mode-A-vs-mode-B summary table.

One row per experiment label (results/<LABEL>/, one config each), pooling its
iter-NNN/ iterations: Run A (MODE_A, LIMIT_A) against Run B (MODE_B, LIMIT_B)
on the same load. Labels whose runs used different mode pairs (e.g. a swap
test) are grouped into separate tables.

Reported per arm:
  - success tps: transactions included in checkpoints MINUS cancelled ones.
    Cancelled transactions are executed (as cancelled) and counted in the
    checkpoint total, so the raw rate alone overstates useful throughput.
  - cancelled/s: transactions dropped at max_deferral_rounds.
  - checkpoint lag p50/p95: pooled histogram quantiles across all iterations.
  - deferral rounds above max_deferral_rounds: should be 0; every such
    observation is the signature of a skipped leader round (the deferral
    budget is a commit-round difference, so a skipped round spends budget
    without a scheduling attempt).
  - skipped leader rounds: leader-round advance minus commits (validator-1),
    mean per run. Needs a dump that captured consensus_handler_leader_round.

Rates divide by the actual sample span, not the nominal window: Prometheus
starts with the network, so the first iteration's Run A window can miss its
first seconds and the nominal window would understate the rate.

The experiment-agnostic machinery is shared with h1 in ../aggregate.py.
Pure stdlib.

Usage: aggregate.py [results_dir] [out.md]
  results_dir: the results root holding label dirs (default .), or a single
               label dir. out.md defaults to <results_dir>/summary.md.
"""

import glob
import json
import os
import sys

# The parent dir goes at sys.path[0], AHEAD of this script's own directory, so
# `aggregate` resolves to the shared ../aggregate.py and not to this file.
sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
from aggregate import (  # noqa: E402
    crash_incidents,
    delta,
    fmt,
    hquantile,
    load,
    mean,
    pooled_buckets,
    series_list,
    series_max,
)

CRASH_RUN_DIRS = (("run-a-node-logs", "A"), ("run-b-node-logs", "B"))

# Safety counters that MUST stay 0 across every run; any non-zero one flags the
# label's numbers as suspect. NOTE: past fork panics left these at 0 while the
# validator crashed, so the _state.log crash scan below is the authority.
SAFETY_COUNTERS = [
    ("validator_attestation_task_panics", "attestation task panics"),
    ("split_brain_checkpoint_forks", "split-brain checkpoint forks"),
    ("remote_checkpoint_forks", "remote checkpoint forks"),
    ("global_state_hash_inconsistent_state", "inconsistent state hash"),
    ("total_client_double_spend_attempts_detected", "double-spend attempts detected"),
]


def host_rate(values):
    """increase / actual sample span for one series, None if under 2 samples."""
    if not values or len(values) < 2:
        return None
    span = float(values[-1][0]) - float(values[0][0])
    if span <= 0:
        return None
    return (float(values[-1][1]) - float(values[0][1])) / span


def rate_mean(runs, metric):
    """Mean across runs of the per-run mean validator-host rate.

    Used for counters every validator observes near-identically (checkpoint
    inclusion, cancellations), so hosts are averaged, not summed."""
    per_run = []
    for r in runs:
        rates = [
            host_rate(s.get("values"))
            for s in series_list(r.get("series", {}), metric)
            if s.get("metric", {}).get("host", "").startswith("validator")
        ]
        per_run.append(mean(rates))
    return mean(per_run)


def units_per_tx(runs):
    """Measured attested computation units per transaction: pooled
    delta(sum)/delta(count) of the attested_computation_units histogram."""
    num = den = 0.0
    for r in runs:
        s = r.get("series", {})
        for x in series_list(s, "attested_computation_units_sum"):
            num += delta(x.get("values", []))
        for x in series_list(s, "attested_computation_units_count"):
            den += delta(x.get("values", []))
    return num / den if den > 0 else None


def v1_delta(run, metric):
    for s in series_list(run.get("series", {}), metric):
        if s.get("metric", {}).get("host") == "validator-1":
            return delta(s.get("values", []))
    return None


def skipped_rounds(runs):
    """Mean per run of leader-round advance minus commits on validator-1.
    None when the dump predates the consensus_handler_leader_round metric."""
    per_run = []
    for r in runs:
        lr = v1_delta(r, "consensus_handler_leader_round")
        cs = v1_delta(r, "consensus_committed_subdags")
        if lr is not None and cs is not None:
            per_run.append(lr - cs)
    return mean(per_run)


def over_max_deferrals(runs, max_rounds):
    """Pooled count of deferral-round observations ABOVE max_deferral_rounds."""
    bk = pooled_buckets(
        [r.get("series", {}) for r in runs],
        "consensus_handler_transaction_deferral_rounds",
    )
    if not bk:
        return None
    total = bk.get("+Inf", 0.0)
    at_max = [c for le, c in bk.items() if le != "+Inf" and float(le) >= max_rounds]
    return total - min(at_max) if at_max else None


def aggregate_arm(runs):
    tps = rate_mean(runs, "transactions_included_in_checkpoint")
    canc = rate_mean(runs, "consensus_handler_cancelled_transactions")
    lag = pooled_buckets(
        [r.get("series", {}) for r in runs], "checkpoint_creation_latency"
    )
    return {
        "succ": (tps - canc) if (tps is not None and canc is not None) else tps,
        "canc": canc,
        "lag50": hquantile(0.5, lag),
        "lag95": hquantile(0.95, lag),
        "skips": skipped_rounds(runs),
        "safety": {
            m: series_max([r.get("series", {}) for r in runs], m)
            for m, _ in SAFETY_COUNTERS
        },
    }


def limit_key(cfg, key):
    try:
        return int(cfg.get(key, ""))
    except (TypeError, ValueError):
        return None


def label_row(root, label):
    d = os.path.join(root, label)
    try:
        cfg = json.load(open(os.path.join(d, "config.json")))
    except OSError:
        cfg = {}
    runs = {
        arm: load(os.path.join(d, "iter-*", f"run-{arm}-timeseries.json"))
        for arm in "ab"
    }
    if not runs["a"] and not runs["b"]:
        return None
    max_rounds = limit_key(cfg, "max_deferral_rounds") or 10
    row = {
        "label": label,
        "cfg": cfg,
        "iters": max(len(runs["a"]), len(runs["b"])),
        "units": units_per_tx(runs["b"]),
        "limit_b": limit_key(cfg, "limit_b"),
        "a": aggregate_arm(runs["a"]),
        "b": aggregate_arm(runs["b"]),
        "over_max": {arm: over_max_deferrals(runs[arm], max_rounds) for arm in "ab"},
        "incidents": crash_incidents(d, CRASH_RUN_DIRS),
    }
    row["txcmt"] = (
        int(row["limit_b"] // row["units"]) if row["limit_b"] and row["units"] else None
    )
    return row


def safety_failed(row):
    return bool(
        row["incidents"]
        or any(v > 0 for v in row["a"]["safety"].values())
        or any(v > 0 for v in row["b"]["safety"].values())
    )


def fmt_rate(x):
    return "—" if x is None else f"{x:.1f}"


def fmt_lag(a50, a95):
    if a50 is None or a95 is None:
        return "—"
    return f"{a50:.2f} / {a95:.2f}"


def ab(x, y, f=fmt_rate):
    return f"{f(x)} → {f(y)}"


def main():
    results_dir = sys.argv[1] if len(sys.argv) > 1 else "."
    out = sys.argv[2] if len(sys.argv) > 2 else os.path.join(results_dir, "summary.md")

    if glob.glob(os.path.join(results_dir, "iter-*")):
        root = os.path.dirname(os.path.abspath(results_dir)) or "."
        labels = [os.path.basename(os.path.abspath(results_dir))]
    else:
        root = results_dir
        labels = sorted(
            os.path.basename(p)
            for p in glob.glob(os.path.join(results_dir, "*"))
            if glob.glob(os.path.join(p, "iter-*", "run-a-timeseries.json"))
        )
    rows = [r for label in labels if (r := label_row(root, label))]
    if not rows:
        print(
            f"no <label>/iter-*/run-a-timeseries.json under {results_dir}",
            file=sys.stderr,
        )
        sys.exit(1)
    rows.sort(key=lambda r: (r["units"] or 0, r["limit_b"] or 0))

    L = [
        "# H2 — congestion mode comparison: aggregated results\n",
        "- One row per label; its iterations are pooled (histogram buckets",
        "  summed before taking quantiles; rates are means across runs).",
        "- success tps = included in checkpoints − cancelled; cancelled",
        "  transactions execute as cancelled and count toward the raw rate.",
        "- units/tx is measured from Run B's attested computation units;",
        "  B tx/commit = LIMIT_B / units-per-tx, what Run B admits where",
        "  Run A always admits LIMIT_A transactions.",
        "- deferrals > max and skipped rounds: see the module docstring —",
        "  both track skipped leader rounds, not longer waits.\n",
    ]

    # One table per (mode_a, mode_b) pair, so a swap test doesn't silently
    # relabel which mode is A and which is B mid-table.
    groups = []
    for r in rows:
        key = (r["cfg"].get("mode_a", "?"), r["cfg"].get("mode_b", "?"))
        if groups and groups[-1][0] == key:
            groups[-1][1].append(r)
        else:
            groups.append((key, [r]))

    for (mode_a, mode_b), grp in groups:
        L += [
            f"## A = {mode_a}, B = {mode_b}\n",
            "| label | units/tx | B tx/cmt | iters | success tps A → B |"
            " cancelled/s A → B | ckpt lag s p50/p95 A | p50/p95 B |"
            " skips A → B | safety |",
            "| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |",
        ]
        for r in grp:
            a, b = r["a"], r["b"]
            L.append(
                f"| {r['label']} | {fmt(r['units'])} | {fmt(r['txcmt'])} |"
                f" {r['iters']} | {ab(a['succ'], b['succ'])} |"
                f" {ab(a['canc'], b['canc'])} |"
                f" {fmt_lag(a['lag50'], a['lag95'])} |"
                f" {fmt_lag(b['lag50'], b['lag95'])} |"
                f" {ab(a['skips'], b['skips'])} |"
                f" {'FAIL ✗' if safety_failed(r) else 'ok'} |"
            )
        L.append("")

    L += [
        "## deferrals past max_deferral_rounds (skipped-round signature)\n",
        "| label | A | B |",
        "| --- | --- | --- |",
    ]
    for r in rows:
        L.append(
            f"| {r['label']} | {fmt(r['over_max']['a'])} | {fmt(r['over_max']['b'])} |"
        )
    L.append("")

    failed = [r for r in rows if safety_failed(r)]
    L.append("## safety\n")
    if not failed:
        L.append(
            "All safety counters zero (checkpoint forks, inconsistent state, "
            "double-spend, attestor panics) and no validator crash / restart "
            "/ OOM across every label."
        )
    else:
        L.append("> [!CAUTION]")
        L.append("> Safety violations — treat these labels' numbers as suspect:")
        for r in failed:
            L.append(f">")
            L.append(f"> **{r['label']}**")
            for m, name in SAFETY_COUNTERS:
                va, vb = r["a"]["safety"][m], r["b"]["safety"][m]
                if va > 0 or vb > 0:
                    L.append(f"> - {name}: A={fmt(va)} B={fmt(vb)}")
            for x in r["incidents"]:
                L.append(f"> - {x}")
    with open(out, "w") as f:
        f.write("\n".join(L) + "\n")
    print(f"{len(rows)} label(s) -> {out}", file=sys.stderr)
    if failed:
        print(f"SAFETY: {len(failed)} label(s) flagged", file=sys.stderr)


if __name__ == "__main__":
    main()
