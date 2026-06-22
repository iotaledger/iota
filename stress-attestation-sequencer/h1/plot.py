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
}


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
    metric_m = re.search(r"([a-z_][a-z0-9_]+)\s*\{", expr) or re.search(r"([a-z_][a-z0-9_]+)\s*\[", expr)
    metric = metric_m.group(1) if metric_m else None
    block = re.search(r"\{([^}]*)\}", expr)
    filters = re.findall(r'(\w+)\s*(=~|!~|=|!=)\s*"([^"]*)"', block.group(1)) if block else []
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
    items = sorted(((np.inf if le == "+Inf" else float(le)), r) for le, r in le_rates.items())
    if not items:
        return np.nan
    total = items[-1][1]  # +Inf bucket = total
    if total <= 0:
        return np.nan
    target = q * total
    prev_le, prev_c = 0.0, 0.0
    for le, c in items:
        if c >= target:
            if le == np.inf:
                return prev_le if prev_le > 0 else items[-2][0] if len(items) > 1 else np.nan
            if c == prev_c:
                return le
            return prev_le + (le - prev_le) * ((target - prev_c) / (c - prev_c))
        prev_le, prev_c = le, c
    return items[-1][0]


def eval_target(spec, run, grid, window, host_stat):
    """Compute ONE network-level series for this target over `grid` (relative s).

    Every honest validator does the SAME post-consensus work, so a metric's
    per-validator series are ~identical replicas. We collapse them to one network
    value by `host_stat` (mean/median ACROSS validators) — NOT a sum, which would
    multiply a replicated counter by the validator count (the 4x inflation bug).
    Histograms instead pool their `le` buckets across validators, which is the
    correct way to form the network-wide distribution (union of samples)."""
    series = run["series"].get(spec["metric"])
    if not isinstance(series, list):
        return np.full(len(grid), np.nan)
    start = run["start_epoch"]
    matched = [s for s in series if match(s["metric"], spec["filters"]) and s.get("values")]
    if not matched:
        return np.full(len(grid), np.nan)
    reduce_hosts = ((lambda M: np.nanmedian(M, axis=0)) if host_stat == "median"
                    else (lambda M: np.nanmean(M, axis=0)))

    if spec["q"] is not None:
        # pool buckets across hosts per `le` (union of samples), rate, then quantile.
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
            out[i] = quantile_from_buckets({le: arr[i] for le, arr in le_stack.items()}, spec["q"])
        return out

    if spec["is_rate"]:
        # per-host windowed rate, then mean/median across the (replica) hosts.
        rates = [windowed_rate(to_arrays(s["values"])[0] - start, to_arrays(s["values"])[1], grid, window)
                 for s in matched]
        return reduce_hosts(np.vstack(rates))

    # gauge: per-host value on grid, then mean/median across hosts.
    stacks = [interp_on_grid(to_arrays(s["values"])[0] - start, to_arrays(s["values"])[1], grid)
              for s in matched]
    return reduce_hosts(np.vstack(stacks))


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
            out.append({"row": cur, "title": p["title"], "unit": unit, "exprs": targets})
    return out


def target_tag(spec, multi):
    if not multi:
        return ""
    bits = []
    if spec["q"] is not None:
        bits.append(f"p{int(round(spec['q'] * 100))}")
    m = spec["metric"].replace("_bucket", "")
    # shorten common metric names for the legend
    short = m.split("_")[-2] + "_" + m.split("_")[-1] if m.count("_") >= 1 else m
    bits.append(short)
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


def main():
    here = os.path.dirname(os.path.abspath(__file__))
    ap = argparse.ArgumentParser()
    ap.add_argument("--root", default=os.path.join(here, "results"),
                    help="results dir holding <LABEL>/ experiment folders")
    ap.add_argument("--dashboard", default=os.path.join(
        here, "..", "..", "dev-tools", "grafana-local", "dashboards",
        "attestation-sequencer-stress.json"))
    ap.add_argument("--label", default=None,
                    help="plot only this experiment label (default: all under --root)")
    ap.add_argument("--stat", choices=["mean", "median"], default="median")
    ap.add_argument("--band", choices=["iqr", "std", "none"], default="iqr")
    ap.add_argument("--rate-window", type=int, default=10, help="rate() window (s)")
    ap.add_argument("--all", action="store_true",
                    help="render every panel (default: skip Tier-3 sanity gates)")
    args = ap.parse_args()

    panels = panels_from_dashboard(args.dashboard)

    # Each results/<LABEL>/ is one experiment (one config, guaranteed by run.sh's
    # config gate). Its iter-NNN/ subdirs are the iterations to pool. The directory
    # IS the group — no config-signature inference, no archive.
    labels = ([args.label] if args.label
              else sorted(os.path.basename(d) for d in glob.glob(os.path.join(args.root, "*"))
                          if os.path.isdir(d)))
    groups = {}
    for label in labels:
        ld = os.path.join(args.root, label)
        v1 = sorted(glob.glob(os.path.join(ld, "iter-*", "run-a-v1-timeseries.json")))
        v2 = sorted(glob.glob(os.path.join(ld, "iter-*", "run-b-v2-timeseries.json")))
        if not v1 and not v2:
            continue
        groups[label] = {"V1": [json.load(open(f)) for f in v1],
                         "V2": [json.load(open(f)) for f in v2]}
    if not groups:
        print(f"no <LABEL>/iter-*/run-*-timeseries.json under {args.root}", file=sys.stderr)
        sys.exit(1)

    VER_STYLE = {"V1": dict(color="#1f77b4"), "V2": dict(color="#d62728")}

    for label, g in groups.items():
        # Figures live alongside the data: results/<LABEL>/plots/.
        outdir = os.path.join(args.root, label, "plots")
        os.makedirs(outdir, exist_ok=True)

        for panel in panels:
            if not args.all and panel["title"] in SKIP_PANELS:
                continue
            specs = [parse_expr(e) for e in panel["exprs"]]
            multi = len(specs) > 1
            # window length: use the max run window in this group so the grid spans it
            allruns = g["V1"] + g["V2"]
            win = max((r["end_epoch"] - r["start_epoch"]) for r in allruns)
            grid = np.arange(0, win + 1, 1.0)

            fig, ax = plt.subplots(figsize=(9, 4.5))
            plotted = False
            for ver in ("V1", "V2"):
                runs = g[ver]
                if not runs:
                    continue
                for spec in specs:
                    per_run = [eval_target(spec, r, grid, args.rate_window, args.stat) for r in runs]
                    if not per_run or all(np.all(np.isnan(x)) for x in per_run):
                        continue
                    center, M = aggregate_runs(per_run, args.stat)
                    if np.all(np.isnan(center)):
                        continue
                    tag = target_tag(spec, multi)
                    leg = f"{ver}" + (f" {tag}" if tag else "") + (f" (n={len(runs)})" if not multi else "")
                    style = dict(VER_STYLE[ver])
                    if multi and spec["q"] is not None:
                        style["alpha"] = 0.4 + 0.6 * spec["q"]  # higher q -> darker
                    (line,) = ax.plot(grid, center, label=leg, linewidth=1.8, **style)
                    if args.band != "none" and len(runs) > 1:
                        lo, hi = band_bounds(M, center, args.band)
                        if lo is not None:
                            ax.fill_between(grid, lo, hi, color=line.get_color(), alpha=0.15, linewidth=0)
                    plotted = True

            ax.set_title(f"{panel['row']} — {panel['title']}", fontsize=10)
            ax.set_xlabel("time since run start (s)")
            ax.set_ylabel(panel["unit"] or "")
            ax.grid(True, alpha=0.25)
            if not plotted:
                ax.text(0.5, 0.5, "no data", ha="center", va="center", transform=ax.transAxes,
                        color="gray")
            else:
                ax.legend(fontsize=7, ncol=2)
            fig.tight_layout()
            fn = re.sub(r"[^a-z0-9]+", "_", f"{panel['row']}__{panel['title']}".lower()).strip("_") + ".png"
            fig.savefig(os.path.join(outdir, fn), dpi=110)
            plt.close(fig)


if __name__ == "__main__":
    main()
