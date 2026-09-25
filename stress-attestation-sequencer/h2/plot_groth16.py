#!/usr/bin/env python3
"""plot_groth16.py — W8 figure: groth16 native calls against Move code.

Reads results/probe/groth16-<machine>.csv and the same machine's
calibration-<machine>.csv, and renders, per machine,
results/probe/groth16_exec_vs_cu-<machine>.png: execution time per transaction
against computation units (log-log), one line per groth16 curve and function,
drawn over the `slow` points (Move code). Dotted diagonals mark a constant time
per CU.

Run with a matplotlib venv, e.g.:
  ../h1/.venv/bin/python plot_groth16.py
"""

import csv
import glob
import os

import matplotlib

matplotlib.use("Agg")
import matplotlib.pyplot as plt

HERE = os.path.dirname(os.path.abspath(__file__))
OUTDIR = os.path.join(HERE, "results", "probe")

# Colour follows the curve, line style the function.
CURVE_COLOR = {"bn254": "#2a78d6", "bls12381": "#eb6834"}
CURVE_NAME = {"bn254": "BN254", "bls12381": "BLS12-381"}
FUNCTION_STYLE = {"verify": ("o", "-"), "prepare": ("s", "--")}
SLOW_COLOR = "#52514e"
INK = "#2b2b2b"
MUTED = "#8a8985"
# The slow transactions at the 5,000,000 metering ceiling ran out of gas before
# finishing, so their time is not the time of the whole product.
CEILING_CU = 5_000_000
# From the 701st native call in a transaction, each call is charged as
# instructions, so these points sit far to the right at almost the same time.
FLAT_PRICE_CALLS = 700


def load(path):
    return list(csv.DictReader(open(path)))


def slug_of(path, prefix):
    return os.path.basename(path)[len(prefix) : -len(".csv")]


# How probe-test.md names the two machines; any other slug is shown as is.
MACHINE_NAME = {
    "ryzen-9-9950x3d": "the WS (Ryzen 9 9950X3D)",
    "epyc-9454p": "the EPYC (EPYC 9454P)",
}


for g16_path in sorted(glob.glob(os.path.join(OUTDIR, "groth16-*.csv"))):
    slug = slug_of(g16_path, "groth16-")
    machine = MACHINE_NAME.get(slug, slug.replace("-", " ").upper())
    rows = load(g16_path)
    slow_path = os.path.join(OUTDIR, f"calibration-{slug}.csv")
    slow = load(slow_path) if os.path.exists(slow_path) else []

    fig, ax = plt.subplots(figsize=(9.5, 6.5))

    # Constant time per CU: guides for reading the gap.
    xs = [700, 6_000_000]
    for us_per_cu, label_x in ((0.1, 4_500_000), (10, 1_300_000)):
        ax.plot(
            xs, [x * us_per_cu / 1000 for x in xs], ls=":", lw=1, color=MUTED, zorder=1
        )
        ax.text(
            label_x,
            label_x * us_per_cu / 1000 * 1.25,
            f"{us_per_cu:g} µs per CU",
            color=MUTED,
            fontsize=8,
            ha="right",
        )
    ax.text(
        1_080,
        0.12,
        "every transaction under 1,000 CUs\nis charged 1,000",
        color=MUTED,
        fontsize=8,
    )

    # Move code: the slow ladder (size 100), below the metering ceiling.
    lad = sorted(
        (
            r
            for r in slow
            if int(r["slow_size"]) == 100 and float(r["actual_cu"]) < CEILING_CU
        ),
        key=lambda r: int(r["product"]),
    )
    if lad:
        ax.plot(
            [float(r["actual_cu"]) for r in lad],
            [float(r["exec_mean_ms"]) for r in lad],
            "o-",
            color=SLOW_COLOR,
            ms=4,
            lw=1.5,
            label="slow (Move code)",
            zorder=2,
        )

    workloads = sorted(
        {(r["curve"], r["function"]) for r in rows},
        key=lambda w: (list(CURVE_COLOR).index(w[0]), list(FUNCTION_STYLE).index(w[1])),
    )
    for curve, function in workloads:
        pts = sorted(
            (r for r in rows if (r["curve"], r["function"]) == (curve, function)),
            key=lambda r: int(r["calls"]),
        )
        marker, style = FUNCTION_STYLE[function]
        ax.plot(
            [float(r["actual_cu"]) for r in pts],
            [float(r["exec_mean_ms"]) for r in pts],
            marker=marker,
            ls=style,
            color=CURVE_COLOR[curve],
            ms=5,
            lw=2,
            label=f"groth16 {CURVE_NAME[curve]} {function}",
            zorder=3,
        )

    # Point at the jump past 700 calls on the heaviest workload.
    tail = [
        r
        for r in rows
        if (r["curve"], r["function"]) == ("bn254", "verify")
        and int(r["calls"]) > FLAT_PRICE_CALLS
    ]
    if tail:
        last = max(tail, key=lambda r: int(r["calls"]))
        ax.annotate(
            "701, 710 and 750 calls: charged far more\n"
            "from the 701st native call, about the same time",
            xy=(float(last["actual_cu"]), float(last["exec_mean_ms"])),
            xytext=(1.1e5, 3.2e3),
            fontsize=8,
            color=INK,
            arrowprops={"arrowstyle": "->", "color": MUTED, "lw": 1},
        )

    ax.set_xscale("log")
    ax.set_yscale("log")
    ax.set_xlim(700, 6_000_000)
    ax.set_ylim(0.1, 20_000)
    ax.set_xlabel("computation units per transaction")
    ax.set_ylabel("execution time per transaction (ms)")
    ax.grid(True, which="major", ls=":", alpha=0.4)
    for side in ("top", "right"):
        ax.spines[side].set_visible(False)
    ax.legend(fontsize=8, loc="upper left", frameon=False)
    fig.suptitle(
        f"groth16 native calls against Move code on {machine}: "
        "execution time per transaction against its computation units.\n"
        "Each point is the mean of 400 executions (100 transactions on "
        "4 validators); each groth16 point adds calls to one function.",
        fontsize=10,
        ha="center",
    )
    fig.tight_layout()
    out = os.path.join(OUTDIR, f"groth16_exec_vs_cu-{slug}.png")
    fig.savefig(out, dpi=120)
    print("wrote", out)
