#!/usr/bin/env python3
"""plot.py — the H1 figures cut down for a 5 minute talk.

Draws three slide figures into this folder from the summary tables
make_table.py writes (results/summary_table_*.csv), so they redraw whenever
those tables change:

  1-moveauth.png             the Move authenticator campaign: post-consensus
                             validation time and throughput with attestation
                             off and on.
  2-attestation-latency.png  what attestation adds per transaction on the
                             fullnode path of the 4-validator network: the
                             dry-run against the real execution, and
                             settlement finality with attestation off and on,
                             pale where the machine falls behind the requested
                             rate even with attestation off.
  3-cpu.png                  the busiest validator's extra CPU with
                             attestation on, at slow(50, 50) on 4 and 24
                             validators, with every transaction sent to one
                             validator and with them spread over all.

The numbers are the order the slides show the figures in; the H2 figures in
../../h2/slides-plot/ carry on from 4.

Every figure uses the 1,000 tx/s configurations. Data loading comes from
../summary_plot.py, and the deck's look from ../../slides_style.py, which also
says where a figure goes on the slide. The figures have no titles: the slide
title says what each one shows. Run with the H1 venv:
  ../.venv/bin/python plot.py
"""

import importlib.util
import os

import matplotlib

matplotlib.use("Agg")
import matplotlib.pyplot as plt  # noqa: E402
from matplotlib.ticker import PercentFormatter  # noqa: E402

HERE = os.path.dirname(os.path.abspath(__file__))
H1 = os.path.dirname(HERE)
RESULTS = os.path.join(H1, "results")

# ../summary_plot.py, loaded under another name so this file's name never
# shadows it.
_spec = importlib.util.spec_from_file_location(
    "h1plot", os.path.join(H1, "summary_plot.py")
)
h1 = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(h1)

_spec = importlib.util.spec_from_file_location(
    "slides_style", os.path.join(os.path.dirname(H1), "slides_style.py")
)
style = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(style)
# In the body text colour, like the x axis labels each figure sets.
plt.rcParams["ytick.labelcolor"] = style.SLATE

SLOW_SIZES = (0, 50, 100, 200, 500)
AUTH_CYCLES = (1, 5, 10, 20, 50)
QPS = 1000
# The heaviest slow size at which the machine delivers the requested rate on
# 24 validators too.
CPU_SIZE = 50
# For the line above the cards: where the runs ran and at what rate.
MACHINE = "AMD EPYC 9454P"
RATE = f"requested {QPS // 1000}K tx/s"
RUNS = "60 s, 10 iterations"
SETUP = f"{MACHINE}, 4 validators, 1 fullnode, {RATE}, {RUNS}"
# Attestation off is how the network runs today, in the deck's navy; on is in
# its accent colour.
OFF_COLOR = style.NAVY
ON_COLOR = style.TEAL


def table(name):
    return os.path.join(RESULTS, f"summary_table_{name}.csv")


def values(csv_path, metric, configs, version):
    """The metric's mean for each config, None where it is missing."""
    rows, _ = h1.load(csv_path, metric)
    out = []
    for c in configs:
        v = rows.get(c, {}).get(version, (float("nan"),))[0]
        out.append(v if v == v else None)
    return out


def ms_values(csv_path, metric, configs, version):
    """`values` for a metric in seconds, converted to milliseconds."""
    return [
        v * 1000 if v is not None else None
        for v in values(csv_path, metric, configs, version)
    ]


def slow_configs(path, n):
    return [f"slow{s}-owned-{path}-qps{QPS}-n{n}" for s in SLOW_SIZES]


def cu_label(cu):
    if cu is None:
        return ""
    return f"{cu / 1e6:.2g}M" if cu >= 1e6 else f"{cu / 1e3:.3g}K"


def num(v):
    """1307.2 -> 1,307; 94.85 -> 94.9; 4.8 -> 4.80."""
    if v >= 100:
        return f"{v:,.0f}"
    return f"{v:.1f}" if v >= 10 else f"{v:.2f}"


def grouped_bars(ax, series, label=None, faded=()):
    """One group of bars per x position; `series` is [(name, colour, values)].
    `label` writes each bar's value over it. The groups at the positions in
    `faded` are drawn pale, their labels on the PALE fill `fade` puts behind
    them."""
    width = 0.8 / len(series)
    for k, (name, color, vals) in enumerate(series):
        points = [
            (i, i + (k - (len(series) - 1) / 2) * width, v)
            for i, v in enumerate(vals)
            if v is not None
        ]
        bars = ax.bar(
            [x for _, x, _ in points],
            [v for _, _, v in points],
            width=width * 0.9,
            color=color,
            label=name,
        )
        for (i, x, v), bar in zip(points, bars):
            pale = i in faded
            if pale:
                bar.set_alpha(0.35)
            if label:
                ax.annotate(
                    label(v),
                    (x, v),
                    xytext=(0, 5),
                    textcoords="offset points",
                    ha="center",
                    va="bottom",
                    fontsize=13,
                    color=style.GREY if pale else style.SLATE,
                    bbox={**style.LABEL_BOX, "fc": style.PALE}
                    if pale
                    else style.LABEL_BOX,
                )
    ax.grid(True, axis="y")


def fade(ax, start, note):
    """A PALE fill from x = `start` to the right edge of ax, with `note` at
    its top."""
    lo, hi = ax.get_xlim()
    ax.axvspan(start, hi, color=style.PALE, lw=0, zorder=0)
    ax.set_xlim(lo, hi)
    ax.text(
        (start + hi) / 2,
        0.97,
        note,
        transform=ax.get_xaxis_transform(),
        ha="center",
        va="top",
        fontsize=13,
        color=style.GREY,
    )


def slow_axis(ax, cus):
    """The five slow sizes on the x axis, each labelled by its CUs."""
    ax.set_xticks(range(len(SLOW_SIZES)))
    ax.set_xticklabels(
        [cu_label(cu) for cu in cus], fontsize=15, color=style.SLATE
    )
    ax.set_xlabel("computation units per tx")


def save(fig, name):
    style.save(fig, os.path.join(HERE, name))


def fig_attestation_latency():
    csv_path = table("n4")
    configs = slow_configs("f1", 4)
    cus = values(csv_path, "CUs", configs, "V2")
    # Where the machine falls behind the requested rate with attestation off
    # too, finality measures that backlog more than attestation.
    behind = [
        i
        for i, tps in enumerate(values(csv_path, "TPS", configs, "V1"))
        if tps is not None and tps < 0.9 * QPS
    ]
    fig = plt.figure(figsize=style.SIZE)
    h = style.subtitle(fig, SETUP)
    (x1, w1), (x2, w2) = style.columns((1, 1))
    pad = (1.3, 1.0, 0.35, None)
    ax1 = style.panel(fig, x1, 0, w1, h, "Dry-run vs execution time", pad=pad)
    grouped_bars(
        ax1,
        [
            (
                "dry-run (attestation)",
                style.TEAL,
                ms_values(csv_path, "attest. exec p95 (s)", configs, "V2"),
            ),
            (
                "real execution",
                style.GREY,
                ms_values(csv_path, "exec. lat. p95 (s)", configs, "V2"),
            ),
        ],
        label=num,
    )
    ax1.set_yscale("log")
    style.log_axis(ax1.yaxis, style.ms)
    ax1.set_ylim(0.5, 1e4)
    ax1.set_ylabel("95th percentile per tx")
    slow_axis(ax1, cus)
    ax1.legend(loc="upper left")
    ax2 = style.panel(fig, x2, 0, w2, h, "Settlement finality", pad=pad)
    grouped_bars(
        ax2,
        [
            (
                "attestation off",
                OFF_COLOR,
                values(csv_path, "final. lat. p50 (s)", configs, "V1"),
            ),
            (
                "attestation on",
                ON_COLOR,
                values(csv_path, "final. lat. p50 (s)", configs, "V2"),
            ),
        ],
        label=lambda v: f"{v:.2f}" if v < 1 else f"{v:.1f}",
        faded=behind,
    )
    ax2.set_yscale("log")
    style.log_axis(ax2.yaxis, lambda v: f"{v:g} s")
    ax2.set_ylim(0.1, 60)
    ax2.set_ylabel("median settlement finality")
    slow_axis(ax2, cus)
    if behind:
        fade(ax2, min(behind) - 0.5, "Machine overloaded,\neven with attestation off")
    ax2.legend(loc="upper left")
    save(fig, "2-attestation-latency.png")


def fig_cpu():
    fig = plt.figure(figsize=style.SIZE)
    paths = (
        ("v1", "every transaction\nto one validator", style.ORANGE),
        ("vN", "spread over\nall validators", style.TEAL),
    )
    h = style.subtitle(fig, f"{MACHINE}, {RATE}, {RUNS}")
    for (x, w), n in zip(style.columns((1, 1)), (4, 24)):
        ax = style.panel(
            fig, x, 0, w, h, f"{n} validators", pad=(1.25, 0.9, 0.35, None)
        )
        csv_path = table(f"n{n}")
        for i, (path, _, color) in enumerate(paths):
            p = f"v{n}" if path == "vN" else path
            config = [f"slow{CPU_SIZE}-owned-{p}-qps{QPS}-n{n}"]
            off = values(csv_path, "node CPU", config, "V1")[0]
            on = values(csv_path, "node CPU", config, "V2")[0]
            if not (off and on):
                continue
            extra = (on / off - 1) * 100
            ax.bar(i, extra, width=0.5, color=color)
            ax.annotate(
                f"{extra:+.0f}%",
                (i, extra),
                xytext=(0, 6),
                textcoords="offset points",
                ha="center",
                va="bottom",
                fontsize=16,
                fontweight="bold",
                color=style.ink(color),
            )
        ax.set_xticks(range(len(paths)))
        ax.set_xticklabels(
            [name for _, name, _ in paths], fontsize=15, color=style.SLATE
        )
        ax.set_xlim(-0.6, len(paths) - 0.4)
        ax.set_ylim(0, 50)
        ax.yaxis.set_major_formatter(PercentFormatter(decimals=0))
        ax.grid(True, axis="y")
        ax.set_ylabel("extra CPU on the busiest validator")
    save(fig, "3-cpu.png")


def fig_moveauth():
    csv_path = table("auth_n4")
    configs = [f"auth{c}-owned-f1-qps{QPS}-n4" for c in AUTH_CYCLES]
    fig = plt.figure(figsize=style.SIZE)
    h = style.subtitle(fig, SETUP)
    (x1, w1), (x2, w2) = style.columns((1, 1))
    pad = (1.3, 1.0, 0.35, None)
    ax1 = style.panel(fig, x1, 0, w1, h, "Post-consensus validation latency", pad=pad)
    grouped_bars(
        ax1,
        [
            (
                "attestation off",
                OFF_COLOR,
                ms_values(csv_path, "pc valid. lat. p95 (s)", configs, "V1"),
            ),
            (
                "attestation on",
                ON_COLOR,
                ms_values(csv_path, "pc valid. lat. p95 (s)", configs, "V2"),
            ),
        ],
        label=num,
    )
    ax1.set_yscale("log")
    style.log_axis(ax1.yaxis, style.ms)
    ax1.set_ylim(1, 2e3)
    # The pass is timed once per consensus commit, over all of its transactions.
    ax1.set_ylabel("95th percentile per commit")
    style.subtitle_legend(fig, ax1)
    ax2 = style.panel(fig, x2, 0, w2, h, "Throughput", pad=pad)
    grouped_bars(
        ax2,
        [
            ("attestation off", OFF_COLOR, values(csv_path, "TPS", configs, "V1")),
            ("attestation on", ON_COLOR, values(csv_path, "TPS", configs, "V2")),
        ],
        label=num,
    )
    ax2.axhline(QPS, color=style.GREY, lw=1.5, ls=":", zorder=1)
    ax2.text(
        len(AUTH_CYCLES) - 0.5,
        QPS + 25,
        "requested rate",
        ha="right",
        va="bottom",
        fontsize=13,
        color=style.GREY,
    )
    ax2.set_ylim(0, 1200)
    ax2.set_ylabel("finalized tx/s")
    for ax in (ax1, ax2):
        ax.set_xticks(range(len(AUTH_CYCLES)))
        ax.set_xticklabels(
            [str(c) for c in AUTH_CYCLES], fontsize=15, color=style.SLATE
        )
        ax.set_xlabel("Ed25519 signature verifications per tx")
    save(fig, "1-moveauth.png")


if __name__ == "__main__":
    fig_moveauth()
    fig_attestation_latency()
    fig_cpu()
