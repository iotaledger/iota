#!/usr/bin/env python3
"""
plot.py — turn sweep.jsonl into figures + a summary table.

Tells the "graduated vs binary load shedding" story across whatever policies
are present in the JSONL. Handles mixed schemas (older iters may lack
permit_hold / inflight_stddev / saturation_75pct / consensus_lat_p99 /
honest pool / timeseries).

Usage (from repo root):
    sweep-plot/.venv/bin/python sweep-plot/plot.py
    # or with an explicit input path (e.g. archived data files):
    sweep-plot/.venv/bin/python sweep-plot/plot.py path/to/sweep.jsonl

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
DEFAULT_INPUT = HERE.parent / "sweep.jsonl"

PATH = Path(sys.argv[1]) if len(sys.argv) > 1 else DEFAULT_INPUT
if not PATH.exists():
    sys.exit(f"error: {PATH} not found (pass an explicit path as arg 1)")

# Optional arg 2: suffix appended to summary.{csv,md} and every group dir.
# Lets one repo hold outputs for multiple sweeps side-by-side (e.g. workstation
# vs EPYC) without overwriting. Pass e.g. "-epyc" → summary-epyc.csv,
# grad-no-sem-shed-epyc/, etc. Empty string (default) keeps the original names.
SUFFIX = sys.argv[2] if len(sys.argv) > 2 else ""

# How the inflight-timeseries (sawtooth) plot selects what to draw per policy:
#   "mean"             — mean across all iters at each 0.1s offset (averages noise)
#   "median_peak"      — single iter whose peak_inflight is closest to the
#                        policy's median peak (representative trajectory)
#   "median_tps"       — single iter whose useful_tps is closest to the median
#   "median_cv"        — single iter whose tps_cv is closest to the median
#   "max_cap_crossings"— single iter that crossed max_pending most often
#                        (binary-flavored — stress-case)
#   "first"            — iter index 0 per policy (reproducible, no metric tie)
# Defaults to "median_peak" — keeps cross-policy comparison fair while letting
# graduated's smoothness and binary's fluctuations both show through.
SAWTOOTH_MODE = os.environ.get("SAWTOOTH_MODE", "mean")

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

# Preserve a pre-filter copy so we can compute per-policy fork-rate from
# the full record set (failed iters needed for the denominator). All
# downstream plotting and aggregation uses the filtered `df`.
raw_df = df.copy()

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
    parts = [
        f"max={int(r['validator.max_pending_transactions'])}",
        f"sem={int(r['validator.max_pending_local_submissions'])}",
        f"pct={int(r['validator.graduated_load_shedding_soft_limit_pct'])}",
    ]
    sat = r.get("validator.graduated_load_shedding_saturation_pct")
    if sat is not None and not (isinstance(sat, float) and pd.isna(sat)):
        parts.append(f"sat={int(sat)}")
    ss = r.get("validator.semaphore_shedding_enabled")
    if ss is True:
        parts.append("sem_shed=true")
    elif ss is False:
        parts.append("sem_shed=false")
    return " ".join(parts)


df["policy"] = df.apply(policy_label, axis=1)
# Apply the same labelling on the pre-filter copy so fork-rate stats can
# count by policy. raw_df has the validator config columns too.
if len(raw_df) > 0:
    raw_df["policy"] = raw_df.apply(policy_label, axis=1)

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

# ---------- RED-fairness ratio (Floyd & Jacobson 1993 Claim 2) ----------
# honest_admit_fraction / spammer_admit_fraction. 1.0 = uniform drop
# probability across sources (RED-ideal). >>1 = phase-effect bias under
# tail-drop (binary). Closer to 1 under graduated.
#
# spammer_admit_count is derived: total_commits − honest_cl_commits.
# Open-loop honest's `bench_success` is unreliable (stress.rs caps
# latency_ms at ~41 entries per proc), so we use honest_cl exclusively.
def _compute_red_ratio(r):
    iw = r.get("iter_window") or {}
    spam_dur = (iw.get("spam_end_epoch") or 0) - (iw.get("spam_start_epoch") or 0)
    if spam_dur <= 0:
        spam_dur = (r.get("spammer") or {}).get("duration_secs") or 15
    tps = (r.get("results") or {}).get("useful_tps") or 0
    hcl = r.get("honest_cl") or {}
    hcl_commits = hcl.get("bench_success") or 0
    hcl_offered = hcl.get("offered") or 0
    spam_offered = (r.get("spammer") or {}).get("offered") or 0
    total_commits = tps * spam_dur
    spam_commits = max(total_commits - hcl_commits, 0)
    if spam_offered == 0 or hcl_offered == 0:
        return np.nan
    spam_admit = spam_commits / spam_offered
    hcl_admit = hcl_commits / hcl_offered
    if spam_admit == 0:
        return np.nan
    return hcl_admit / spam_admit

# Recompute per-window aggregates from the raw timeseries, restricted to
# [spam_start_epoch, spam_end_epoch]. The harness-side PromQL scalars
# computed in stress-multi.sh are biased: their [WINDOW:1s] subqueries
# read back from query-time which lands ~5s past spam_end, leaking
# pre/post-spam quiet tail into the aggregates:
#   inflight_mean       → understated (zeros pull mean down)
#   inflight_stddev     → overstated (zeros inflate variance)
#   latency percentiles → understated (low-traffic samples dilute the tail)
#
# As of 2026-06-01 stress-multi.sh's prom_scalar pins @ SPAM_END_EPOCH via
# the API's `time=` param, so new sweeps are fixed at source. These ts-
# derived columns also fix existing records retroactively when re-plotted,
# and remain authoritative going forward.
#
# Returns NaN per metric where the source timeseries is missing / empty
# inside the spam window. Inflight ts is 100ms-step; latency ts is 1s-step.
# AIMD cwnd ramp-up plus consensus warm-up typically takes 8-10s after
# spam_start. Steady-state TPS stats skip this prefix so the headline
# scalar isn't dragged down by ramp-up samples.
WARMUP_SECONDS = 10

# Committee size for our docker validator setup. `timeseries.tps` is the
# PromQL `sum(rate(total_transaction_effects))` aggregated across all
# validator hosts, so per-validator TPS = sum-rate / COMMITTEE_SIZE.
# This matches the scalar `useful_tps`, which uses `max(rate)` (per-
# validator throughput, since all replicas execute the same finalized set).
COMMITTEE_SIZE = 4


def _compute_ts_window_stats(r):
    iw = r.get("iter_window") or {}
    spam_start = iw.get("spam_start_epoch") or 0
    spam_end = iw.get("spam_end_epoch") or 0
    ts = r.get("timeseries") or {}
    out = {}

    def values_in_window(key):
        series = ts.get(key) or []
        return [v for t, v in series if spam_start <= t <= spam_end]

    # Same as values_in_window but skips the AIMD/consensus ramp-up
    # prefix so steady-state statistics aren't dragged by warm-up.
    def values_in_steady(key):
        series = ts.get(key) or []
        return [v for t, v in series
                if spam_start + WARMUP_SECONDS <= t <= spam_end]

    inflight = values_in_window("inflight")
    if len(inflight) >= 2 and spam_end > spam_start:
        arr = np.asarray(inflight, dtype=float)
        out["inflight_mean_ts"] = float(arr.mean())
        out["inflight_stddev_ts"] = float(arr.std())
        out["inflight_peak_ts"] = float(arr.max())
        out["inflight_firstdiff_std_ts"] = float(np.std(np.diff(arr)))
        # Queue-depth percentiles over the spam window. Reveals shape that
        # mean+peak alone hides: e.g. broken iters show p50≈0, p75≈0,
        # peak≈18000 (one early spike then empty) vs healthy iters where
        # p50≈peak (sustained near-cap load).
        out["inflight_p50_ts"] = float(np.percentile(arr, 50))
        out["inflight_p75_ts"] = float(np.percentile(arr, 75))
        out["inflight_p99_ts"] = float(np.percentile(arr, 99))

    # Latency ts are already per-second `histogram_quantile(0.99, ...)`
    # values. Taking the median across the spam window yields "typical
    # per-second p99" — a more honest tail-load metric than PromQL's
    # window-p99 (which mixes high-traffic and post-spam quiet samples).
    for src, dst in [
        ("consensus_lat_p99", "consensus_lat_p99_ts"),
        ("permit_wait_p99", "permit_wait_p99_ts"),
        ("permit_hold_p99", "permit_hold_p99_ts"),
    ]:
        vals = values_in_window(src)
        if vals:
            out[dst] = float(np.median(vals))

    # TPS timeseries → steady-state per-validator throughput. The scalar
    # `results.useful_tps` is `total_effects / spam_duration`, a single
    # average over the whole spam window. That conflates the AIMD/consensus
    # ramp-up prefix with steady-state operation and obscures per-policy
    # differences. Instead, take the median of samples AFTER the warm-up
    # prefix, normalized by committee size (the ts metric is sum across
    # validators; per-validator throughput = sum / COMMITTEE_SIZE).
    tps_vals = values_in_steady("tps")
    if len(tps_vals) >= 2:
        arr = np.asarray(tps_vals, dtype=float) / COMMITTEE_SIZE
        out["tps_median_ts"] = float(np.median(arr))
        out["tps_mean_ts"] = float(arr.mean())
        out["tps_stddev_ts"] = float(arr.std())
        out["tps_p25_ts"] = float(np.percentile(arr, 25))
        out["tps_p75_ts"] = float(np.percentile(arr, 75))
        # CV = stddev / mean. RED's "smoother throughput" claim is a
        # direct prediction about this metric: graduated should have
        # lower CV than binary at comparable mean.
        if arr.mean() > 0:
            out["tps_cv_ts"] = float(arr.std() / arr.mean())

    return out


# Re-load raw records (df is already json-normalized; we need the nested
# dicts back for the helpers). Simpler: read the JSONL again here.
import json as _json_for_red
try:
    with open(PATH) as _f:
        _raw_recs = [_json_for_red.loads(_l) for _l in _f if _l.strip()]
    _ratios = []
    _ts_stats = []
    for _r in _raw_recs:
        if _r.get("failed"):
            _ratios.append(np.nan)
            _ts_stats.append({})
        else:
            _ratios.append(_compute_red_ratio(_r))
            _ts_stats.append(_compute_ts_window_stats(_r))
    _ts_columns = [
        "inflight_mean_ts", "inflight_stddev_ts", "inflight_peak_ts",
        "inflight_firstdiff_std_ts",
        "inflight_p50_ts", "inflight_p75_ts", "inflight_p99_ts",
        "consensus_lat_p99_ts", "permit_wait_p99_ts", "permit_hold_p99_ts",
        "tps_median_ts", "tps_mean_ts", "tps_stddev_ts",
        "tps_p25_ts", "tps_p75_ts", "tps_cv_ts",
    ]
    if len(_ratios) == len(df):
        df["red_ratio"] = _ratios
        _ts_df = pd.DataFrame(_ts_stats, index=df.index)
        for _col in _ts_columns:
            df[_col] = _ts_df.get(_col, pd.Series([np.nan] * len(df)))
    else:
        df["red_ratio"] = np.nan
        for _col in _ts_columns:
            df[_col] = np.nan
except Exception:
    df["red_ratio"] = np.nan
    for _col in [
        "inflight_mean_ts", "inflight_stddev_ts", "inflight_peak_ts",
        "inflight_firstdiff_std_ts",
        "consensus_lat_p99_ts", "permit_wait_p99_ts", "permit_hold_p99_ts",
        "tps_median_ts", "tps_mean_ts", "tps_stddev_ts",
        "tps_p25_ts", "tps_p75_ts", "tps_cv_ts",
    ]:
        df[_col] = np.nan

# Overlay ts-derived values onto the scalar columns. combine_first prefers
# the ts column where it's non-NaN, falls back to the PromQL scalar otherwise.
# Keeps the *_ts columns around as separate inspection points (visible in
# summary.md) while the plots silently consume the more accurate version.
def _overlay_ts(scalar_col, ts_col):
    if scalar_col in df.columns and ts_col in df.columns:
        df[scalar_col] = df[ts_col].combine_first(df[scalar_col])

_overlay_ts("results.inflight_mean",   "inflight_mean_ts")
_overlay_ts("results.inflight_stddev", "inflight_stddev_ts")
_overlay_ts("results.peak_inflight",   "inflight_peak_ts")
# Latency p99 overlay: median-of-per-second-p99 replaces window-p99.
# They're not the same physical quantity (see helper docstring), but the
# ts version is a more honest "typical bad-second" tail metric and matches
# the iter-by-iter operating point better.
_overlay_ts("results.consensus_lat_p99", "consensus_lat_p99_ts")
_overlay_ts("results.permit_wait_p99",   "permit_wait_p99_ts")
_overlay_ts("results.permit_hold_p99",   "permit_hold_p99_ts")
# Replace the scalar useful_tps (whole-window average that includes
# AIMD ramp-up) with steady-state median per-validator TPS. All
# downstream tps boxplots, scatter, and summaries silently pick up
# the corrected value via this overlay.
_overlay_ts("results.useful_tps",        "tps_median_ts")

# Keep the original "inflight_firstdiff_std" name for plot_group's existing
# reference (was created by the helper previously; now sourced from ts).
df["inflight_firstdiff_std"] = df["inflight_firstdiff_std_ts"]

# Recompute CV from the (now ts-corrected) inflight columns. Was computed
# above (line ~128) from the biased scalar values — those values are stale
# after the overlay. The CV plot is in GLOBAL_SKIP_PLOTS but the summary
# table still shows it; keep it consistent with the underlying mean/stddev.
if "results.inflight_stddev" in df.columns and "results.inflight_mean" in df.columns:
    _mean = df["results.inflight_mean"].replace(0, np.nan)
    df["inflight_cv"] = df["results.inflight_stddev"] / _mean

# Validator-side drop probability — the authoritative RED-Claim-2 surface.
# Computed from validator-side counters (preventive + saturated + reactive)
# divided by total admission decisions (drops + commits). Spammer-side
# drop_prob is structurally unreachable in our open-loop + TD setup: TD
# honours the validator's retry_after_secs=30 hint on overload rejections,
# but OPEN_LOOP_TASK_TIMEOUT=2s elapses long before that retry budget, so
# rejections never propagate to the spammer process. Validator-side
# counters are the only authoritative source.
#
# sweep.sh precomputes results.validator_drop_prob inline; this also
# recomputes here as a fallback for older JSONLs that predate the field.
if "results.validator_drop_prob" in df.columns:
    df["validator_drop_prob"] = df["results.validator_drop_prob"]
else:
    df["validator_drop_prob"] = np.nan
_drops_cols = [
    "results.reject_grad_preventive",
    "results.reject_grad_saturated",
    "results.reject_grad_reactive",
]
if all(c in df.columns for c in _drops_cols) and "results.useful_tps" in df.columns:
    _drops = sum(df[c].fillna(0) for c in _drops_cols)
    _spam_dur = (df.get("iter_window.spam_end_epoch", 0)
                 - df.get("iter_window.spam_start_epoch", 0))
    _commits = df["results.useful_tps"].fillna(0) * _spam_dur
    _decisions = _drops + _commits
    _fallback = (_drops / _decisions).where(_decisions > 0, np.nan)
    df["validator_drop_prob"] = df["validator_drop_prob"].combine_first(_fallback)


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
    # Intra-iter TPS coefficient of variation. RED's "smoother throughput
    # variance" claim is a direct prediction here: graduated should show
    # lower tps_cv_med than binary at comparable mean.
    "tps_cv_med": ("tps_cv_ts", "median"),
    "tps_stddev_med": ("tps_stddev_ts", "median"),
    "sat75_med": ("results.saturation_75pct", "median"),
    "hold_p99_med": ("results.permit_hold_p99", "median"),
    "cons_p99_med": ("results.consensus_lat_p99", "median"),
    "cv_med": ("inflight_cv", "median"),
    "inflight_mean_med": ("results.inflight_mean", "median"),
    "inflight_p50_med": ("inflight_p50_ts", "median"),
    "inflight_p75_med": ("inflight_p75_ts", "median"),
    "inflight_p99_med": ("inflight_p99_ts", "median"),
    "firstdiff_std_med": ("inflight_firstdiff_std", "median"),
    "red_ratio_med": ("red_ratio", "median"),
    "drop_prob_med": ("validator_drop_prob", "median"),
}

# Only include aggregations whose source column exists.
agg_spec = {k: v for k, v in metrics.items() if v[0] in df.columns or k == "n"}
agg = df.groupby("policy").agg(**agg_spec).reindex(policy_order).round(3)

# Fork-rate / safe-rate columns — counted from raw_df (pre-filter) so the
# denominator includes failed iters. Key safety metric: graduated should
# show fork_pct ≈ 0 while binary @ max≥19000 shows fork_pct > 0 under
# sustained heavy spam. silent_collapse specifically isolates the
# checkpoint-fork failure mode (see project memory "checkpoint-fork-panic")
# from other harness failures.
if "policy" in raw_df.columns and len(raw_df) > 0:
    fork_stats = raw_df.groupby("policy").agg(
        total_iters=("policy", "count"),
        failed_iters=("failed", lambda s: int(s.fillna(False).sum())),
        silent_collapse_iters=(
            "silent_collapse",
            lambda s: int(s.fillna(False).sum()) if s.notna().any() else 0,
        ) if "silent_collapse" in raw_df.columns else ("failed", lambda s: 0),
    )
    fork_stats["fork_pct"] = (
        100.0 * fork_stats["silent_collapse_iters"] / fork_stats["total_iters"]
    ).round(1)
    # Stitch fork-rate columns onto the main agg table. reindex on
    # policy_order to preserve display order; missing policies fill NaN.
    fork_stats = fork_stats.reindex(policy_order)
    agg["total_iters"] = fork_stats["total_iters"]
    agg["fork_iters"] = fork_stats["silent_collapse_iters"]
    agg["fork_pct"] = fork_stats["fork_pct"]

summary_csv = f"summary{SUFFIX}.csv"
summary_md = f"summary{SUFFIX}.md"
agg.to_csv(summary_csv)
print(f"Wrote {summary_csv}")

# Markdown table for easy paste into notes / Slack
with open(summary_md, "w") as f:
    f.write("# cap-policy-sweep summary\n\n")
    f.write(agg.to_markdown())
    f.write("\n")
print(f"Wrote {summary_md}")

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


def add_box_legend(ax, extra_handles=None):
    handles = list(BOX_LEGEND_HANDLES)
    if extra_handles:
        handles.extend(extra_handles)
    ax.legend(handles=handles, loc="best", fontsize=8)


# Highlight conventions for x-tick labels / legend entries. Each rule is
# (list_of_substrings_ALL_required, color). Matched against the FULL policy
# string so we can require fields that may be stripped from the shortened
# label. Order matters: first match wins.
#
# Cap-agnostic: the rules match by pct/sat alone, so they fire across any
# max_pending value (1000, 10000, 19000, 20000, ...). The proposed default
# is graduated pct=50 with the machine-tuned sat (90 EPYC, 95 WS); the
# binary baseline is pct=100 sat=100 regardless of cap.
TICK_LABEL_HIGHLIGHTS = [
    (["pct=50 sat=90"],   "#9bd49b"),  # green: proposed default (EPYC-tuned)
    (["pct=50 sat=95"],   "#9bd49b"),  # green: proposed default (WS-tuned)
    (["pct=100 sat=100"], "#e6a3a3"),  # red:   binary baseline (any cap)
]

# Plot names to skip across ALL groups. Add a name here to temporarily disable
# a figure without touching individual group specs. Re-enable by removing it.
#   cv / cv-saturated — CV (stddev/mean) cancels graduated's mean reduction;
#                       see comment in plot_group near `inflight-mean`.
#   red-ratio         — admit-fairness needs a more robust spammer/honest
#                       accounting split before it's worth plotting.
#   tps-mean          — redundant with the tps boxplot when n ≥ ~10/policy.
GLOBAL_SKIP_PLOTS = {"cv-saturated", "red-ratio", "tps-cv", "tps-timeseries"}


def _highlight_color(policy):
    for substrs, color in TICK_LABEL_HIGHLIGHTS:
        if all(s in policy for s in substrs):
            return color
    return None


def highlight_tick_labels(ax, policies):
    """Paint x-tick label backgrounds based on TICK_LABEL_HIGHLIGHTS rules
    matched against the full policy strings (positional with tick labels).
    Call after labels have been set + rotated."""
    for tick_label, policy in zip(ax.get_xticklabels(), policies):
        color = _highlight_color(policy)
        if color:
            tick_label.set_bbox(dict(facecolor=color, alpha=0.5,
                                     edgecolor="none",
                                     boxstyle="round,pad=0.25"))


def shortened_labels(policies):
    """Strip fields that are constant across all policies in the group.
    E.g. ["max=1000 sem=2000 pct=100", "max=1000 sem=2000 pct=75"] →
         ["pct=100", "pct=75"] (only the varying field remains).

    Field order in the output preserves intent: max → sem → pct → sat →
    sem_shed. Fields not present in a policy's label string are simply
    skipped for that policy."""
    parsed = [dict(part.split("=") for part in p.split()) for p in policies]
    all_fields = ("max", "sem", "pct", "sat", "sem_shed")
    varying = [k for k in all_fields
               if len({d.get(k) for d in parsed if k in d}) > 1
               or any((k in d) != (k in parsed[0]) for d in parsed)]
    if not varying:
        return policies  # nothing varies (n=1 policy), keep full label
    return [" ".join(f"{k}={d[k]}" for k in varying if k in d) for d in parsed]


# ---------- experiment groups ----------
# Groups are discovered from the data, not hardcoded. Two comparison shapes
# are recognised:
#
#   grad-no-sem-shed   — at fixed (max, sem, sem_shed), all available
#                        (pct, sat) variants. The "policy sweep at fixed cap".
#   just-lower-the-cap — at fixed (sem, sem_shed), binary policies
#                        (pct=100, sat=100) across multiple `max` values,
#                        plus the most-graduated policy at the largest `max`
#                        for head-to-head.
#
# `ratio_col` and `overshoot_col` switch between max-bound and sem-bound
# framings depending on which cap is the binding constraint.
def _parse_policy(p):
    """'max=1000 sem=50 pct=100 sat=100 sem_shed=false' → dict."""
    return dict(part.split("=", 1) for part in p.split() if "=" in part)


def _is_binary(f):
    return f.get("pct") == "100" and f.get("sat", "100") == "100"


def _policy_role(f):
    pct = int(f.get("pct", 100))
    sat = int(f.get("sat", 100))
    if _is_binary(f):
        return "binary baseline (legacy)"
    if sat == 100:
        return "graduated, legacy saturation (sat isolation)"
    if pct == sat:
        return "graduated, narrow soft zone"
    return f"graduated, soft zone [{int(int(f['max']) * pct / 100)}, {int(int(f['max']) * sat / 100)}]"


def discover_groups(df):
    """Inspect `df` and emit one group spec per recognised comparison shape."""
    from collections import defaultdict
    parsed = {p: _parse_policy(p) for p in df["policy"].unique()}
    groups = []

    # --- Group 1: grad-no-sem-shed (fixed cap, varying policy) ---
    # Pick the (max, sem, sem_shed) tuple with the most distinct (pct, sat)
    # variants. If multiple tuples tie, prefer the one with a non-binary
    # variant present (otherwise the group degenerates to one binary row).
    by_cfg = defaultdict(list)
    for p, f in parsed.items():
        by_cfg[(f.get("max"), f.get("sem"), f.get("sem_shed"))].append(p)
    scored = sorted(
        by_cfg.items(),
        key=lambda kv: (len(kv[1]), any(not _is_binary(parsed[p]) for p in kv[1])),
        reverse=True,
    )
    if scored and len(scored[0][1]) >= 2:
        (max_, sem_, shed_), policies = scored[0]
        # binary first (pct=100), then graduated by pct desc, sat desc within pct
        policies = sorted(policies, key=lambda p: (
            -int(parsed[p].get("pct", 100)),
            -int(parsed[p].get("sat", 100)),
        ))
        rows = "\n".join(
            f"| {parsed[p].get('pct','-'):>3} | {parsed[p].get('sat','100'):>3} | {_policy_role(parsed[p])} |"
            for p in policies
        )
        groups.append({
            "dir": "grad-no-sem-shed",
            "policies": policies,
            "ratio_col": "results.ratio_peak_over_max_pending",
            "ratio_label": "peak_inflight/max_pending",
            "overshoot_col": "overshoot_above_max",
            "overshoot_label": "peak_inflight−max_pending",
            "skip_plots": ["stddev"],
            "description": f"""\
# grad-no-sem-shed

**Question:** Does graduated load shedding reduce peak overshoot at
`max_pending` without sacrificing throughput?

**Policies** (all `max_pending_transactions={max_}`,
`max_pending_local_submissions={sem_}`, `semaphore_shedding_enabled={shed_}`):

| pct | sat | role |
|---|---|---|
{rows}

**Safety metric:** `peak_inflight / max_pending`. Values < 1.0 indicate
the saturation_limit kept peak structurally below the declared cap.
""",
        })

    # --- Group 2: just-lower-the-cap (varying cap, fixed sem) ---
    # Binary policies (pct=100, sat=100) at different `max`, at the most-common
    # (sem, sem_shed). Append the most-graduated policy at the largest `max`
    # for head-to-head against the cap-lowering alternative.
    binary_by_sem = defaultdict(list)
    for p, f in parsed.items():
        if _is_binary(f):
            binary_by_sem[(f.get("sem"), f.get("sem_shed"))].append(p)
    if binary_by_sem:
        (best_sem, best_shed), binaries = max(
            binary_by_sem.items(), key=lambda kv: len(kv[1])
        )
        if len(binaries) >= 2:
            binaries = sorted(binaries, key=lambda p: int(parsed[p].get("max", 0)))
            max_largest = int(parsed[binaries[-1]].get("max", 0))
            grads = [
                p for p, f in parsed.items()
                if int(f.get("max", 0)) == max_largest
                and f.get("sem") == best_sem
                and f.get("sem_shed") == best_shed
                and not _is_binary(f)
            ]
            # Prefer the "proposed default" policy (green-highlighted in
            # TICK_LABEL_HIGHLIGHTS, currently pct=50 sat=90/95) over the most
            # aggressive variant — this group is meant to compare the cap-
            # lowering alternative against the *recommended* graduated policy,
            # not the most extreme one.
            grads.sort(key=lambda p: (
                0 if _highlight_color(p) == "#9bd49b" else 1,
                int(parsed[p].get("sat", 100)),
                int(parsed[p].get("pct", 100)),
            ))
            if grads:
                policies = binaries + [grads[0]]
                rows = "\n".join(
                    f"| `{p}` | "
                    f"{'binary' if _is_binary(parsed[p]) else '**graduated**'} | "
                    f"{parsed[p].get('max')} |"
                    for p in policies
                )
                groups.append({
                    "dir": "just-lower-the-cap",
                    "policies": policies,
                    "ratio_col": "results.ratio_peak_over_max_pending",
                    "ratio_label": "peak_inflight/max_pending",
                    "overshoot_col": "overshoot_above_max",
                    "overshoot_label": "peak_inflight−max_pending",
                    "skip_plots": ["stddev"],
                    "description": f"""\
# just-lower-the-cap

**Question:** A natural counter-proposal to graduated shedding is "just
lower `max_pending` to a tighter cap". This group tests it head-to-head
against graduated.

**Policies** (all `sem={best_sem}`, `semaphore_shedding_enabled={best_shed}`):

| label | mechanism | max_pending |
|---|---|---|
{rows}

**Safety metric:** `peak_inflight / max_pending` — normalises across caps.
""",
                })

    return groups


GROUPS = discover_groups(df)


# ---------- plotting helpers ----------
def boxplot(col, title, ylabel, out_path, policies, log=False, data_df=None, tick_labels=None, hline=None, highlight=True):
    """Box plot of `col` grouped by `policies`. Writes to `out_path`.
    `tick_labels` overrides the x-axis labels (defaults to `policies`).
    `hline` draws a horizontal reference line at that y value (e.g. 0 for
    "no overshoot" on the absolute plot, 1 for the same on the ratio plot)."""
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
        box.set_alpha(0.8)
    extra_handles = None
    if hline is not None:
        ax.axhline(hline, color="black", linewidth=1.2, linestyle="--", alpha=0.7, zorder=0)
        extra_handles = [Line2D([0], [0], color="black", linewidth=1.2,
                                linestyle="--", alpha=0.7,
                                label=f"y={hline}")]
    ax.set_title(title)
    ax.set_ylabel(ylabel)
    if log:
        ax.set_yscale("log")
    ax.minorticks_on()
    ax.grid(which="major", axis="y", alpha=0.5)
    ax.grid(which="minor", axis="y", alpha=0.5, linestyle=":", linewidth=0.8)
    plt.setp(ax.get_xticklabels(), fontsize=8, rotation=45, ha="right")
    if highlight:
        highlight_tick_labels(ax, policies)
    add_box_legend(ax, extra_handles=extra_handles)
    plt.tight_layout()
    plt.savefig(out_path, dpi=130)
    plt.close()
    print(f"Wrote {out_path}")


def plot_drop_classification(out_path, policies, tick_labels, data_df):
    """Stacked bar per policy showing the breakdown of rejection counts
    into the three Option-B categories:
      reactive   — drop AT the hard cap (binary's only mode)
      saturated  — drop in the 100%-shed band [sat_pct, 100%]
      preventive — probabilistic drop in the soft zone [start_pct, sat_pct]
    Aggregates as MEAN across iters for each policy. Binary: all reactive.
    Graduated: mostly saturated + preventive, near-zero reactive."""
    # Order: preventive (bottom, green) → saturated (middle, orange) →
    # reactive (top, red). Reading visually bottom-up = early to forced,
    # so a tall "red tip" on a graduated bar = the bit that escaped the
    # soft + saturated zones. Binary policies have only red (all reactive),
    # graduated mostly green/orange with tiny red.
    cols = [
        ("reject_grad_preventive", "preventive [soft ≤ q < sat]", "#9bd49b"),
        ("reject_grad_saturated",  "saturated [sat ≤ q < max]",   "#ff7f0e"),
        ("reject_grad_reactive",   "reactive [q ≥ max]",          "#d62728"),
    ]
    fig, ax = plt.subplots(figsize=(max(8, 1.4 * len(policies)), 5.5))
    x = np.arange(len(policies))
    bottoms = np.zeros(len(policies))
    for col, label, color in cols:
        full_col = f"results.{col}"
        if full_col not in data_df.columns:
            continue
        means = np.array([
            data_df.loc[data_df.policy == p, full_col].fillna(0).mean()
            for p in policies
        ])
        ax.bar(x, means, bottom=bottoms, label=label,
               color=color, edgecolor="black", alpha=0.85)
        bottoms += means
    ax.set_xticks(x)
    ax.set_xticklabels(tick_labels, fontsize=8, rotation=45, ha="right")
    ax.set_ylabel("Mean drops per iter (log scale)")
    ax.set_title("Drop classification — RED's early-vs-forced split")
    # Log scale on stacked bars is mathematically lossy (segment heights
    # aren't additive on log axis), but it reveals zero-vs-tiny vs huge
    # categories at a glance — important here because reactive drops are
    # 0 for graduated and ~98K for binary, a range linear scale can't show.
    ax.set_yscale("log")
    # Log requires positive baseline. Top is sized to the tallest stack
    # plus a multiplicative margin so the legend can sit clear regardless
    # of dataset scale. `bottoms` holds the cumulative per-bar height
    # after the stacking loop, so its max is the tallest total stack.
    top = max(bottoms.max() * 3.0, 10.0)
    ax.set_ylim(1, top)
    ax.minorticks_on()
    ax.grid(which="major", axis="y", alpha=0.5)
    ax.grid(which="minor", axis="y", alpha=0.5, linestyle=":", linewidth=0.8)
    ax.legend(fontsize=8, loc="best", framealpha=0.92)
    highlight_tick_labels(ax, policies)
    plt.tight_layout()
    plt.savefig(out_path, dpi=130)
    plt.close()
    print(f"Wrote {out_path}")


def _line_style_for(policy_str):
    """Map a policy to (color, linestyle, linewidth). max_pending is fixed
    at 20K across all current policies; only (pct, sat) varies and
    determines whether a policy is binary (pct == sat → hard cap at
    max × pct%) or graduated (pct < sat → soft zone between them).

      Binary family (pct == sat) — red shades, lighter for lower cap:
        100/100  dark red,    solid,    thick     (full-cap baseline)
        95/95    medium red,  dashed              (cap=19K)
        50/50    light brown, dotted,   thick     (just-lower-the-cap 10K)

      Graduated family (pct < sat) — distinct colors per (pct, sat):
        50/100   purple,      dash-dot            (sat isolation, no cushion)
        75/95    orange,      dashed              (small soft zone)
        50/95    green,       solid,    thick     (proposed default)
        25/95    teal,        dash-dot, thick     (aggressive)
        fallback blue,        solid
    """
    fields = dict(part.split("=", 1) for part in policy_str.split() if "=" in part)
    pct = int(fields.get("pct", 100))
    sat = int(fields.get("sat", 100))
    # Binary family: pct == sat (hard cap at max × pct%).
    if pct == sat:
        if pct == 100:
            return ("#c0392b", "-",  2.5)   # dark red — full-cap baseline
        if pct >= 90:
            return ("#e67e22", "--", 2.0)   # medium red/orange — cap ~19K
        return ("#8b4513", ":",  2.5)       # brown — just-lower-the-cap
    # Graduated family: pct < sat.
    if sat == 100:
        return ("#8e44ad", "-.", 2.0)       # purple — sat isolation
    if pct == 75:
        return ("#d35400", "--", 2.0)       # orange — small soft zone
    if pct == 50:
        return ("#27ae60", "-",  2.5)       # green — proposed default
    if pct == 25:
        return ("#16a085", "-.", 2.5)       # teal — aggressive
    return ("#2980b9", "-", 2.0)


def _marker_for(policy_str):
    """Map a policy to (marker, size_multiplier). Multiplier compensates for
    the fact that matplotlib's `s=` is area but visual weight differs by
    shape — dense shapes (square, diamond, circle) look bigger than thin
    shapes (plus, star, X) at the same area. Multipliers are tuned so all
    markers look roughly equal in apparent size.

      Binary (pct == sat):
        100/100  circle  (o)   ×1.00  — full-cap baseline
        95/95    square  (s)   ×0.80
        50/50    diamond (D)   ×0.85  — just-lower-the-cap
      Graduated (pct < sat):
        50/100   triangle-up   (^)  ×1.10  — sat isolation
        75/95    triangle-down (v)  ×1.10
        50/95    plus-filled   (P)  ×1.30  — proposed default
        25/95    star          (*)  ×1.50  — aggressive
        fallback X (x)               ×1.20
    """
    fields = dict(part.split("=", 1) for part in policy_str.split() if "=" in part)
    pct = int(fields.get("pct", 100))
    sat = int(fields.get("sat", 100))
    if pct == sat:
        if pct == 100: return ("o", 1.00)
        if pct >= 90:  return ("s", 0.80)
        return ("D", 0.85)
    if sat == 100:    return ("^", 1.10)
    if pct == 75:     return ("v", 1.10)
    if pct == 50:     return ("P", 1.30)
    if pct == 25:     return ("*", 2.20)
    return ("X", 1.20)


def _rec_policy(r):
    """Reconstruct the policy string from a raw_rec the same way
    df['policy'] is built. Pulled out as module-level helper so the
    sawtooth plot and selector helpers can share it."""
    v = r.get("validator", {})
    parts = [
        f"max={int(v['max_pending_transactions'])}",
        f"sem={int(v['max_pending_local_submissions'])}",
        f"pct={int(v['graduated_load_shedding_soft_limit_pct'])}",
    ]
    sat = v.get("graduated_load_shedding_saturation_pct")
    if sat is not None:
        parts.append(f"sat={int(sat)}")
    ss = v.get("semaphore_shedding_enabled")
    if ss is True:   parts.append("sem_shed=true")
    elif ss is False: parts.append("sem_shed=false")
    return " ".join(parts)


def _select_iter(candidates, mode):
    """Pick one representative iter out of `candidates` (raw recs for one
    policy) per the requested selector. Returns the chosen rec or None
    if candidates lack the metric needed for the mode."""
    if not candidates:
        return None
    if mode == "first":
        return candidates[0]

    def metric(r, name):
        return (r.get("results") or {}).get(name)

    def cap_crossings(r):
        ts = (r.get("timeseries") or {}).get("inflight") or []
        cap = (r.get("validator") or {}).get("max_pending_transactions") or 0
        if not cap:
            return 0
        # Count rising edges where inflight transitions from <cap to >=cap.
        was_below = True
        count = 0
        for _, v in ts:
            now_below = v < cap
            if was_below and not now_below:
                count += 1
            was_below = now_below
        return count

    if mode == "max_cap_crossings":
        return max(candidates, key=cap_crossings)

    key_map = {
        "median_peak": "peak_inflight",
        "median_tps":  "useful_tps",
        "median_cv":   None,  # tps_cv lives on the df, not raw rec; handled below
    }
    if mode in key_map and key_map[mode]:
        vals = [(r, metric(r, key_map[mode])) for r in candidates]
        vals = [(r, v) for r, v in vals if v is not None]
        if not vals:
            return candidates[0]
        med = sorted(v for _, v in vals)[len(vals) // 2]
        return min(vals, key=lambda rv: abs(rv[1] - med))[0]

    if mode == "median_cv":
        # tps_cv was computed onto the df; cross-reference by iter index.
        def tps_cv(r):
            ts = (r.get("timeseries") or {}).get("tps") or []
            iw = r.get("iter_window") or {}
            ss = iw.get("spam_start_epoch") or 0
            se = iw.get("spam_end_epoch") or 0
            vals = [v for t, v in ts
                    if ss + WARMUP_SECONDS <= t <= se]
            if len(vals) < 2:
                return None
            arr = np.asarray(vals, dtype=float)
            mu = arr.mean()
            return float(arr.std() / mu) if mu > 0 else None
        scored = [(r, tps_cv(r)) for r in candidates]
        scored = [(r, v) for r, v in scored if v is not None]
        if not scored:
            return candidates[0]
        med = sorted(v for _, v in scored)[len(scored) // 2]
        return min(scored, key=lambda rv: abs(rv[1] - med))[0]

    # Fallback: mean mode is handled at the caller, but if an unknown
    # mode lands here, just return the first iter rather than crashing.
    return candidates[0]


def plot_inflight_timeseries(out_path, policies, tick_labels, data_df, raw_recs):
    """Inflight depth over the spam window (the iconic RED sawtooth-vs-smooth
    figure). Driven by `SAWTOOTH_MODE`:
      - "mean": one line per policy = mean across all healthy iters at each
                0.1s offset relative to spam_start. Averages noise; reveals
                signal phase-locked to spam_start.
      - any other selector ("median_peak", "median_tps", "median_cv",
                "max_cap_crossings", "first"): plot ONE representative iter
                per policy, raw 10Hz samples, no cross-iter averaging.
                Keeps oscillation/fluctuation detail visible — useful for
                showing graduated's smoothness and binary's per-iter swings."""
    fig, ax = plt.subplots(figsize=(11, 6))
    rec_policy = _rec_policy  # local alias preserves call sites below

    from collections import defaultdict
    plotted_any = False
    for i, p in enumerate(policies):
        candidates = [r for r in raw_recs
                     if not r.get("failed") and rec_policy(r) == p]
        if not candidates:
            continue

        color, linestyle, linewidth = _line_style_for(p)

        if SAWTOOTH_MODE == "mean":
            # Mean across all iters per 0.1s offset.
            by_offset = defaultdict(list)
            for rec in candidates:
                ts = (rec.get("timeseries") or {}).get("inflight")
                if not ts:
                    continue
                iw = rec.get("iter_window") or {}
                spam_start = iw.get("spam_start_epoch") or 0
                spam_end = iw.get("spam_end_epoch") or 0
                for t, v in ts:
                    if spam_start <= t <= spam_end:
                        by_offset[round(t - spam_start, 1)].append(v)
            if not by_offset:
                continue
            xs = sorted(by_offset.keys())
            ys = [sum(by_offset[x]) / len(by_offset[x]) for x in xs]
            label = f"{tick_labels[i]} (n={len(candidates)})"
        else:
            # Single iter selected by the configured rule.
            rec = _select_iter(candidates, SAWTOOTH_MODE)
            ts = (rec.get("timeseries") or {}).get("inflight") if rec else None
            if not ts:
                continue
            iw = rec.get("iter_window") or {}
            spam_start = iw.get("spam_start_epoch") or 0
            spam_end = iw.get("spam_end_epoch") or 0
            in_window = [(t - spam_start, v) for t, v in ts
                         if spam_start <= t <= spam_end]
            if not in_window:
                continue
            xs = [t for t, _ in in_window]
            ys = [v for _, v in in_window]
            label = f"{tick_labels[i]}"

        ax.plot(xs, ys, label=label,
                linewidth=linewidth, linestyle=linestyle, alpha=0.9, color=color)
        plotted_any = True

    if not plotted_any:
        plt.close()
        return

    # Reference line(s) at the hard cap (max_pending). For single-max groups
    # this is one line; for multi-max groups (e.g. just-lower-the-cap mixes
    # max=500/900/1000) we draw one per distinct cap so readers can see
    # each policy peaking against its own ceiling at a glance.
    maxes = sorted({int(r["validator"]["max_pending_transactions"])
                    for r in raw_recs
                    if rec_policy(r) in policies and not r.get("failed")})
    for m in maxes:
        ax.axhline(m, color="black", linestyle="--", linewidth=1,
                   alpha=0.6, label=f"max_pending={m}")
    ax.set_xlabel("Seconds since spam start")
    ax.set_ylabel("In-flight transactions")
    title_qual = "mean across iters" if SAWTOOTH_MODE == "mean" else f"single iter per policy: {SAWTOOTH_MODE}"
    ax.set_title(f"In-flight depth over the spam window ({title_qual}) — RED's iconic sawtooth vs smooth")
    if maxes:
        ax.set_ylim(0.25 * max(maxes), max(maxes) * 1.05)
    ax.minorticks_on()
    ax.grid(which="major", alpha=0.5)
    ax.grid(which="minor", alpha=0.5, linestyle=":", linewidth=0.8)
    # Longer handle samples so the linestyle (dashed/dotted/dash-dot)
    # is unambiguous in the legend — default handlelength=2.0 looks
    # almost identical for dashed vs dash-dot.
    ax.legend(fontsize=8, loc="best", ncol=2, handlelength=5.0)
    plt.tight_layout()
    plt.savefig(out_path, dpi=130)
    plt.close()
    print(f"Wrote {out_path}")


def plot_tps_timeseries(out_path, policies, tick_labels, data_df, raw_recs):
    """TPS over the spam window — mean across iters at each 1s offset.
    Tests RED's "smoother throughput variance" claim directly: graduated
    should produce smoother lines than binary at comparable mean. The
    warm-up prefix (AIMD/consensus ramp-up) is left in the figure so the
    reader can see how each policy converges to steady state, but the
    scalar-overlay path uses only the post-warmup samples."""
    fig, ax = plt.subplots(figsize=(11, 6))
    rec_policy = _rec_policy

    from collections import defaultdict
    plotted_any = False
    means = []
    for i, p in enumerate(policies):
        candidates = [r for r in raw_recs
                     if not r.get("failed") and rec_policy(r) == p]
        if not candidates:
            continue
        by_offset = defaultdict(list)
        for rec in candidates:
            ts = (rec.get("timeseries") or {}).get("tps")
            if not ts:
                continue
            iw = rec.get("iter_window") or {}
            spam_start = iw.get("spam_start_epoch") or 0
            spam_end = iw.get("spam_end_epoch") or 0
            for t, v in ts:
                if spam_start <= t <= spam_end:
                    # Per-validator TPS (raw ts metric is sum across committee).
                    by_offset[round(t - spam_start, 1)].append(v / COMMITTEE_SIZE)
        if not by_offset:
            continue
        xs = sorted(by_offset.keys())
        ys = [sum(by_offset[x]) / len(by_offset[x]) for x in xs]
        means.append(max(ys))
        color, linestyle, linewidth = _line_style_for(p)
        ax.plot(xs, ys, label=f"{tick_labels[i]} (n={len(candidates)})",
                linewidth=linewidth, linestyle=linestyle, alpha=0.9, color=color)
        plotted_any = True

    if not plotted_any:
        plt.close()
        return

    # Shade the warm-up region so the reader knows the steady-state stats
    # skip this prefix. AIMD/consensus ramp typically completes by ~10s.
    ax.axvspan(0, WARMUP_SECONDS, color="gray", alpha=0.12,
               label=f"warm-up (excluded from steady-state stats)")
    ax.set_xlabel("Seconds since spam start")
    ax.set_ylabel("Per-validator TPS")
    ax.set_title("TPS over the spam window (mean across iters) — RED's smoother-throughput claim")
    # Zoom y-axis to the steady-state band. Warm-up samples (0..1500)
    # would otherwise dominate the visual range and hide policy-to-policy
    # differences in the 1500-1600 band where the real comparison lives.
    ax.set_ylim(bottom=1300)
    ax.minorticks_on()
    ax.grid(which="major", alpha=0.5)
    ax.grid(which="minor", alpha=0.5, linestyle=":", linewidth=0.8)
    ax.legend(fontsize=8, loc="best", ncol=2, handlelength=5.0)
    plt.tight_layout()
    plt.savefig(out_path, dpi=130)
    plt.close()
    print(f"Wrote {out_path}")


def plot_group(group):
    """Generate the full plot set for one experiment group."""
    out_dir = HERE / f"{group['dir']}{SUFFIX}"
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

    skip = set(group.get("skip_plots", ())) | GLOBAL_SKIP_PLOTS

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
                f"Peak over/under-shoot ratio — {ratio_label} — [lower = safer]",
                ratio_label, out_dir / "ratio.png", policies,
                tick_labels=short_labels, hline=1)
    boxplot(overshoot_col,
            f"Absolute over/under-shoot — {overshoot_label} — [lower = safer]",
            "Transactions", out_dir / "overshoot.png", policies,
            tick_labels=short_labels, hline=0)
    boxplot("results.useful_tps",
            "Useful TPS — [higher = better]",
            "TPS", out_dir / "tps.png", policies, tick_labels=short_labels)

    # Stage B (permit_wait) and e2e (consensus_lat) latency boxplots at
    # both p50 and p99. Stage B is where graduated's queue-depth reduction
    # cashes out as latency reduction; e2e is the user-visible outcome.
    # Stages A and C are intentionally NOT plotted: A is structurally tiny
    # (~3ms p50) and C is independent of queue depth (per-tx submit_inner
    # work). See methodology appendix for the full A/B/C decomposition.
    if "latency" not in skip:
        for col, title_metric, fname in [
            ("results.permit_wait_p50",   "Semaphore permit wait time — p50",   "wait-p50.png"),
            ("results.permit_wait_p99",   "Semaphore permit wait time — p99",   "wait-p99.png"),
            ("results.consensus_lat_p50", "Admission-to-consensus latency — p50", "e2e-p50.png"),
            ("results.consensus_lat_p99", "Admission-to-consensus latency — p99", "e2e-p99.png"),
        ]:
            boxplot(col,
                    f"{title_metric} — [lower = better]",
                    "seconds", out_dir / fname, policies, tick_labels=short_labels)

    # Mean useful TPS, one bar per policy. The boxplot above shows the full
    # distribution; this gives a single-number headline that's robust to the
    # bimodal-saturation effect that confuses the median.
    if "tps-mean" not in skip:
        means = [df.loc[df.policy == p, "results.useful_tps"].dropna().mean()
                 for p in policies]
        fig, ax = plt.subplots(figsize=(max(7, 1.4 * len(policies)), 5))
        bars = ax.bar(short_labels, means, color="#c5d9f1", edgecolor="black", alpha=0.8)
        for bar, value in zip(bars, means):
            ax.annotate(f"{value:.0f}",
                        xy=(bar.get_x() + bar.get_width() / 2, value),
                        xytext=(0, 3), textcoords="offset points",
                        ha="center", fontsize=9)
        ax.set_title("Mean useful TPS per policy — [higher = better]")
        ax.set_ylabel("Mean useful TPS")
        ax.minorticks_on()
        ax.grid(which="major", axis="y", alpha=0.5)
        ax.grid(which="minor", axis="y", alpha=0.5, linestyle=":", linewidth=0.8)
        plt.setp(ax.get_xticklabels(), fontsize=8, rotation=45, ha="right")
        highlight_tick_labels(ax, policies)
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

    # ---------- RED-canonical additions ----------

    # RED Claim "smaller queue at same throughput": mean in-flight per
    # policy. Pairs visually with tps.png (same throughput) and overshoot
    # (same effective ceiling). Lower = smaller average queue depth.
    if "inflight-mean" not in skip and "results.inflight_mean" in df.columns:
        boxplot("results.inflight_mean",
                "Mean in-flight depth — RED's lower queue at same TPS — [lower = better]",
                "Transactions", out_dir / "inflight-mean.png", policies, tick_labels=short_labels)

    # RED's saw-tooth-amplitude claim: std(Δ inflight) over the spam
    # window. Independent of mean level (unlike CV). Hidden by default
    # via GLOBAL_SKIP_PLOTS — the data shows graduated is NOT smoother
    # in saw-tooth amplitude, so a standalone plot would misframe the
    # story. The firstdiff_std_med column in summary.md still carries
    # the numbers for inspection.
    if "inflight-firstdiff" not in skip and "inflight_firstdiff_std" in df.columns:
        boxplot("inflight_firstdiff_std",
                "In-flight saw-tooth amplitude — std(Δ inflight) — [lower = smoother]",
                "stddev of Δ inflight (txs)",
                out_dir / "inflight-firstdiff.png", policies, tick_labels=short_labels)

    # RED Claim 2 (uniform drop probability): honest_cl_admit_frac /
    # spammer_admit_frac. 1.0 = uniform across sources. >>1 = phase-effect
    # bias (binary's tail-drop pathology).
    if "red-ratio" not in skip and "red_ratio" in df.columns:
        boxplot("red_ratio",
                "Admit fairness — honest / spammer — [1.0 = uniform]",
                "Ratio", out_dir / "red-ratio.png", policies,
                tick_labels=short_labels, hline=1.0)

    # Validator-side drop probability — the authoritative Claim-2 surface.
    # drop_prob = (preventive + saturated + reactive) / (drops + commits).
    # At heavy overload approaches 1.0; differences between policies are
    # small (both binary and graduated saturate the validator). The story
    # is in HOW the validator drops (drop-classification.png shows the band
    # split), not in the total drop fraction.
    if ("drop-prob" not in skip
            and "validator_drop_prob" in df.columns
            and df["validator_drop_prob"].notna().any()):
        boxplot("validator_drop_prob",
                "Validator drop probability — rejected / (rejected + finalized) — [higher = better]",
                "Drop probability",
                out_dir / "drop-prob.png", policies,
                tick_labels=short_labels)

    # Drop classification (RED's Option B / "early vs forced" drops).
    # Stacked bar per policy: reactive (hard cap), saturated (100%-shed
    # band), preventive (probabilistic soft zone). Binary: all reactive.
    # Graduated: mostly saturated + preventive, near-zero reactive.
    if "drop-classification" not in skip:
        plot_drop_classification(out_dir / "drop-classification.png", policies,
                                 short_labels, df)

    # Inflight time-series (the iconic RED sawtooth-vs-smooth figure).
    # One representative iter per policy, inflight(t) at 100ms granularity,
    # scoped to the spam window via spam_start/spam_end epochs.
    if "inflight-timeseries" not in skip:
        plot_inflight_timeseries(out_dir / "inflight-timeseries.png", policies,
                                 short_labels, df, _raw_recs)

    # TPS time-series (per-validator throughput over time, mean across iters).
    # Counterpart to inflight-timeseries — directly tests RED's "smoother
    # throughput variance" claim by showing whether graduated produces flatter
    # TPS curves than binary.
    if "tps-timeseries" not in skip:
        plot_tps_timeseries(out_dir / "tps-timeseries.png", policies,
                            short_labels, df, _raw_recs)

    # Intra-iter TPS coefficient of variation. RED predicts graduated has
    # lower TPS CV than binary at comparable mean throughput.
    if "tps-cv" not in skip and "tps_cv_ts" in df.columns:
        if df["tps_cv_ts"].notna().any():
            boxplot("tps_cv_ts",
                    "Intra-iter TPS coefficient of variation — CV = std_dev / mean — [lower = smoother throughput]",
                    "TPS CV",
                    out_dir / "tps-cv.png", policies,
                    data_df=df, tick_labels=short_labels)

    # Saturated-only CV: filter the data to iters where the system was actually
    # loaded for ≥30% of the run. Skipped automatically if no iters qualify
    # (typical for sem-bound configs where saturation_75pct ≈ 0).
    if "cv-saturated" not in skip and "results.saturation_75pct" in df.columns:
        sat_df = df[df["results.saturation_75pct"].fillna(0) > 0.3].copy()
        if any((sat_df["policy"] == p).any() for p in policies):
            boxplot("inflight_cv",
                    "In-flight stability under sustained load — CV = std_dev / mean — [lower = smoother]",
                    "Coefficient of Variation",
                    out_dir / "cv-saturated.png", policies,
                    data_df=sat_df, tick_labels=short_labels)

    # Tradeoff scatter — uses the group's ratio metric on x-axis. Each
    # policy gets a distinct marker + the family color from _line_style_for
    # so visual identity matches the inflight-timeseries plot.
    _, ax = plt.subplots(figsize=(9, 6))
    for p in policies:
        sub = df[df.policy == p]
        color, _, _ = _line_style_for(p)
        marker, mult = _marker_for(p)
        ax.scatter(sub[ratio_col], sub["results.useful_tps"],
                   label=label_for[p], alpha=0.7, s=36 * mult, edgecolor="white",
                   color=color, marker=marker)
    ax.set_xlabel(f"{ratio_label}  (← safer)")
    ax.set_ylabel("Useful TPS  (better →)")
    ax.set_title("Safety / throughput trade-off")
    ax.minorticks_on()
    ax.grid(which="major", alpha=0.5)
    ax.grid(which="minor", alpha=0.5, linestyle=":", linewidth=0.8)
    leg = ax.legend(fontsize=8, loc="best")
    # Apply the same green/red highlights to legend entries. The legend
    # is in the same order as `policies`, so we match by index into the
    # full policy string (same matching logic as x-tick labels).
    for text, policy in zip(leg.get_texts(), policies):
        color = _highlight_color(policy)
        if color:
            text.set_bbox(dict(facecolor=color, alpha=0.5,
                               edgecolor="none",
                               boxstyle="round,pad=0.25"))
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
