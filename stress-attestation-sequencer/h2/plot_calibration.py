#!/usr/bin/env python3
"""plot_calibration.py — H2 calibration figures.

Reads the two calibration CSVs (EPYC + WS) and renders, into
results/summary_plots/:
  - cu_exec_vs_product.png : how CUs and internal exec time rise with the
    workload product n*size (log-log). CUs are machine-independent (one curve);
    exec time is per-machine.
  - exec_vs_cu.png : internal exec time vs CUs (log-log) — the per-transaction
    compute-cost curve, one line per machine.

Uses only the ladder (size=100) points for the trend lines; the split-invariance
points (product 40000) are overlaid as hollow markers to show they land on the
same curve.

Run with a matplotlib venv, e.g.:
  ../h1/.venv/bin/python plot_calibration.py
"""

import csv
import glob
import os

import matplotlib

matplotlib.use("Agg")
import matplotlib.pyplot as plt

HERE = os.path.dirname(os.path.abspath(__file__))
OUTDIR = os.path.join(HERE, "results", "summary_plots")
COLORS = ["tab:red", "tab:blue", "tab:green", "tab:purple", "tab:orange"]


def label_from(path):
    """calibration-<cpu-slug>.csv -> machine label (e.g. 'EPYC 9454P')."""
    slug = os.path.basename(path)[len("calibration-") : -len(".csv")]
    return slug.replace("-", " ").upper()


def load(path):
    return list(csv.DictReader(open(path)))


def ladder(rows):
    pts = [r for r in rows if int(r["slow_size"]) == 100]
    pts.sort(key=lambda r: int(r["product"]))
    return pts


def split(rows):
    pts = [r for r in rows if int(r["product"]) == 40000]
    pts.sort(key=lambda r: int(r["slow_n"]))
    return pts


def col(rows, key, f=float):
    return [f(r[key]) for r in rows]


# Discover every machine's sweep: results/calibration-<cpu-slug>.csv, one per
# machine (probe.sh names it from the CPU it ran on). Nothing hardcoded.
data = []
for i, path in enumerate(
    sorted(glob.glob(os.path.join(HERE, "results", "calibration-*.csv")))
):
    rows = load(path)
    data.append((label_from(path), COLORS[i % len(COLORS)], ladder(rows), split(rows)))
if not data:
    raise SystemExit("no calibration-*.csv found in results/")

os.makedirs(OUTDIR, exist_ok=True)

# ---- Figure 1: CUs and exec time vs product -----------------------------------
fig, (ax_cu, ax_ex) = plt.subplots(2, 1, figsize=(9, 8), sharex=True)
fig.suptitle("Calibration: computation units and execution time vs workload product")

# CUs are machine-independent — plot one curve, from the machine with the most
# ladder points (so the CU ceiling at the top of the ladder shows).
lad0 = max((d[2] for d in data), key=len)
spl0 = max((d[3] for d in data), key=len)
ax_cu.loglog(
    col(lad0, "product"),
    col(lad0, "actual_cu"),
    "o-",
    color="black",
    label="attested = actual CUs (machine-independent)",
)
ax_cu.loglog(
    col(spl0, "product"),
    col(spl0, "actual_cu"),
    "s",
    mfc="none",
    color="black",
    label="product 40000, n/size splits",
)
ax_cu.axhline(1000, ls=":", color="gray", lw=1)
ax_cu.text(120, 1080, "gas_rounding_step floor (1000)", color="gray", fontsize=8)
ax_cu.axhline(5_000_000, ls=":", color="crimson", lw=1)
ax_cu.text(
    120,
    5.3e6,
    "computation cap = 5M (CU flatlines below it)",
    color="crimson",
    fontsize=8,
)
ax_cu.set_ylabel("computation units")
ax_cu.grid(True, which="both", ls=":", alpha=0.4)
ax_cu.legend(fontsize=8)

for label, color, lad, spl in data:
    ax_ex.errorbar(
        col(lad, "product"),
        col(lad, "exec_mean_ms"),
        yerr=col(lad, "exec_sem_ms"),
        fmt="o-",
        color=color,
        capsize=2,
        label=label,
    )
    ax_ex.loglog(
        col(spl, "product"), col(spl, "exec_mean_ms"), "s", mfc="none", color=color
    )
ax_ex.set_xscale("log")
ax_ex.set_yscale("log")
ax_ex.set_xlabel("workload product  (n × size)")
ax_ex.set_ylabel("internal exec time (ms)")
ax_ex.grid(True, which="both", ls=":", alpha=0.4)
ax_ex.legend(fontsize=8)

fig.tight_layout()
fig.savefig(os.path.join(OUTDIR, "cu_exec_vs_product.png"), dpi=120)
print("wrote", os.path.join(OUTDIR, "cu_exec_vs_product.png"))

# ---- Figure 2: exec time vs CUs ----------------------------------------------
fig2, ax = plt.subplots(figsize=(9, 6))
fig2.suptitle("Calibration: internal execution time vs computation units")
for label, color, lad, spl in data:
    ax.errorbar(
        col(lad, "actual_cu"),
        col(lad, "exec_mean_ms"),
        yerr=col(lad, "exec_sem_ms"),
        fmt="o-",
        color=color,
        capsize=2,
        label=label,
    )
    ax.loglog(
        col(spl, "actual_cu"), col(spl, "exec_mean_ms"), "s", mfc="none", color=color
    )
ax.set_xscale("log")
ax.set_yscale("log")
ax.set_xlabel("computation units")
ax.set_ylabel("internal exec time (ms)")
ax.grid(True, which="both", ls=":", alpha=0.4)
ax.legend(fontsize=8)
fig2.tight_layout()
fig2.savefig(os.path.join(OUTDIR, "exec_vs_cu.png"), dpi=120)
print("wrote", os.path.join(OUTDIR, "exec_vs_cu.png"))
