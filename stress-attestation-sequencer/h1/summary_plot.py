#!/usr/bin/env python3
"""summary_plot — grouped bar chart of ONE metric across all configs (A vs B).

Reads the tidy CSV from make_table.py (results/summary_table.csv) and draws, for a
single metric, one bar per (config, version): configs on the x-axis, metric value on
the y-axis, error bars = std (or sem).

Bars are grouped to mirror the experiment structure. For each (slow, qps) point there
are four bars — f/A f/B  v/A v/B — laid out as:
  - A (V1, attestation OFF) and B (V2, attestation ON) adjacent, two colors, touching;
  - a SMALL gap between the f and v configs that share the same (slow, qps);
  - a LARGER gap between different (slow, qps) groups.

Usage:
  .venv/bin/python summary_plot.py                 # TPS -> results/summary_plots/TPS.png
  .venv/bin/python summary_plot.py --metric "node CPU" --disp sem
"""

import argparse
import csv
import os
import re
import sys

import matplotlib

matplotlib.use("Agg")
import matplotlib.pyplot as plt  # noqa: E402
from matplotlib.lines import Line2D  # noqa: E402
from matplotlib.patches import Patch  # noqa: E402

# what the error bar represents, per --disp
DISP_LABEL = {
    "std": "error bar = ±1 std (pooled over time & iterations)",
    "sem": "error bar = ±1 SEM (across iterations)",
}

V1_COLOR = "#1f77b4"  # A — attestation OFF
V2_COLOR = "#d62728"  # B — attestation ON

# (version, color, legend label, title tag) in draw order.
VER_INFO = [
    ("V1", V1_COLOR, "A — V1 (attestation off)", "A (V1, attestation off)"),
    ("V2", V2_COLOR, "B — V2 (attestation on)", "B (V2, attestation on)"),
]

# layout, in bar-width units (bar width = 1.0)
W = 1.0
GAP_SMALL = 0.5  # between the f and v configs of one (slow, qps)
GAP_LARGE = 1.8  # between (slow, qps) groups


def parse_cfg(label):
    """slow{S}-owned-{f1|v1|v4}-qps{Q}[-n{N}] -> (n, slow, qps, path). The -n
    suffix is the network size; labels without it are the old 4-validator runs."""
    slow = int(re.search(r"slow(\d+)", label).group(1))
    qps = int(re.search(r"qps(\d+)", label).group(1))
    wl = re.search(r"owned-([a-z0-9]+)", label).group(1)
    m = re.search(r"-n(\d+)$", label)
    n = int(m.group(1)) if m else 4
    return n, slow, qps, wl


def load(csv_path, metric):
    """config -> {version: (center, std, sem)} for the requested metric; plus its unit."""
    rows, unit = {}, ""

    def f(x):
        try:
            return float(x)
        except (TypeError, ValueError):
            return float("nan")

    for r in csv.DictReader(open(csv_path)):
        if r["metric"] != metric:
            continue
        unit = r.get("unit", "") or ""
        rows.setdefault(r["config"], {})[r["version"]] = (
            f(r["center"]),
            f(r["std"]),
            f(r["sem"]),
        )
    return rows, unit


def all_configs(csv_path):
    """Every config in the CSV, in table order — so all subplots share one x-layout."""
    seen = set()
    for r in csv.DictReader(open(csv_path)):
        seen.add(r["config"])
    return sorted(seen, key=parse_cfg)


def select_versions(metric_data, configs, versions):
    """Figure-wide version slots (a subset of VER_INFO). "auto" keeps a version if
    ANY metric has data for it — computed once for the whole figure so every subplot
    reserves the same bar slots and stays aligned on the shared x-axis (a subplot
    lacking a version just leaves that slot empty)."""

    def has_data(ver):
        return any(
            rows.get(c, {}).get(ver, (float("nan"),))[0]
            == rows.get(c, {}).get(ver, (float("nan"),))[0]
            for rows, _ in metric_data.values()
            for c in configs
        )

    if versions == "auto":
        return [vi for vi in VER_INFO if has_data(vi[0])]
    want = {"A": {"V1"}, "B": {"V2"}, "AB": {"V1", "V2"}}[versions]
    return [vi for vi in VER_INFO if vi[0] in want]


def draw_metric(
    ax,
    metric,
    rows,
    unit,
    configs,
    disp,
    logy,
    versions,
    show_xlabels,
    show_legend,
    title=None,
    ylabel=None,
    fs=None,
):
    # Font sizes scale with the figure's subplot count (see make_figure): a
    # taller figure is displayed smaller, so its text must be bigger to end up
    # the same apparent size.
    fs = fs or {"tick": 9, "title": 11, "legend": 9}
    """Draw one metric's grouped bars on `ax`, reserving one slot per `versions`
    entry (a missing value leaves an empty slot). `title`/`ylabel` override the
    defaults (version tag / metric name)."""
    disp_idx = {"std": 1, "sem": 2}.get(disp)  # index into (center, std, sem)
    nbars = len(versions)

    x = 0.0
    xticks, xlabels, centers, slow_separators = [], [], [], []
    # components constant across the whole figure (e.g. qps on a
    # single-rate slice, n within one campaign) carry no information —
    # drop them from the tick labels.
    show_qps = len({parse_cfg(c)[2] for c in configs}) > 1
    show_n = len({parse_cfg(c)[0] for c in configs}) > 1
    prev_group, prev_slow = None, None
    for cfg in configs:
        n, slow, qps, wl = parse_cfg(cfg)
        group = (n, slow, qps)
        if prev_group is not None:
            gap = GAP_SMALL if group == prev_group else GAP_LARGE
            if slow != prev_slow:
                slow_separators.append(x + gap / 2)  # middle of the between-slow gap
            x += gap
        prev_group, prev_slow = group, slow
        left = x
        for ver, color, _, _ in versions:
            center, std, sem = rows.get(cfg, {}).get(ver, (float("nan"),) * 3)
            if center == center:
                centers.append(center)
            yerr = None
            if disp_idx is not None:
                e = (std, sem)[disp_idx - 1]
                yerr = e if e == e else None  # drop NaN
            ax.bar(
                left + W / 2,
                center,
                width=W,
                color=color,
                yerr=yerr,
                capsize=2.5,
                ecolor="#333",
                edgecolor="white",
                linewidth=0.5,
            )
            left += W
        xticks.append(x + nbars * W / 2)  # center of the config's bars
        xlabels.append(
            f"s{slow}" + (f"·q{qps}" if show_qps else "") + f"·{wl}"
            + (f"·n{n}" if show_n else "")
        )
        x += nbars * W

    for sx in slow_separators:
        ax.axvline(sx, color="#888", linestyle="--", linewidth=0.7, alpha=0.7)
    ax.set_xticks(xticks)
    ax.set_xticklabels(
        xlabels if show_xlabels else [], rotation=45, ha="right", fontsize=fs["tick"]
    )
    # sharex=True auto-hides tick labels on non-bottom subplots; force them on so
    # every subplot carries its own x-labels.
    ax.tick_params(axis="x", labelbottom=show_xlabels)
    ax.tick_params(axis="y", labelsize=fs["tick"])
    ax.set_xlim(-GAP_LARGE, x + GAP_SMALL)
    if ylabel is None:
        ylabel = metric + (f" ({unit})" if unit and unit != "none" else "")
    ax.set_ylabel(ylabel, fontsize=fs["tick"] + 1)
    ax.set_title(
        title if title is not None else " vs ".join(vi[3] for vi in versions),
        fontsize=fs["title"],
    )
    ax.grid(True, axis="y", alpha=0.25)
    if logy:
        pos = [c for c in centers if c > 0]
        if pos:
            ax.set_yscale("log")
            ax.set_ylim(min(pos) / 3, max(pos) * 2)  # floor below the smallest bar
    if show_legend:
        handles = [Patch(facecolor=vi[1], label=vi[2]) for vi in versions]
        if disp in DISP_LABEL:
            handles.append(
                Line2D(
                    [0],
                    [0],
                    color="#333",
                    marker="_",
                    markersize=8,
                    linewidth=1.2,
                    label=DISP_LABEL[disp],
                )
            )
        ax.legend(handles=handles, fontsize=fs["legend"])


# subplot title / suptitle helpers for multi-metric figures
def pct_of(metric):
    m = re.search(r"p\d+", metric)
    return m.group() if m else None


def unit_of(metric):
    m = re.search(r"\(([^)]+)\)\s*$", metric)
    return m.group(1) if m else ""


def strip_unit(metric):
    """metric without its trailing (unit) — e.g. 'node memory RSS (bytes)' -> 'node memory RSS'."""
    return re.sub(r"\s*\([^)]*\)\s*$", "", metric).strip()


def base_of(metric):
    """metric minus its percentile token and trailing (unit), normalized — the
    shared family name for a suptitle (e.g. 'attest. lat. p99 (s)' -> 'attest. lat.')."""
    b = re.sub(r"\s*p\d+\s*", " ", metric)
    b = re.sub(r"\s*\([^)]*\)\s*$", "", b)
    return re.sub(r"\s+", " ", b).strip()


# Per-figure config filters keep the x-axis readable: the findings quote
# qps1000 almost everywhere, slow50 duplicates slow0's gas bucket, so the
# default view is qps1000 x {0,100,200,500} x all paths (12 groups instead of
# 45). Figures that need another slice override:
#   "slow" / "qps" / "paths" — allow-lists applied to parse_cfg's fields.
# The full 45-config grid stays in summary_table_n<N>.{md,csv}; this trims the
# FIGURES only.
DEF_SLOW = (0, 50, 100, 200, 500)
DEF_QPS = (1000,)


def fig_configs(configs, spec):
    """Filter the global config list down to one figure's slice."""
    slow = spec.get("slow", DEF_SLOW)
    qps = spec.get("qps", DEF_QPS)
    paths = spec.get("paths")
    out = []
    for c in configs:
        n, s, q, wl = parse_cfg(c)
        if s in slow and q in qps and (paths is None or wl in paths):
            out.append(c)
    return out


# Default figure set — `summary_plot.py` with no --metric renders all of these,
# covering every table metric. `file` is the output basename; `metrics` are stacked
# top→bottom (percentiles descending); `versions` overrides the auto A/B selection.
FIGURES = [
    {
        "file": "TPS",
        "title": "Throughput, attestation rate, and validation drops rate",
        "metrics": ["TPS", "attest. / sec", "valid. drop. / sec"],
        "subtitles": {
            "attest. / sec": "attestations / sec",
            "valid. drop. / sec": "post-consensus validation drops / sec",
        },
    },
    {
        "file": "attestation_latency_exec",
        "title": "Attestation: computation units and dry-run execution latency",
        "metrics": [
            "CUs",
            "attest. exec p50 (s)",
            "attest. exec p95 (s)",
            "exec. lat. p95 (s)",
        ],
        "subtitles": {
            "CUs": "computation units",
            "attest. exec p95 (s)": "attestation dry-run execution p95",
            "attest. exec p50 (s)": "attestation dry-run execution p50",
            "exec. lat. p95 (s)": "execution p95",
        },
    },
    {
        "file": "attestation_latency_full",
        "title": "Full attestation latency (spawn_blocking wait + exec + resume)",
        "metrics": [
            "attest. full p99 (s)",
            "attest. full p95 (s)",
            "attest. full p50 (s)",
        ],
    },
    {
        "file": "attestation_latency_wait",
        "title": "Attestation pool wait (spawn_blocking queue)",
        "metrics": [
            "attest. wait p99 (s)",
            "attest. wait p95 (s)",
            "attest. wait p50 (s)",
        ],
    },
    {
        "file": "attestation_latency_resume",
        "title": "Attestation async resume (runtime reschedule)",
        "metrics": [
            "attest. resume p99 (s)",
            "attest. resume p95 (s)",
            "attest. resume p50 (s)",
        ],
    },
    {
        "file": "receipt_to_exec_latency",
        "title": "Receipt → execution latency",
        "metrics": [
            "rec. → exec. p99 (s)",
            "rec. → exec. p95 (s)",
            "rec. → exec. p50 (s)",
        ],
    },
    {
        "file": "post_consensus_validation_latency",
        "title": "Post-consensus validation latency",
        "metrics": ["pc valid. lat. p95 (s)", "pc valid. lat. p50 (s)"],
    },
    {
        "file": "checkpoint_creation_latency",
        "title": "Checkpoint creation lag (consensus commit → checkpoint built)",
        "metrics": [
            "ckpt lag p99 (s)",
            "ckpt lag p95 (s)",
            "ckpt lag p50 (s)",
        ],
    },
    {
        "file": "load_shedding_post_consensus",
        "slow": (200, 500),
        "title": "Post-consensus load shedding",
        "metrics": [
            "post-cons shed drops / sec",
            "shed % quorum",
            "shed % local",
        ],
        "subtitles": {
            "post-cons shed drops / sec": "user txns dropped after consensus / sec",
            "shed % quorum": "enforced quorum (2f+1) shed %",
            "shed % local": "locally computed shed % (broadcast)",
        },
    },
    {
        "file": "settlement_finality_latency",
        "paths": ("f1",),
        "title": "Settlement finality latency (client-side)",
        "metrics": [
            "final. lat. p99 (s)",
            "final. lat. p95 (s)",
            "final. lat. p50 (s)",
        ],
    },
    {
        "file": "submit_latency",
        "paths": ("f1",),
        "title": "Submit-transaction latency (client-side)",
        "metrics": ["submit lat. p95 (s)", "submit lat. p50 (s)"],
    },
    {
        "file": "queues",
        "title": "Execution queues and backpressure",
        "metrics": [
            "exec. dispatch queue",
            "pending txs",
            "exec. queue. delay p95 (s)",
        ],
        "subtitles": {
            "exec. dispatch queue": "execution dispatch queue",
            "pending txs": "pending transactions",
            "exec. queue. delay p95 (s)": "execution queue delay p95",
        },
    },
    {
        "file": "resources",
        "title": "CPU and memory (host and per-validator)",
        "metrics": ["host CPU", "node CPU", "node memory RSS (bytes)"],
    },
]


def make_figure(
    csv_path,
    configs,
    metrics,
    out,
    versions_mode,
    disp,
    logy,
    title=None,
    subtitles=None,
):
    """Render one figure (one metric = single axes; several = stacked shared-x
    subplots) to `out`. Returns the output path, or None if no data. `title` is a
    descriptive figure title; the A-vs-B version tag is appended to it. `subtitles`
    optionally overrides individual subplot titles (metric name -> label)."""
    n = len(metrics)
    metric_data = {m: load(csv_path, m) for m in metrics}  # m -> (rows, unit)

    # Figure-wide version slots (shared across subplots -> consistent x-layout).
    versions = select_versions(metric_data, configs, versions_mode)
    if not versions:
        print(f"skip {out}: no data for {metrics}", file=sys.stderr)
        return None

    # Version tag (A vs B) and the composed figure title.
    tag = " vs ".join(vi[3] for vi in versions)
    composed = f"{title}:\n{tag}" if title else tag

    # A homogeneous percentile family (e.g. attest. lat. p99/p95/p50) gets percentile
    # subplot titles + a shared suptitle; a heterogeneous set (e.g. host/node CPU, mem)
    # keeps each metric's name as its subplot title.
    bases = {base_of(m) for m in metrics}
    homogeneous = n > 1 and len(bases) == 1 and all(pct_of(m) for m in metrics)

    # font sizes grow with subplot count so every figure reads the same when
    # scaled to a common display size (a 4-subplot figure is ~2x taller than a
    # 2-subplot one and shrinks that much more).
    tick = round(3 + 2.5 * n)
    fs = {"tick": tick, "title": tick + 2, "legend": tick}
    fig, axes = plt.subplots(
        n, 1, sharex=True, figsize=(11.4, 3.4 * n + 1.5)
    )
    axes = [axes] if n == 1 else list(axes)
    for i, (ax, metric) in enumerate(zip(axes, metrics)):
        if n == 1:
            # single metric: the composed figure title goes on the one axes.
            subplot_title, ylabel = composed, None
        elif homogeneous:
            subplot_title, ylabel = pct_of(metric), (unit_of(metric) or None)
        else:
            # heterogeneous: metric name (minus unit) as title, unit on the y-axis
            subplot_title, ylabel = strip_unit(metric), unit_of(metric)
        if n > 1 and subtitles and metric in subtitles:
            subplot_title = subtitles[metric]  # per-figure subplot-title override
        rows, unit = metric_data[metric]
        draw_metric(
            ax,
            metric,
            rows,
            unit,
            configs,
            disp,
            logy,
            versions,
            show_xlabels=True,  # x-labels on every subplot, not just the bottom
            show_legend=(i == 0),  # only the top subplot
            title=subplot_title,
            ylabel=ylabel,
            fs=fs,
        )

    if n > 1:
        if title:
            sup = composed
        else:
            sup = f"{bases.pop()}:\n{tag}" if homogeneous else tag
        fig.suptitle(sup, fontsize=fs["title"] + 3)
        # h_pad opens vertical space between stacked subplots so a subplot's title
        # doesn't read as the x-label of the plot above it. The top margin holds
        # the two-line suptitle: ~2.6 line-heights converted to figure fraction.
        sup_headroom = 2.6 * (fs["title"] + 3) / 72 / (3.4 * n + 1.5)
        fig.tight_layout(rect=(0, 0, 1, 1 - sup_headroom), h_pad=3.0)
    else:
        fig.tight_layout()

    os.makedirs(os.path.dirname(out), exist_ok=True)
    fig.savefig(out, dpi=120)
    plt.close(fig)
    print(f"wrote {out}  ({len(configs)} configs, {n} metric(s))")
    return out


def main():
    here = os.path.dirname(os.path.abspath(__file__))
    ap = argparse.ArgumentParser()
    ap.add_argument(
        "--net",
        type=int,
        default=4,
        help="which campaign to plot: picks results/summary_table_n<net>.csv and "
        "writes to results/summary_plots_n<net>/ unless --csv/--outdir override",
    )
    ap.add_argument("--csv", default=None, help="tidy CSV from make_table.py "
        "(default: results/summary_table_n<net>.csv)")
    ap.add_argument("--outdir", default=None, help="figure output directory "
        "(default: results/summary_plots_n<net>)")
    ap.add_argument(
        "--metric",
        nargs="+",
        default=None,
        help="one or more metric names, stacked as shared-x subplots (top to bottom). "
        "Omit to render the full default figure set (see FIGURES).",
    )
    ap.add_argument("--disp", choices=["std", "sem", "none"], default="std")
    ap.add_argument(
        "--logy",
        action=argparse.BooleanOptionalAction,
        default=True,
        help="log-scale y-axis (default on; pass --no-logy for linear)",
    )
    ap.add_argument(
        "--versions",
        choices=["auto", "A", "B", "AB"],
        default="auto",
        help="which of A (V1) / B (V2) to draw; auto skips a version with no data "
        "(e.g. CUs is V2-only)",
    )
    ap.add_argument(
        "--out",
        default=None,
        help="output png for a single custom --metric figure "
        "(default: results/summary_plots/<metric(s)>.png)",
    )
    ap.add_argument(
        "--title",
        default=None,
        help="figure title (suptitle / single-axes title); the A-vs-B tag is appended",
    )
    args = ap.parse_args()

    if args.csv is None:
        args.csv = os.path.join(here, "results", f"summary_table_n{args.net}.csv")
    configs = all_configs(args.csv)
    if not configs:
        sys.exit(f"no configs in {args.csv}")
    outdir = args.outdir or os.path.join(here, "results", f"summary_plots_n{args.net}")

    if args.metric:
        out = args.out or os.path.join(
            outdir,
            "__".join(re.sub(r"[^A-Za-z0-9]+", "_", m).strip("_") for m in args.metric)
            + ".png",
        )
        make_figure(
            args.csv,
            fig_configs(configs, {}),
            args.metric,
            out,
            args.versions,
            args.disp,
            args.logy,
            title=args.title,
        )
    else:
        # no --metric: render the whole default set, each on its config slice.
        for spec in FIGURES:
            make_figure(
                args.csv,
                fig_configs(configs, spec),
                spec["metrics"],
                os.path.join(outdir, spec["file"] + ".png"),
                spec.get("versions", "auto"),
                args.disp,
                args.logy,
                title=spec.get("title"),
                subtitles=spec.get("subtitles"),
            )


if __name__ == "__main__":
    main()
