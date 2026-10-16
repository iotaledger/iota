#!/usr/bin/env python3
"""plot.py — the H2 figures cut down for a 7–10 minute talk.

Draws three slide figures into this folder from the data the other H2
scripts read, so they redraw whenever that data changes:

  4-exec-vs-cu.png     execution time per transaction against its computation
                       units on each machine, with the ≈50 ms between commits
                       (results/probe/calibration-*.csv).
  5-lag-over-time.png  checkpoint lag over the 300 s 1K/100K runs, count limit
                       against unit limit (results/matrix/lag_over_time.csv).
  6-machines.png       the 1K/100K and 1K/500K ladders on each machine: success
                       tps and checkpoint lag (results/matrix*/summary.csv). A
                       machine whose results directory is missing is left out.

The numbers are the order the slides show the figures in, carrying on from the
three H1 figures in ../../h1/slides-plot/.

Data loading and filters come from ../plot.py, so the slides match the full
figures, and the deck's look from ../../slides_style.py, which also says where
a figure goes on the slide. The figures have no titles: the slide title says
what each one shows. Run with the matplotlib venv the other plot scripts use:
  ../../h1/.venv/bin/python plot.py
"""

import csv
import importlib.util
import os

import matplotlib

matplotlib.use("Agg")
import matplotlib.pyplot as plt  # noqa: E402

HERE = os.path.dirname(os.path.abspath(__file__))
H2 = os.path.dirname(HERE)
RESULTS = os.path.join(H2, "results")
PROBE = os.path.join(RESULTS, "probe")

# ../plot.py, loaded under another name, since this file is plot.py too.
_spec = importlib.util.spec_from_file_location("h2plot", os.path.join(H2, "plot.py"))
h2 = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(h2)

# Loaded after ../plot.py, so the deck's settings replace the ones it sets.
_spec = importlib.util.spec_from_file_location(
    "slides_style", os.path.join(os.path.dirname(H2), "slides_style.py")
)
style = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(style)

COMMIT_MS = 50  # about the time between two consensus commits
# For the line above the cards: where the 5-minute runs ran, at what rate and
# how many times.
SETUP = (
    "AMD EPYC 9454P, 4 validators, 1 fullnode, requested 1K tx/s, 300 s,"
    " 10 iterations"
)
# The count limit is how the network runs today, in the deck's navy; the unit
# limit is in its accent colour.
A_COLOR = style.NAVY
B_COLOR = style.TEAL

# (name, probe CSV slug, matrix results directory, colour, marker, line style).
# The reference machine, the server type the mainnet validators run, is in the
# deck's accent colour, and its turbo-boost-on runs in a lighter shade of it.
MACHINES = [
    ("EPYC server", "epyc-9454p", "matrix", style.NAVY, "o", "-"),
    (
        "reference machine, turbo off",
        "xeon-gold-5412u",
        "matrix-ref",
        style.TEAL,
        "^",
        "-",
    ),
    (
        "reference machine, turbo on",
        "xeon-gold-5412u-turbo",
        "matrix-refturbo",
        style.LIGHT_TEAL,
        "^",
        "--",
    ),
]

# `slow_n` of the twelve cost points the mode comparison runs (matrix.sh),
# 1,000 to 5,000,000 CUs at size 100.
COST_N = (1, 70, 120, 160, 217, 267, 350, 516, 1015, 1848, 3511, 8000)


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
    ax.axhline(COMMIT_MS, ls=":", lw=1.5, color=style.GREY, zorder=1)
    ax.annotate(
        "≈50 ms between commits",
        (x, COMMIT_MS),
        xytext=(0, 6),
        textcoords="offset points",
        fontsize=13,
        color=style.GREY,
    )


def time_vs_units(ax):
    """Log axes for execution time per transaction against its CUs."""
    ax.set_xscale("log")
    ax.set_yscale("log")
    style.log_axis(ax.xaxis, style.units)
    style.log_axis(ax.yaxis, style.ms)
    ax.grid(True)
    ax.set_xlabel("computation units (CUs) per transaction")
    ax.set_ylabel("execution time per transaction")


def save(fig, name):
    style.save(fig, os.path.join(HERE, name))


def fig_exec_vs_cu():
    fig = plt.figure(figsize=style.SIZE)
    h = style.subtitle(fig, "4 validators, requested 5 tx/s, 20 s, 1 iteration")
    ax = style.panel(
        fig,
        0,
        0,
        style.SIZE[0],
        h,
        "The Best Unit Limit Depends on the Machine",
        "Mean per transaction on each machine. The labels are for a 100K-CU"
        " transaction",
        (1.25, 0.95, 0.4, None),
    )
    at_100k = []
    for name, slug, _, color, marker, ls in MACHINES:
        pts = cost_points(slug)
        if not pts:
            continue
        ax.plot(*zip(*pts), ls=ls, marker=marker, color=color, label=name)
        at = dict(pts).get(100_000)
        if at:
            at_100k.append((at, f"{at:.1f} ms", style.ink(color)))
    commit_line(ax, 1_100)
    time_vs_units(ax)
    ax.set_xlim(700, 7e6)
    ax.set_ylim(0.15, 700)
    style.label_column(
        ax,
        100_000,
        at_100k,
        -14,
        ha="right",
        fontsize=14,
        fontweight="bold",
        bbox=style.LABEL_BOX,
    )
    ax.legend(loc="lower right")
    save(fig, "4-exec-vs-cu.png")


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
    top = max(v for _, label in limits for pts in fine[label].values() for _, v in pts)
    fig = plt.figure(figsize=style.SIZE)
    h = style.subtitle(
        fig,
        SETUP,
        "1K and 100K CU transactions, 4 : 1. Thin line: mean per 10 s. Thick line:"
        " mean per minute",
    )
    for (x0, w), (lim, label) in zip(style.columns([1] * len(limits)), limits):
        ax = style.panel(
            fig, x0, 0, w, h, f"Unit limit {lim} CUs", pad=(1.0, 0.95, 1.0, None)
        )
        ends = []
        for run, color, name in (
            ("a", A_COLOR, "count limit, 10 tx"),
            ("b", B_COLOR, f"unit limit, {lim} CUs"),
        ):
            pts = sorted(fine[label].get(run, []))
            ax.plot(
                [t + 5 for t, _ in pts],
                [v for _, v in pts],
                "-o",
                color=color,
                lw=1.2,
                ms=4,
                alpha=0.4,
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
                ends.append((steps[-1][1], f"{steps[-1][1]:.1f} s", style.ink(color)))
        ax.set_xlim(0, 300)
        ax.set_ylim(0, top * 1.1)
        ax.yaxis.set_major_formatter(lambda v, _: f"{v:g} s")
        ax.grid(True)
        ax.set_xlabel("seconds since the run started")
        ax.set_ylabel("mean checkpoint lag")
        style.label_column(ax, 300, ends, 10, fontsize=14, fontweight="bold")
        ax.legend(loc="upper left")
    save(fig, "5-lag-over-time.png")


MIXES = (
    ("mix20800", "1K / 100K CUs\n4 : 1"),
    ("mix50900", "1K / 500K CUs\n9 : 1"),
)
MACHINE_PANELS = (
    ("succ_tps", "Successful tx/s", 1),
    ("lag_mean_s", "Mean checkpoint lag (s)", 1),
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
    fig = plt.figure(figsize=style.SIZE)
    fw, fh = style.SIZE
    h = style.subtitle(
        fig,
        "4 validators, 1 fullnode, requested 1K tx/s, 60 s, 10 iterations on the"
        " EPYC server, 5 on the reference machine",
    )
    style.card(
        fig,
        0,
        0,
        fw,
        h,
        "The unit limit on each machine",
        "Dotted line: the count limit on the same machine. Shaded: room for one"
        " expensive transaction per commit, not two",
    )
    # Inches: the axes grid's margins inside the card, and the gaps in it.
    left, right, bottom, top = 1.5, 0.35, 0.8, 1.5
    col_gap, row_gap = 0.7, 0.55
    ncol, nrow = len(MACHINE_PANELS), len(MIXES)
    aw = (fw - left - right - col_gap * (ncol - 1)) / ncol
    ah = (h - bottom - top - row_gap * (nrow - 1)) / nrow
    for i, (mix, mix_title) in enumerate(MIXES):
        limits = sorted(
            {
                r["limit_b"]
                for _, by, *_ in machines
                for r in by.values()
                if r["point"] == mix
            }
        )
        expensive = next(
            h2.expensive_level(r)
            for _, by, *_ in machines
            for r in by.values()
            if r["point"] == mix
        )
        for j, (key, title, scale) in enumerate(MACHINE_PANELS):
            ax = fig.add_axes(
                [
                    (left + j * (aw + col_gap)) / fw,
                    (bottom + (nrow - 1 - i) * (ah + row_gap)) / fh,
                    aw / fw,
                    ah / fh,
                ]
            )
            ax.axvspan(expensive, 2 * expensive, color=style.TINT, lw=0)
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
                        sum(a_vals) / len(a_vals) / scale, color=color, lw=1.8, ls=":"
                    )
            ax.set_xscale("log")
            ax.set_xticks(limits)
            ax.set_xticklabels([style.units(x) for x in limits], fontsize=13)
            ax.minorticks_off()
            ax.set_ylim(bottom=0)
            ax.grid(True)
            if i == 0:
                ax.set_title(title, loc="left")
            if j == 0:
                ax.set_ylabel(mix_title)
            if i == nrow - 1:
                ax.set_xlabel("unit limit, CUs per object per commit")
    handles = [
        plt.Line2D([], [], color=color, ls=ls, marker=marker, lw=2.5, ms=7)
        for _, _, color, marker, ls in machines
    ]
    fig.legend(
        handles,
        [name for name, *_ in machines],
        loc="upper right",
        bbox_to_anchor=((fw - 0.3) / fw, (h - 0.14) / fh),
        ncol=len(handles),
    )
    save(fig, "6-machines.png")


if __name__ == "__main__":
    fig_exec_vs_cu()
    fig_lag_over_time()
    fig_machines()
