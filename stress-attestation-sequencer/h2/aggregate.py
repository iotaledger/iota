#!/usr/bin/env python3
"""Aggregate H2 raw timeseries into a mode-A-vs-mode-B summary table.

One row per experiment label (results/matrix/<LABEL>/, one config each),
pooling its
iter-NNN/ iterations: Run A (MODE_A, LIMIT_A) against Run B (MODE_B, LIMIT_B)
on the same load. Labels whose runs used different mode pairs (e.g. a swap
test) are grouped into separate tables.

Reported per run (Run A and Run B):
  - success tps: user transactions that did real work, as executed minus
    cancelled minus commits (see aggregate_arm for why each term is there).
    Also its spread across iterations (sample standard deviation), likewise
    for cancelled/s and the checkpoint-lag mean.
  - finalized tps: the checkpoint-inclusion rate as scraped, prologues
    included — comparable to the client's own reported throughput.
  - cancelled/s: transactions dropped at max_deferral_rounds.
  - checkpoint lag: the exact mean (from the histogram's _sum) and the exact
    share above 30s (a bucket boundary), plus the pooled p95. Only the first
    two are measurements past 30s — the buckets step 25, 30, 60, 90, so a
    quantile landing in that gap is an interpolation, and prints as ">30".
  - latency: settlement finality (client-facing), receipt to executed (the
    validator pipeline, including time spent deferred) and user VM execution,
    each as an exact mean plus p50/p95.
  - admitted per commit: what the run actually let onto the hot object, the
    check that a limit enforced what it was set to. Its whole distribution
    also goes to admits_hist.csv, since for a mixed-cost config the mean hides
    the point: a count limit pins the number, a unit limit spreads it.
  - executed at the expensive level (mixed-cost configs only): transactions
    per second whose actual computation units reached the most expensive
    level of the mix, from the actual_computation_units histogram. Cancelled
    transactions are charged the minimum, so they never count here; success
    tps minus this is the cheap level's throughput.
  - checkpoint lag over time: the exact lag mean per 10 s and per 60 s slice
    of the run window, pooled across validators and iterations, to
    lag_over_time.csv — a pooled mean cannot tell a queue that is high but
    stable from one that keeps growing; the slices can.
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
(one row per label, a_*/b_* column pairs, plus the run's rate, duration and
overshoot so a reader can tell a baseline config from a variant), the
admission histogram to
admits_hist.csv and the lag slices to lag_over_time.csv — the inputs plot.py
draws from, so the pooling arithmetic lives only here.

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

# Attested computation units per `slow_n` at SLOW_SIZE=100, from the
# calibration in probe-test.md (the same on both machines). Used to find the
# most expensive level of a SLOW_MIX config in the actual-units histogram.
N_TO_UNITS = {
    1: 1000,
    70: 2000,
    120: 5000,
    160: 10000,
    217: 20000,
    267: 50000,
    350: 100000,
    516: 200000,
    1015: 500000,
    1848: 1000000,
    3511: 2000000,
    8000: 5000000,
}

# Slice widths for the checkpoint-lag-over-time output, in seconds.
LAG_WINDOWS = (10, 60)

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
    # config its mean describes the cancellations, not the workload. Never
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


def rate_runs(runs, metric):
    """Per-run mean validator-host rate of a counter, one value per run.

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
    return per_run


def rate_mean(runs, metric):
    return mean(rate_runs(runs, metric))


def sd(xs):
    """Sample standard deviation, None under two values."""
    xs = [x for x in xs if x is not None]
    if len(xs) < 2:
        return None
    m = sum(xs) / len(xs)
    return (sum((x - m) ** 2 for x in xs) / (len(xs) - 1)) ** 0.5


def top_level_units(cfg):
    """Computation units of the most expensive level of a SLOW_MIX config,
    None for a fixed-cost config or an unknown level."""
    ns = []
    for part in str(cfg.get("slow_mix", "")).split(","):
        n = part.split(":")[0].strip()
        if n.isdigit():
            ns.append(int(n))
    if len(ns) < 2:
        return None
    return N_TO_UNITS.get(max(ns))


def executed_at_least(runs, units):
    """Per-run rate of executed transactions whose actual computation units
    reached `units`: Δ(+Inf) − Δ(largest bucket edge below `units`) of the
    actual_computation_units histogram, per validator, averaged over the
    validators. A cancelled transaction is charged the minimum, so it falls
    below any level above the 1,000-unit floor and is not counted."""
    per_run = []
    for r in runs:
        by_host = {}
        for s in series_list(r.get("series", {}), "actual_computation_units_bucket"):
            m = s.get("metric", {})
            if m.get("host", "").startswith("validator"):
                by_host.setdefault(m["host"], {})[m.get("le")] = s.get("values", [])
        rates = []
        for by_le in by_host.values():
            inf = by_le.get("+Inf")
            below = [le for le in by_le if le != "+Inf" and float(le) < units]
            if not inf or len(inf) < 2 or not below:
                continue
            span = float(inf[-1][0]) - float(inf[0][0])
            if span <= 0:
                continue
            edge = max(below, key=float)
            rates.append((delta(inf) - delta(by_le[edge])) / span)
        per_run.append(mean(rates))
    return per_run


def window_delta(values, a, b):
    """Counter increase between the first sample at or after `a` and the last
    sample at or before `b`, or None if the window holds under two samples.
    Consecutive windows share their boundary sample, so nothing is counted
    twice: each window covers the increments after its start sample."""
    inside = [v for ts, v in values if a <= ts <= b]
    if len(inside) < 2:
        return None
    d = inside[-1] - inside[0]
    return d if d >= 0 else None


def lag_windows(runs, window):
    """Exact checkpoint-lag mean per `window`-second slice of the run window,
    pooled across validators and iterations: {t_start: (lag_mean, count)}."""
    acc = {}
    for r in runs:
        t0, t1 = r.get("start_epoch"), r.get("end_epoch")
        if t0 is None or t1 is None:
            continue
        per_host = {}
        for kind in ("_sum", "_count"):
            for s in series_list(
                r.get("series", {}), "checkpoint_creation_latency" + kind
            ):
                h = s.get("metric", {}).get("host", "")
                if h.startswith("validator"):
                    per_host.setdefault(h, {})[kind] = [
                        (float(ts), float(v)) for ts, v in s.get("values", [])
                    ]
        for i in range(int((t1 - t0) // window)):
            a, b = t0 + i * window, t0 + (i + 1) * window
            for series in per_host.values():
                if "_sum" not in series or "_count" not in series:
                    continue
                ds = window_delta(series["_sum"], a, b)
                dc = window_delta(series["_count"], a, b)
                if ds is None or dc is None:
                    continue
                cell = acc.setdefault(i * window, [0.0, 0.0])
                cell[0] += ds
                cell[1] += dc
    return {
        t: ((v[0] / v[1]) if v[1] > 0 else None, v[1]) for t, v in sorted(acc.items())
    }


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


def aggregate_arm(runs, top_units=None):
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
    execd_runs = rate_runs(runs, "execution_driver_executed_transactions")
    canc_runs = rate_runs(runs, "consensus_handler_cancelled_transactions")
    commit_runs = rate_runs(runs, "consensus_committed_subdags")
    succ_runs = [
        e - c - k if None not in (e, c, k) else None
        for e, c, k in zip(execd_runs, canc_runs, commit_runs)
    ]
    canc = mean(canc_runs)
    commits = mean(commit_runs)
    ckpt = rate_mean(runs, "transactions_included_in_checkpoint")
    lag = pooled_buckets(
        [r.get("series", {}) for r in runs], "checkpoint_creation_latency"
    )
    lag_runs = [
        hmean([r.get("series", {})], "checkpoint_creation_latency") for r in runs
    ]
    series = [r.get("series", {}) for r in runs]
    expensive_runs = executed_at_least(runs, top_units) if top_units else []
    return {
        "succ": mean(succ_runs),
        # Spread across iterations, so a difference between the runs can be
        # read against the run-to-run noise of the same configuration.
        "succ_sd": sd(succ_runs),
        "canc_sd": sd(canc_runs),
        "lag_mean_sd": sd(lag_runs),
        "n_runs": len(runs),
        # Mixed-cost configs: transactions per second executed at the most
        # expensive level (None elsewhere). Success minus this is the cheap
        # level's throughput.
        "expensive": mean(expensive_runs) if expensive_runs else None,
        "expensive_sd": sd(expensive_runs) if expensive_runs else None,
        # Checkpoint lag per slice of the run window; see lag_windows.
        "lag_over_time": {w: lag_windows(runs, w) for w in LAG_WINDOWS},
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
        # Transactions this run actually admitted to the hot object per commit
        # — the check that each limit enforced what it was set to: Run A should
        # sit at LIMIT_A, Run B at LIMIT_B / units-per-tx.
        "admits": hmean(
            series,
            "consensus_handler_scheduled_transactions_per_object_per_commit",
            "validator",
        ),
        # The whole distribution behind that mean, for the mixed-cost configs:
        # a count limit pins the number admitted per commit, a unit limit
        # lets it swing with how many expensive transactions a commit holds,
        # and only the histogram shows that.
        "admits_buckets": pooled_buckets(
            series,
            "consensus_handler_scheduled_transactions_per_object_per_commit",
            "validator",
        ),
        "skips": skipped_rounds(runs),
        # consensus commits per second — what turns a per-commit limit into an
        # admitted rate (tx/commit x commits/s), so plot.py needs it per run.
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
        "a": aggregate_arm(runs["a"], top_level_units(cfg)),
        "b": aggregate_arm(runs["b"], top_level_units(cfg)),
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
        "- admits/cmt is what each run actually let onto the hot object per",
        "  commit, so it is the check that a limit enforced what it was set",
        "  to: Run A should sit at LIMIT_A, Run B at LIMIT_B / units-per-tx.",
        "- settlement finality is the client-facing latency (measured on the",
        "  fullnode the runs submit through); receipt-to-executed is the",
        "  validator pipeline, which includes any time spent deferred; VM",
        "  execution is the pure Move cost, and is filtered to validators",
        "  because the fullnode reports its checkpoint replays under the same",
        "  metric. It is blank for runs dumped before that metric was",
        "  captured — better empty than the blended value, which in a",
        "  cancel-heavy config describes the cancellations, not the workload.",
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
        "## spread across iterations (sample standard deviation)\n",
        "| label | iters | success tps sd A → B | cancelled/s sd A → B |"
        " ckpt lag mean s sd A → B |",
        "| --- | --- | --- | --- | --- |",
    ]
    for r in rows:
        a, b = r["a"], r["b"]
        L.append(
            f"| {r['label']} | {r['iters']} |"
            f" {ab(a['succ_sd'], b['succ_sd'])} |"
            f" {ab(a['canc_sd'], b['canc_sd'])} |"
            f" {ab(a['lag_mean_sd'], b['lag_mean_sd'], fmt_secs)} |"
        )
    L.append("")

    mixed = [r for r in rows if r["a"]["expensive"] is not None]
    if mixed:
        L += [
            "## mixed cost: executed per second at the expensive level\n",
            "Transactions whose actual computation units reached the most",
            "expensive level of the mix; cancelled transactions are charged",
            "the minimum and do not count. success tps minus this is the",
            "cheap level's throughput.\n",
            "| label | expensive executed/s A → B | cheap success tps A → B |",
            "| --- | --- | --- |",
        ]
        for r in mixed:
            a, b = r["a"], r["b"]
            cheap = [
                (x["succ"] - x["expensive"])
                if None not in (x["succ"], x["expensive"])
                else None
                for x in (a, b)
            ]
            L.append(
                f"| {r['label']} | {ab(a['expensive'], b['expensive'])} |"
                f" {ab(cheap[0], cheap[1])} |"
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
        "succ_tps_sd",
        "cancelled_per_s_sd",
        "lag_mean_s_sd",
        "n_runs",
        "expensive_per_s",
        "expensive_per_s_sd",
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
        "succ_sd",
        "canc_sd",
        "lag_mean_sd",
        "n_runs",
        "expensive",
        "expensive_sd",
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
                "run_duration",
                "overshoot_a",
                "overshoot_b",
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
                    r["cfg"].get("run_duration", ""),
                    r["cfg"].get("overshoot_a", ""),
                    r["cfg"].get("overshoot_b", ""),
                ]
                + vals
                + [int(not safety_failed(r))]
            )

    # The admitted-per-commit distribution per run, one row per histogram
    # bucket: (label, run, upper edge, count in that bucket). plot.py reads it
    # for the mixed-cost figure.
    hist_path = os.path.join(os.path.dirname(csv_path), "admits_hist.csv")
    with open(hist_path, "w", newline="") as f:
        w = csv.writer(f)
        w.writerow(["label", "run", "le", "count"])
        for r in rows:
            for arm in "ab":
                by_le = r[arm]["admits_buckets"]
                edges = sorted(
                    by_le, key=lambda x: float("inf") if x == "+Inf" else float(x)
                )
                prev = 0.0
                for le in edges:
                    count = by_le[le] - prev
                    prev = by_le[le]
                    if count > 0:
                        w.writerow([r["label"], arm, le, cell(count)])

    # Checkpoint lag per slice of the run window, one row per (label, run,
    # slice width, slice start): the exact mean over the checkpoints built in
    # that slice, pooled across validators and iterations, and their count.
    lag_path = os.path.join(os.path.dirname(csv_path), "lag_over_time.csv")
    with open(lag_path, "w", newline="") as f:
        w = csv.writer(f)
        w.writerow(["label", "run", "window_s", "t_start_s", "lag_mean_s", "checkpoints"])
        for r in rows:
            for arm in "ab":
                for window, slices in r[arm]["lag_over_time"].items():
                    for t, (lag_mean, count) in slices.items():
                        w.writerow([r["label"], arm, window, t, cell(lag_mean), cell(count)])

    print(f"{len(rows)} label(s) -> {out}", file=sys.stderr)
    print(f"scalar table -> {csv_path}", file=sys.stderr)
    print(f"admitted-per-commit buckets -> {hist_path}", file=sys.stderr)
    print(f"checkpoint lag per slice -> {lag_path}", file=sys.stderr)
    if failed:
        print(f"SAFETY: {len(failed)} label(s) flagged", file=sys.stderr)


if __name__ == "__main__":
    main()
