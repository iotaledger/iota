#!/usr/bin/env python3
"""
plot.py — turn cap-policy-sweep.jsonl into figures + a summary table.

Tells the "graduated vs binary load shedding" story across whatever policies
are present in the JSONL. Handles mixed schemas (older iters may lack
permit_hold / inflight_stddev / saturation_75pct / consensus_lat_p99).

Usage (from repo root):
    cap-policy-plot/.venv/bin/python cap-policy-plot/plot.py
    # or with an explicit input path:
    cap-policy-plot/.venv/bin/python cap-policy-plot/plot.py path/to/cap-policy-sweep.jsonl

Outputs (next to the script):
    summary.csv          per-policy median + IQR table
    summary.md           same table, markdown-formatted for pasting
    ratio.png            box: peak/max_pending ratio per policy (safety)
    overshoot.png        box: absolute peak − max_pending per policy
    tps.png              box: useful_tps per policy
    hold-p99.png         box: permit_hold_p99 per policy
    cv.png               box: inflight CV per policy (stability)
    tradeoff.png         scatter: tps vs ratio, one point per iter
    tps-by-sat.png       grouped bar: median tps split by saturation bucket

Dependencies: pandas, matplotlib, numpy, tabulate.
"""

import json
import os
import sys
from pathlib import Path

import numpy as np
import pandas as pd
import matplotlib.pyplot as plt


# ---------- paths ----------
HERE = Path(__file__).resolve().parent
DEFAULT_INPUT = HERE.parent / "cap-policy-sweep.jsonl"

PATH = Path(sys.argv[1]) if len(sys.argv) > 1 else DEFAULT_INPUT
if not PATH.exists():
    sys.exit(f"error: {PATH} not found (pass an explicit path as arg 1)")

# Always write outputs next to this script, no matter where it's invoked from.
os.chdir(HERE)

rows = []
for line in PATH.read_text().splitlines():
    line = line.strip()
    if not line:
        continue
    try:
        rows.append(json.loads(line))
    except json.JSONDecodeError:
        pass

if not rows:
    sys.exit(f"error: no JSON records parsed from {PATH}")

df = pd.json_normalize(rows)
print(f"Loaded {len(df)} records from {PATH}")

# ---------- filter ----------
if "failed" in df.columns:
    df = df[~df["failed"].fillna(False)]
if "results.exit_codes_ok" in df.columns:
    df = df[df["results.exit_codes_ok"].fillna(True)]
df = df.reset_index(drop=True)
print(f"After filtering failed iters: {len(df)} records")
if df.empty:
    sys.exit("nothing to plot")


# ---------- policy label + ordering ----------
def policy_label(r):
    return (
        f"max={int(r['validator.max_pending_transactions'])} "
        f"sem={int(r['validator.max_pending_local_submissions'])} "
        f"pct={int(r['validator.graduated_load_shedding_soft_limit_pct'])}"
    )


df["policy"] = df.apply(policy_label, axis=1)

# Order: by (max_pending asc, sem asc, start_pct desc).
# Putting binary (pct=100) leftmost makes the "as we graduate more, things get
# better" story read naturally left-to-right when comparing within a max/sem group.
order_key = df.groupby("policy").agg(
    max_=("validator.max_pending_transactions", "first"),
    sem_=("validator.max_pending_local_submissions", "first"),
    pct_=("validator.graduated_load_shedding_soft_limit_pct", "first"),
)
order_key["pct_desc"] = -order_key["pct_"]
policy_order = order_key.sort_values(["max_", "sem_", "pct_desc"]).index.tolist()


# ---------- derived columns ----------
df["overshoot_above_max"] = (
    df["results.peak_inflight"] - df["validator.max_pending_transactions"]
)
if "results.inflight_stddev" in df.columns and "results.inflight_mean" in df.columns:
    mean = df["results.inflight_mean"].replace(0, np.nan)
    df["inflight_cv"] = df["results.inflight_stddev"] / mean
else:
    df["inflight_cv"] = np.nan


# ---------- summary table ----------
def q(s, p):
    s = s.dropna()
    return float(s.quantile(p)) if len(s) else float("nan")


def safe_col(name):
    return df[name] if name in df.columns else pd.Series([np.nan] * len(df))


metrics = {
    "n": ("results.peak_inflight", "count"),
    "peak_med": ("results.peak_inflight", "median"),
    "peak_p25": ("results.peak_inflight", lambda s: q(s, 0.25)),
    "peak_p75": ("results.peak_inflight", lambda s: q(s, 0.75)),
    "ratio_med": ("results.ratio_peak_over_max_pending", "median"),
    "ratio_p25": ("results.ratio_peak_over_max_pending", lambda s: q(s, 0.25)),
    "ratio_p75": ("results.ratio_peak_over_max_pending", lambda s: q(s, 0.75)),
    "overshoot_med": ("overshoot_above_max", "median"),
    "tps_med": ("results.useful_tps", "median"),
    "tps_p25": ("results.useful_tps", lambda s: q(s, 0.25)),
    "tps_p75": ("results.useful_tps", lambda s: q(s, 0.75)),
    "sat75_med": ("results.saturation_75pct", "median"),
    "hold_p99_med": ("results.permit_hold_p99", "median"),
    "cons_p99_med": ("results.consensus_lat_p99", "median"),
    "cv_med": ("inflight_cv", "median"),
}

# Only include aggregations whose source column exists.
agg_spec = {k: v for k, v in metrics.items() if v[0] in df.columns or k == "n"}
agg = df.groupby("policy").agg(**agg_spec).reindex(policy_order).round(3)

agg.to_csv("summary.csv")
print("Wrote summary.csv")

# Markdown table for easy paste into notes / Slack
with open("summary.md", "w") as f:
    f.write("# cap-policy-sweep summary\n\n")
    f.write(agg.to_markdown())
    f.write("\n")
print("Wrote summary.md")

# Console
print("\nPer-policy summary (median + IQR):")
print(agg.to_string())


# ---------- plotting helpers ----------
def boxplot(col, title, ylabel, fname, log=False):
    if col not in df.columns:
        return
    data = [df.loc[df.policy == p, col].dropna().values for p in policy_order]
    if not any(len(d) for d in data):
        return
    fig, ax = plt.subplots(figsize=(max(8, 1.4 * len(policy_order)), 5))
    bp = ax.boxplot(data, tick_labels=policy_order, showfliers=True, patch_artist=True)
    for box in bp["boxes"]:
        box.set_facecolor("#c5d9f1")
    ax.set_title(title)
    ax.set_ylabel(ylabel)
    if log:
        ax.set_yscale("log")
    ax.grid(axis="y", alpha=0.3)
    plt.setp(ax.get_xticklabels(), rotation=15, ha="right", fontsize=8)
    plt.tight_layout()
    plt.savefig(fname, dpi=130)
    plt.close()
    print(f"Wrote {fname}")


# ---------- boxplots ----------
boxplot(
    "results.ratio_peak_over_max_pending",
    "Peak overshoot ratio (peak_inflight / max_pending) — lower = safer",
    "ratio",
    "ratio.png",
)
boxplot(
    "overshoot_above_max",
    "Absolute overshoot above max_pending (peak − max_pending)",
    "transactions",
    "overshoot.png",
)
boxplot(
    "results.useful_tps",
    "useful_tps per policy — higher = better",
    "tps",
    "tps.png",
)
boxplot(
    "results.permit_hold_p99",
    "Permit hold time p99 (validator-side, seconds) — lower = better",
    "seconds",
    "hold-p99.png",
)
boxplot(
    "inflight_cv",
    "In-flight stability — coefficient of variation (stddev/mean), lower = smoother",
    "CV",
    "cv.png",
)

# ---------- tradeoff scatter ----------
fig, ax = plt.subplots(figsize=(9, 6))
for p in policy_order:
    sub = df[df.policy == p]
    ax.scatter(
        sub["results.ratio_peak_over_max_pending"],
        sub["results.useful_tps"],
        label=p,
        alpha=0.7,
        s=42,
        edgecolor="white",
    )
ax.set_xlabel("ratio_peak_over_max_pending  (← safer)")
ax.set_ylabel("useful_tps  (better →)")
ax.set_title("Safety / throughput trade-off — each point is one iter")
ax.grid(alpha=0.3)
ax.legend(fontsize=8, loc="best")
plt.tight_layout()
plt.savefig("tradeoff.png", dpi=130)
plt.close()
print("Wrote tradeoff.png")

# ---------- saturation-stratified TPS ----------
if "results.saturation_75pct" in df.columns:
    df["sat_bucket"] = pd.cut(
        df["results.saturation_75pct"],
        bins=[-0.01, 0.2, 0.4, 0.6, 1.01],
        labels=["<0.2", "0.2-0.4", "0.4-0.6", ">0.6"],
    )
    pivot = (
        df.groupby(["policy", "sat_bucket"], observed=False)["results.useful_tps"]
        .median()
        .unstack()
        .reindex(policy_order)
    )
    fig, ax = plt.subplots(figsize=(max(9, 1.4 * len(policy_order)), 5))
    pivot.plot(kind="bar", ax=ax, edgecolor="black")
    ax.set_title("Median useful_tps by saturation bucket — apples-to-apples view")
    ax.set_ylabel("median useful_tps")
    ax.set_xlabel("")
    ax.legend(title="saturation_75pct", fontsize=8)
    ax.grid(axis="y", alpha=0.3)
    plt.setp(ax.get_xticklabels(), rotation=15, ha="right", fontsize=8)
    plt.tight_layout()
    plt.savefig("tps-by-sat.png", dpi=130)
    plt.close()
    print("Wrote tps-by-sat.png")

print("\nDone.")
