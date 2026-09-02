#!/usr/bin/env python3
"""plot.py — cross-label mode-comparison figures from summary.csv.

Reads <results>/summary.csv (written by aggregate.py, which owns all the
pooling arithmetic) and renders into <results>/summary_plots/:

  modes_knee.png      checkpoint lag and cancelled fraction against the
                      admitted rate (tx/commit x commits/s), one curve per
                      cost point. Run A is the same admitted rate in every
                      cell, so it collapses to one vertical line — where that
                      line sits right of a curve's knee the count limit
                      over-admits, left of it under-admits.
  modes_heatmaps.png  the read-any-cell view: annotated values (success tps,
                      cancelled fraction, lag mean, lag >30s share) for Run A
                      and every Run B
                      cell, colored by magnitude on one scale per panel so
                      equal values are equal colors everywhere.
  modes_tradeoff.png  success tps vs lag mean per cell; Run A starred. The
                      lower-right frontier is "fast and stable". Success tps
                      is executed - cancelled - commits (aggregate.py owns
                      the definition), so it excludes both the transactions
                      that were cancelled and the per-commit system ones.

The x-axis collapse works because tx/commit = LIMIT_B / units-per-tx: the
grid's two axes only act through their ratio, so cost points become curves
over one physical axis instead of a 4-variable table.

Needs matplotlib — run from a venv such as ../h1/.venv (like
plot_calibration.py); everything else is stdlib. Deliberately NOT a
dashboard replay like ../h1/plot.py: these are cross-label figures with
derived axes, in the spirit of plot_calibration.py.

Lag is the exact histogram mean throughout, not p95: the buckets step
25, 30, 60, 90, so a p95 landing past 30s is an interpolation across a
30-second bucket and two such values cannot be compared. The mean (from
the histogram _sum) and the >30s share carry no bucket error.

Usage: plot.py [results_dir]   (default .; expects summary.csv inside)
"""

import csv
import math
import os
import sys

import matplotlib

matplotlib.use("Agg")
import matplotlib.pyplot as plt  # noqa: E402
from matplotlib.colors import LinearSegmentedColormap  # noqa: E402
from matplotlib.patches import Rectangle  # noqa: E402

# Palette (dataviz reference, light mode). The 5-step ordinal blue ramp is the
# validated maximum for one panel — more cost points than that are faceted.
SURFACE = "#fcfcfb"
INK = "#0b0b0b"
INK2 = "#52514e"
MUTED = "#898781"
GRID = "#e1e0d9"
AXIS = "#c3c2b7"
RAMP5 = ["#86b6ef", "#5598e7", "#2a78d6", "#1c5cab", "#0d366b"]

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
            if k not in ("label", "mode_a", "mode_b", "safety_ok"):
                d[k] = fnum(r[k])
        d["point"] = r["label"].split("-")[0]
        rows.append(d)
    return rows


class Point:
    """One cost point: its cells across limits, plus the Run A reference."""

    def __init__(self, name, cells):
        self.name = name
        self.cells = sorted(cells, key=lambda c: c["limit_b"])
        self.units = self.cells[0]["units_per_tx"]
        # Run A is one measurement per cell of the same config; average them.
        self.a = {
            k: self._mean(f"a_{k}")
            for k in (
                "succ_tps",
                "cancelled_per_s",
                "lag_mean_s",
                "lag_over_30s_share",
                "commit_rate",
            )
        }

    def _mean(self, key):
        vs = [c[key] for c in self.cells if c[key] is not None]
        return sum(vs) / len(vs) if vs else None

    def admitted(self, c):
        rate = c["b_commit_rate"] or 20.0
        return c["tx_per_commit"] * rate

    def curve(self):
        """(admitted, cell) for cells that admit anything, x-sorted."""
        pts = [(self.admitted(c), c) for c in self.cells if c["tx_per_commit"]]
        return sorted(pts, key=lambda t: t[0])

    def drain(self):
        """Execution-bound plateau: success well below what the limit admits
        while cancellations are quiet means execution is the constraint, and
        the success rate IS the per-object drain rate. Cells whose limit
        admits more than the client offers are excluded — there the shortfall
        is the offered rate, not execution (cu1k's whole ladder)."""
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


def plot_knee(points, outdir):
    cols = facets(points)
    fig, axes = plt.subplots(
        2,
        len(cols),
        figsize=(6.4 * len(cols), 7.2),
        sharex=True,
        squeeze=False,
    )
    for j, grp in enumerate(cols):
        top, bot = axes[0][j], axes[1][j]
        colors = ramp(len(grp))
        for p, c in zip(grp, colors):
            xs = [adm for adm, _ in p.curve()]
            lag = [cell["b_lag_mean_s"] for _, cell in p.curve()]
            frac = [
                cell["b_cancelled_per_s"] / cell["target_qps"] for _, cell in p.curve()
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
            d = p.drain()
            if d:
                top.axvline(d, color=c, ls=":", lw=1, alpha=0.6)
            # Run A: same config measured in every cell of this point.
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
                    qps = p.cells[0]["target_qps"] or 1
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
        "stars = Run A; dashed = Run A's admitted rate; dotted = drain",
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
    fig.savefig(os.path.join(outdir, "modes_knee.png"), dpi=150)
    plt.close(fig)


# (metric key, panel title, format, color scale)
HEAT_PANELS = [
    ("b_succ_tps", "success tps", "{:.0f}", "linear"),
    ("canc_frac", "cancelled fraction of offered", "{:.2f}", "unit"),
    ("b_lag_mean_s", "checkpoint lag mean (s)", "{:.2f}", "log"),
    ("b_lag_over_30s_share", "checkpoint lag: share over 30s", "{:.2f}", "unit"),
]

# Sequential blue, light -> dark (the reference ramp's 100..700 steps).
SEQ_RAMP = ["#cde2fb", "#9ec5f4", "#6da7ec", "#3987e5", "#256abf", "#184f95", "#0d366b"]


def heat_value(cell, key):
    if key == "canc_frac":
        c, q = cell["b_cancelled_per_s"], cell["target_qps"]
        return c / q if (c is not None and q) else None
    return cell[key]


def heat_a_value(p, key):
    if key == "canc_frac":
        c = p.a["cancelled_per_s"]
        q = p.cells[0]["target_qps"]
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
    limits = sorted({c["limit_b"] for p in points for c in p.cells})
    cmap = LinearSegmentedColormap.from_list("seq", SEQ_RAMP)
    nrow, ncol = len(points), len(limits)
    fig, axes = plt.subplots(
        len(HEAT_PANELS),
        1,
        figsize=(1.1 * (ncol + 2.2), (0.62 * nrow + 1.25) * len(HEAT_PANELS)),
        squeeze=False,
    )
    for (ax,), (key, title, valfmt, scale) in zip(axes, HEAT_PANELS):
        # One color scale per panel, covering Run A and Run B alike, so equal
        # values are equal colors everywhere — including the Run A column.
        vals = [
            v
            for p in points
            for v in [heat_a_value(p, key)] + [heat_value(c, key) for c in p.cells]
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
                fontsize=8,
            )

        match_a = []  # ≡A cells, outlined last so neighbours can't clip them
        for i, p in enumerate(points):
            y = nrow - 1 - i  # cheapest point on the top row
            cell(ax, -1.5, y, heat_a_value(p, key))
            for c in p.cells:
                x = limits.index(c["limit_b"])
                cell(ax, x, y, heat_value(c, key))
                if c["tx_per_commit"] == 10:
                    match_a.append((x, y))
        for x, y in match_a:
            ax.add_patch(
                Rectangle((x, y), 1, 1, facecolor="none", edgecolor=INK, lw=1.6)
            )
        # Limits along the top (Run A's count limit is always 10); the arm
        # names sit centered underneath their columns.
        top_y = nrow + 0.2
        ax.text(-1.7, top_y, "CUs/tx", ha="right", va="bottom", fontsize=8, color=INK2)
        ax.text(-1.0, top_y, "10", ha="center", va="bottom", fontsize=8, color=INK2)
        for x, v in enumerate(limits):
            lbl = kfmt(v) + (" CUs" if x == ncol - 1 else "")
            ax.text(
                x + 0.5, top_y, lbl, ha="center", va="bottom", fontsize=8, color=INK2
            )
        ax.text(-1.0, -0.25, "Run A", ha="center", va="top", fontsize=8, color=INK2)
        ax.text(ncol / 2, -0.25, "Run B", ha="center", va="top", fontsize=8, color=INK2)
        ax.set_xlim(-1.6, ncol)
        ax.set_ylim(-0.75, nrow + 0.8)
        ax.set_xticks([])
        ax.set_yticks([nrow - 1 - i + 0.5 for i in range(len(points))])
        ax.set_yticklabels([kfmt(p.units) for p in points], fontsize=8, color=INK2)
        ax.set_title(title, loc="left", fontsize=9)
        ax.tick_params(length=0)
        for side in ax.spines.values():
            side.set_visible(False)
    fig.suptitle(
        "Per-cell values; color = magnitude, one scale per panel (lag mean\n"
        "on a log scale); dark outline = admits 10/commit like Run A",
        fontsize=11,
    )
    fig.tight_layout(rect=(0, 0, 1, 0.965))
    fig.savefig(os.path.join(outdir, "modes_heatmaps.png"), dpi=150)
    plt.close(fig)


def plot_tradeoff(points, outdir):
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
            xs = [
                cell["b_succ_tps"] for cell in p.cells if cell["b_succ_tps"] is not None
            ]
            ys = [
                cell["b_lag_mean_s"]
                for cell in p.cells
                if cell["b_succ_tps"] is not None
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
    fig.savefig(os.path.join(outdir, "modes_tradeoff.png"), dpi=150)
    plt.close(fig)


def plot_utilization(points, outdir):
    """The collapse test: lag vs admitted/drain. If cost only enters through
    utilization, the per-cost curves land on one master curve. Points with no
    execution-bound cell (nothing to measure drain from) are left out."""
    withd = [(p, p.drain()) for p in points]
    withd = [(p, d) for p, d in withd if d]
    if len(withd) < 2:
        return
    fig, ax = plt.subplots(figsize=(6.4, 4.6))
    colors = ramp(len(withd))
    for (p, d), c in zip(withd, colors):
        xs = [adm / d for adm, _ in p.curve()]
        ys = [cell["b_lag_mean_s"] for _, cell in p.curve()]
        ax.plot(xs, ys, "-o", color=c, lw=2, ms=5, label=p.name)
    ax.axvline(1.0, color=INK2, ls="--", lw=1.2)
    ax.set_xscale("log")
    ax.set_yscale("log")
    ticks = [0.2, 0.5, 1, 2, 5]
    ax.set_xticks(ticks)
    ax.set_xticklabels([f"{t:g}" for t in ticks])
    ax.minorticks_off()
    style_axes(ax)
    ax.set_xlabel("admitted rate / drain rate (utilization, log)")
    ax.set_ylabel("checkpoint lag mean (s, log)")
    ax.legend(frameon=False, fontsize=8, loc="upper left")
    ax.set_title(
        "dashed = utilization 1; curves coinciding means lag"
        " depends only on utilization",
        loc="left",
        color=INK2,
        fontsize=9,
    )
    fig.suptitle("The same curves over admitted/drain", x=0.01, ha="left", fontsize=12)
    fig.tight_layout(rect=(0, 0, 1, 0.94))
    fig.savefig(os.path.join(outdir, "modes_knee_utilization.png"), dpi=150)
    plt.close(fig)


def main():
    results = sys.argv[1] if len(sys.argv) > 1 else "."
    path = os.path.join(results, "summary.csv")
    if not os.path.exists(path):
        print(f"{path} not found — run aggregate.py first", file=sys.stderr)
        sys.exit(1)
    rows = load_rows(path)
    unsafe = [r["label"] for r in rows if not r["safety_ok"]]
    if unsafe:
        print(f"WARN: safety-flagged labels included: {unsafe}", file=sys.stderr)
    by_point = {}
    for r in rows:
        by_point.setdefault(r["point"], []).append(r)
    points = sorted(
        (Point(n, cs) for n, cs in by_point.items()),
        key=lambda p: p.units or 0,
    )
    outdir = os.path.join(results, "summary_plots")
    os.makedirs(outdir, exist_ok=True)
    plot_knee(points, outdir)
    plot_heatmaps(points, outdir)
    plot_tradeoff(points, outdir)
    plot_utilization(points, outdir)
    print(
        f"{len(points)} cost point(s), {len(rows)} cells -> {outdir}/modes_*.png",
        file=sys.stderr,
    )


if __name__ == "__main__":
    main()
