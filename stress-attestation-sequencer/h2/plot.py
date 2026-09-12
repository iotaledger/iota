#!/usr/bin/env python3
"""plot.py — cross-label mode-comparison figures from aggregate.py's outputs.

Reads <results>/summary.csv, admits_hist.csv and lag_over_time.csv (all
written by aggregate.py, which owns the pooling arithmetic) and renders into
<results>/summary_plots/:

  modes_admitted_rate.png
                      checkpoint lag and cancelled fraction against the
                      admitted rate (tx/commit x commits/s), one curve per
                      cost point. Run A admits the same rate in every config,
                      so it is one vertical line: where that line sits right
                      of a curve's bend the count limit admits more than the
                      object executes, left of it less.
  modes_heatmaps.png  every config at a glance: success tps, cancelled
                      fraction, lag mean and the share of checkpoints past
                      30 s, for Run A and every Run B config, coloured on one
                      scale per panel so equal values look equal everywhere.
  modes_utilization.png
                      the same lag and cancelled curves over admitted rate
                      divided by each cost point's drain rate. If cost acts
                      only through how full the object is, the curves land
                      on one line.
  modes_tradeoff.png  success tps against checkpoint lag mean, one point per
                      config, Run A starred. The lower-right corner is fast
                      and stable.
  modes_matched.png   the configs where LIMIT_B is ten times the cost, Run A
                      next to Run B with the spread across iterations: the
                      check that the two limits agree at one cost per run.
  modes_mix.png       the mixed-cost configs at LIMIT_B = 10 x mean cost:
                      how many transactions each commit admitted, Run A next
                      to Run B, with success, cancellations and lag below.
                      Reads admits_hist.csv.
  modes_mix_ladders.png
                      the mixes run at several limits: success, cancellations,
                      lag and the expensive level's execution rate against
                      LIMIT_B, Run A as the reference, error bars = one
                      standard deviation across iterations, shaded band = the
                      expensive cost to twice it.
  modes_lag_over_time.png
                      checkpoint lag per 10 s slice of the 300 s runs, Run A
                      against Run B. Reads lag_over_time.csv.
  modes_two_machines.png
                      only with a second results directory: the mix ladders
                      both machines ran, one colour per machine.

The x-axis collapse works because tx/commit = LIMIT_B / units-per-tx: the
grid's two axes only act through their ratio, so cost points become curves
over one axis instead of a table.

Lag is the exact histogram mean throughout, not p95: the buckets step
25, 30, 60, 90, so a p95 landing past 30s is an interpolation across a
30-second bucket and two such values cannot be compared. The mean (from
the histogram _sum) and the >30s share carry no bucket error.

Needs matplotlib — run from a venv such as ../h1/.venv; everything else is
stdlib. Not a dashboard replay like ../h1/plot.py: these are cross-label
figures with derived axes.

Only the baseline configs are drawn: the burst off in both runs, the run
duration every config shares, and one target rate. A config that varies any
of those is a different experiment and would otherwise land on top of a
baseline point at the same limit, so it is left out and read from
summary.md instead. The exception is the longer runs, which
modes_lag_over_time.png draws on their own. Two rules select them, one on
the config columns aggregate.py writes and one on the label: the label's
last part must name the rate (`qps1000`), so a variant suffix such as
`-burst` or `-dur300` is dropped even from an older CSV without those
columns.

Usage: plot.py [results_dir] [second_results_dir]
  results_dir: expects summary.csv inside (default .). The optional second
  directory holds the same grid run on another machine; the labels it shares
  with the first, minus a trailing machine suffix such as "-ws", are drawn
  together in modes_two_machines.png. MACHINES="EPYC,WS" names the two.
Env: QPS (default 1000) picks the target rate to draw; figures for any other
  rate get a "-qps<rate>" suffix so they never overwrite the default ones.
  RUN_DURATION (default 60s) picks the run length.
"""

import csv
import math
import os
import re
import sys

import matplotlib

matplotlib.use("Agg")
import matplotlib.pyplot as plt  # noqa: E402
from matplotlib.colors import LinearSegmentedColormap  # noqa: E402
from matplotlib.patches import Patch, Rectangle  # noqa: E402

# Palette (dataviz reference, light mode). The 5-step ordinal blue ramp is the
# validated maximum for one panel — more cost points than that are faceted.
SURFACE = "#fcfcfb"
INK = "#0b0b0b"
INK2 = "#52514e"
MUTED = "#898781"
GRID = "#e1e0d9"
AXIS = "#c3c2b7"
RAMP5 = ["#86b6ef", "#5598e7", "#2a78d6", "#1c5cab", "#0d366b"]
# Run A and Run B side by side: categorical slots 1 and 2 of the same
# reference palette (blue, orange), validated as a pair on this surface.
A_COLOR = RAMP5[2]
B_COLOR = "#eb6834"
# The second machine in modes_two_machines.png (categorical slot 3).
SECOND_COLOR = "#1f8a70"

plt.rcParams.update(
    {
        "figure.facecolor": SURFACE,
        "axes.facecolor": SURFACE,
        "savefig.facecolor": SURFACE,
        "text.color": INK,
        "axes.labelcolor": INK2,
        "axes.edgecolor": AXIS,
        "xtick.color": MUTED,
        "ytick.color": MUTED,
        "grid.color": GRID,
        "grid.linewidth": 0.8,
        "font.family": "sans-serif",
        "font.size": 9,
        "axes.titlesize": 10,
    }
)

A_NAME = "Run A — TotalTxCount, limit 10"
B_NAME = "Run B — TotalComputationUnits"

# Which configs the figures cover; see the module docstring.
MAIN_QPS = float(os.environ.get("QPS", "1000"))
MAIN_DURATION = os.environ.get("RUN_DURATION", "60s")
# Appended to every file name when the selection is not the default one, so a
# second rate's figures sit next to the default ones instead of replacing them.
FILE_SUFFIX = "" if MAIN_QPS == 1000 else f"-qps{MAIN_QPS:g}"
# Columns that are not numbers.
TEXT_COLS = ("label", "mode_a", "mode_b", "safety_ok", "run_duration")


def fnum(s):
    return float(s) if s not in ("", None) else None


def kfmt(v):
    """10000 -> 10K, 1000000 -> 1M."""
    if v >= 1e6:
        return f"{v / 1e6:g}M"
    return f"{v / 1e3:g}K"


def load_rows(path):
    rows = []
    for r in csv.DictReader(open(path)):
        d = {"label": r["label"], "safety_ok": r["safety_ok"] == "1"}
        for k in r:
            if k in TEXT_COLS:
                if k != "label":
                    d[k] = r[k]
            else:
                d[k] = fnum(r[k])
        d["point"] = r["label"].split("-")[0]
        rows.append(d)
    return rows


def machine_suffix(rows):
    """The trailing label part naming the machine, when every label in the
    directory carries the same all-letter one (`results/matrix-ws` -> "ws").
    A rate part like `qps1000` has digits, so it is never taken for one."""
    last = {r["label"].rsplit("-", 1)[-1] for r in rows}
    if len(last) == 1:
        only = next(iter(last))
        if only.isalpha():
            return only
    return None


def baseline(rows, suffix=None):
    """The configs the figures cover: one rate, one run length, the burst off,
    and no variant suffix on the label. The config columns are missing from a
    CSV written before aggregate.py recorded them, hence the label rule too."""
    kept = []
    for r in rows:
        label = r["label"]
        if suffix and label.endswith(f"-{suffix}"):
            label = label[: -len(suffix) - 1]
        if not re.fullmatch(r"qps\d+", label.rsplit("-", 1)[-1]):
            continue
        if r.get("target_qps") != MAIN_QPS:
            continue
        if r.get("run_duration") not in (None, "", MAIN_DURATION):
            continue
        if (r.get("overshoot_a") or 0) or (r.get("overshoot_b") or 0):
            continue
        kept.append(r)
    return kept


class Point:
    """One cost point: its configs across limits, plus the Run A reference."""

    A_KEYS = (
        "succ_tps",
        "cancelled_per_s",
        "lag_mean_s",
        "lag_over_30s_share",
        "commit_rate",
        "succ_tps_sd",
        "cancelled_per_s_sd",
        "lag_mean_s_sd",
    )

    def __init__(self, name, configs):
        self.name = name
        self.configs = sorted(configs, key=lambda c: c["limit_b"])
        self.units = self.configs[0]["units_per_tx"]
        # Run A is the same configuration in every config of the point, so
        # its values are averaged over them.
        self.a = {k: self._mean(f"a_{k}") for k in self.A_KEYS}

    def _mean(self, key):
        vs = [c.get(key) for c in self.configs if c.get(key) is not None]
        return sum(vs) / len(vs) if vs else None

    def admitted(self, c):
        rate = c["b_commit_rate"] or 20.0
        return c["tx_per_commit"] * rate

    def curve(self):
        """(admitted, config) for configs that admit anything, x-sorted."""
        pts = [(self.admitted(c), c) for c in self.configs if c["tx_per_commit"]]
        return sorted(pts, key=lambda t: t[0])

    def matched(self):
        """The config whose LIMIT_B is ten times the cost, if it was run."""
        for c in self.configs:
            if c["tx_per_commit"] == 10:
                return c
        return None

    def drain(self):
        """How fast one object executes this cost: in a config whose success
        settles well below what the limit admits while cancellations stay
        quiet, execution is the only constraint left, so the success rate is
        the drain rate. Configs whose limit admits more than the client
        offers are excluded — there the shortfall is the offered rate, not
        execution (cu1k's whole ladder)."""
        vs = []
        for adm, c in self.curve():
            succ, canc = c["b_succ_tps"], c["b_cancelled_per_s"]
            if succ is None or canc is None or not c["target_qps"]:
                continue
            if adm > c["target_qps"]:
                continue
            if succ < 0.7 * adm and canc / c["target_qps"] < 0.1:
                vs.append(succ)
        vs.sort()
        return vs[len(vs) // 2] if vs else None


def facets(points, size=len(RAMP5)):
    return [points[i : i + size] for i in range(0, len(points), size)]


def ramp(k):
    if k == 1:
        return [RAMP5[2]]
    return [RAMP5[round(i * (len(RAMP5) - 1) / (k - 1))] for i in range(k)]


def style_axes(ax):
    ax.grid(True, which="major")
    ax.set_axisbelow(True)
    for side in ("top", "right"):
        ax.spines[side].set_visible(False)


def yerr_for_log(values, sds):
    """Error bars that stay positive on a log axis: the lower bar is cut at
    95 % of the value."""
    lo = [min(s or 0.0, 0.95 * v) if v else 0.0 for v, s in zip(values, sds)]
    hi = [s or 0.0 for s in sds]
    return [lo, hi]


def plot_admitted_rate(points, outdir):
    cols = facets(points)
    fig, axes = plt.subplots(
        2,
        len(cols),
        figsize=(6.4 * len(cols), 7.2),
        sharex=True,
        squeeze=False,
    )
    # One y range for every column, so the columns can be read against each
    # other.
    lags = [
        v
        for p in points
        for v in [p.a["lag_mean_s"]] + [c["b_lag_mean_s"] for _, c in p.curve()]
        if v
    ]
    lag_lim = (min(lags) * 0.7, max(lags) * 1.5) if lags else None
    for j, grp in enumerate(cols):
        top, bot = axes[0][j], axes[1][j]
        colors = ramp(len(grp))
        for p, c in zip(grp, colors):
            xs = [adm for adm, _ in p.curve()]
            lag = [cfg["b_lag_mean_s"] for _, cfg in p.curve()]
            frac = [
                cfg["b_cancelled_per_s"] / cfg["target_qps"] for _, cfg in p.curve()
            ]
            top.plot(xs, lag, "-o", color=c, lw=2, ms=5, label=p.name)
            bot.plot(xs, frac, "-o", color=c, lw=2, ms=5)
            if xs:
                top.annotate(
                    p.name,
                    (xs[-1], lag[-1]),
                    xytext=(6, 0),
                    textcoords="offset points",
                    color=INK2,
                    fontsize=8,
                    va="center",
                )
            # Run A: the same configuration measured in every config of this
            # point.
            if p.a["commit_rate"] and p.a["lag_mean_s"] is not None:
                xa = 10 * p.a["commit_rate"]
                top.plot(
                    xa,
                    p.a["lag_mean_s"],
                    "*",
                    color=c,
                    ms=13,
                    markeredgecolor=INK,
                    markeredgewidth=0.6,
                )
                if p.a["cancelled_per_s"] is not None:
                    qps = p.configs[0]["target_qps"] or 1
                    bot.plot(
                        xa,
                        p.a["cancelled_per_s"] / qps,
                        "*",
                        color=c,
                        ms=13,
                        markeredgecolor=INK,
                        markeredgewidth=0.6,
                    )
        xa_all = [10 * p.a["commit_rate"] for p in grp if p.a["commit_rate"]]
        if xa_all:
            xa = sum(xa_all) / len(xa_all)
            for ax in (top, bot):
                ax.axvline(xa, color=INK2, ls="--", lw=1.2)
        top.set_xscale("log")
        top.set_yscale("log")
        if lag_lim:
            top.set_ylim(*lag_lim)
        bot.set_ylim(-0.03, 1.03)
        style_axes(top)
        style_axes(bot)
        bot.set_xlabel("admitted rate (tx/s = tx/commit x commits/s, log)")
        if j == 0:
            top.set_ylabel("checkpoint lag mean (s, log)")
            bot.set_ylabel("cancelled fraction of offered")
        bot.legend(
            *top.get_legend_handles_labels(),
            frameon=False,
            fontsize=8,
            loc="upper right",
        )
    axes[0][0].set_title(
        "stars = Run A; dashed = Run A's admitted rate (10 per commit)",
        loc="left",
        color=INK2,
        fontsize=9,
    )
    fig.suptitle(
        "What a per-object limit admits vs what the object can execute",
        x=0.01,
        ha="left",
        fontsize=12,
    )
    fig.tight_layout(rect=(0, 0, 1, 0.96))
    fig.savefig(os.path.join(outdir, f"modes_admitted_rate{FILE_SUFFIX}.png"), dpi=150)
    plt.close(fig)


# (metric key, panel title, format, color scale)
HEAT_PANELS = [
    ("b_succ_tps", "success tps (colour on a log scale)", "{:.0f}", "log"),
    ("canc_frac", "cancelled fraction of offered", "{:.2f}", "unit"),
    ("b_lag_mean_s", "checkpoint lag mean (s, colour on a log scale)", "{:.2f}", "log"),
    ("b_lag_over_30s_share", "checkpoint lag: share over 30s", "{:.2f}", "unit"),
]

# Sequential blue, light -> dark (the reference ramp's 100..700 steps).
SEQ_RAMP = ["#cde2fb", "#9ec5f4", "#6da7ec", "#3987e5", "#256abf", "#184f95", "#0d366b"]


def heat_value(cfg, key):
    if key == "canc_frac":
        c, q = cfg["b_cancelled_per_s"], cfg["target_qps"]
        return c / q if (c is not None and q) else None
    return cfg[key]


def heat_a_value(p, key):
    if key == "canc_frac":
        c = p.a["cancelled_per_s"]
        q = p.configs[0]["target_qps"]
        return c / q if (c is not None and q) else None
    return p.a[key.replace("b_", "")]


def seq_norm(x, lo, hi, scale):
    """Map a value to [0, 1] for the sequential ramp."""
    if scale == "unit":
        return max(0.0, min(1.0, x))
    if scale == "log":
        if hi <= lo:
            return 0.0
        x = max(x, lo)
        return (math.log10(x) - math.log10(lo)) / (math.log10(hi) - math.log10(lo))
    return x / hi if hi > 0 else 0.0


def plot_heatmaps(points, outdir):
    limits = sorted({c["limit_b"] for p in points for c in p.configs})
    cmap = LinearSegmentedColormap.from_list("seq", SEQ_RAMP)
    nrow, ncol = len(points), len(limits)
    cw, ch = 0.66, 0.46  # inches per grid cell
    fig, axes = plt.subplots(
        2,
        2,
        figsize=(2 * cw * (ncol + 2.6), 2 * (ch * nrow + 1.4)),
        squeeze=False,
    )
    for ax, (key, title, valfmt, scale) in zip(axes.flat, HEAT_PANELS):
        # One color scale per panel, covering Run A and Run B alike, so equal
        # values are equal colors everywhere — including the Run A column.
        vals = [
            v
            for p in points
            for v in [heat_a_value(p, key)] + [heat_value(c, key) for c in p.configs]
            if v is not None and v > 0
        ]
        lo, hi = (min(vals), max(vals)) if vals else (1.0, 1.0)

        def cell(ax, x, y, v):
            if v is None:
                return
            t = seq_norm(v, lo, hi, scale)
            ax.add_patch(
                Rectangle((x, y), 1, 1, facecolor=cmap(t), edgecolor=SURFACE, lw=1.5)
            )
            ink = "#ffffff" if t > 0.55 else INK
            ax.text(
                x + 0.5,
                y + 0.5,
                valfmt.format(v),
                ha="center",
                va="center",
                color=ink,
                fontsize=7.5,
            )

        match_a = []  # the 10-per-commit configs, outlined last so nothing clips them
        for i, p in enumerate(points):
            y = nrow - 1 - i  # cheapest point on the top row
            cell(ax, -1.5, y, heat_a_value(p, key))
            for c in p.configs:
                x = limits.index(c["limit_b"])
                cell(ax, x, y, heat_value(c, key))
                if c["tx_per_commit"] == 10:
                    match_a.append((x, y))
        for x, y in match_a:
            ax.add_patch(
                Rectangle((x, y), 1, 1, facecolor="none", edgecolor=INK, lw=1.6)
            )
        # Limits along the top (Run A's count limit is always 10); the run
        # names sit centered underneath their columns.
        top_y = nrow + 0.2
        ax.text(-1.7, top_y, "CUs/tx", ha="right", va="bottom", fontsize=7.5, color=INK2)
        ax.text(-1.0, top_y, "10", ha="center", va="bottom", fontsize=7.5, color=INK2)
        for x, v in enumerate(limits):
            lbl = kfmt(v) + (" CUs" if x == ncol - 1 else "")
            ax.text(
                x + 0.5, top_y, lbl, ha="center", va="bottom", fontsize=7.5, color=INK2
            )
        ax.text(-1.0, -0.25, "Run A", ha="center", va="top", fontsize=7.5, color=INK2)
        ax.text(ncol / 2, -0.25, "Run B", ha="center", va="top", fontsize=7.5, color=INK2)
        ax.set_xlim(-1.6, ncol)
        ax.set_ylim(-0.75, nrow + 0.8)
        ax.set_xticks([])
        ax.set_yticks([nrow - 1 - i + 0.5 for i in range(len(points))])
        ax.set_yticklabels([kfmt(p.units) for p in points], fontsize=7.5, color=INK2)
        ax.set_title(title, loc="left", fontsize=9)
        ax.tick_params(length=0)
        for side in ax.spines.values():
            side.set_visible(False)
    fig.suptitle(
        "Per-config values; colour = magnitude, one scale per panel; "
        "dark outline = admits 10 per commit like Run A",
        fontsize=11,
    )
    fig.tight_layout(rect=(0, 0, 1, 0.97))
    fig.savefig(os.path.join(outdir, f"modes_heatmaps{FILE_SUFFIX}.png"), dpi=150)
    plt.close(fig)


def plot_utilization(points, outdir):
    """Lag and cancelled fraction against admitted rate divided by the drain
    rate. If cost only acts through how full the object is, the per-cost
    curves land on one line. Points with no config that measures a drain rate
    are left out."""
    withd = [(p, p.drain()) for p in points]
    withd = [(p, d) for p, d in withd if d]
    if len(withd) < 2:
        return
    fig, (top, bot) = plt.subplots(2, 1, figsize=(6.4, 7.8), sharex=True)
    colors = ramp(len(withd))
    for (p, d), c in zip(withd, colors):
        xs = [adm / d for adm, _ in p.curve()]
        lag = [cfg["b_lag_mean_s"] for _, cfg in p.curve()]
        frac = [cfg["b_cancelled_per_s"] / cfg["target_qps"] for _, cfg in p.curve()]
        top.plot(xs, lag, "-o", color=c, lw=2, ms=5, label=p.name)
        bot.plot(xs, frac, "-o", color=c, lw=2, ms=5)
    for ax in (top, bot):
        ax.axvline(1.0, color=INK2, ls="--", lw=1.2)
        style_axes(ax)
    top.set_xscale("log")
    top.set_yscale("log")
    ticks = [0.2, 0.5, 1, 2, 5]
    bot.set_xticks(ticks)
    bot.set_xticklabels([f"{t:g}" for t in ticks])
    bot.minorticks_off()
    bot.set_ylim(-0.03, 1.03)
    bot.set_xlabel("admitted rate / drain rate (log)")
    top.set_ylabel("checkpoint lag mean (s, log)")
    bot.set_ylabel("cancelled fraction of offered")
    top.legend(frameon=False, fontsize=8, loc="upper left")
    top.set_title(
        "dashed = the limit admits exactly what the object executes",
        loc="left",
        color=INK2,
        fontsize=9,
    )
    fig.suptitle(
        "The same curves over admitted rate divided by drain rate",
        x=0.01,
        ha="left",
        fontsize=12,
    )
    fig.tight_layout(rect=(0, 0, 1, 0.955))
    fig.savefig(os.path.join(outdir, f"modes_utilization{FILE_SUFFIX}.png"), dpi=150)
    plt.close(fig)


def plot_tradeoff(points, outdir):
    """Success against lag, one point per config: the lower-right corner is
    fast and stable, and a cost point's configs arc up and right as its limit
    loosens."""
    cols = facets(points)
    fig, axes = plt.subplots(
        1,
        len(cols),
        figsize=(6.0 * len(cols), 5.2),
        squeeze=False,
        sharey=True,
    )
    for j, grp in enumerate(cols):
        ax = axes[0][j]
        colors = ramp(len(grp))
        for p, c in zip(grp, colors):
            xs = [c2["b_succ_tps"] for c2 in p.configs if c2["b_succ_tps"] is not None]
            ys = [
                c2["b_lag_mean_s"] for c2 in p.configs if c2["b_succ_tps"] is not None
            ]
            ax.plot(xs, ys, "-", color=c, lw=1, alpha=0.5)
            ax.plot(xs, ys, "o", color=c, ms=6, label=p.name)
            if p.a["succ_tps"] is not None and p.a["lag_mean_s"] is not None:
                ax.plot(
                    p.a["succ_tps"],
                    p.a["lag_mean_s"],
                    "*",
                    color=c,
                    ms=14,
                    markeredgecolor=INK,
                    markeredgewidth=0.6,
                )
        ax.set_yscale("log")
        style_axes(ax)
        ax.set_xlabel("success tps (executed - cancelled - commits)")
        if j == 0:
            ax.set_ylabel("checkpoint lag mean (s, log)")
        ax.legend(frameon=False, fontsize=8, loc="upper right")
        ax.set_title(
            "lower right = fast and stable", loc="left", color=INK2, fontsize=9
        )
    fig.suptitle(
        "Throughput vs stability (dots = Run B limits, stars = Run A)",
        x=0.01,
        ha="left",
        fontsize=12,
    )
    fig.tight_layout(rect=(0, 0, 1, 0.94))
    fig.savefig(os.path.join(outdir, f"modes_tradeoff{FILE_SUFFIX}.png"), dpi=150)
    plt.close(fig)


def plot_matched(points, outdir):
    """The configs where LIMIT_B is ten times the cost: Run A next to Run B,
    error bars of one standard deviation across iterations."""
    pairs = [(p, p.matched()) for p in points]
    pairs = [(p, c) for p, c in pairs if c]
    if not pairs:
        return
    panels = (
        ("succ_tps", "success tps"),
        ("cancelled_per_s", "cancelled / s"),
        ("lag_mean_s", "checkpoint lag mean (s)"),
    )
    fig, axes = plt.subplots(1, len(panels), figsize=(4.6 * len(panels), 4.4))
    xs = list(range(len(pairs)))
    for ax, (key, title) in zip(axes, panels):
        a = [c[f"a_{key}"] for _, c in pairs]
        b = [c[f"b_{key}"] for _, c in pairs]
        asd = [c.get(f"a_{key}_sd") for _, c in pairs]
        bsd = [c.get(f"b_{key}_sd") for _, c in pairs]
        ax.errorbar(
            [x - 0.12 for x in xs],
            a,
            yerr=yerr_for_log(a, asd),
            fmt="*",
            color=A_COLOR,
            ms=11,
            markeredgecolor=INK,
            markeredgewidth=0.5,
            capsize=2,
            lw=1,
        )
        ax.errorbar(
            [x + 0.12 for x in xs],
            b,
            yerr=yerr_for_log(b, bsd),
            fmt="o",
            color=B_COLOR,
            ms=6,
            capsize=2,
            lw=1,
        )
        for x, va, vb, sa, sb in zip(xs, a, b, asd, bsd):
            if va and vb:
                ax.annotate(
                    f"{vb / va:.2f}",
                    (x, max(va + (sa or 0), vb + (sb or 0))),
                    xytext=(0, 6),
                    textcoords="offset points",
                    ha="center",
                    fontsize=6.5,
                    color=INK2,
                )
        ax.set_yscale("log")
        ax.set_xticks(xs)
        ax.set_xticklabels([p.name for p, _ in pairs], rotation=45, ha="right", fontsize=7.5)
        ax.set_title(title, loc="left")
        style_axes(ax)
        ax.grid(False, axis="x")
    fig.legend(
        [
            plt.Line2D([], [], color=A_COLOR, marker="*", ls="", ms=11, markeredgecolor=INK),
            plt.Line2D([], [], color=B_COLOR, marker="o", ls="", ms=6),
        ],
        [A_NAME, "Run B — TotalComputationUnits, limit 10 × cost"],
        loc="upper right",
        frameon=False,
        fontsize=8,
        ncol=2,
        bbox_to_anchor=(0.99, 0.985),
    )
    fig.suptitle(
        "One cost per run: the count limit and its unit-limit equivalent agree\n"
        "error bars = one standard deviation across iterations; number = B / A",
        x=0.01,
        ha="left",
        fontsize=12,
    )
    fig.tight_layout(rect=(0, 0, 1, 0.9))
    fig.savefig(os.path.join(outdir, f"modes_matched{FILE_SUFFIX}.png"), dpi=150)
    plt.close(fig)


def load_hist(path):
    """admits_hist.csv -> {label: {run: {le: count}}} (per-bucket counts)."""
    hist = {}
    if not os.path.exists(path):
        return hist
    for r in csv.DictReader(open(path)):
        run = r.get("run") or r.get("arm")
        hist.setdefault(r["label"], {}).setdefault(run, {})[r["le"]] = float(r["count"])
    return hist


def bucket_labels(edges):
    """Histogram upper edges -> range labels: ≤1, 1–2, ..., >700."""
    out, prev = [], None
    for le in edges:
        if le == "+Inf":
            out.append(f">{prev:g}")
        elif prev is None:
            out.append(f"≤{float(le):g}")
        else:
            out.append(f"{prev:g}–{float(le):g}")
        prev = None if le == "+Inf" else float(le)
    return out


def mix_meta(c):
    """(mean units, expensive weight %, LIMIT_B) parsed from a mix label."""
    parts = c["label"].split("-")
    weight = int(parts[1].lstrip("w"))
    return c["units_per_tx"], weight, c["limit_b"]


def expensive_level(c):
    """The expensive level's cost of a two-level mix whose cheap level is
    1,000 units: from mean = cheap + weight x (expensive - cheap), rounded to
    the level itself rather than the noisy mean."""
    mean, weight, _ = mix_meta(c)
    return round(1000 + (mean - 1000) / (weight / 100.0), -3)


def plot_mix(configs, hist, outdir):
    """The mixes at LIMIT_B = 10 x mean: one panel per config with the share
    of commits admitting each number of transactions, Run A next to Run B;
    below, the outcome."""
    configs = sorted(configs, key=lambda c: c["units_per_tx"] or 0)
    edges = sorted(
        {
            le
            for c in configs
            for by_run in hist.get(c["label"], {}).values()
            for le in by_run
        },
        key=lambda x: float("inf") if x == "+Inf" else float(x),
    )
    labels = bucket_labels(edges)
    n = len(configs)
    per_row = 6
    rows = -(-n // per_row)
    fig = plt.figure(figsize=(2.6 * per_row, 3.6 * rows + 3.4))
    gs = fig.add_gridspec(rows + 1, per_row, height_ratios=[1.0] * rows + [0.95])
    ys = list(range(len(edges)))
    for j, c in enumerate(configs):
        ax = fig.add_subplot(gs[j // per_row, j % per_row])
        for run, color, off in (("a", A_COLOR, 0.19), ("b", B_COLOR, -0.19)):
            by = hist.get(c["label"], {}).get(run, {})
            total = sum(by.values()) or 1.0
            shares = [by.get(le, 0.0) / total for le in edges]
            ax.barh(
                [y + off for y in ys], shares, height=0.36, color=color, linewidth=0
            )
            for y, s in zip(ys, shares):
                if s >= 0.05:
                    ax.text(
                        s + 0.02,
                        y + off,
                        f"{100 * s:.0f}%",
                        va="center",
                        fontsize=6.5,
                        color=INK2,
                    )
        ax.set_yticks(ys)
        ax.set_yticklabels(labels if j % per_row == 0 else [], fontsize=7)
        ax.set_xlim(0, 1.2)
        ax.set_xticks([0, 0.5, 1.0])
        ax.set_xticklabels(["0", "50%", "100%"], fontsize=7)
        ax.invert_yaxis()
        mean, weight, _ = mix_meta(c)
        ax.set_title(
            f"{c['point']}\n{mean / 1e3:.1f}K mean · {weight}% expensive", fontsize=8.5
        )
        style_axes(ax)
        ax.grid(False, axis="y")
        if j % per_row == 0:
            ax.set_ylabel("admitted per commit", fontsize=8)
    fig.axes[0].set_xlabel("share of commits", fontsize=8)

    # Bottom row: the outcome per config, A next to B. Run A's label is
    # right-aligned and Run B's left-aligned, and Run B's moves up one line
    # when the two values are close, so the labels never print on top of
    # each other.
    sub = gs[rows, :].subgridspec(1, 3, wspace=0.3)
    panels = (
        ("succ_tps", "success tps", "{:.0f}"),
        ("cancelled_per_s", "cancelled / s", "{:.0f}"),
        ("lag_mean_s", "checkpoint lag mean (s)", "{:.1f}"),
    )
    xs = list(range(n))
    for k, (key, title, valfmt) in enumerate(panels):
        ax = fig.add_subplot(sub[0, k])
        a_vals = [c[f"a_{key}"] or 0.0 for c in configs]
        b_vals = [c[f"b_{key}"] or 0.0 for c in configs]
        top = max(a_vals + b_vals) or 1.0
        for run, vals, color, off, ha in (
            ("a", a_vals, A_COLOR, -0.18, "right"),
            ("b", b_vals, B_COLOR, 0.18, "left"),
        ):
            ax.bar([x + off for x in xs], vals, width=0.34, color=color, linewidth=0)
            for x, v, va, vb in zip(xs, vals, a_vals, b_vals):
                lift = 7 if run == "b" and abs(va - vb) < 0.07 * top else 1
                ax.annotate(
                    valfmt.format(v),
                    (x + off, v),
                    xytext=(0, lift),
                    textcoords="offset points",
                    ha=ha,
                    va="bottom",
                    fontsize=6,
                    color=INK2,
                )
        ax.set_xticks(xs)
        ax.set_xticklabels(
            [c["point"] for c in configs], fontsize=7, rotation=45, ha="right"
        )
        ax.set_title(title)
        ax.set_ylim(0, ax.get_ylim()[1] * 1.12)
        style_axes(ax)
        ax.grid(False, axis="x")

    handles = [
        plt.Rectangle((0, 0), 1, 1, color=A_COLOR),
        plt.Rectangle((0, 0), 1, 1, color=B_COLOR),
    ]
    fig.legend(
        handles,
        [A_NAME, "Run B — TotalComputationUnits, limit 10 × mean cost"],
        loc="upper right",
        frameon=False,
        fontsize=8,
        ncol=2,
        bbox_to_anchor=(0.99, 0.99),
    )
    fig.suptitle(
        "Mixed cost at the count limit's equivalent: a count limit pins the "
        "number\nadmitted per commit, a unit limit lets it swing",
        x=0.01,
        ha="left",
        fontsize=12,
    )
    fig.tight_layout(rect=(0, 0, 1, 0.955), h_pad=1.6)
    fig.savefig(os.path.join(outdir, f"modes_mix{FILE_SUFFIX}.png"), dpi=150)
    plt.close(fig)


LADDER_PANELS = (
    ("succ_tps", "success tps", "{:.0f}"),
    ("cancelled_per_s", "cancelled / s", "{:.0f}"),
    ("lag_mean_s", "checkpoint lag mean (s)", "{:.1f}"),
    ("expensive_per_s", "expensive level executed / s", "{:.0f}"),
)


def ladder_axis(ax, xs):
    ax.set_xscale("log")
    ax.set_xticks(xs)
    ax.set_xticklabels([kfmt(x) for x in xs], fontsize=7)
    ax.minorticks_off()
    ax.set_ylim(bottom=0)
    style_axes(ax)


def plot_mix_ladders(ladders, outdir):
    """For each mix run at several limits: success, cancellations, lag and
    the expensive level's execution rate against LIMIT_B, Run A as the
    horizontal reference. `ladders` maps the mix name to its configs."""
    names = sorted(ladders, key=lambda k: ladders[k][0]["units_per_tx"] or 0)
    fig, axes = plt.subplots(
        len(LADDER_PANELS), len(names), figsize=(3.4 * len(names), 10.4), squeeze=False
    )
    for j, name in enumerate(names):
        cfgs = sorted(ladders[name], key=lambda c: c["limit_b"])
        mean, weight, _ = mix_meta(cfgs[0])
        expensive = expensive_level(cfgs[0])
        xs = [c["limit_b"] for c in cfgs]
        for i, (key, title, valfmt) in enumerate(LADDER_PANELS):
            ax = axes[i][j]
            b = [c.get(f"b_{key}") for c in cfgs]
            a_vals = [c.get(f"a_{key}") for c in cfgs if c.get(f"a_{key}") is not None]
            if any(v is None for v in b) or not a_vals:
                ax.set_visible(False)
                continue
            bsd = [c.get(f"b_{key}_sd") or 0.0 for c in cfgs]
            a = sum(a_vals) / len(a_vals)
            a_sds = [c.get(f"a_{key}_sd") for c in cfgs if c.get(f"a_{key}_sd")]
            a_sd = sum(a_sds) / len(a_sds) if a_sds else 0.0
            # The band from the expensive cost to twice it: the rungs that
            # admit one expensive transaction per commit.
            ax.axvspan(expensive, 2 * expensive, color=GRID, alpha=0.7, lw=0)
            ax.axhspan(a - a_sd, a + a_sd, color=A_COLOR, alpha=0.12, lw=0)
            ax.axhline(a, color=A_COLOR, lw=1.6, ls="--")
            ax.errorbar(xs, b, yerr=bsd, fmt="-o", color=B_COLOR, lw=2, ms=5, capsize=2)
            for x, v in zip(xs, b):
                ax.annotate(
                    valfmt.format(v),
                    (x, v),
                    xytext=(0, 7),
                    textcoords="offset points",
                    ha="center",
                    fontsize=6.5,
                    color=INK2,
                )
            ax.axvline(expensive, color=INK2, lw=1, ls=":")
            ax.axvline(10 * mean, color=MUTED, lw=1.2, ls="-.")
            ladder_axis(ax, xs)
            if i == 0:
                ax.set_title(
                    f"{name}\n1K / {kfmt(expensive)} units, {weight}% expensive",
                    fontsize=9,
                )
            if j == 0:
                ax.set_ylabel(title)
            if i == len(LADDER_PANELS) - 1:
                ax.set_xlabel("LIMIT_B (units per object per commit)", fontsize=8)
    fig.legend(
        [
            plt.Line2D([], [], color=A_COLOR, lw=1.6, ls="--"),
            plt.Line2D([], [], color=B_COLOR, lw=2, marker="o"),
            plt.Line2D([], [], color=INK2, lw=1, ls=":"),
            plt.Line2D([], [], color=MUTED, lw=1.2, ls="-."),
            Patch(facecolor=GRID, alpha=0.7),
        ],
        [
            A_NAME,
            B_NAME,
            "the expensive transaction's cost",
            "10 × mean cost",
            "the expensive cost to twice it",
        ],
        loc="upper left",
        frameon=False,
        fontsize=8,
        ncol=5,
        bbox_to_anchor=(0.01, 0.95),
    )
    fig.suptitle(
        "Which unit limit: the best one keeps what a commit admits inside one "
        "commit interval of execution time\n"
        "(one expensive transaction per commit at 1K/100K, three at 1K/10K); "
        "error bars and band = one standard deviation across iterations",
        x=0.01,
        ha="left",
        fontsize=12,
    )
    fig.tight_layout(rect=(0, 0, 1, 0.925))
    fig.savefig(os.path.join(outdir, f"modes_mix_ladders{FILE_SUFFIX}.png"), dpi=150)
    plt.close(fig)


def load_lag_slices(path, window=10):
    """lag_over_time.csv -> {label: {run: [(t_start, lag_mean), ...]}} for one
    slice width."""
    out = {}
    if not os.path.exists(path):
        return out
    for r in csv.DictReader(open(path)):
        if int(float(r["window_s"])) != window or r["lag_mean_s"] == "":
            continue
        out.setdefault(r["label"], {}).setdefault(r["run"], []).append(
            (float(r["t_start_s"]), float(r["lag_mean_s"]))
        )
    return out


def plot_lag_over_time(fine, coarse, outdir):
    """Checkpoint lag over the 300 s runs, Run A against Run B: the 10 s
    slices as a thin line, the 60 s slices as steps."""
    # A 60 s run leaves about six 10 s slices; anything with many more is a
    # longer run, which is what this figure is for.
    labels = sorted(l for l, by_run in fine.items() if max(map(len, by_run.values())) > 8)
    if not labels:
        return
    fig, axes = plt.subplots(
        1, len(labels), figsize=(6.2 * len(labels), 4.2), squeeze=False, sharey=True
    )
    top = 0.0
    for ax, label in zip(axes[0], labels):
        for run, color, name in (("a", A_COLOR, A_NAME), ("b", B_COLOR, B_NAME)):
            pts = sorted(fine[label].get(run, []))
            ax.plot(
                [t + 5 for t, _ in pts],
                [v for _, v in pts],
                "-o",
                color=color,
                lw=1.1,
                ms=3,
                alpha=0.55,
            )
            steps = sorted(coarse.get(label, {}).get(run, []))
            if steps:
                xs = [t for t, _ in steps] + [steps[-1][0] + 60]
                ys = [v for _, v in steps] + [steps[-1][1]]
                ax.step(xs, ys, where="post", color=color, lw=2.6, label=name)
            top = max(top, max((v for _, v in pts), default=0.0))
        ax.set_title(label, loc="left", fontsize=9)
        ax.set_xlabel("seconds into the run")
        style_axes(ax)
    axes[0][0].set_ylim(0, top * 1.08)
    axes[0][0].set_ylabel("checkpoint lag mean (s)")
    axes[0][0].legend(frameon=False, fontsize=8, loc="upper left")
    fig.suptitle(
        "Checkpoint lag over the 300 s runs: thin = 10 s slices, steps = 60 s "
        "slices (mean over the checkpoints built in the slice, all iterations)",
        x=0.01,
        ha="left",
        fontsize=12,
    )
    fig.tight_layout(rect=(0, 0, 1, 0.94))
    fig.savefig(os.path.join(outdir, "modes_lag_over_time.png"), dpi=150)
    plt.close(fig)


def strip_machine(label):
    """`mix20800-w20-lim208k-qps1000-ws` -> `mix20800-w20-lim208k-qps1000`: a
    trailing all-letter part names the machine, not the config."""
    return re.sub(r"-[a-z]+$", "", label)


def plot_two_machines(rows1, rows2, names, outdir):
    """The ladders both machines ran, one colour per machine: Run B as the
    line with markers, that machine's Run A as the dashed reference."""
    by1 = {r["label"]: r for r in rows1}
    by2 = {strip_machine(r["label"]): r for r in rows2}
    common = sorted(set(by1) & set(by2), key=lambda l: by1[l]["limit_b"])
    mixes = {}
    for l in common:
        mixes.setdefault(by1[l]["point"], []).append(l)
    mixes = {k: v for k, v in mixes.items() if len(v) >= 2}
    if not mixes:
        return
    fig, axes = plt.subplots(
        len(mixes),
        len(LADDER_PANELS),
        figsize=(3.6 * len(LADDER_PANELS), 3.6 * len(mixes)),
        squeeze=False,
    )
    for i, (mix, labels) in enumerate(sorted(mixes.items())):
        labels = sorted(labels, key=lambda l: by1[l]["limit_b"])
        xs = [by1[l]["limit_b"] for l in labels]
        expensive = expensive_level(by1[labels[0]])
        for j, (key, title, valfmt) in enumerate(LADDER_PANELS):
            ax = axes[i][j]
            ax.axvspan(expensive, 2 * expensive, color=GRID, alpha=0.7, lw=0)
            for by, color, marker, mname in (
                (by1, B_COLOR, "o", names[0]),
                (by2, SECOND_COLOR, "s", names[1]),
            ):
                b = [by[l].get(f"b_{key}") for l in labels]
                if any(v is None for v in b):
                    continue
                bsd = [by[l].get(f"b_{key}_sd") or 0.0 for l in labels]
                ax.errorbar(
                    xs, b, yerr=bsd, fmt=f"-{marker}", color=color, lw=2, ms=5, capsize=2
                )
                a_vals = [by[l].get(f"a_{key}") for l in labels if by[l].get(f"a_{key}") is not None]
                if a_vals:
                    ax.axhline(sum(a_vals) / len(a_vals), color=color, lw=1.4, ls="--")
            ladder_axis(ax, xs)
            if i == 0:
                ax.set_title(title, loc="left", fontsize=9)
            if j == 0:
                ax.set_ylabel(f"{mix}\n1K / {kfmt(expensive)} units")
            if i == len(mixes) - 1:
                ax.set_xlabel("LIMIT_B (units per object per commit)", fontsize=8)
    fig.legend(
        [
            plt.Line2D([], [], color=B_COLOR, lw=2, marker="o"),
            plt.Line2D([], [], color=B_COLOR, lw=1.4, ls="--"),
            plt.Line2D([], [], color=SECOND_COLOR, lw=2, marker="s"),
            plt.Line2D([], [], color=SECOND_COLOR, lw=1.4, ls="--"),
            Patch(facecolor=GRID, alpha=0.7),
        ],
        [
            f"Run B on {names[0]}",
            f"Run A on {names[0]}",
            f"Run B on {names[1]}",
            f"Run A on {names[1]}",
            "the expensive cost to twice it",
        ],
        loc="upper left",
        frameon=False,
        fontsize=8,
        ncol=5,
        bbox_to_anchor=(0.01, 0.94),
    )
    fig.suptitle(
        f"The same limits on two machines ({names[0]} and {names[1]}); "
        "error bars = one standard deviation across iterations",
        x=0.01,
        ha="left",
        fontsize=12,
    )
    fig.tight_layout(rect=(0, 0, 1, 0.9))
    fig.savefig(os.path.join(outdir, f"modes_two_machines{FILE_SUFFIX}.png"), dpi=150)
    plt.close(fig)


def main():
    results = sys.argv[1] if len(sys.argv) > 1 else "."
    second = sys.argv[2] if len(sys.argv) > 2 else None
    path = os.path.join(results, "summary.csv")
    if not os.path.exists(path):
        print(f"{path} not found — run aggregate.py first", file=sys.stderr)
        sys.exit(1)
    every_row = load_rows(path)
    all_rows = baseline(every_row, machine_suffix(every_row))
    if not all_rows:
        print(
            f"no configs in {path} at QPS={MAIN_QPS:g}, RUN_DURATION={MAIN_DURATION}"
            " with the burst off",
            file=sys.stderr,
        )
        sys.exit(1)
    left_out = len(every_row) - len(all_rows)
    unsafe = [r["label"] for r in all_rows if not r["safety_ok"]]
    if unsafe:
        print(f"WARN: safety-flagged labels included: {unsafe}", file=sys.stderr)
    # Mixed-cost labels get their own figures: their tx/commit is LIMIT_B over
    # a mean cost, so the cost-point figures would place them wrongly.
    mix = [r for r in all_rows if r["point"].startswith("mix")]
    rows = [r for r in all_rows if not r["point"].startswith("mix")]
    by_point = {}
    for r in rows:
        by_point.setdefault(r["point"], []).append(r)
    points = sorted(
        (Point(n, cs) for n, cs in by_point.items()),
        key=lambda p: p.units or 0,
    )
    outdir = os.path.join(results, "summary_plots")
    os.makedirs(outdir, exist_ok=True)
    if points:
        plot_admitted_rate(points, outdir)
        plot_heatmaps(points, outdir)
        plot_utilization(points, outdir)
        plot_tradeoff(points, outdir)
        plot_matched(points, outdir)
    if mix:
        # The 300 s runs are drawn over time, not on the ladders. Of the rest,
        # a mix run at several limits is a ladder; a mix at LIMIT_B = 10 x
        # mean is one of the matched configs.
        by_mix = {}
        for r in mix:
            by_mix.setdefault(r["point"], []).append(r)
        matched = [
            r
            for r in mix
            if r["units_per_tx"] and abs(r["limit_b"] / r["units_per_tx"] - 10) < 0.5
        ]
        ladders = {k: v for k, v in by_mix.items() if len(v) >= 2}
        hist = load_hist(os.path.join(results, "admits_hist.csv"))
        if matched:
            plot_mix(matched, hist, outdir)
        if ladders:
            plot_mix_ladders(ladders, outdir)
    lag_path = os.path.join(results, "lag_over_time.csv")
    plot_lag_over_time(load_lag_slices(lag_path, 10), load_lag_slices(lag_path, 60), outdir)
    if second:
        names = os.environ.get("MACHINES", "EPYC,WS").split(",")
        every_row2 = load_rows(os.path.join(second, "summary.csv"))
        rows2 = baseline(every_row2, machine_suffix(every_row2))
        plot_two_machines(all_rows, rows2, names, outdir)
    else:
        # Every other figure has just been redrawn; this one needs the second
        # results directory, so without it the file on disk is left behind at
        # whatever it was.
        kept = os.path.join(outdir, f"modes_two_machines{FILE_SUFFIX}.png")
        if os.path.exists(kept):
            print(
                f"NOTE: {os.path.basename(kept)} not redrawn — it needs a second"
                " results directory, e.g. plot.py results/matrix results/matrix-ws",
                file=sys.stderr,
            )
    print(
        f"{len(points)} cost point(s), {len(rows)} configs, {len(mix)} mixed-cost"
        f" config(s) -> {outdir}/modes_*{FILE_SUFFIX}.png",
        file=sys.stderr,
    )
    if left_out:
        print(
            f"{left_out} config(s) left out: not QPS={MAIN_QPS:g} /"
            f" RUN_DURATION={MAIN_DURATION} / burst off",
            file=sys.stderr,
        )


if __name__ == "__main__":
    main()
