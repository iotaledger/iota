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
  .venv/bin/python make_table.py                     # -> results/summary_table.md
  .venv/bin/python make_table.py --stat median --disp sem
  .venv/bin/python make_table.py --all --csv results/summary_table.csv
"""

import argparse
import glob
import json
import os
import re
import sys

import numpy as np

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
DROP_COLUMNS = {
    "actual/attested CUs — mean": 1.0,
}
DROP_TOL = 1e-4


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
    return cols


def compute_row(task):
    """Load ONE label and reduce every column to its A/B cells. Runs in a worker
    process (labels are independent), so it loads its own JSON rather than being
    handed the big run dicts. -> (label, {col_key: {"A": (c,s,sem,n), "B": ...}})."""
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

    center/std pool every finite timepoint across all iterations (the sample IS
    the timeseries values). sem = std of the per-iteration temporal means / sqrt(n)
    — the cross-iteration uncertainty (undefined for a single iteration)."""
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
    """slow{N}-owned-{f|v}-qps{Q} -> (N, workload, Q) so rows read in a sane order."""
    slow = re.search(r"slow(\d+)", label)
    qps = re.search(r"qps(\d+)", label)
    wl = re.search(r"owned-([a-z]+)", label)
    return (
        int(slow.group(1)) if slow else 0,
        wl.group(1) if wl else "",
        int(qps.group(1)) if qps else 0,
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
        "--out", default=None, help="output .md (default: results/summary_table.md)"
    )
    ap.add_argument("--csv", default=None, help="also write a tidy CSV here")
    ap.add_argument(
        "--jobs",
        type=int,
        default=min(os.cpu_count() or 1, 16),
        help="parallel worker processes, one label at a time (default: min(cpus,16))",
    )
    args = ap.parse_args()

    iter_counts = discover_labels(args.root, args.label)  # {label: (n_v1, n_v2)}
    if not iter_counts:
        print(
            f"no <LABEL>/iter-*/run-*-timeseries.json under {args.root}",
            file=sys.stderr,
        )
        sys.exit(1)

    panels = plot.panels_from_dashboard(args.dashboard)
    all_cols = build_columns(panels, args.all)
    labels = sorted(iter_counts, key=sort_key)

    # Columns to render vs. columns to compute-only for their DROP_COLUMNS guard.
    if args.keep_dropped:
        cols = all_cols
        verify_cols = []
    else:
        cols = [c for c in all_cols if c["key"] not in DROP_COLUMNS]
        verify_cols = [
            c
            for c in all_cols
            if c["key"] in DROP_COLUMNS and DROP_COLUMNS[c["key"]] is not None
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
    for col in verify_cols:
        exp = DROP_COLUMNS[col["key"]]
        bad = []
        for label in labels:
            for side, ver in (("A", "V1"), ("B", "V2")):
                c = rows[label][col["key"]][side][0]
                if np.isfinite(c) and abs(c - exp) > DROP_TOL:
                    bad.append((label, ver, c))
        if bad:
            print(
                f"WARNING: dropped column '{col['key']}' expected ~{exp:g} but deviates "
                f"in {len(bad)} config/version(s) — still dropped (use --keep-dropped to inspect):",
                file=sys.stderr,
            )
            for label, ver, c in bad:
                print(f"    {label} {ver}: {c:.6g}", file=sys.stderr)

    disp_idx = 1 if args.disp == "std" else 2  # index into (center,std,sem,n)

    # ---- Markdown ------------------------------------------------------------
    out_path = args.out or os.path.join(args.root, "summary_table.md")
    lines = []
    lines.append("# H1 — attestation A/B comparison across configs\n")
    lines.append(
        f"- **A** = V1 (attestation OFF), **B** = V2 (attestation ON).\n"
        f"- Cell = `{args.stat} ± {args.disp}` of the network-level series over time, "
        f"pooled across iterations (`—` = no data / metric absent).\n"
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
            header.append(f"{col['key']}{u} (A / B)")
        else:
            header.append(f"{col['key']}{u} · A")
            header.append(f"{col['key']}{u} · B")
    lines.append("\n## Full table\n")
    if args.layout == "combined":
        lines.append("Each cell is `A / B` (V1 / V2).\n")
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

    # ---- tidy CSV (optional) -------------------------------------------------
    if args.csv:
        import csv

        with open(args.csv, "w", newline="") as f:
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
        print(f"wrote {args.csv}")


if __name__ == "__main__":
    main()
