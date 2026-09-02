#!/usr/bin/env python3
"""Aggregate H2 raw timeseries into a mode-A-vs-mode-B summary table.

One row per experiment label (results/<LABEL>/, one config each), pooling its
iter-NNN/ iterations: Run A (MODE_A, LIMIT_A) against Run B (MODE_B, LIMIT_B)
on the same load. Labels whose runs used different mode pairs (e.g. a swap
test) are grouped into separate tables.

Reported per arm:
  - success tps: user transactions that did real work, as executed minus
    cancelled minus commits (see aggregate_arm for why each term is there).
  - finalized tps: the checkpoint-inclusion rate as scraped, prologues
    included — comparable to the client's own reported throughput.
  - cancelled/s: transactions dropped at max_deferral_rounds.
  - checkpoint lag p50/p95: pooled histogram quantiles across all iterations.
    Above 30s these carry roughly one bucket :f resolution (the histogram
    steps 25, 30, 60, 90), so read anything past 30s as "30 to 60s".
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

Besides the markdown, the same rows are written as scalars to summary.csv
(one row per label, a_*/b_* column pairs) — the input plot.py draws from,
so the pooling arithmetic lives only here.

Usage: aggregate.py [results_dir] [out.md]
  results_dir: the results root holding label dirs (default .), or a single
               label dir. out.md defaults to <results_dir>/summary.md; the
               CSV lands next to it with the same basename.
"""

import csv
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
    hmean,
    hquantile,
    htail_share,
    load,
    mean,
    pooled_buckets,
    series_list,
    series_max,
)

# The bucket edge above which a checkpoint-lag quantile stops being a
# measurement: LATENCY_SEC_BUCKETS (iota-metrics) steps 25, 30, 60, 90, so a
# quantile landing past 30s is an interpolation across a 30-second-wide
# bucket. The exact mean and the share above this edge carry no such error.
LAG_COARSE_EDGE = 30.0

CRASH_RUN_DIRS = (("run-a-node-logs", "A"), ("run-b-node-logs", "B"))

# The latencies the stress plan asks H2 to report, with the host kind that
# reports each. `authority_state_internal_execution_latency` is the one that
# needs filtering: the fullnode reports it too, for its checkpoint replays,
# which is a different population from the validators executing user
# transactions. The client-facing driver metrics only exist on the fullnode
# (the runs submit through it), so they must NOT be filtered to validators.
LATENCIES = [
    # (key, base metric, host prefix, display label)
    (
        "fin",
        "transaction_driver_settlement_finality_latency",
        "fullnode",
        "settlement finality (client)",
    ),
    (
        "recv",
        "validator_transaction_execution_latency",
        "validator",
        "receipt to executed",
    ),
    (
        "vm",
        "authority_state_internal_execution_latency_user",
        "validator",
        "VM execution (user transactions)",
    ),
    # The unqualified variant, kept for runs dumped before the _user one was
    # captured. It blends in the per-commit system transactions AND the
    # cancelled transactions, which do no Move work — so in a cancel-heavy
    # cell its mean describes the cancellations, not the workload. Never
    # headline it; it is in the CSV for continuity only.
    (
        "vmall",
        "authority_state_internal_execution_latency",
        "validator",
        "VM execution (all transactions)",
    ),
]

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
    """Measured attested computation units per transaction — the exact mean of
    the attested_computation_units histogram."""
    return hmean([r.get("series", {}) for r in runs], "attested_computation_units")


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
    # Success throughput counts only user transactions that did real work:
    #
    #   executed - cancelled - commits
    #
    # Executed rather than checkpoint-included, because checkpoint building
    # lags execution — once that lag approaches the run window, inclusion
    # undercounts what the window actually processed (at the most expensive
    # cost points it undercounts so far that included - cancelled goes
    # negative). Minus cancelled, which execute but do no work. Minus the
    # commit rate, since every commit carries one consensus commit prologue,
    # a system transaction both counters count as a transaction.
    execd = rate_mean(runs, "execution_driver_executed_transactions")
    canc = rate_mean(runs, "consensus_handler_cancelled_transactions")
    commits = rate_mean(runs, "consensus_committed_subdags")
    ckpt = rate_mean(runs, "transactions_included_in_checkpoint")
    lag = pooled_buckets(
        [r.get("series", {}) for r in runs], "checkpoint_creation_latency"
    )
    succ = execd - canc - commits if None not in (execd, canc, commits) else None
    series = [r.get("series", {}) for r in runs]
    return {
        "succ": succ,
        # Finalized rate as scraped, prologues included: comparable to the
        # client's reported tps, and the basis the succ formula replaced.
        "ckpt_tps": ckpt,
        "canc": canc,
        "lag50": hquantile(0.5, lag),
        "lag95": hquantile(0.95, lag),
        # Exact, unlike the quantiles above LAG_COARSE_EDGE: the mean comes
        # from the histogram's _sum, the share from a bucket boundary.
        "lag_mean": hmean(series, "checkpoint_creation_latency"),
        "lag_gt_coarse": htail_share(lag, LAG_COARSE_EDGE),
        # Latency, per the stress plan's H2 ask. Reported as the exact mean
        # plus p50/p95 pooled across iterations.
        **{
            f"{key}_{stat}": value
            for key, base, host, _ in LATENCIES
            for stat, value in (
                ("mean", hmean(series, base, host)),
                ("p50", hquantile(0.5, pooled_buckets(series, base, host))),
                ("p95", hquantile(0.95, pooled_buckets(series, base, host))),
            )
        },
        # Transactions this arm actually admitted to the hot object per commit
        # — the check that each limit enforced what it was set to: Run A should
        # sit at LIMIT_A, Run B at LIMIT_B / units-per-tx.
        "admits": hmean(
            series,
            "consensus_handler_scheduled_transactions_per_object_per_commit",
            "validator",
        ),
        "skips": skipped_rounds(runs),
        # consensus commits per second — what turns a per-commit limit into an
        # admitted rate (tx/commit x commits/s), so plot.py needs it per arm.
        "commit_rate": commits,
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


def fmt_q(v):
    """A quantile past LAG_COARSE_EDGE is an interpolation across a
    30-second bucket, so print the bound rather than a number nobody
    should compare."""
    if v is None:
        return "—"
    return f">{LAG_COARSE_EDGE:.0f}" if v > LAG_COARSE_EDGE else f"{v:.2f}"


def fmt_secs(v):
    if v is None:
        return "—"
    return f"{v:.2f}" if v < 10 else f"{v:.1f}"


def fmt_share(v):
    return "—" if v is None else f"{100 * v:.0f}%"


def fmt_ms(v):
    """Seconds in, milliseconds out — these latencies span microseconds to
    seconds, and ms keeps both ends readable."""
    if v is None:
        return "—"
    ms = v * 1000
    return f"{ms:.2f}" if ms < 10 else f"{ms:.0f}"


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
        "- success tps = executed − cancelled − commits: user transactions",
        "  that did real work. Cancelled ones execute but do nothing, and",
        "  every commit carries one consensus commit prologue, which the",
        "  transaction counters count as a transaction.",
        "- checkpoint lag: the mean and the >30s share are exact; the p95",
        "  is not past 30s, where the buckets jump 30 to 60, so it prints",
        '  as ">30" there. Compare the mean and the share, not that bound.',
        "- admits/cmt is what each arm actually let onto the hot object per",
        "  commit, so it is the check that a limit enforced what it was set",
        "  to: Run A should sit at LIMIT_A, Run B at LIMIT_B / units-per-tx.",
        "- settlement finality is the client-facing latency (measured on the",
        "  fullnode the runs submit through); receipt-to-executed is the",
        "  validator pipeline, which includes any time spent deferred; VM",
        "  execution is the pure Move cost, and is filtered to validators",
        "  because the fullnode reports its checkpoint replays under the same",
        "  metric. It is blank for runs dumped before that metric was",
        "  captured — better empty than the blended value, which in a",
        "  cancel-heavy cell describes the cancellations, not the workload.",
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
            " cancelled/s A → B | ckpt lag mean s A → B |"
            " lag >30s A → B | lag p95 s A → B | skips A → B | safety |",
            "| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |",
        ]
        for r in grp:
            a, b = r["a"], r["b"]
            L.append(
                f"| {r['label']} | {fmt(r['units'])} | {fmt(r['txcmt'])} |"
                f" {r['iters']} | {ab(a['succ'], b['succ'])} |"
                f" {ab(a['canc'], b['canc'])} |"
                f" {ab(a['lag_mean'], b['lag_mean'], fmt_secs)} |"
                f" {ab(a['lag_gt_coarse'], b['lag_gt_coarse'], fmt_share)} |"
                f" {ab(a['lag95'], b['lag95'], fmt_q)} |"
                f" {ab(a['skips'], b['skips'])} |"
                f" {'FAIL ✗' if safety_failed(r) else 'ok'} |"
            )
        L.append("")

        L += [
            f"### A = {mode_a}, B = {mode_b} — latency and admission\n",
            "| label | admits/cmt A → B | settlement p50 ms A → B |"
            " settlement p95 ms A → B | receipt→exec p95 ms A → B |"
            " user VM exec mean ms A → B |",
            "| --- | --- | --- | --- | --- | --- |",
        ]
        for r in grp:
            a, b = r["a"], r["b"]
            L.append(
                f"| {r['label']} |"
                f" {ab(a['admits'], b['admits'], fmt_secs)} |"
                f" {ab(a['fin_p50'], b['fin_p50'], fmt_ms)} |"
                f" {ab(a['fin_p95'], b['fin_p95'], fmt_ms)} |"
                f" {ab(a['recv_p95'], b['recv_p95'], fmt_ms)} |"
                f" {ab(a['vm_mean'], b['vm_mean'], fmt_ms)} |"
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

    # The same rows as scalars, for plot.py.
    csv_path = os.path.splitext(out)[0] + ".csv"
    arm_cols = (
        "succ_tps",
        "ckpt_tps",
        "cancelled_per_s",
        "lag_mean_s",
        "lag_over_30s_share",
        "lag_p50_s",
        "lag_p95_s",
        "admits_per_commit",
        "commit_rate",
        "skipped_rounds",
        *(
            f"{key}_{stat}_s"
            for key, _, _, _ in LATENCIES
            for stat in ("mean", "p50", "p95")
        ),
        "over_max_deferrals",
    )
    arm_keys = (
        "succ",
        "ckpt_tps",
        "canc",
        "lag_mean",
        "lag_gt_coarse",
        "lag50",
        "lag95",
        "admits",
        "commit_rate",
        "skips",
        *(
            f"{key}_{stat}"
            for key, _, _, _ in LATENCIES
            for stat in ("mean", "p50", "p95")
        ),
    )

    def cell(v):
        return "" if v is None else f"{v:.6g}"

    with open(csv_path, "w", newline="") as f:
        w = csv.writer(f)
        w.writerow(
            [
                "label",
                "units_per_tx",
                "limit_b",
                "tx_per_commit",
                "iters",
                "mode_a",
                "mode_b",
                "target_qps",
            ]
            + [f"a_{c}" for c in arm_cols]
            + [f"b_{c}" for c in arm_cols]
            + ["safety_ok"]
        )
        for r in rows:
            vals = []
            for arm in "ab":
                vals += [cell(r[arm][k]) for k in arm_keys]
                vals.append(cell(r["over_max"][arm]))
            w.writerow(
                [
                    r["label"],
                    cell(r["units"]),
                    cell(r["limit_b"]),
                    cell(r["txcmt"]),
                    r["iters"],
                    r["cfg"].get("mode_a", ""),
                    r["cfg"].get("mode_b", ""),
                    r["cfg"].get("target_qps", ""),
                ]
                + vals
                + [int(not safety_failed(r))]
            )

    print(f"{len(rows)} label(s) -> {out}", file=sys.stderr)
    print(f"scalar table -> {csv_path}", file=sys.stderr)
    if failed:
        print(f"SAFETY: {len(failed)} label(s) flagged", file=sys.stderr)


if __name__ == "__main__":
    main()
