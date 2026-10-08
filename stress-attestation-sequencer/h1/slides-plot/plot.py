#!/usr/bin/env python3
"""plot.py — the H1 figures cut down for a 5 minute talk.

Draws three slide figures (16:9) into this folder from the summary tables
make_table.py writes (results/summary_table_*.csv), so they redraw whenever
those tables change:

  1-attestation-latency.png  what attestation adds per transaction on the
                             fullnode path of the 4-validator network: the
                             dry-run against the real execution and the full
                             attestation time, and settlement finality with
                             attestation off and on.
  2-cpu.png                  busiest-validator CPU with attestation on divided
                             by off, per client path, on 4 and 24 validators.
  3-moveauth.png             the Move authenticator campaign: post-consensus
                             validation time and throughput with attestation
                             off and on.

Every figure uses the 1,000 tx/s configurations. Data loading and the A/B
colours come from ../summary_plot.py. Run with the H1 venv:
  ../.venv/bin/python plot.py
"""

import importlib.util
import os

import matplotlib

matplotlib.use("Agg")
import matplotlib.pyplot as plt  # noqa: E402

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

plt.rcParams.update(
    {
        "font.size": 14,
        "axes.titlesize": 15,
        "axes.labelsize": 14,
        "xtick.labelsize": 12,
        "ytick.labelsize": 12,
        "legend.fontsize": 12,
        "axes.spines.top": False,
        "axes.spines.right": False,
    }
)

SLIDE = (13.33, 7.5)  # inches, 16:9
DPI = 150
INK2 = "#52514e"
SLOW_SIZES = (0, 50, 100, 200, 500)
AUTH_CYCLES = (1, 5, 10, 20, 50)
QPS = 1000


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
    return f"{cu / 1e6:.2g}M CUs" if cu >= 1e6 else f"{cu / 1e3:.3g}K CUs"


def num(v):
    """1307.2 -> 1,307; 94.85 -> 94.9; 4.8 -> 4.80."""
    if v >= 100:
        return f"{v:,.0f}"
    return f"{v:.1f}" if v >= 10 else f"{v:.2f}"


def grouped_bars(ax, groups, series, log=False):
    """One group of bars per x label; `series` is [(label, colour, values)]."""
    width = 0.8 / len(series)
    for k, (label, color, vals) in enumerate(series):
        xs = [i + (k - (len(series) - 1) / 2) * width for i in range(len(groups))]
        ax.bar(
            [x for x, v in zip(xs, vals) if v is not None],
            [v for v in vals if v is not None],
            width=width * 0.95,
            color=color,
            label=label,
        )
        for x, v in zip(xs, vals):
            if v is not None:
                ax.text(
                    x,
                    v * (1.12 if log else 1.0) + (0 if log else 12),
                    num(v),
                    ha="center",
                    va="bottom",
                    fontsize=9.5,
                    rotation=90 if len(series) > 2 else 0,
                )
    ax.set_xticks(range(len(groups)))
    ax.set_xticklabels(groups)
    if log:
        ax.set_yscale("log")
    ax.grid(True, axis="y", alpha=0.4)
    ax.set_axisbelow(True)


def save(fig, name):
    path = os.path.join(HERE, name)
    fig.savefig(path, dpi=DPI)
    plt.close(fig)
    print("wrote", path)


def fig_attestation_latency():
    csv_path = table("n4")
    configs = slow_configs("f1", 4)
    cus = values(csv_path, "CUs", configs, "V2")
    groups = [f"slow{s}\n{cu_label(cu)}" for s, cu in zip(SLOW_SIZES, cus)]
    fig, (ax1, ax2) = plt.subplots(
        1, 2, figsize=SLIDE, gridspec_kw={"width_ratios": [3, 2]}
    )
    grouped_bars(
        ax1,
        groups,
        [
            (
                "dry-run (attestation)",
                "#f2a19a",
                ms_values(csv_path, "attest. exec p95 (s)", configs, "V2"),
            ),
            (
                "real execution",
                INK2,
                ms_values(csv_path, "exec. lat. p95 (s)", configs, "V2"),
            ),
            (
                "full attestation: wait + dry-run + resume",
                h1.V2_COLOR,
                ms_values(csv_path, "attest. full p95 (s)", configs, "V2"),
            ),
        ],
        log=True,
    )
    ax1.set_ylabel("p95 per transaction (ms)")
    ax1.set_title("Attestation on: time per transaction", loc="left")
    ax1.legend(frameon=False, loc="upper left")
    ax1.set_ylim(0.5, 2e4)
    grouped_bars(
        ax2,
        [f"slow{s}" for s in SLOW_SIZES],
        [
            (
                "attestation off",
                h1.V1_COLOR,
                ms_values(csv_path, "final. lat. p50 (s)", configs, "V1"),
            ),
            (
                "attestation on",
                h1.V2_COLOR,
                ms_values(csv_path, "final. lat. p50 (s)", configs, "V2"),
            ),
        ],
        log=True,
    )
    ax2.set_ylabel("settlement finality p50 (ms)")
    ax2.set_title("What the client sees", loc="left")
    ax2.legend(frameon=False, loc="upper left")
    ax2.set_ylim(100, 6e4)
    fig.suptitle(
        "Attestation adds one dry-run; under heavy compute the wait around it grows",
        fontsize=18,
        fontweight="bold",
    )
    fig.text(
        0.5,
        0.905,
        "4 validators, fullnode path, 1,000 tx/s. Light load: about 1 ms more and"
        " the same finality. Heavy compute: seconds of waiting, about 2× finality.",
        ha="center",
        fontsize=12,
        color=INK2,
    )
    fig.tight_layout(rect=(0, 0, 1, 0.9))
    save(fig, "1-attestation-latency.png")


def fig_cpu():
    fig, axes = plt.subplots(1, 2, figsize=SLIDE, sharey=True)
    paths = (
        ("f1", "fullnode path", "#2a78d6", "o"),
        ("v1", "pinned to one validator", "#eb6834", "^"),
        ("vN", "direct to all validators", "#1f8a70", "s"),
    )
    for ax, n in zip(axes, (4, 24)):
        csv_path = table(f"n{n}")
        lines = []
        for path, name, color, marker in paths:
            p = f"v{n}" if path == "vN" else path
            configs = slow_configs(p, n)
            a = values(csv_path, "node CPU", configs, "V1")
            b = values(csv_path, "node CPU", configs, "V2")
            ratio = [y / x if (x and y) else None for x, y in zip(a, b)]
            ax.plot(
                range(len(SLOW_SIZES)),
                ratio,
                marker=marker,
                color=color,
                lw=2.5,
                ms=9,
                label=f"{name} ({p})",
            )
            lines.append((color, ratio))
        # Value labels only where no other line is within 0.07, so labels never
        # sit on top of each other where the lines meet.
        for color, ratio in lines:
            for i, r in enumerate(ratio):
                near = [o[i] for c, o in lines if c != color and o[i] is not None]
                if r is not None and all(abs(r - o) >= 0.07 for o in near):
                    ax.annotate(
                        f"{r:.2f}",
                        (i, r),
                        xytext=(0, 8),
                        textcoords="offset points",
                        ha="center",
                        fontsize=10,
                        color=color,
                    )
        ax.axhline(1, color=INK2, lw=1.2, ls=":")
        ax.set_xticks(range(len(SLOW_SIZES)))
        ax.set_xticklabels([f"slow{s}" for s in SLOW_SIZES])
        ax.set_title(f"{n} validators", loc="left")
        ax.grid(True, axis="y", alpha=0.4)
        ax.legend(frameon=False, loc="upper left")
    axes[0].set_ylabel("busiest validator's CPU, attestation on / off")
    axes[0].set_ylim(0.8, 2.4)
    fig.suptitle(
        "The CPU cost follows each validator's share of the attestation stream",
        fontsize=18,
        fontweight="bold",
    )
    fig.text(
        0.5,
        0.905,
        "1 = no extra CPU. Spread over 24 validators the cost almost disappears;"
        " a validator that attests everything pays up to 2×.",
        ha="center",
        fontsize=12,
        color=INK2,
    )
    fig.tight_layout(rect=(0, 0, 1, 0.9))
    save(fig, "2-cpu.png")


def fig_moveauth():
    csv_path = table("auth_n4")
    configs = [f"auth{c}-owned-f1-qps{QPS}-n4" for c in AUTH_CYCLES]
    groups = [str(c) for c in AUTH_CYCLES]
    fig, (ax1, ax2) = plt.subplots(1, 2, figsize=SLIDE)
    grouped_bars(
        ax1,
        groups,
        [
            (
                "attestation off",
                h1.V1_COLOR,
                ms_values(csv_path, "pc valid. lat. p95 (s)", configs, "V1"),
            ),
            (
                "attestation on",
                h1.V2_COLOR,
                ms_values(csv_path, "pc valid. lat. p95 (s)", configs, "V2"),
            ),
        ],
        log=True,
    )
    ax1.set_ylabel("post-consensus validation p95 (ms)")
    ax1.set_xlabel("signature verifications per transaction")
    ax1.set_title("Validation after consensus", loc="left")
    ax1.legend(frameon=False, loc="upper left")
    ax1.set_ylim(1, 2e3)
    grouped_bars(
        ax2,
        groups,
        [
            ("attestation off", h1.V1_COLOR, values(csv_path, "TPS", configs, "V1")),
            ("attestation on", h1.V2_COLOR, values(csv_path, "TPS", configs, "V2")),
        ],
    )
    ax2.axhline(QPS, color=INK2, lw=1.2, ls=":")
    ax2.text(
        len(groups) - 0.5,
        QPS + 15,
        "requested rate",
        ha="right",
        va="bottom",
        fontsize=11,
        color=INK2,
    )
    ax2.set_ylabel("finalized tx/s")
    ax2.set_xlabel("signature verifications per transaction")
    ax2.set_title("Throughput", loc="left")
    ax2.legend(frameon=False, loc="upper center", ncol=2)
    ax2.set_ylim(0, 1300)
    fig.suptitle(
        "When the checked work is outside execution, attestation pays for itself",
        fontsize=18,
        fontweight="bold",
    )
    fig.text(
        0.5,
        0.905,
        "Move authenticators, 4 validators, fullnode path, 1,000 tx/s. Off: every"
        " validator verifies after consensus. On: one validator verifies before it.",
        ha="center",
        fontsize=12,
        color=INK2,
    )
    fig.tight_layout(rect=(0, 0, 1, 0.9))
    save(fig, "3-moveauth.png")


if __name__ == "__main__":
    fig_attestation_latency()
    fig_cpu()
    fig_moveauth()
