#!/usr/bin/env python3
"""plot.py — the H2 figures cut down for a 7–10 minute talk.

Draws five slide figures (16:9) into this folder from the data the other H2
scripts read, so they redraw whenever that data changes:

  1-exec-vs-cu.png     execution time per transaction against its computation
                       units on each machine, with the ≈50 ms between commits
                       (results/probe/calibration-*.csv).
  2-fixed-limits.png   one cost per run: cancelled fraction and checkpoint lag
                       for the count limit and four unit limits, down the cost
                       points (results/matrix/summary.csv).
  3-lag-over-time.png  checkpoint lag over the 300 s 1K/100K runs, count limit
                       against unit limit (results/matrix/lag_over_time.csv).
  4-machines.png       the 1K/100K and 1K/500K ladders on each machine: success
                       tps, checkpoint lag and M units executed per second
                       (results/matrix*/summary.csv). A machine whose results
                       directory is missing is left out.
  5-groth16.png        groth16 native calls against Move code on the reference
                       machine (results/probe/groth16-*.csv).

Data loading, filters and colours come from ../plot.py, so the slides match
the full figures. Run with the matplotlib venv the other plot scripts use:
  ../../h1/.venv/bin/python plot.py
"""

import csv
import importlib.util
import os

import matplotlib

matplotlib.use("Agg")
import matplotlib.pyplot as plt  # noqa: E402
from matplotlib.colors import LinearSegmentedColormap  # noqa: E402
from matplotlib.patches import Patch, Rectangle  # noqa: E402

HERE = os.path.dirname(os.path.abspath(__file__))
H2 = os.path.dirname(HERE)
RESULTS = os.path.join(H2, "results")
PROBE = os.path.join(RESULTS, "probe")

# ../plot.py, loaded under another name, since this file is plot.py too.
_spec = importlib.util.spec_from_file_location("h2plot", os.path.join(H2, "plot.py"))
h2 = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(h2)

plt.rcParams.update(
    {
        "font.size": 14,
        "axes.titlesize": 15,
        "axes.labelsize": 14,
        "xtick.labelsize": 12,
        "ytick.labelsize": 12,
        "legend.fontsize": 12,
    }
)

SLIDE = (13.33, 7.5)  # inches, 16:9
DPI = 150
COMMIT_MS = 50  # about the time between two consensus commits

# (name, probe CSV slug, matrix results directory, colour, marker, line style).
# The colours follow ../plot.py's machine figure: EPYC orange, WS green,
# reference machine purple, and a lighter purple for its turbo-boost-on runs.
MACHINES = [
    ("EPYC", "epyc-9454p", "matrix", h2.B_COLOR, "o", "-"),
    ("WS", "ryzen-9-9950x3d", "matrix-ws", h2.OTHER_COLORS[0], "s", "-"),
    (
        "reference machine (turbo off)",
        "xeon-gold-5412u",
        "matrix-ref",
        h2.OTHER_COLORS[1],
        "^",
        "-",
    ),
    (
        "reference machine (turbo on)",
        "xeon-gold-5412u-turbo",
        "matrix-refturbo",
        "#b39ce6",
        "^",
        "--",
    ),
]

# `slow_n` of the twelve cost points the mode comparison runs (matrix.sh),
# 1,000 to 5,000,000 CUs at size 100.
COST_N = (1, 70, 120, 160, 217, 267, 350, 516, 1015, 1848, 3511, 8000)
CEILING_CU = 5_000_000


def load_csv(path):
    return list(csv.DictReader(open(path))) if os.path.exists(path) else []


def cost_points(slug):
    """(CUs, execution ms) at the cost points, from one machine's probe."""
    rows = {
        int(r["slow_n"]): r
        for r in load_csv(os.path.join(PROBE, f"calibration-{slug}.csv"))
        if r["slow_size"] == "100"
    }
    return [
        (float(rows[n]["actual_cu"]), float(rows[n]["exec_mean_ms"]))
        for n in COST_N
        if n in rows
    ]


def commit_line(ax, x):
    ax.axhline(COMMIT_MS, ls=":", lw=1.6, color=h2.INK2, zorder=1)
    ax.text(x, COMMIT_MS * 1.12, "≈50 ms between commits", color=h2.INK2, fontsize=12)


def save(fig, name):
    path = os.path.join(HERE, name)
    fig.savefig(path, dpi=DPI)
    plt.close(fig)
    print("wrote", path)


def fig_exec_vs_cu():
    fig, ax = plt.subplots(figsize=SLIDE)
    for name, slug, _, color, marker, ls in MACHINES:
        pts = cost_points(slug)
        if not pts:
            continue
        ax.plot(*zip(*pts), ls=ls, marker=marker, color=color, lw=2.5, ms=8, label=name)
        at = dict(pts).get(100_000)
        if at:
            ax.annotate(
                f"{at:.1f} ms",
                (100_000, at),
                xytext=(-12, 0),
                textcoords="offset points",
                ha="right",
                va="center",
                color=color,
                fontsize=12,
                fontweight="bold",
            )
    commit_line(ax, 1_100)
    ax.set_xscale("log")
    ax.set_yscale("log")
    ax.set_xlabel("computation units per transaction")
    ax.set_ylabel("execution time per transaction (ms)")
    h2.style_axes(ax)
    ax.legend(frameon=False, loc="upper left")
    fig.suptitle(
        "The same CUs take a different time on each machine",
        fontsize=18,
        fontweight="bold",
    )
    ax.set_title(
        "Move code at the twelve cost points of the mode comparison; the labels"
        " give the time of a 100K-CU transaction.",
        fontsize=12,
        color=h2.INK2,
    )
    fig.tight_layout()
    save(fig, "1-exec-vs-cu.png")


# Unit limits for figure 2: one for cheap transactions, one in the middle,
# two for expensive ones.
FIXED_LIMITS = (20_000, 100_000, 1_000_000, 5_000_000)
FIXED_PANELS = (
    ("canc_frac", "cancelled fraction of offered", "{:.2f}", "unit"),
    ("b_lag_mean_s", "checkpoint lag mean (s)", "{:.1f}", "log"),
)


def fig_fixed_limits():
    rows = h2.load_rows(os.path.join(RESULTS, "matrix", "summary.csv"))
    rows = [
        r
        for r in h2.baseline(rows, h2.machine_suffix(rows))
        if not r["point"].startswith("mix")
    ]
    by_point = {}
    for r in rows:
        by_point.setdefault(r["point"], []).append(r)
    points = sorted(
        (h2.Point(n, cs) for n, cs in by_point.items()), key=lambda p: p.units
    )
    cmap = LinearSegmentedColormap.from_list("seq", h2.SEQ_RAMP)
    nrow = len(points)
    # Run A's column, a gap, then one column per unit limit.
    xs = [0.0] + [1.4 + k for k in range(len(FIXED_LIMITS))]
    fig, axes = plt.subplots(1, 2, figsize=SLIDE)
    for ax, (key, title, valfmt, scale) in zip(axes, FIXED_PANELS):
        cells = []
        for i, p in enumerate(points):
            y = nrow - 1 - i  # cheapest point on the top row
            cells.append((xs[0], y, h2.heat_a_value(p, key)))
            by_limit = {c["limit_b"]: c for c in p.configs}
            for x, limit in zip(xs[1:], FIXED_LIMITS):
                c = by_limit.get(limit)
                cells.append((x, y, h2.heat_value(c, key) if c else None))
        vals = [v for _, _, v in cells if v is not None and v > 0]
        lo, hi = min(vals), max(vals)
        for x, y, v in cells:
            if v is None:
                continue
            t = h2.seq_norm(v, lo, hi, scale)
            ax.add_patch(
                Rectangle((x, y), 1, 1, facecolor=cmap(t), edgecolor=h2.SURFACE, lw=2)
            )
            ax.text(
                x + 0.5,
                y + 0.5,
                valfmt.format(v),
                ha="center",
                va="center",
                color="#ffffff" if t > 0.55 else h2.INK,
                fontsize=11,
            )
        # The count limit and the 100K unit limit, the two columns the talk reads.
        for x, color in ((xs[0], h2.A_COLOR), (xs[2], h2.B_COLOR)):
            ax.add_patch(
                Rectangle(
                    (x - 0.04, -0.04),
                    1.08,
                    nrow + 0.08,
                    facecolor="none",
                    edgecolor=color,
                    lw=3,
                )
            )
        top = nrow + 0.15
        ax.text(xs[0] + 0.5, top, "10 tx", ha="center", va="bottom", fontsize=12)
        for x, limit in zip(xs[1:], FIXED_LIMITS):
            ax.text(x + 0.5, top, h2.kfmt(limit), ha="center", va="bottom", fontsize=12)
        ax.text(
            xs[0] + 0.5,
            -0.3,
            "count\nlimit",
            ha="center",
            va="top",
            fontsize=12,
            color=h2.A_COLOR,
        )
        ax.text(
            (xs[1] + xs[-1] + 1) / 2,
            -0.3,
            "unit limit (CUs per object per commit)",
            ha="center",
            va="top",
            fontsize=12,
            color=h2.B_COLOR,
        )
        ax.set_xlim(-0.2, xs[-1] + 1.2)
        ax.set_ylim(-1.3, nrow + 0.9)
        ax.set_xticks([])
        ax.set_yticks([nrow - 1 - i + 0.5 for i in range(nrow)])
        ax.set_yticklabels([h2.kfmt(p.units) for p in points])
        ax.set_ylabel("CUs per transaction")
        ax.set_title(title, loc="left")
        ax.tick_params(length=0)
        for side in ax.spines.values():
            side.set_visible(False)
    fig.suptitle(
        "One cost per run: no fixed limit fits every cost",
        fontsize=18,
        fontweight="bold",
    )
    fig.text(
        0.5,
        0.9,
        "Read down a column: the same limit as transactions get more expensive."
        " Empty cells were not run.",
        ha="center",
        fontsize=12,
        color=h2.INK2,
    )
    fig.tight_layout(rect=(0, 0, 1, 0.9))
    save(fig, "2-fixed-limits.png")


def fig_lag_over_time():
    path = os.path.join(RESULTS, "matrix", "lag_over_time.csv")
    fine, coarse = h2.load_lag_slices(path, 10), h2.load_lag_slices(path, 60)
    limits = [
        (lim, f"mix20800-w20-lim{lim.lower()}-qps1000-dur300")
        for lim in ("100K", "150K")
    ]
    limits = [(lim, label) for lim, label in limits if label in fine]
    if not limits:
        return
    fig, axes = plt.subplots(1, len(limits), figsize=SLIDE, sharey=True, squeeze=False)
    top = 0.0
    for ax, (lim, label) in zip(axes[0], limits):
        for run, color, name in (
            ("a", h2.A_COLOR, "count limit, 10 tx"),
            ("b", h2.B_COLOR, f"unit limit, {lim} CUs"),
        ):
            pts = sorted(fine[label].get(run, []))
            ax.plot(
                [t + 5 for t, _ in pts],
                [v for _, v in pts],
                "-o",
                color=color,
                lw=1.2,
                ms=4,
                alpha=0.5,
            )
            steps = sorted(coarse.get(label, {}).get(run, []))
            if steps:
                ax.step(
                    [t for t, _ in steps] + [steps[-1][0] + 60],
                    [v for _, v in steps] + [steps[-1][1]],
                    where="post",
                    color=color,
                    lw=3.5,
                    label=name,
                )
            top = max(top, max((v for _, v in pts), default=0.0))
        ax.set_title(f"unit limit {lim}")
        ax.set_xlabel("time since the start of the run (s)")
        h2.style_axes(ax)
        ax.legend(frameon=False, loc="upper left")
    axes[0][0].set_ylabel("checkpoint lag mean (s)")
    axes[0][0].set_ylim(0, top * 1.1)
    fig.suptitle(
        "1K/100K mix over 300 s: the count limit's checkpoint lag keeps climbing",
        fontsize=18,
        fontweight="bold",
    )
    fig.text(
        0.5,
        0.885,
        "Thin line: mean per 10 s. Steps: mean per minute. EPYC, mix20800"
        " (1K and 100K CUs, 4:1).",
        ha="center",
        fontsize=12,
        color=h2.INK2,
    )
    fig.tight_layout(rect=(0, 0, 1, 0.88))
    save(fig, "3-lag-over-time.png")


MIXES = (
    ("mix20800", "1K / 100K CUs, 4:1"),
    ("mix50900", "1K / 500K CUs, 9:1"),
)
MACHINE_PANELS = (
    ("succ_tps", "success tps", 1),
    ("lag_mean_s", "checkpoint lag mean (s)", 1),
    ("units_per_s", "M units executed / s", 1e6),
)


def fig_machines():
    machines = []
    for name, _, rdir, color, marker, ls in MACHINES:
        path = os.path.join(RESULTS, rdir, "summary.csv")
        if not os.path.exists(path):
            continue
        rows = h2.load_rows(path)
        rows = h2.baseline(rows, h2.machine_suffix(rows))
        by = {h2.strip_machine(r["label"]): r for r in rows}
        machines.append((name, by, color, marker, ls))
    fig, axes = plt.subplots(
        len(MIXES), len(MACHINE_PANELS), figsize=SLIDE, squeeze=False
    )
    for i, (mix, mix_title) in enumerate(MIXES):
        limits = sorted(
            {r["limit_b"] for _, by, *_ in machines for r in by.values() if r["point"] == mix}
        )
        expensive = next(
            h2.expensive_level(r)
            for _, by, *_ in machines
            for r in by.values()
            if r["point"] == mix
        )
        for j, (key, title, scale) in enumerate(MACHINE_PANELS):
            ax = axes[i][j]
            ax.axvspan(expensive, 2 * expensive, color=h2.GRID, alpha=0.8, lw=0)
            for _, by, color, marker, ls in machines:
                cfgs = sorted(
                    (r for r in by.values() if r["point"] == mix),
                    key=lambda r: r["limit_b"],
                )
                cfgs = [r for r in cfgs if r.get(f"b_{key}") is not None]
                if len(cfgs) < 2:
                    continue
                ax.errorbar(
                    [r["limit_b"] for r in cfgs],
                    [r[f"b_{key}"] / scale for r in cfgs],
                    yerr=[(r.get(f"b_{key}_sd") or 0.0) / scale for r in cfgs],
                    ls=ls,
                    marker=marker,
                    color=color,
                    lw=2.5,
                    ms=7,
                    capsize=2,
                )
                a_vals = [r[f"a_{key}"] for r in cfgs if r.get(f"a_{key}") is not None]
                if a_vals:
                    ax.axhline(
                        sum(a_vals) / len(a_vals) / scale,
                        color=color,
                        lw=1.4,
                        ls=(0, (4, 3)),
                        alpha=0.8,
                    )
            ax.set_xscale("log")
            ax.set_xticks(limits)
            ax.set_xticklabels([h2.kfmt(x) for x in limits], fontsize=11)
            ax.minorticks_off()
            ax.set_ylim(bottom=0)
            h2.style_axes(ax)
            if i == 0:
                ax.set_title(title)
            if j == 0:
                ax.set_ylabel(f"{mix_title}\n{mix}", fontsize=13)
            if i == len(MIXES) - 1:
                ax.set_xlabel("unit limit (CUs per object per commit)", fontsize=12)
    handles = [
        plt.Line2D([], [], color=color, ls=ls, marker=marker, lw=2.5, ms=7)
        for _, _, color, marker, ls in machines
    ] + [
        plt.Line2D([], [], color=h2.INK2, lw=1.4, ls=(0, (4, 3))),
        Patch(facecolor=h2.GRID, alpha=0.8),
    ]
    texts = [name for name, *_ in machines] + [
        "count limit (same colour)",
        "fits one expensive tx per commit, not two",
    ]
    fig.legend(
        handles,
        texts,
        loc="upper center",
        bbox_to_anchor=(0.5, 0.94),
        ncol=3,
        frameon=False,
        fontsize=11,
    )
    fig.suptitle(
        "The best unit limit depends on how fast the machine executes",
        fontsize=18,
        fontweight="bold",
    )
    fig.tight_layout(rect=(0, 0, 1, 0.86))
    save(fig, "4-machines.png")


def fig_groth16(slug="xeon-gold-5412u", calls_at=700):
    slow = [p for p in cost_points(slug) if p[0] < CEILING_CU]
    rows = load_csv(os.path.join(PROBE, f"groth16-{slug}.csv"))
    if not slow or not rows:
        return
    fig, ax = plt.subplots(figsize=SLIDE)
    ax.plot(
        *zip(*slow),
        "-o",
        color=h2.INK2,
        lw=2.5,
        ms=7,
        label="Move code (slow)",
    )
    marked = None
    for curve, name, color in (
        ("bn254", "groth16 BN254 verify", "#2a78d6"),
        ("bls12381", "groth16 BLS12-381 verify", "#eb6834"),
    ):
        pts = sorted(
            (int(r["calls"]), float(r["actual_cu"]), float(r["exec_mean_ms"]))
            for r in rows
            if r["curve"] == curve
            and r["function"] == "verify"
            and 50 <= int(r["calls"]) <= calls_at
        )
        ax.plot(
            [cu for _, cu, _ in pts],
            [ms for _, _, ms in pts],
            "-o",
            color=color,
            lw=2.5,
            ms=7,
            label=f"{name} (50 to {calls_at} calls)",
        )
        if curve == "bn254" and pts and pts[-1][0] == calls_at:
            marked = pts[-1]
    if marked:
        _, cu, ms = marked
        lo = max(p for p in slow if p[0] <= cu)
        hi = min(p for p in slow if p[0] >= cu)
        move = lo[1] + (cu - lo[0]) / (hi[0] - lo[0]) * (hi[1] - lo[1])
        ax.annotate(
            "",
            xy=(cu, ms),
            xytext=(cu, move),
            arrowprops={"arrowstyle": "<->", "color": h2.INK, "lw": 1.6},
        )
        ax.text(
            cu * 1.12,
            (ms * move) ** 0.5,
            f"{ms / move:.0f}× longer at the same\n{cu / 1000:.0f}K CUs:"
            f" {ms:,.0f} ms against {move:.0f} ms",
            va="center",
            fontsize=13,
            fontweight="bold",
        )
    commit_line(ax, 1_100)
    ax.set_xscale("log")
    ax.set_yscale("log")
    ax.set_xlabel("computation units per transaction")
    ax.set_ylabel("execution time per transaction (ms)")
    h2.style_axes(ax)
    ax.legend(frameon=False, loc="upper left")
    fig.suptitle(
        "Native calls are charged far less than they take to execute",
        fontsize=18,
        fontweight="bold",
    )
    ax.set_title(
        "Reference machine (Xeon Gold 5412U, turbo off): groth16 proof checks"
        " against Move code of the same CUs.",
        fontsize=12,
        color=h2.INK2,
    )
    fig.tight_layout()
    save(fig, "5-groth16.png")


if __name__ == "__main__":
    fig_exec_vs_cu()
    fig_fixed_limits()
    fig_lag_over_time()
    fig_machines()
    fig_groth16()
