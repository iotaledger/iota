#!/usr/bin/env python3
"""h1-plot — render the attestation-sequencer Grafana panels from saved timeseries.

Grafana can only show ONE Prometheus TSDB live; it cannot pool many H1 runs into a
mean/median with a variance band. This does:

  1. Parse the dashboard JSON (dev-tools/grafana-local/dashboards/
     attestation-sequencer-stress.json) for every panel: title, unit, and its
     PromQL target expr(s). The plot stays in sync with the dashboard — add a
     panel there and it appears here, no edits.
  2. Replay each expr against the saved per-run series JSON, reproducing what
     Grafana computes — rate() for counters, histogram_quantile() over pooled
     `le` buckets, sum/max for gauges — but collapsed to ONE network-level series
     per run (hosts summed/maxed/pooled), since the cross-run comparison is the point.
  3. Group by experiment label: each results/<LABEL>/ is one experiment (one
     config, guaranteed by run.sh's config gate); its iter-NNN/ subdirs are the
     iterations. The directory IS the group — pool all V1 and all V2 iterations.
  4. One panel -> one figure: V1 vs V2, each a mean/median line across its iterations
     with a shaded variance band (IQR or ±std). Time is RELATIVE seconds from each
     run's window start, so runs with different wall-clock windows overlay correctly.

By default it renders the H1 set (Tier 1 attestation-overhead + Tier 2 context) —
every dashboard panel except the Tier-3 safety gates (forks / inconsistent-state /
double-spend), which are flat 0 by design. Pass --all to render every panel.

Usage:
  python plot.py [--root ./results] [--label NAME] [--stat median|mean]
                 [--band iqr|std|none] [--rate-window 10] [--all]
                 [--dashboard ../../dev-tools/grafana-local/dashboards/attestation-sequencer-stress.json]

Figures are written next to the data, in results/<LABEL>/plots/.
"""

import argparse
import glob
import json
import os
import re
import sys

import matplotlib

matplotlib.use("Agg")
import matplotlib.pyplot as plt  # noqa: E402
import numpy as np  # noqa: E402

# Gauges legitimately rise and fall — never rate() them (matches the dashboard,
# which uses sum/max by(host) on these, not rate()).
RATE_FUNCS = ("rate(", "irate(", "increase(")

# H1 plots Tier 1 (core attestation-overhead) + Tier 2 (context) — i.e. every
# dashboard panel EXCEPT these Tier-3 safety gates, which are flat 0 by design
# and not worth a curve (run.sh's node-log / _crashes capture is the real check).
# `--all` overrides to render every panel. Keep the dashboard itself broad.
SKIP_PANELS = {
    "split-brain checkpoint forks",
    "remote checkpoint forks",
    "inconsistent state hash",
    "double-spend attempts detected",
    # finalized TPS (transactions_included_in_checkpoint) already captures network
    # throughput; the execution-driver rate tracks it 1:1, so don't plot both.
    "execution rate (executed transactions)",
    # attestation task panics should always be 0 — not worth a curve; aggregate.py
    # surfaces the count in summary.md (and flags it if it ever goes non-zero).
    "attestation task panics (per validator)",
    # H1 doesn't plot sequencing & congestion detail — EXCEPT cancelled txs/sec,
    # which is shown as a subplot on tps.png (see COMPOSITE_FIGURES).
    "scheduled transactions per object per commit — p50",
    "scheduled transactions per object per commit — p95",
    "transaction deferral rounds — p50",
    "transaction deferral rounds — p95",
    "transaction deferral rounds — p99",
    "deferred txs / sec",
    "congested txs / sec",
    "max scheduled object cost per commit — regular",
    "max scheduled object cost per commit — randomness",
    # ratio is plotted as a MEAN (rate(_sum)/rate(_count)) on attestation_accuracy.png,
    # not as histogram quantiles — the buckets straddle 1.0 and interpolate the
    # quantile off the true value (see draw_mean_ratio). Skip the quantile panels.
    "actual / attested CUs — p50 (1.0 = perfect)",
    "actual / attested CUs — p95 (1.0 = perfect)",
}

# Per-panel display overrides, keyed by dashboard panel title. Any of "file"
# (output basename, no extension), "title" (figure title), "ylabel" may be set;
# omitted keys fall back to the dashboard-derived defaults.
PANEL_OVERRIDES = {
    # finalized TPS + cancelled are combined into tps.png (see COMPOSITE_FIGURES).
    "finalized TPS (included in checkpoint)": {
        "title": "Finalized TPS — rate(transactions_included_in_checkpoint)",
        "ylabel": "txs/s",
    },
    "cancelled txs / sec": {
        "title": "Cancelled txs / sec — rate(consensus_handler_cancelled_transactions)",
        "ylabel": "txs/s",
    },
    "validation dropped txs / sec": {
        "title": "Post-consensus drops — rate(consensus_handler_validation_dropped_transactions)",
        "ylabel": "txs/s",
    },
    # receipt->executed includes per-validator queueing, which can differ across
    # validators — so collapse by MAX (the worst/slowest node) rather than pooling
    # the network distribution. One curve per version instead of 8. (Only meaningful
    # for DIRECT=false, where multiple validators stamp receipt times; under pinning
    # only validator-1 reports, so max == that one validator.)
    "receipt → executed — p50": {
        "host_reduce": "max",
        "title": "From receipt to execution latency — p50",
    },
    "receipt → executed — p95": {
        "host_reduce": "max",
        "title": "From receipt to execution latency — p95",
    },
    "receipt → executed — p99": {
        "host_reduce": "max",
        "title": "From receipt to execution latency — p99",
    },
    # drop the redundant "_latency" in the filename (row already says "latency_").
    "post-consensus validation latency — p50": {
        "file": "latency_post_consensus_validation_p50",
        "title": "Post-consensus validation latency — p50",
    },
    "post-consensus validation latency — p95": {
        "file": "latency_post_consensus_validation_p95",
        "title": "Post-consensus validation latency — p95",
    },
    "internal execution latency p95": {
        "title": "Internal execution latency — p95",
    },
    "submit transaction latency (client, via fullnode)": {
        "title": "Submit transaction latency (client, via fullnode)",
        "color_by": "version_pct",  # distinct hue per (version, percentile)
    },
    "settlement finality latency (client, via fullnode)": {
        "title": "Settlement finality latency (client, via fullnode)",
        "color_by": "version_pct",
    },
    "attestation latency p50 (pre-consensus dry-run)": {
        "title": "Attestation latency — p50 (pre-consensus dry-run)",
    },
    "attestation latency p95 (pre-consensus dry-run)": {
        "title": "Attestation latency — p95 (pre-consensus dry-run)",
    },
    "attestation latency p99 (pre-consensus dry-run)": {
        "title": "Attestation latency — p99 (pre-consensus dry-run)",
    },
    # attestation is V2-only (V1 is a flat-zero line — drop it); show the busiest
    # validator (max), same treatment as receipt->executed.
    "attestations / sec": {
        "versions": ["V2"],
        "host_reduce": "max",
        "title": "Attestations / sec",
    },
    "execution dispatch queue": {
        "title": "Execution dispatch queue (execution_driver_dispatch_queue)",
        "ylabel": "count",
    },
    "pending transactions (waiting for inputs)": {
        "title": "Pending transactions (transaction_manager_num_pending_certificates)",
        "ylabel": "txs",
    },
    "execution queueing delay p95": {
        "title": "Execution queueing delay — p95 (execution_queueing_delay_s)",
    },
    "attested vs actual computation units (CUs, p50)": {
        "title": "Attested vs actual computation units — p50",
        "ylabel": "CUs",
        "color_by": "target",  # attested vs actual in different colors
    },
    "execution backpressure active (0/1)": {
        "title": "Execution backpressure active 0/1 (execution_cache_backpressure_status)",
        "ylabel": "0/1",
    },
    "backpressure toggles / sec": {
        "title": "Backpressure toggles / sec — rate(execution_cache_backpressure_toggles)",
    },
    "soft-lock rejections / sec": {
        "title": "Soft-lock rejections / sec — rate(validator_service_num_rejected_tx_soft_lock_conflict)",
        "ylabel": "txs/s",
    },
    "host CPU (busy cores, whole machine)": {
        # whole-machine busy cores = SUM of per-core non-idle rates.
        "title": "Whole-machine CPU — busy cores (node_cpu_seconds_total)",
        "ylabel": "cores",
        "host_reduce": "sum",
    },
    "per-validator CPU (busy cores, cadvisor)": {
        # MAX over validators (busiest) — robust to dead-container series that
        # the kept TSDB carries across runs (they'd dilute a mean).
        "title": "Per-validator CPU — busy cores (container_cpu_usage_seconds_total)",
        "ylabel": "cores",
        "host_reduce": "max",
    },
    "per-validator memory RSS (cadvisor)": {
        "title": "Per-validator memory RSS (container_memory_rss)",
        "ylabel": "bytes",
        "host_reduce": "max",
    },
}

VER_STYLE = {"V1": {"color": "#1f77b4"}, "V2": {"color": "#d62728"}}

# For multi-quantile panels using color_by="version_pct": a distinct hue per
# (version, percentile) so all 6 curves are separable (warm = V2, cool = V1).
#   V2: p99 red,  p95 orange, p50 gold      V1: p99 blue, p95 cyan, p50 green
VER_PCT_COLORS = {
    ("V2", 0.99): "#d62728",  # red
    ("V2", 0.95): "#ff7f0e",  # orange
    ("V2", 0.50): "#e6b800",  # gold (a visible "yellow")
    ("V1", 0.99): "#1f77b4",  # blue
    ("V1", 0.95): "#17becf",  # cyan
    ("V1", 0.50): "#2ca02c",  # green
}

# Composite figures: stack several dashboard panels into ONE figure (subplots,
# shared x-axis), written as <file>.png. Member panels are NOT also rendered
# individually. Order in `panels` sets top→bottom row order.
COMPOSITE_FIGURES = [
    {
        "file": "tps",
        "panels": [
            "finalized TPS (included in checkpoint)",
            "cancelled txs / sec",
            "validation dropped txs / sec",
        ],
    },
    {
        "file": "latency_receipt_executed",
        "panels": [
            "receipt → executed — p50",
            "receipt → executed — p95",
            "receipt → executed — p99",
        ],
    },
    {
        "file": "latency_post_consensus_validation",
        "panels": [
            "post-consensus validation latency — p50",
            "post-consensus validation latency — p95",
        ],
    },
    {
        "file": "latency_client",
        "panels": [
            "submit transaction latency (client, via fullnode)",
            "settlement finality latency (client, via fullnode)",
        ],
    },
    {
        "file": "latency_attestation",
        "rows": [
            # row 1: the three attestation-latency pcts overlaid on one axes.
            {
                "overlay": [
                    "attestation latency p50 (pre-consensus dry-run)",
                    "attestation latency p95 (pre-consensus dry-run)",
                    "attestation latency p99 (pre-consensus dry-run)",
                ],
                "title": "Attestation latency (pre-consensus dry-run)",
                "ylabel": "s",
            },
            # row 2: internal (post-consensus VM) execution latency for contrast.
            {"panel": "internal execution latency p95"},
        ],
    },
    {
        "file": "queues",
        "panels": [
            "execution dispatch queue",
            "pending transactions (waiting for inputs)",
            "execution queueing delay p95",
        ],
    },
    {
        "file": "health",
        "panels": [
            "execution backpressure active (0/1)",
            "backpressure toggles / sec",
            "soft-lock rejections / sec",
        ],
    },
    {
        "file": "cpu_mem",
        "panels": [
            "host CPU (busy cores, whole machine)",
            "per-validator CPU (busy cores, cadvisor)",
            "per-validator memory RSS (cadvisor)",
        ],
    },
    {
        # `rows`: each row is its own subplot. A row is either {"overlay": [titles]}
        # (curves overlaid on one axes) or {"panel": title} (a single panel).
        "file": "attestation_accuracy",
        "rows": [
            # MEAN ratio = rate(_sum)/rate(_count), not a histogram quantile: the
            # quantile reads off coarse buckets and gets interpolated off the true
            # value (owned objects are exactly 1.0 but land in the (0.99,1.0] bucket,
            # so the quantile shows ~0.995). The mean uses the raw _sum, so it's exact.
            {
                "mean_ratio": "actual_to_attested_computation_units_ratio",
                "title": "Actual / attested CUs — mean (1.0 = perfect)",
                "ylabel": "ratio",
            },
            {"panel": "attested vs actual computation units (CUs, p50)"},
            {"panel": "attestations / sec"},
        ],
    },
]


# ---------------------------------------------------------------------------
# expr parsing — the dashboard only uses a handful of shapes:
#   sum by (host) (rate(METRIC{f}[$__rate_interval]))
#   histogram_quantile(Q, sum by (le, host) (rate(METRIC_bucket{f}[...])))
#   histogram_quantile(Q, sum by (le) (rate(METRIC_bucket{f}[...])))
#   sum by (host) (METRIC{f})        max by (host) (METRIC{f})
#   sum by (name) (rate(METRIC{f}[...]))   sum by (name) (METRIC{f})
#   sum(rate(METRIC{f}[...]))
# ---------------------------------------------------------------------------
def parse_expr(expr):
    q = re.search(r"histogram_quantile\(\s*([\d.]+)", expr)
    metric_m = re.search(r"([a-z_][a-z0-9_]+)\s*\{", expr) or re.search(
        r"([a-z_][a-z0-9_]+)\s*\[", expr
    )
    metric = metric_m.group(1) if metric_m else None
    block = re.search(r"\{([^}]*)\}", expr)
    filters = (
        re.findall(r'(\w+)\s*(=~|!~|=|!=)\s*"([^"]*)"', block.group(1)) if block else []
    )
    return {
        "expr": expr,
        "q": float(q.group(1)) if q else None,
        "metric": metric,
        "is_rate": any(f in expr for f in RATE_FUNCS),
        "outer": "max" if re.match(r"\s*max", expr) else "sum",
        "filters": filters,
    }


def match(labels, filters):
    for lab, op, val in filters:
        if "$" in val:  # template var (e.g. $host) -> match all
            continue
        lv = labels.get(lab, "")
        if op == "=" and lv != val:
            return False
        if op == "!=" and lv == val:
            return False
        if op == "=~" and not re.fullmatch(val, lv):
            return False
        if op == "!~" and re.fullmatch(val, lv):
            return False
    return True


def to_arrays(values):
    """[[ts,'v'],...] -> (ts:int[], v:float[]) ignoring NaN/inf strings."""
    ts, v = [], []
    for t, s in values:
        try:
            fv = float(s)
        except (TypeError, ValueError):
            continue
        if fv != fv:  # NaN
            continue
        ts.append(int(t))
        v.append(fv)
    return np.array(ts, dtype=float), np.array(v, dtype=float)


def windowed_rate(ts, v, grid, window):
    """Prometheus-style rate on grid: (v[t]-v[t-W]) / (t - (t-W)). Series are
    reset-trimmed by dump_timeseries, so monotonic within the window."""
    if len(ts) < 2:
        return np.full(len(grid), np.nan)
    out = np.full(len(grid), np.nan)
    for i, t in enumerate(grid):
        lo = t - window
        # newest sample <= t and oldest sample >= lo
        j_hi = np.searchsorted(ts, t, side="right") - 1
        j_lo = np.searchsorted(ts, lo, side="left")
        if j_hi <= j_lo:
            continue
        dt = ts[j_hi] - ts[j_lo]
        if dt > 0:
            out[i] = (v[j_hi] - v[j_lo]) / dt
    return out


def interp_on_grid(ts, v, grid):
    if len(ts) == 0:
        return np.full(len(grid), np.nan)
    return np.interp(grid, ts, v, left=np.nan, right=np.nan)


def quantile_from_buckets(le_rates, q):
    """le_rates: dict le->rate(count). Standard Prometheus histogram_quantile."""
    items = sorted(
        ((np.inf if le == "+Inf" else float(le)), r) for le, r in le_rates.items()
    )
    if not items:
        return np.nan
    total = items[-1][1]  # +Inf bucket = total
    if not (
        total > 0
    ):  # catches NaN (e.g. rate undefined early in the window) and <= 0
        return np.nan
    target = q * total
    prev_le, prev_c = 0.0, 0.0
    for le, c in items:
        if c >= target:
            if le == np.inf:
                return (
                    prev_le
                    if prev_le > 0
                    else items[-2][0]
                    if len(items) > 1
                    else np.nan
                )
            if c == prev_c:
                return le
            return prev_le + (le - prev_le) * ((target - prev_c) / (c - prev_c))
        prev_le, prev_c = le, c
    return items[-1][0]


def _reduce_op(name):
    """Return a fn collapsing a (hosts, grid) matrix to (grid,) by the named op."""
    fn = {"max": np.nanmax, "median": np.nanmedian, "sum": np.nansum}.get(
        name, np.nanmean
    )
    return lambda M: fn(M, axis=0)


def eval_target(spec, run, grid, window, host_stat, host_reduce=None):
    """Compute ONE network-level series for this target over `grid` (relative s).

    Every honest validator does the SAME post-consensus work, so a metric's
    per-validator series are ~identical replicas. We collapse them to one network
    value by `host_stat` (mean/median ACROSS validators) — NOT a sum, which would
    multiply a replicated counter by the validator count (the 4x inflation bug).
    Histograms instead pool their `le` buckets across validators, the correct way
    to form the network-wide distribution (union of samples).

    `host_reduce` (max/mean/median) overrides the validator collapse for a panel:
    for histograms it computes the quantile PER validator then reduces (e.g. max =
    the worst/slowest node) instead of pooling; for counters/gauges it picks the op."""
    series = run["series"].get(spec["metric"])
    if not isinstance(series, list):
        return np.full(len(grid), np.nan)
    start = run["start_epoch"]
    matched = [
        s for s in series if match(s["metric"], spec["filters"]) and s.get("values")
    ]
    if not matched:
        return np.full(len(grid), np.nan)
    reduce_hosts = _reduce_op(host_reduce or host_stat)

    if spec["q"] is not None:
        if host_reduce:
            # per-validator quantile (own buckets), then reduce across validators.
            per_host = {}
            for s in matched:
                le = s["metric"].get("le")
                if le is None:
                    continue
                ts, v = to_arrays(s["values"])
                per_host.setdefault(s["metric"].get("host", "?"), {})[le] = (
                    windowed_rate(ts - start, v, grid, window)
                )
            curves = []
            for le_rates in per_host.values():
                out = np.full(len(grid), np.nan)
                for i in range(len(grid)):
                    out[i] = quantile_from_buckets(
                        {le: a[i] for le, a in le_rates.items()}, spec["q"]
                    )
                curves.append(out)
            return (
                reduce_hosts(np.vstack(curves))
                if curves
                else np.full(len(grid), np.nan)
            )
        # default: pool buckets across hosts per `le` (union of samples), rate, then quantile.
        per_le = {}
        for s in matched:
            le = s["metric"].get("le")
            if le is None:
                continue
            ts, v = to_arrays(s["values"])
            per_le.setdefault(le, []).append(windowed_rate(ts - start, v, grid, window))
        if not per_le:
            return np.full(len(grid), np.nan)
        le_stack = {le: np.nansum(np.vstack(rs), axis=0) for le, rs in per_le.items()}
        out = np.full(len(grid), np.nan)
        for i in range(len(grid)):
            out[i] = quantile_from_buckets(
                {le: arr[i] for le, arr in le_stack.items()}, spec["q"]
            )
        return out

    if spec["is_rate"]:
        # per-host windowed rate, then mean/median across the (replica) hosts.
        rates = [
            windowed_rate(
                to_arrays(s["values"])[0] - start,
                to_arrays(s["values"])[1],
                grid,
                window,
            )
            for s in matched
        ]
        return reduce_hosts(np.vstack(rates))

    # gauge: per-host value on grid, then mean/median across hosts.
    stacks = [
        interp_on_grid(
            to_arrays(s["values"])[0] - start, to_arrays(s["values"])[1], grid
        )
        for s in matched
    ]
    return reduce_hosts(np.vstack(stacks))


def eval_mean_ratio(base, run, grid, window):
    """Mean of a histogram's observed values over the window: pooled
    rate(_sum) / rate(_count) across validators (Δsum/Δcount, the average of the
    per-tx values). Exact — it reads the raw _sum, so it dodges the bucket-grid
    interpolation that pulls histogram_quantile OFF a spike value (e.g. every
    owned-object ratio is exactly 1.0, but the `(0.99,1.0]` bucket smears the
    quantile to ~0.995). Series are reset-trimmed, so windowed_rate is monotonic."""
    s, start = run["series"], run["start_epoch"]

    def pooled_rate(suffix):
        series = s.get(base + suffix)
        if not isinstance(series, list):
            return None
        rates = [
            windowed_rate(
                to_arrays(x["values"])[0] - start,
                to_arrays(x["values"])[1],
                grid,
                window,
            )
            for x in series
            if x.get("values")
        ]
        return np.nansum(np.vstack(rates), axis=0) if rates else None

    num, den = pooled_rate("_sum"), pooled_rate("_count")
    if num is None or den is None:
        return np.full(len(grid), np.nan)
    with np.errstate(all="ignore"):
        out = num / den
    out[~(den > 0)] = np.nan  # no observations in this window slice -> undefined
    return out


# ---------------------------------------------------------------------------
def panels_from_dashboard(path):
    j = json.load(open(path))
    rows, cur = [], "misc"
    out = []
    for p in j["panels"]:
        if p.get("type") == "row":
            cur = p["title"]
            continue
        unit = (p.get("fieldConfig", {}).get("defaults", {}) or {}).get("unit", "")
        targets = [t["expr"] for t in p.get("targets", []) if t.get("expr")]
        if targets:
            out.append(
                {"row": cur, "title": p["title"], "unit": unit, "exprs": targets}
            )
    return out


def target_tag(spec, multi):
    """Legend label for one target of a multi-target panel. Uses the metric's
    first token (e.g. attested/actual) + pXX, so targets that differ only in
    metric (attested vs actual, same q) or only in q (same metric) both stay
    distinguishable."""
    if not multi:
        return ""
    bits = [spec["metric"].replace("_bucket", "").split("_")[0]]
    if spec["q"] is not None:
        bits.append(f"p{int(round(spec['q'] * 100))}")
    return " ".join(bits)


def aggregate_runs(per_run, stat):
    """per_run: list of 1D arrays on the same grid. -> (center, lo, hi)."""
    M = np.vstack(per_run)
    with np.errstate(all="ignore"):
        if stat == "mean":
            center = np.nanmean(M, axis=0)
        else:
            center = np.nanmedian(M, axis=0)
    return center, M


def band_bounds(M, center, band):
    with np.errstate(all="ignore"):
        if band == "std":
            sd = np.nanstd(M, axis=0)
            return center - sd, center + sd
        if band == "iqr":
            return np.nanpercentile(M, 25, axis=0), np.nanpercentile(M, 75, axis=0)
    return None, None


def draw_panel(ax, panel, g, grid, args):
    """Render one dashboard panel (V1 vs V2, mean/median + band) onto `ax`."""
    ov = PANEL_OVERRIDES.get(panel["title"], {})
    host_reduce = ov.get("host_reduce")
    versions = ov.get("versions") or (
        "V1",
        "V2",
    )  # e.g. ["V2"] for attestation-only panels
    color_by = ov.get("color_by")  # "target" -> color per target (not per version)
    specs = [parse_expr(e) for e in panel["exprs"]]
    multi = len(specs) > 1
    plotted = False
    for ver in versions:
        runs = g[ver]
        if not runs:
            continue
        for ti, spec in enumerate(specs):
            per_run = [
                eval_target(spec, r, grid, args.rate_window, args.stat, host_reduce)
                for r in runs
            ]
            if not per_run or all(np.all(np.isnan(x)) for x in per_run):
                continue
            center, M = aggregate_runs(per_run, args.stat)
            if np.all(np.isnan(center)):
                continue
            tag = target_tag(spec, multi)
            leg = (
                f"{ver}"
                + (f" {tag}" if tag else "")
                + (f" (n={len(runs)})" if not multi else "")
            )
            if color_by == "version_pct":
                # distinct hue per (version, percentile); clean "V2 p50" legend.
                pct = f"p{int(round(spec['q'] * 100))}" if spec["q"] is not None else ""
                style = {
                    "color": VER_PCT_COLORS.get(
                        (ver, spec["q"]), VER_STYLE[ver]["color"]
                    )
                }
                leg = f"{ver} {pct}".strip()
            elif color_by == "target":
                # one color per target; version (if >1) shown via linestyle.
                style = {"color": OVERLAY_PALETTE[ti % len(OVERLAY_PALETTE)]}
                if len(versions) > 1:
                    style["linestyle"] = "-" if ver == "V2" else "--"
            else:
                style = dict(VER_STYLE[ver])
                if multi and spec["q"] is not None:
                    style["alpha"] = 0.4 + 0.6 * spec["q"]  # higher q -> darker
            (line,) = ax.plot(grid, center, label=leg, linewidth=1.8, **style)
            if args.band != "none" and len(runs) > 1:
                lo, hi = band_bounds(M, center, args.band)
                if lo is not None:
                    ax.fill_between(
                        grid, lo, hi, color=line.get_color(), alpha=0.15, linewidth=0
                    )
            plotted = True

    title = ov.get("title", f"{panel['row']} — {panel['title']}")
    if host_reduce == "max":  # "worst/busiest node" — sum/mean are plain aggregates
        title += " (max across validators)"
    ax.set_title(title, fontsize=10)
    ax.set_ylabel(ov.get("ylabel", panel["unit"] or ""))
    ax.grid(True, alpha=0.25)
    if plotted:
        ax.legend(fontsize=7, ncol=2)
    else:
        ax.text(
            0.5,
            0.5,
            "no data",
            ha="center",
            va="center",
            transform=ax.transAxes,
            color="gray",
        )
    return plotted


# distinct colors for overlaid single-target panels (per percentile), chosen to
# not collide with the V1/V2 blue/red used elsewhere.
OVERLAY_PALETTE = ["#1f77b4", "#ff7f0e", "#2ca02c", "#9467bd", "#8c564b"]


def draw_overlay(ax, member_panels, g, grid, args, comp):
    """Overlay several single-target panels as one curve each on ONE axes,
    colored per panel (per percentile). V2 solid, V1 dashed (if present)."""
    plotted = False
    for ci, panel in enumerate(member_panels):
        ov = PANEL_OVERRIDES.get(panel["title"], {})
        host_reduce = ov.get("host_reduce")
        spec = parse_expr(panel["exprs"][0])  # overlay assumes single-target panels
        base = (
            f"p{int(round(spec['q'] * 100))}"
            if spec["q"] is not None
            else panel["title"]
        )
        color = OVERLAY_PALETTE[ci % len(OVERLAY_PALETTE)]
        for ver, ls in (("V2", "-"), ("V1", "--")):
            runs = g[ver]
            if not runs:
                continue
            per_run = [
                eval_target(spec, r, grid, args.rate_window, args.stat, host_reduce)
                for r in runs
            ]
            if not per_run or all(np.all(np.isnan(x)) for x in per_run):
                continue
            center, M = aggregate_runs(per_run, args.stat)
            if np.all(np.isnan(center)):
                continue
            leg = base if ver == "V2" else f"{base} (V1)"
            ax.plot(grid, center, label=leg, linewidth=1.8, color=color, linestyle=ls)
            if args.band != "none" and len(runs) > 1:
                lo, hi = band_bounds(M, center, args.band)
                if lo is not None:
                    ax.fill_between(grid, lo, hi, color=color, alpha=0.12, linewidth=0)
            plotted = True

    ax.set_title(comp.get("title") or comp.get("file", ""), fontsize=10)
    ax.set_ylabel(
        comp.get("ylabel") or (member_panels[0]["unit"] if member_panels else "")
    )
    ax.grid(True, alpha=0.25)
    if plotted:
        ax.legend(fontsize=8)
    else:
        ax.text(
            0.5,
            0.5,
            "no data",
            ha="center",
            va="center",
            transform=ax.transAxes,
            color="gray",
        )
    return plotted


def draw_mean_ratio(ax, base, g, grid, args, row):
    """Plot the MEAN actual/attested ratio (rate(_sum)/rate(_count)) per version,
    instead of a histogram quantile — exact at 1.0 for owned objects and still
    meaningful where attestation genuinely over-estimates (shared/slow)."""
    plotted = False
    color = OVERLAY_PALETTE[0]
    for ver, ls in (("V2", "-"), ("V1", "--")):
        runs = g[ver]
        if not runs:
            continue
        per_run = [eval_mean_ratio(base, r, grid, args.rate_window) for r in runs]
        if not per_run or all(np.all(np.isnan(x)) for x in per_run):
            continue
        center, M = aggregate_runs(per_run, args.stat)
        if np.all(np.isnan(center)):
            continue
        leg = ("mean" if ver == "V2" else "mean (V1)") + f" (n={len(runs)})"
        ax.plot(grid, center, label=leg, linewidth=1.8, color=color, linestyle=ls)
        if args.band != "none" and len(runs) > 1:
            lo, hi = band_bounds(M, center, args.band)
            if lo is not None:
                ax.fill_between(grid, lo, hi, color=color, alpha=0.12, linewidth=0)
        plotted = True
    ax.axhline(
        1.0, color="gray", linestyle=":", linewidth=1, alpha=0.7
    )  # 1.0 = perfect
    ax.set_title(row.get("title", ""), fontsize=10)
    ax.set_ylabel(row.get("ylabel", "ratio"))
    ax.grid(True, alpha=0.25)
    if plotted:
        ax.legend(fontsize=8)
    else:
        ax.text(
            0.5,
            0.5,
            "no data",
            ha="center",
            va="center",
            transform=ax.transAxes,
            color="gray",
        )
    return plotted


def main():
    here = os.path.dirname(os.path.abspath(__file__))
    ap = argparse.ArgumentParser()
    ap.add_argument(
        "--root",
        default=os.path.join(here, "results"),
        help="results dir holding <LABEL>/ experiment folders",
    )
    ap.add_argument(
        "--dashboard",
        default=os.path.join(
            here,
            "..",
            "..",
            "dev-tools",
            "grafana-local",
            "dashboards",
            "attestation-sequencer-stress.json",
        ),
    )
    ap.add_argument(
        "--label",
        default=None,
        help="plot only this experiment label (default: all under --root)",
    )
    ap.add_argument("--stat", choices=["mean", "median"], default="median")
    ap.add_argument("--band", choices=["iqr", "std", "none"], default="iqr")
    ap.add_argument("--rate-window", type=int, default=10, help="rate() window (s)")
    ap.add_argument(
        "--all",
        action="store_true",
        help="render every panel (default: skip Tier-3 sanity gates)",
    )
    args = ap.parse_args()

    panels = panels_from_dashboard(args.dashboard)

    # Each results/<LABEL>/ is one experiment (one config, guaranteed by run.sh's
    # config gate). Its iter-NNN/ subdirs are the iterations to pool. The directory
    # IS the group — no config-signature inference, no archive.
    labels = (
        [args.label]
        if args.label
        else sorted(
            os.path.basename(d)
            for d in glob.glob(os.path.join(args.root, "*"))
            if os.path.isdir(d)
        )
    )
    groups = {}
    for label in labels:
        ld = os.path.join(args.root, label)
        v1 = sorted(glob.glob(os.path.join(ld, "iter-*", "run-a-v1-timeseries.json")))
        v2 = sorted(glob.glob(os.path.join(ld, "iter-*", "run-b-v2-timeseries.json")))
        if not v1 and not v2:
            continue
        groups[label] = {
            "V1": [json.load(open(f)) for f in v1],
            "V2": [json.load(open(f)) for f in v2],
        }
    if not groups:
        print(
            f"no <LABEL>/iter-*/run-*-timeseries.json under {args.root}",
            file=sys.stderr,
        )
        sys.exit(1)

    by_title = {p["title"]: p for p in panels}
    # Every panel title owned by a composite (flat `panels`, or `rows` of
    # {"panel":…} / {"overlay":[…]}) — excluded from individual rendering.
    composite_titles = set()
    for c in COMPOSITE_FIGURES:
        composite_titles.update(c.get("panels", []))
        for row in c.get("rows", []):
            if row.get("panel"):
                composite_titles.add(row["panel"])
            composite_titles.update(row.get("overlay", []))
    XLABEL = "time since run start (s)"

    for label, g in groups.items():
        # Figures live alongside the data: results/<LABEL>/plots/.
        outdir = os.path.join(args.root, label, "plots")
        os.makedirs(outdir, exist_ok=True)
        # One grid per label: span the longest run window in the group.
        win = max((r["end_epoch"] - r["start_epoch"]) for r in g["V1"] + g["V2"])
        grid = np.arange(0, win + 1, 1.0)

        # Individual panels — skip Tier-3 gates and any panel owned by a composite.
        for panel in panels:
            if not args.all and panel["title"] in SKIP_PANELS:
                continue
            if panel["title"] in composite_titles:
                continue
            fig, ax = plt.subplots(figsize=(9, 4.5))
            draw_panel(ax, panel, g, grid, args)
            ax.set_xlabel(XLABEL)
            fig.tight_layout()
            ov = PANEL_OVERRIDES.get(panel["title"], {})
            fn = ov.get("file") or re.sub(
                r"[^a-z0-9]+", "_", f"{panel['row']}__{panel['title']}".lower()
            ).strip("_")
            fig.savefig(os.path.join(outdir, fn + ".png"), dpi=110)
            plt.close(fig)

        # Composite figures — combine several panels into one figure.
        for comp in COMPOSITE_FIGURES:
            if comp.get("rows"):
                # mixed multi-subplot: each row is one panel or an overlay of several.
                rows = comp["rows"]
                fig, axes = plt.subplots(
                    len(rows), 1, sharex=True, figsize=(9, 3.4 * len(rows))
                )
                if len(rows) == 1:
                    axes = [axes]
                for ax, row in zip(axes, rows):
                    if row.get("overlay"):
                        rps = [by_title[t] for t in row["overlay"] if t in by_title]
                        draw_overlay(ax, rps, g, grid, args, row)
                    elif row.get("mean_ratio"):
                        draw_mean_ratio(ax, row["mean_ratio"], g, grid, args, row)
                    elif row.get("panel") in by_title:
                        draw_panel(ax, by_title[row["panel"]], g, grid, args)
                axes[-1].set_xlabel(XLABEL)
                fig.tight_layout(h_pad=2.5)
                fig.savefig(os.path.join(outdir, comp["file"] + ".png"), dpi=110)
                plt.close(fig)
                continue

            ps = [by_title[t] for t in comp["panels"] if t in by_title]
            if not ps:
                continue
            if comp.get("overlay"):
                # one axes, all member panels overlaid as colored curves.
                fig, ax = plt.subplots(figsize=(9, 4.5))
                draw_overlay(ax, ps, g, grid, args, comp)
                ax.set_xlabel(XLABEL)
                fig.tight_layout()
            else:
                # stacked subplots, one panel per row.
                fig, axes = plt.subplots(
                    len(ps), 1, sharex=True, figsize=(9, 3.4 * len(ps))
                )
                if len(ps) == 1:
                    axes = [axes]
                for ax, panel in zip(axes, ps):
                    draw_panel(ax, panel, g, grid, args)
                axes[-1].set_xlabel(XLABEL)
                fig.tight_layout(h_pad=2.5)  # extra vertical space between subplots
            fig.savefig(os.path.join(outdir, comp["file"] + ".png"), dpi=110)
            plt.close(fig)


if __name__ == "__main__":
    main()
