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
    summary.csv                       per-policy median + IQR table (all policies)
    summary.md                        same table, markdown-formatted
    grad-no-sem-shed/*.png            graduated pct sweep (sem inactive)
    just-lower-the-cap/*.png          graduated@1000 vs binary@500 alternative test
    max-sem-prod-ratio/*.png          sem-bound regime (peak/sem ≫ 1)

Per-group plots: ratio.png, overshoot.png, tps.png, tps-mean.png,
hold-p99.png, cv.png, cv-saturated.png, stddev.png, tradeoff.png. Group
membership is configured in the GROUPS list below.

Dependencies: pandas, matplotlib, numpy, tabulate.
"""

import json
import os
import sys
from pathlib import Path

import numpy as np
import pandas as pd
import matplotlib.pyplot as plt
from matplotlib.lines import Line2D
from matplotlib.patches import Patch


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
# For sem-bound configs (small sem, peak << max_pending) the meaningful
# overshoot is peak above the sem cap, not above max_pending.
df["overshoot_above_sem"] = (
    df["results.peak_inflight"] - df["validator.max_pending_local_submissions"]
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


# Legend explaining boxplot anatomy. Same set of patches works for every box-
# style figure in this script — keep one set, reuse via add_box_legend().
BOX_LEGEND_HANDLES = [
    Patch(facecolor="#c5d9f1", edgecolor="black", label="Q1-Q3 (IQR)"),
    Line2D([0], [0], color="C1", lw=2, label="Median"),
    Line2D([0], [0], color="black", lw=1, label="~1.5 × IQR"),
    Line2D([0], [0], marker="o", color="w",
           markerfacecolor="none", markeredgecolor="black",
           markersize=6, label="Outliers"),
]


def add_box_legend(ax):
    ax.legend(handles=BOX_LEGEND_HANDLES, loc="best", fontsize=8)


def shortened_labels(policies):
    """Strip fields that are constant across all policies in the group.
    E.g. ["max=1000 sem=2000 pct=100", "max=1000 sem=2000 pct=75"] →
         ["pct=100", "pct=75"] (only the varying field remains)."""
    parsed = [dict(part.split("=") for part in p.split()) for p in policies]
    varying = [k for k in ("max", "sem", "pct")
               if len({d[k] for d in parsed}) > 1]
    if not varying:
        return policies  # nothing varies (n=1 policy), keep full label
    return [" ".join(f"{k}={d[k]}" for k in varying) for d in parsed]


# ---------- experiment groups ----------
# Each group is a self-contained comparison answering one research question.
# Policies are listed in the order they should appear left-to-right in the
# boxplots. `ratio_col` and `overshoot_col` switch between max-bound and
# sem-bound framings depending on which cap is the binding constraint.
# `description` is written verbatim to README.md inside the group's folder.
GROUPS = [
    {
        "dir": "grad-no-sem-shed",
        "policies": [
            "max=1000 sem=2000 pct=100",
            "max=1000 sem=2000 pct=75",
            "max=1000 sem=2000 pct=50",
            "max=1000 sem=2000 pct=25",
        ],
        "ratio_col": "results.ratio_peak_over_max_pending",
        "ratio_label": "peak_inflight/max_pending",
        "overshoot_col": "overshoot_above_max",
        "overshoot_label": "peak_inflight−max_pending",
        # Stability metrics don't tell a clear story for this group at n=50 —
        # stddev is flat across policies (~450), CV(all) is noisy from
        # bimodal saturation, CV(saturated) shows only ~20% gain. Headline
        # lives in overshoot.png + tps.png. Re-evaluate if more iters land.
        "skip_plots": ["cv", "cv-saturated", "stddev"],
        "description": """\
# grad-no-sem-shed

**Question:** Does graduated shedding reduce peak overshoot at the max_pending
gate? How does the soft-zone width (controlled by `start_pct`) affect the
safety/throughput trade-off?

**Policies** (all `max_pending_transactions=1000`, `max_pending_local_submissions=2000`):

| pct | soft zone | semantics |
|---|---|---|
| 100 | (none) | binary: hard reject at max_pending |
| 75  | 750–1000 | graduated: gentle shedding in soft zone |
| 50  | 500–1000 | graduated: wider soft zone |
| 25  | 250–1000 | graduated: very early onset |

**Binding cap:** `max_pending` — sem is inert here (sem=2000 ≫ observed peak
~1000), so the only shedding policy actively engaging is the graduated one.

**Safety metric:** `peak_inflight / max_pending` — values >1.0 mean the validator
admitted more in-flight than its own max_pending promise. Lower = safer.
""",
    },
    {
        "dir": "just-lower-the-cap",
        "policies": [
            "max=500 sem=2000 pct=100",
            "max=1000 sem=2000 pct=50",
            "max=1000 sem=2000 pct=100",
        ],
        "ratio_col": "results.ratio_peak_over_max_pending",
        "ratio_label": "peak_inflight/max_pending",
        "overshoot_col": "overshoot_above_max",
        "overshoot_label": "peak_inflight−max_pending",
        # No clean stability story here either: CV(all) shows graduated worst
        # due to bimodality, stddev confounded with mean (max=500 mean ≪ max=1000),
        # only CV(sat) supports graduated and only on small n≈20 saturated subset.
        # Re-evaluate if more iters land.
        "skip_plots": ["cv", "cv-saturated", "stddev"],
        "description": """\
# just-lower-the-cap

**Question:** If graduated@1000/pct=50 effectively starts shedding around
in-flight=500, isn't that equivalent to binary at `max_pending=500`? I.e. would
just lowering the binary cap give the same safety + throughput?

**Policies:**

| label | semantics |
|---|---|
| max=500  sem=2000 pct=100 | binary at the tight cap (sheds starting at 500) |
| max=1000 sem=2000 pct=50  | graduated with soft zone 500–1000 |
| max=1000 sem=2000 pct=100 | binary at the loose cap (sheds starting at 1000) |

**What to look at:** Compare binary@500 vs graduated@1000-pct50 directly. If
graduated wins on either safety (lower peak) or throughput (higher useful_tps),
the soft zone is reclaiming real capacity that a lower binary cap throws away.
If they're equivalent, the simpler "just use a lower cap" alternative is no
worse than the more complex graduated policy.

**Safety metric:** `peak_inflight / max_pending`. Note this normalizes against
different caps (500 vs 1000), so absolute overshoot (`peak − max_pending`) is
also worth looking at.
""",
    },
    {
        "dir": "max-sem-prod-ratio",
        "policies": [
            "max=1000 sem=20 pct=100",
            "max=1000 sem=20 pct=50",
        ],
        "ratio_col": "results.ratio_peak_over_sem",
        "ratio_label": "peak_inflight/sem_cap",
        "overshoot_col": "overshoot_above_sem",
        "overshoot_label": "peak_inflight−sem_cap",
        # cv.png: CVs are identical between policies (~1.24), no story.
        # cv-saturated.png: saturation_75pct is defined vs max_pending, but
        # sem is the binding cap here — peak never reaches 75% of max, so
        # graduated has zero saturated iters and the box is empty.
        # stddev tells the only clean stability story here (85 vs 59, ~30%
        # reduction with graduated, comparable because means are similar).
        "skip_plots": ["cv", "cv-saturated"],
        "description": """\
# max-sem-prod-ratio

**Question:** When `max_pending_local_submissions` (sem) is the binding cap
rather than `max_pending_transactions` (the production-grade ratio of
max:sem = 50:1), does the graduated policy still help? And how dramatic is
the sem cap violation under heavy load?

**Policies** (all `max_pending_transactions=1000`, `max_pending_local_submissions=20`):

| pct | semantics |
|---|---|
| 100 | binary: hard reject at max_pending, but sem chokes admission first |
| 50  | graduated: soft zone 500–1000, but sem still chokes |

**Binding cap:** `sem` — peak in-flight typically sits near sem×10-20× (not near
max_pending). `peak / max_pending` is meaningless here because peak ≪ max. The
relevant safety ratio is `peak / sem` instead.

**Safety metric:** `peak / sem` — values ≫ 1 mean the validator advertised that
only N transactions could be in local submission, but allowed ~10-15N in flight.
""",
    },
]


# ---------- plotting helpers ----------
def boxplot(col, title, ylabel, out_path, policies, log=False, data_df=None, tick_labels=None):
    """Box plot of `col` grouped by `policies`. Writes to `out_path`.
    `tick_labels` overrides the x-axis labels (defaults to `policies`)."""
    src = data_df if data_df is not None else df
    if col not in src.columns:
        return
    data = [src.loc[src.policy == p, col].dropna().values for p in policies]
    if not any(len(d) for d in data):
        return
    labels = tick_labels if tick_labels is not None else policies
    fig, ax = plt.subplots(figsize=(max(8, 1.4 * len(policies)), 5.5))
    bp = ax.boxplot(data, tick_labels=labels, showfliers=True, patch_artist=True,
                    medianprops={"color": "C1", "linewidth": 2})
    for box in bp["boxes"]:
        box.set_facecolor("#c5d9f1")
    ax.set_title(title)
    ax.set_ylabel(ylabel)
    if log:
        ax.set_yscale("log")
    ax.grid(axis="y", alpha=0.3)
    plt.setp(ax.get_xticklabels(), fontsize=8)
    add_box_legend(ax)
    plt.tight_layout()
    plt.savefig(out_path, dpi=130)
    plt.close()
    print(f"Wrote {out_path}")


def plot_group(group):
    """Generate the full plot set for one experiment group."""
    out_dir = HERE / group["dir"]
    out_dir.mkdir(exist_ok=True)
    policies = group["policies"]
    ratio_col = group["ratio_col"]
    ratio_label = group["ratio_label"]
    overshoot_col = group["overshoot_col"]
    overshoot_label = group["overshoot_label"]
    # Drop constant-across-the-group fields from x-axis labels for readability.
    short_labels = shortened_labels(policies)
    label_for = dict(zip(policies, short_labels))

    print(f"\n=== Group: {group['dir']} ===")

    # README documenting the experiment setup.
    readme_path = out_dir / "README.md"
    readme_path.write_text(group["description"])
    print(f"Wrote {readme_path}")

    skip = set(group.get("skip_plots", ()))

    # ratio = peak / binding_cap. When the binding cap is constant across the
    # group's policies, ratio.png is just a rescaled version of overshoot.png
    # (same box shape, axis units differ). Skip to avoid redundancy. When the
    # cap varies (e.g. just-lower-the-cap mixes max=500 and max=1000), ratio
    # normalizes for the different caps and is the only fair safety metric.
    ratio_denom_field = {
        "results.ratio_peak_over_max_pending": "validator.max_pending_transactions",
        "results.ratio_peak_over_sem": "validator.max_pending_local_submissions",
    }[ratio_col]
    denom_values = df[df.policy.isin(policies)][ratio_denom_field].dropna().unique()
    if len(denom_values) > 1:
        boxplot(ratio_col,
                f"Peak overshoot ratio — {ratio_label} — [lower = safer]",
                ratio_label, out_dir / "ratio.png", policies, tick_labels=short_labels)
    boxplot(overshoot_col,
            f"Absolute overshoot — {overshoot_label} — [lower = safer]",
            "Transactions", out_dir / "overshoot.png", policies, tick_labels=short_labels)
    boxplot("results.useful_tps",
            "Useful TPS — [higher = better]",
            "tps", out_dir / "tps.png", policies, tick_labels=short_labels)

    # Mean useful TPS, one bar per policy. The boxplot above shows the full
    # distribution; this gives a single-number headline that's robust to the
    # bimodal-saturation effect that confuses the median.
    if "tps-mean" not in skip:
        means = [df.loc[df.policy == p, "results.useful_tps"].dropna().mean()
                 for p in policies]
        fig, ax = plt.subplots(figsize=(max(7, 1.4 * len(policies)), 5))
        bars = ax.bar(short_labels, means, color="#c5d9f1", edgecolor="black")
        for bar, value in zip(bars, means):
            ax.annotate(f"{value:.0f}",
                        xy=(bar.get_x() + bar.get_width() / 2, value),
                        xytext=(0, 3), textcoords="offset points",
                        ha="center", fontsize=9)
        ax.set_title("Mean useful TPS per policy — [higher = better]")
        ax.set_ylabel("Mean useful TPS")
        ax.grid(axis="y", alpha=0.3)
        plt.setp(ax.get_xticklabels(), fontsize=8)
        plt.tight_layout()
        plt.savefig(out_dir / "tps-mean.png", dpi=130)
        plt.close()
        print(f"Wrote {out_dir / 'tps-mean.png'}")

    # Permit hold time differentiates policies only when sem varies across them.
    # When sem is fixed within a group, hold time is essentially flat — skip the
    # plot to keep the output focused.
    sem_values = (
        df[df.policy.isin(policies)]["validator.max_pending_local_submissions"]
        .dropna().unique()
    )
    if len(sem_values) > 1:
        boxplot("results.permit_hold_p99",
                "Semaphore permit hold time — p99 — [lower = better]",
                "seconds", out_dir / "hold-p99.png", policies, tick_labels=short_labels)

    if "cv" not in skip:
        boxplot("inflight_cv",
                "In-flight stability — CV = std_dev / mean — [lower = smoother]",
                "Coefficient of Variation", out_dir / "cv.png", policies, tick_labels=short_labels)
    if "stddev" not in skip:
        boxplot("results.inflight_stddev",
                "In-flight std_dev — raw oscillation amplitude (not normalized)",
                "stddev (txs)", out_dir / "stddev.png", policies, tick_labels=short_labels)

    # Saturated-only CV: filter the data to iters where the system was actually
    # loaded for ≥30% of the run. Skipped automatically if no iters qualify
    # (typical for sem-bound configs where saturation_75pct ≈ 0).
    if "cv-saturated" not in skip and "results.saturation_75pct" in df.columns:
        sat_df = df[df["results.saturation_75pct"].fillna(0) > 0.3].copy()
        if any((sat_df["policy"] == p).any() for p in policies):
            boxplot("inflight_cv",
                    "In-flight CV — saturated iters only (sat75 > 0.3)",
                    "Coefficient of Variation",
                    out_dir / "cv-saturated.png", policies,
                    data_df=sat_df, tick_labels=short_labels)

    # Tradeoff scatter — uses the group's ratio metric on x-axis.
    _, ax = plt.subplots(figsize=(9, 6))
    for p in policies:
        sub = df[df.policy == p]
        ax.scatter(sub[ratio_col], sub["results.useful_tps"],
                   label=label_for[p], alpha=0.7, s=42, edgecolor="white")
    ax.set_xlabel(f"{ratio_label}  (← safer)")
    ax.set_ylabel("Useful TPS  (better →)")
    ax.set_title("Safety / throughput trade-off")
    ax.grid(alpha=0.3)
    ax.legend(fontsize=8, loc="best")
    plt.tight_layout()
    plt.savefig(out_dir / "tradeoff.png", dpi=130)
    plt.close()
    print(f"Wrote {out_dir / 'tradeoff.png'}")



# Generate per-group plots.
for group in GROUPS:
    # Skip groups where none of the listed policies are present in the data
    # (e.g. an unfinished sweep where some configs haven't run yet).
    if not any(p in df["policy"].values for p in group["policies"]):
        print(f"\n=== Group: {group['dir']} — SKIPPED (no data) ===")
        continue
    plot_group(group)

print("\nDone.")
