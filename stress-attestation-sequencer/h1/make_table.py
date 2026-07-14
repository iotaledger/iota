#!/usr/bin/env python3
"""h1-table — one comparison table (Markdown) across all experiment labels.

Companion to plot.py: where plot.py draws one figure per dashboard panel, this
collapses each panel to a single scalar per config and lays them out in a table.

  - Rows are experiment labels (results/<LABEL>/ — one config each, per run.sh's
    config gate). Its iter-NNN/ subdirs are the iterations, pooled.
  - Columns are metrics, one pair per metric: A = V1 (attestation OFF),
    B = V2 (attestation ON) — the H1 comparison.
  - Each cell reduces the metric to one number: the network-level per-run series
    (computed EXACTLY as plot.py does — rate() for counters, histogram_quantile()
    over pooled buckets, per-validator collapse) is reduced over time by --stat
    (mean/median), pooled across all timepoints of all iterations. The dispersion
    (± std by default, or sem) is shown next to it.

The metric set is derived from the SAME dashboard plot.py reads, so adding a panel
there adds a column here — no edits. --all keeps the Tier-3 flat-zero safety gates
(dropped by default, matching plot.py's SKIP_PANELS).

Usage:
  .venv/bin/python make_table.py                     # n4 -> results/summary_table_n4.md
  .venv/bin/python make_table.py --net 48            # n48 -> results/summary_table_n48.md
  .venv/bin/python make_table.py --stat median --disp sem
  .venv/bin/python make_table.py --all --csv results/summary_table.csv
"""

import argparse
import glob
import json
import os
import re
import sys
import warnings

import numpy as np


# A metric may be absent for a config/version, so reducing its (all-NaN) series is
# expected and raises benign numpy RuntimeWarnings — silence just those messages.
# Set at import so forked workers inherit it; compute_row re-applies for spawn.
def _silence_numpy_warnings():
    for msg in (
        "Mean of empty slice",
        "All-NaN slice encountered",
        "invalid value encountered",
        "Degrees of freedom <= 0",
    ):
        warnings.filterwarnings("ignore", message=msg)


_silence_numpy_warnings()

# Reuse plot.py's dashboard parsing + PromQL replay so the table matches the plots
# exactly and stays in sync with the dashboard.
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import plot  # noqa: E402


def _iter_files(root, label):
    """(v1_paths, v2_paths) for a label — the run JSONs to pool, same as plot.py."""
    ld = os.path.join(root, label)
    v1 = sorted(glob.glob(os.path.join(ld, "iter-*", "run-a-v1-timeseries.json")))
    v2 = sorted(glob.glob(os.path.join(ld, "iter-*", "run-b-v2-timeseries.json")))
    return v1, v2


def discover_labels(root, label=None):
    """{label: (n_v1, n_v2)} for every label with data — cheap (globs, no JSON load)."""
    candidates = (
        [label]
        if label
        else sorted(
            os.path.basename(d)
            for d in glob.glob(os.path.join(root, "*"))
            if os.path.isdir(d)
        )
    )
    out = {}
    for lab in candidates:
        v1, v2 = _iter_files(root, lab)
        if v1 or v2:
            out[lab] = (len(v1), len(v2))
    return out


def load_groups(root, label=None):
    """{label: {"V1": [run,...], "V2": [run,...]}} — same grouping as plot.py."""
    groups = {}
    for lab in discover_labels(root, label):
        v1, v2 = _iter_files(root, lab)
        groups[lab] = {
            "V1": [json.load(open(f)) for f in v1],
            "V2": [json.load(open(f)) for f in v2],
        }
    return groups


# Columns pruned from the table after inspecting the full picture — no signal for
# this experiment set. Maps column key (see build_columns) -> expected value, or
# None to drop unconditionally. If an expected value is given, the column is still
# COMPUTED and checked against it: any config/version that deviates (beyond
# DROP_TOL) prints a warning — the assumption behind the drop no longer holds for
# that run, so it shouldn't be silently hidden. The column is dropped either way.
# Pass --keep-dropped to render them regardless.
#   - actual/attested CUs — mean: exactly 1.0 for every owned-object config (attested
#     CUs == actual CUs). A future shared/slow config where attestation over-estimates
#     would trip the guard.
#   - soft-lock rejections / sec: 0 across all configs (no soft-lock equivocation in
#     these workloads). Any non-zero rate would trip the guard.
#   - cancelled txs / sec, backpressure toggles / sec, execution backpressure active:
#     0 across all configs (no congestion cancellation / no execution backpressure in
#     these workloads). Any non-zero value would trip the guard.
#     (NB: validation dropped txs / sec is NOT here — it's a real non-zero V1 signal.)
DROP_COLUMNS = {
    "actual/attested CUs — mean": 1.0,
    # The raw p50 CU twins: superseded by the exact mean "CUs" column (see
    # build_columns). Dropped unconditionally (None) — for this deterministic
    # workload a p50 only bucket-interpolates and lands on impossible values.
    "attested vs actual computation units (CUs, p50) [actual p50]": None,
    "attested vs actual computation units (CUs, p50) [attested p50]": None,
    "soft-lock rejections / sec": 0.0,
    "cancelled txs / sec": 0.0,
    "backpressure toggles / sec": 0.0,
    "execution backpressure active (0/1)": 0.0,
}
DROP_TOL = 1e-4

# Pairs of columns that are the SAME quantity computed two ways (near-equal by
# construction). Keep one under a friendlier name; the twin ("drop") is hidden but
# still COMPUTED and checked — a relative divergence beyond MERGE_RTOL warns (the
# equality no longer holds for that run). Pass --keep-dropped to show the twin.
# (CUs used to merge the actual/attested p50 twins here; it is now an exact mean
# column — see build_columns — with attested == actual guarded by the
# "actual/attested CUs — mean" ratio column.)
MERGE_EQUAL = []
MERGE_RTOL = 1e-2

# Columns whose per-container series must be restricted to validators before the
# host_reduce collapse — the cadvisor panels also scrape the fullnode (name=~"$host"
# matches all containers), which we don't want in a "busiest validator" number.
# Keyed by original column key; adds a `name=~"validator-.*"` filter to the spec.
VALIDATORS_ONLY = {
    "per-validator CPU (busy cores, cadvisor)",
    "per-validator memory RSS (cadvisor)",
}
VALIDATOR_NAME_RE = "validator-.*"

# Display-only column renames, keyed by original column key (see build_columns) ->
# {"name": new key, "unit": optional unit override — "" drops the unit suffix}.
# Purely cosmetic (headers + CSV metric names); does not change any values. Units
# are baked into the names below, so each clears the auto unit suffix ("unit": "").
RENAME_COLUMNS = {
    "finalized TPS (included in checkpoint)": {"name": "TPS", "unit": ""},
    "full attestation latency p50 (wait + exec + resume)": {
        "name": "attest. full p50 (s)",
        "unit": "",
    },
    "full attestation latency p95 (wait + exec + resume)": {
        "name": "attest. full p95 (s)",
        "unit": "",
    },
    "full attestation latency p99 (wait + exec + resume)": {
        "name": "attest. full p99 (s)",
        "unit": "",
    },
    "attestation dry-run execution p50": {"name": "attest. exec p50 (s)", "unit": ""},
    "attestation dry-run execution p95": {"name": "attest. exec p95 (s)", "unit": ""},
    "attestation dry-run execution p99": {"name": "attest. exec p99 (s)", "unit": ""},
    "attestation pool wait p50 (spawn_blocking)": {
        "name": "attest. wait p50 (s)",
        "unit": "",
    },
    "attestation pool wait p95 (spawn_blocking)": {
        "name": "attest. wait p95 (s)",
        "unit": "",
    },
    "attestation pool wait p99 (spawn_blocking)": {
        "name": "attest. wait p99 (s)",
        "unit": "",
    },
    "attestation async resume p50 (runtime reschedule)": {
        "name": "attest. resume p50 (s)",
        "unit": "",
    },
    "attestation async resume p95 (runtime reschedule)": {
        "name": "attest. resume p95 (s)",
        "unit": "",
    },
    "attestation async resume p99 (runtime reschedule)": {
        "name": "attest. resume p99 (s)",
        "unit": "",
    },
    "attestations / sec": {"name": "attest. / sec", "unit": ""},
    "host CPU (busy cores, whole machine)": {"name": "host CPU", "unit": ""},
    "receipt → executed — p50": {"name": "rec. → exec. p50 (s)", "unit": ""},
    "receipt → executed — p95": {"name": "rec. → exec. p95 (s)", "unit": ""},
    "receipt → executed — p99": {"name": "rec. → exec. p99 (s)", "unit": ""},
    "post-consensus validation latency — p50": {
        "name": "pc valid. lat. p50 (s)",
        "unit": "",
    },
    "post-consensus validation latency — p95": {
        "name": "pc valid. lat. p95 (s)",
        "unit": "",
    },
    "internal execution latency p95": {"name": "exec. lat. p95 (s)", "unit": ""},
    "checkpoint creation lag — p50": {"name": "ckpt lag p50 (s)", "unit": ""},
    "checkpoint creation lag — p95": {"name": "ckpt lag p95 (s)", "unit": ""},
    "checkpoint creation lag — p99": {"name": "ckpt lag p99 (s)", "unit": ""},
    # post-consensus load shedding (PR #11301)
    "post-consensus load-shed drops / sec": {
        "name": "post-cons shed drops / sec",
        "unit": "",
    },
    "post-consensus load-shed % — quorum": {"name": "shed % quorum", "unit": ""},
    "post-consensus load-shed % — local": {"name": "shed % local", "unit": ""},
    "consensus-queue load shedding %": {"name": "shed % cons-queue", "unit": ""},
    # pre-consensus admission-control shedding
    "pre-consensus overload rejections / sec": {
        "name": "pre-cons overload rej / sec",
        "unit": "",
    },
    "consensus overload source: graduated / sec": {
        "name": "overload graduated / sec",
        "unit": "",
    },
    "consensus overload source: max-pending / sec": {
        "name": "overload max-pending / sec",
        "unit": "",
    },
    "consensus overload source: semaphore / sec": {
        "name": "overload semaphore / sec",
        "unit": "",
    },
    "consensus in-flight transactions (num_inflight → max_pending)": {
        "name": "num_inflight",
        "unit": "",
    },
    "validation dropped txs / sec": {"name": "valid. drop. / sec", "unit": ""},
    "settlement finality latency (client, via fullnode) [transaction p50]": {
        "name": "final. lat. p50 (s)",
        "unit": "",
    },
    "settlement finality latency (client, via fullnode) [transaction p95]": {
        "name": "final. lat. p95 (s)",
        "unit": "",
    },
    "settlement finality latency (client, via fullnode) [transaction p99]": {
        "name": "final. lat. p99 (s)",
        "unit": "",
    },
    "submit transaction latency (client, via fullnode) [transaction p50]": {
        "name": "submit lat. p50 (s)",
        "unit": "",
    },
    "submit transaction latency (client, via fullnode) [transaction p95]": {
        "name": "submit lat. p95 (s)",
        "unit": "",
    },
    "execution dispatch queue": {"name": "exec. dispatch queue", "unit": ""},
    "pending transactions (waiting for inputs)": {"name": "pending txs", "unit": ""},
    "execution queueing delay p95": {"name": "exec. queue. delay p95 (s)", "unit": ""},
    "per-validator CPU (busy cores, cadvisor)": {"name": "node CPU", "unit": ""},
    "per-validator memory RSS (cadvisor)": {
        "name": "node memory RSS (bytes)",
        "unit": "",
    },
}


# Metrics reduced over TIME by peak (max-over-time) instead of the --stat default
# (mean). Keyed by final column key (post-rename). These are bursty instantaneous
# gauges whose mean-over-time hides the spike that actually hits a limit — e.g.
# num_inflight sits at ~4k on average but peaks to the 10k submit-semaphore size;
# shedding is 0 most of the window and spikes during stalls, so a mean dilutes it
# toward 0. Rates and latencies stay mean (see cell_values).
TIME_REDUCE = {
    "num_inflight": "max",
    "exec. dispatch queue": "max",
    "pending txs": "max",
    "shed % cons-queue": "max",
    "shed % quorum": "max",
    "shed % local": "max",
}


def build_columns(panels, keep_all):
    """Flatten the dashboard into (col_key, kind, payload) column specs.

    Returns ALL columns (dropping is applied later in main, so DROP_COLUMNS guards
    can still be computed and checked).

    kind="target" -> payload is a parsed expr spec (+ host_reduce);
    kind="mean_ratio" -> payload is the histogram base name (the accuracy panel
    plot.py renders as a mean, not a quantile)."""
    cols = []
    for panel in panels:
        title = panel["title"]
        if not keep_all and title in plot.SKIP_PANELS:
            continue
        ov = plot.PANEL_OVERRIDES.get(title, {})
        host_reduce = ov.get("host_reduce")
        specs = [plot.parse_expr(e) for e in panel["exprs"]]
        multi = len(specs) > 1
        for spec in specs:
            if title in VALIDATORS_ONLY:
                # restrict to validator containers (drop the fullnode) before collapse.
                spec["filters"] = spec["filters"] + [("name", "=~", VALIDATOR_NAME_RE)]
            tag = plot.target_tag(spec, multi)
            key = title + (f" [{tag}]" if tag else "")
            unit = panel["unit"] or ""
            cols.append(
                {
                    "key": key,
                    "unit": unit,
                    "kind": "target",
                    "spec": spec,
                    "host_reduce": host_reduce,
                }
            )
    # The attestation-accuracy panel is plotted as a MEAN ratio (rate(_sum)/
    # rate(_count)), not a quantile — mirror that as its own column.
    base = "actual_to_attested_computation_units_ratio"
    cols.append(
        {
            "key": "actual/attested CUs — mean",
            "unit": "ratio",
            "kind": "mean_ratio",
            "base": base,
        }
    )
    # CUs are the exact per-transaction mean (rate(_sum)/rate(_count)), not a p50:
    # the workload is deterministic (every tx identical), so the mean is the exact
    # cost. A histogram p50 interpolates between bucket edges and lands on
    # impossible values (e.g. 850, below the 1000-unit gas_rounding_step floor).
    cols.append(
        {
            "key": "CUs",
            "unit": "",
            "kind": "mean_ratio",
            "base": "actual_computation_units",
        }
    )
    # Rename the kept twin of each MERGE_EQUAL pair; the dropped twin keeps its
    # original key so the equality guard in main can still find it.
    by_key = {c["key"]: c for c in cols}
    for m in MERGE_EQUAL:
        if m["keep"] in by_key:
            by_key[m["keep"]]["key"] = m["name"]
    # Cosmetic display renames (headers + CSV metric names only).
    for c in cols:
        rn = RENAME_COLUMNS.get(c["key"])
        if rn:
            c["key"] = rn["name"]
            if "unit" in rn:
                c["unit"] = rn["unit"]
    return cols


def compute_row(task):
    """Load ONE label and reduce every column to its A/B cells. Runs in a worker
    process (labels are independent), so it loads its own JSON rather than being
    handed the big run dicts. -> (label, {col_key: {"A": (c,s,sem,n), "B": ...}})."""
    _silence_numpy_warnings()  # spawn workers don't inherit the parent's filters
    label, root, cols, window, stat = task
    g = load_groups(root, label)[label]
    runs_all = g["V1"] + g["V2"]
    win = max((r["end_epoch"] - r["start_epoch"]) for r in runs_all)
    grid = np.arange(0, win + 1, 1.0)
    row = {}
    for col in cols:
        a = cell_values(col, g["V1"], grid, window, stat)
        b = cell_values(col, g["V2"], grid, window, stat)
        row[col["key"]] = {"A": a, "B": b}
    return label, row


def cell_values(col, runs, grid, window, stat):
    """Reduce a column's per-run series to (center, std, sem, n).

    Default (mean/median): center/std pool every finite timepoint across all
    iterations (the sample IS the timeseries values); sem = std of the
    per-iteration temporal means / sqrt(n) — the cross-iteration uncertainty.

    Metrics in TIME_REDUCE use PEAK (max-over-time): each iteration is reduced by
    its max, then center = mean of the per-iteration peaks (std/sem across them).
    For a bursty queue/shed gauge the mean-over-time hides the peak that actually
    hits a limit (e.g. num_inflight spikes to the submit-semaphore size)."""
    if not runs:
        return np.nan, np.nan, np.nan, 0
    per_run = []
    for r in runs:
        if col["kind"] == "mean_ratio":
            per_run.append(plot.eval_mean_ratio(col["base"], r, grid, window))
        else:
            per_run.append(
                plot.eval_target(col["spec"], r, grid, window, stat, col["host_reduce"])
            )
    if TIME_REDUCE.get(col["key"]) == "max":
        # peak per iteration, then average the peaks across iterations.
        peaks = [float(np.nanmax(s)) for s in per_run if np.any(np.isfinite(s))]
        if not peaks:
            return np.nan, np.nan, np.nan, len(runs)
        sem = (
            float(np.std(peaks, ddof=1) / np.sqrt(len(peaks)))
            if len(peaks) > 1
            else np.nan
        )
        return float(np.mean(peaks)), float(np.std(peaks)), sem, len(runs)
    pooled = np.concatenate(per_run) if per_run else np.array([])
    pooled = pooled[np.isfinite(pooled)]
    if pooled.size == 0:
        return np.nan, np.nan, np.nan, len(runs)
    center = float(np.mean(pooled) if stat == "mean" else np.median(pooled))
    std = float(np.std(pooled))
    per_iter = [float(np.nanmean(s)) for s in per_run if np.any(np.isfinite(s))]
    sem = (
        float(np.std(per_iter, ddof=1) / np.sqrt(len(per_iter)))
        if len(per_iter) > 1
        else np.nan
    )
    return center, std, sem, len(runs)


def fmt(x):
    if x is None or not np.isfinite(x):
        return "—"
    if x == 0:
        return "0"
    ax = abs(x)
    if ax >= 1e5 or ax < 1e-4:
        return f"{x:.3e}"
    return f"{x:.4g}"


def fmt_cell(center, disp_val, disp):
    if not np.isfinite(center):
        return "—"
    if disp == "none" or not np.isfinite(disp_val):
        return fmt(center)
    return f"{fmt(center)}±{fmt(disp_val)}"


def sort_key(label):
    """slow{S}-owned-{f1|v1|v4}-qps{Q}[-n{N}] -> (network size, slow, qps, path):
    group by network size first, then slow, then qps, with the paths adjacent
    for each qps. Labels without an -n suffix are the old 4-validator runs."""
    slow = re.search(r"slow(\d+)", label)
    qps = re.search(r"qps(\d+)", label)
    wl = re.search(r"owned-([a-z0-9]+)", label)
    n = re.search(r"-n(\d+)$", label)
    return (
        int(n.group(1)) if n else 4,
        int(slow.group(1)) if slow else 0,
        int(qps.group(1)) if qps else 0,
        wl.group(1) if wl else "",
        label,
    )


def main():
    here = os.path.dirname(os.path.abspath(__file__))
    ap = argparse.ArgumentParser()
    ap.add_argument("--root", default=os.path.join(here, "results"))
    ap.add_argument(
        "--dashboard",
        default=os.path.join(
            here,
            "..",
            "..",
            "dev-tools",
            "grafana-local",
            "dashboards",
            "attestation-sequencer-stress.json",
        ),
    )
    ap.add_argument("--label", default=None, help="only this label (default: all)")
    ap.add_argument(
        "--net",
        type=int,
        default=4,
        help="only configs of this network size (the label's -n<N> suffix; "
        "labels without a suffix count as 4). Default 4; pass 48 for the "
        "48-validator campaign, or 0 for all sizes",
    )
    ap.add_argument(
        "--stat",
        choices=["mean", "median"],
        default="mean",
        help="temporal reduction of each run's series (default mean)",
    )
    ap.add_argument(
        "--disp",
        choices=["std", "sem", "none"],
        default="std",
        help="dispersion shown after ±: pooled std, cross-iter sem, or none",
    )
    ap.add_argument("--rate-window", type=int, default=10, help="rate() window (s)")
    ap.add_argument(
        "--all",
        action="store_true",
        help="keep Tier-3 flat-zero safety panels too (default: drop)",
    )
    ap.add_argument(
        "--keep-dropped",
        action="store_true",
        help="keep columns pruned as no-signal (see DROP_COLUMNS)",
    )
    ap.add_argument(
        "--layout",
        choices=["combined", "split"],
        default="combined",
        help="combined: one column per metric, cell `A / B`; "
        "split: separate A and B columns (default combined)",
    )
    ap.add_argument(
        "--out",
        default=None,
        help="output .md (default: results/summary_table_n<net>.md)",
    )
    ap.add_argument(
        "--csv",
        default=None,
        help="tidy CSV path (default: results/summary_table_n<net>.csv, "
        "always written — summary_plot.py reads it)",
    )
    ap.add_argument(
        "--jobs",
        type=int,
        default=min(os.cpu_count() or 1, 16),
        help="parallel worker processes, one label at a time (default: min(cpus,16))",
    )
    args = ap.parse_args()

    iter_counts = discover_labels(args.root, args.label)  # {label: (n_v1, n_v2)}
    if args.net:
        iter_counts = {
            lab: v for lab, v in iter_counts.items() if sort_key(lab)[0] == args.net
        }
    if not iter_counts:
        print(
            f"no <LABEL>/iter-*/run-*-timeseries.json under {args.root}"
            + (f" for network size n{args.net}" if args.net else ""),
            file=sys.stderr,
        )
        sys.exit(1)

    panels = plot.panels_from_dashboard(args.dashboard)
    all_cols = build_columns(panels, args.all)
    labels = sorted(iter_counts, key=sort_key)

    # Columns hidden from the table but still computed for their guard:
    #   DROP_COLUMNS with an expected value, and MERGE_EQUAL twins.
    merge_twins = {m["drop"] for m in MERGE_EQUAL}
    hidden = set(DROP_COLUMNS) | merge_twins
    if args.keep_dropped:
        cols = all_cols
        verify_cols = []
    else:
        cols = [c for c in all_cols if c["key"] not in hidden]
        verify_cols = [
            c
            for c in all_cols
            if c["key"] in merge_twins
            or (c["key"] in DROP_COLUMNS and DROP_COLUMNS[c["key"]] is not None)
        ]

    # Compute every cell up front: rows[label][col_key] = {"A": (c,s,sem,n), "B": ...}.
    # Includes verify_cols so guarded drops can be checked below. Labels are
    # independent, so fan them out across processes (each worker loads its own JSON).
    compute_cols = cols + verify_cols
    tasks = [
        (label, args.root, compute_cols, args.rate_window, args.stat)
        for label in labels
    ]
    rows = {}
    if args.jobs == 1:
        for t in tasks:
            label, row = compute_row(t)
            rows[label] = row
    else:
        from concurrent.futures import ProcessPoolExecutor

        with ProcessPoolExecutor(max_workers=args.jobs) as ex:
            for label, row in ex.map(compute_row, tasks):
                rows[label] = row

    # Guard: a dropped column with an expected value must actually hold it for every
    # config/version, else the reason for dropping it no longer applies — warn (still drop).
    for key, exp in DROP_COLUMNS.items():
        if args.keep_dropped or exp is None:
            continue
        bad = []
        for label in labels:
            for side, ver in (("A", "V1"), ("B", "V2")):
                c = rows[label][key][side][0]
                if np.isfinite(c) and abs(c - exp) > DROP_TOL:
                    bad.append((label, ver, c))
        if bad:
            print(
                f"WARNING: dropped column '{key}' expected ~{exp:g} but deviates "
                f"in {len(bad)} config/version(s) — still dropped (use --keep-dropped to inspect):",
                file=sys.stderr,
            )
            for label, ver, c in bad:
                print(f"    {label} {ver}: {c:.6g}", file=sys.stderr)

    # Guard: each MERGE_EQUAL pair must actually be near-equal — warn (still merge)
    # if the kept column and its hidden twin diverge by more than MERGE_RTOL.
    for m in MERGE_EQUAL:
        if args.keep_dropped:
            continue
        bad = []
        for label in labels:
            for side, ver in (("A", "V1"), ("B", "V2")):
                a = rows[label][m["name"]][side][0]
                b = rows[label][m["drop"]][side][0]
                if np.isfinite(a) and np.isfinite(b):
                    denom = max(abs(a), abs(b), 1e-12)
                    if abs(a - b) / denom > MERGE_RTOL:
                        bad.append((label, ver, a, b))
        if bad:
            print(
                f"WARNING: merged column '{m['name']}' — kept '{m['keep']}' but its twin "
                f"'{m['drop']}' diverges >{MERGE_RTOL:g} rel. in {len(bad)} config/version(s) "
                f"(use --keep-dropped to inspect):",
                file=sys.stderr,
            )
            for label, ver, a, b in bad:
                print(f"    {label} {ver}: kept={a:.6g} twin={b:.6g}", file=sys.stderr)

    disp_idx = 1 if args.disp == "std" else 2  # index into (center,std,sem,n)

    # ---- Markdown ------------------------------------------------------------
    suffix = f"_n{args.net}" if args.net else ""
    out_path = args.out or os.path.join(args.root, f"summary_table{suffix}.md")
    lines = []
    lines.append("# H1 — attestation A/B comparison across configs\n")
    lines.append(
        f"- **A** = V1 (attestation OFF), **B** = V2 (attestation ON).\n"
        f"- Cell = `{args.stat} ± {args.disp}` of the network-level series over time, "
        f"pooled across iterations (`—` = no data / metric absent).\n"
        f"- Bursty queue/shed gauges (num_inflight, dispatch queue, pending txs, "
        f"shed %) instead report the **peak** (max-over-time, mean of per-iter peaks) "
        f"— a mean would hide the spike that hits a limit.\n"
        f"- Series are computed exactly as plot.py does (rate/histogram_quantile, "
        f"per-validator collapse); rate window = {args.rate_window}s.\n"
        f"- {len(labels)} config(s), {len(cols)} metric(s)."
        + (
            ""
            if args.all
            else " Tier-3 flat-zero safety panels dropped (use --all to keep)."
        )
        + "\n"
    )
    # iteration counts per label
    lines.append("\n**Iterations pooled per config** (A / B):\n")
    lines.append("\n| config | A iters | B iters |")
    lines.append("| --- | --- | --- |")
    for label in labels:
        n_v1, n_v2 = iter_counts[label]
        lines.append(f"| {label} | {n_v1} | {n_v2} |")
    lines.append("")

    # main table. combined: one column per metric, cell `A → B`; split: A and B
    # each get their own column.
    def unit_suffix(col):
        return f" ({col['unit']})" if col["unit"] and col["unit"] != "none" else ""

    header = ["config"]
    for col in cols:
        u = unit_suffix(col)
        if args.layout == "combined":
            # cell carries A/B; the note above the table explains it, so the header
            # stays clean (no per-column " (A / B)" suffix).
            header.append(f"{col['key']}{u}")
        else:
            header.append(f"{col['key']}{u} · A")
            header.append(f"{col['key']}{u} · B")
    lines.append("\n## Full table\n")
    if args.layout == "combined":
        lines.append(
            "Each cell is `A / B` = V1 (attestation OFF) / V2 (attestation ON).\n"
        )
    lines.append("| " + " | ".join(header) + " |")
    lines.append("| " + " | ".join(["---"] * len(header)) + " |")
    for label in labels:
        cells = [label]
        for col in cols:
            cv = rows[label][col["key"]]
            a = fmt_cell(cv["A"][0], cv["A"][disp_idx], args.disp)
            b = fmt_cell(cv["B"][0], cv["B"][disp_idx], args.disp)
            if args.layout == "combined":
                cells.append(f"{a} / {b}")
            else:
                cells.extend([a, b])
        lines.append("| " + " | ".join(cells) + " |")

    with open(out_path, "w") as f:
        f.write("\n".join(lines) + "\n")
    print(f"wrote {out_path}  ({len(labels)} rows x {len(cols)} metrics)")

    # ---- tidy CSV (always written — summary_plot.py reads it) -----------------
    import csv

    csv_path = args.csv or os.path.join(args.root, f"summary_table{suffix}.csv")
    with open(csv_path, "w", newline="") as f:
        w = csv.writer(f)
        w.writerow(
            [
                "config",
                "metric",
                "unit",
                "version",
                "center",
                "std",
                "sem",
                "n_iters",
            ]
        )
        for label in labels:
            for col in cols:
                for side, ver in (("A", "V1"), ("B", "V2")):
                    c, s, sem, n = rows[label][col["key"]][side]
                    w.writerow([label, col["key"], col["unit"], ver, c, s, sem, n])
    print(f"wrote {csv_path}")


if __name__ == "__main__":
    main()
