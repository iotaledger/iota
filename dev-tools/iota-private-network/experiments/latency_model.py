#!/usr/bin/env python3

# Copyright (c) 2026 IOTA Stiftung
# SPDX-License-Identifier: Apache-2.0

"""Fixed non-metric latency model for private-network experiments.

The canonical model is an explicit 10-site directed matrix. One ordinary site
is a hub with fast asymmetric spokes, while the remaining ordinary links stay
in a narrow latency band. The tenth site is a single heavy-tail profile with
much larger inbound and outbound delays. For 10 validators this produces
107 / 720 (14.9%) ordered triangle-inequality violations.

Larger validator sets repeat the ten site profiles in order. This is simple,
deterministic, and preserves the original 10-validator matrix as the top-left
submatrix. The seed argument remains accepted for CLI compatibility but does
not affect latency generation.
"""

from __future__ import annotations

from dataclasses import dataclass
from pathlib import Path


SITE_NAMES: tuple[str, ...] = (
    "ordinary-hub",
    "ordinary-b",
    "ordinary-c",
    "ordinary-d",
    "ordinary-e",
    "ordinary-f",
    "ordinary-g",
    "ordinary-h",
    "ordinary-i",
    "heavy-tail",
)

# Directed per-edge netem delays in milliseconds. The values are intentionally
# asymmetric. Zeros on the diagonal are replaced with same_site_delay_ms when
# a larger validator set places multiple validators at the same site profile.
SITE_DELAY_MS: tuple[tuple[int, ...], ...] = (
    (0, 50, 51, 49, 50, 51, 49, 50, 51, 20),
    (2, 0, 53, 54, 55, 56, 57, 58, 54, 40),
    (3, 56, 0, 53, 54, 55, 56, 57, 58, 60),
    (4, 55, 56, 0, 53, 54, 55, 56, 57, 80),
    (1, 54, 55, 56, 0, 53, 54, 55, 56, 100),
    (2, 58, 54, 55, 56, 0, 53, 54, 55, 120),
    (3, 57, 58, 54, 55, 56, 0, 53, 54, 140),
    (4, 56, 57, 58, 54, 55, 56, 0, 53, 160),
    (1, 55, 56, 57, 58, 54, 55, 56, 0, 180),
    (20, 80, 140, 200, 260, 320, 380, 440, 500, 0),
)

HEAVY_TAIL_SITE = SITE_NAMES.index("heavy-tail")


@dataclass
class LatencyConfig:
    """Knobs for the fixed ten-site latency model."""

    num_validators: int
    # Accepted for compatibility with scripts that also seed disruptions.
    seed: int = 42
    same_site_delay_ms: int = 4
    heavy_tail_same_site_delay_ms: int = 250
    jitter_divisor: int = 16
    jitter_min_ms: int = 2
    jitter_max_ms: int = 8
    jitter_correlation_pct: float = 30.0
    heavy_tail_jitter_ms: int = 75
    heavy_tail_jitter_correlation_pct: float = 70.0

    def __post_init__(self) -> None:
        if self.num_validators < 2:
            raise ValueError("num_validators must be >= 2")
        if self.same_site_delay_ms <= 0:
            raise ValueError("same_site_delay_ms must be > 0")
        if self.heavy_tail_same_site_delay_ms <= 0:
            raise ValueError("heavy_tail_same_site_delay_ms must be > 0")
        if self.jitter_divisor <= 0:
            raise ValueError("jitter_divisor must be > 0")
        if not (0 <= self.jitter_min_ms <= self.jitter_max_ms):
            raise ValueError("require 0 <= jitter_min_ms <= jitter_max_ms")
        if not (0.0 <= self.jitter_correlation_pct <= 100.0):
            raise ValueError("jitter_correlation_pct must be in [0, 100]")
        if self.heavy_tail_jitter_ms < 0:
            raise ValueError("heavy_tail_jitter_ms must be >= 0")
        if not (0.0 <= self.heavy_tail_jitter_correlation_pct <= 100.0):
            raise ValueError("heavy_tail_jitter_correlation_pct must be in [0, 100]")


@dataclass
class LatencyMatrix:
    """Output of `generate`.

    Validator indices in the maps are 1-based to match container names.
    """

    cfg: LatencyConfig
    site_of: dict[int, str]
    rtt_ms: dict[tuple[int, int], int]
    jitter_ms: dict[tuple[int, int], int]
    correlation_pct: dict[tuple[int, int], float]
    loss_pct: dict[tuple[int, int], float]


def _site_index(validator: int) -> int:
    return (validator - 1) % len(SITE_NAMES)


def generate(cfg: LatencyConfig) -> LatencyMatrix:
    """Expand the fixed ten-site table to the requested validator count."""
    site_of = {
        validator: SITE_NAMES[_site_index(validator)]
        for validator in range(1, cfg.num_validators + 1)
    }
    rtt: dict[tuple[int, int], int] = {}
    jitter: dict[tuple[int, int], int] = {}
    correlation: dict[tuple[int, int], float] = {}
    loss: dict[tuple[int, int], float] = {}

    for src in range(1, cfg.num_validators + 1):
        for dst in range(1, cfg.num_validators + 1):
            if src == dst:
                continue
            src_site = _site_index(src)
            dst_site = _site_index(dst)
            delay = SITE_DELAY_MS[src_site][dst_site]
            if src_site == dst_site:
                delay = (
                    cfg.heavy_tail_same_site_delay_ms
                    if src_site == HEAVY_TAIL_SITE
                    else cfg.same_site_delay_ms
                )

            edge = (src, dst)
            rtt[edge] = delay
            if src_site == HEAVY_TAIL_SITE or dst_site == HEAVY_TAIL_SITE:
                jitter[edge] = cfg.heavy_tail_jitter_ms
                correlation[edge] = cfg.heavy_tail_jitter_correlation_pct
            else:
                jitter[edge] = min(
                    cfg.jitter_max_ms,
                    max(cfg.jitter_min_ms, round(delay / cfg.jitter_divisor)),
                )
                correlation[edge] = cfg.jitter_correlation_pct
            loss[edge] = 0.0

    return LatencyMatrix(
        cfg=cfg,
        site_of=site_of,
        rtt_ms=rtt,
        jitter_ms=jitter,
        correlation_pct=correlation,
        loss_pct=loss,
    )


def write_tsv(matrix: LatencyMatrix, path: Path) -> None:
    """Write `src dst delay_ms jitter_ms loss_pct corr_pct` TSV rows."""
    cfg = matrix.cfg
    lines = [
        f"# latency-matrix n={cfg.num_validators} model=fixed-ten-site-non-metric",
        "# seed is intentionally ignored by latency generation",
        f"# site profiles repeat every {len(SITE_NAMES)} validators",
        f"# same-site delay={cfg.same_site_delay_ms}ms",
        f"# ordinary jitter={cfg.jitter_min_ms}-{cfg.jitter_max_ms}ms "
        f"corr={cfg.jitter_correlation_pct:.0f}%",
        f"# heavy-tail jitter={cfg.heavy_tail_jitter_ms}ms "
        f"corr={cfg.heavy_tail_jitter_correlation_pct:.0f}%",
        "# src\tdst\tdelay_ms\tjitter_ms\tloss_pct\tcorr_pct",
    ]
    for (src, dst), delay in sorted(matrix.rtt_ms.items()):
        lines.append(
            f"{src}\t{dst}\t{delay}\t"
            f"{matrix.jitter_ms[(src, dst)]}\t"
            f"{matrix.loss_pct[(src, dst)]:.2f}\t"
            f"{matrix.correlation_pct[(src, dst)]:.0f}"
        )
    path.write_text("\n".join(lines) + "\n")


def _percentile(values: list[float], q: float) -> float:
    if not values:
        return 0
    sorted_values = sorted(values)
    index = max(0, min(len(sorted_values) - 1, round(q * (len(sorted_values) - 1))))
    return sorted_values[index]


def _triangle_violations(matrix: LatencyMatrix) -> tuple[int, int]:
    """Count ordered triples (i, j, k) with delay[i,k] > delay[i,j] + delay[j,k]."""
    n = matrix.cfg.num_validators
    rtt = matrix.rtt_ms
    total = 0
    violations = 0
    for i in range(1, n + 1):
        for j in range(1, n + 1):
            if j == i:
                continue
            for k in range(1, n + 1):
                if k == i or k == j:
                    continue
                total += 1
                if rtt[(i, k)] > rtt[(i, j)] + rtt[(j, k)]:
                    violations += 1
    return violations, total


def _asymmetry(matrix: LatencyMatrix) -> tuple[float, int]:
    """Return (mean absolute difference, max absolute difference) in ms."""
    n = matrix.cfg.num_validators
    differences = [
        abs(matrix.rtt_ms[(i, j)] - matrix.rtt_ms[(j, i)])
        for i in range(1, n + 1)
        for j in range(i + 1, n + 1)
    ]
    if not differences:
        return 0.0, 0
    return sum(differences) / len(differences), max(differences)


def summarize(matrix: LatencyMatrix) -> list[str]:
    """Return human-readable summary lines."""
    cfg = matrix.cfg
    n = cfg.num_validators
    rtt = matrix.rtt_ms
    all_rtt = list(rtt.values())
    ordinary_jitter = [
        jitter
        for (src, dst), jitter in matrix.jitter_ms.items()
        if _site_index(src) != HEAVY_TAIL_SITE
        and _site_index(dst) != HEAVY_TAIL_SITE
    ]
    mean = sum(all_rtt) / len(all_rtt) if all_rtt else 0.0
    violations, triples = _triangle_violations(matrix)
    violation_rate = violations / triples if triples else 0.0
    asymmetry_mean, asymmetry_max = _asymmetry(matrix)
    inbound_means = sorted(
        sum(rtt[(src, dst)] for src in range(1, n + 1) if src != dst) / (n - 1)
        for dst in range(1, n + 1)
    )

    return [
        f"  Validators        : {n}",
        f"  Model             : fixed {len(SITE_NAMES)}-site non-metric matrix",
        f"  Delay mean / p50 / p90 / p99 / max : "
        f"{mean:.1f} / {_percentile(all_rtt, 0.5)} / {_percentile(all_rtt, 0.9)} / "
        f"{_percentile(all_rtt, 0.99)} / {max(all_rtt)} ms",
        f"  Per-validator inbound mean delay spread: "
        f"min {inbound_means[0]:.0f} / p25 {_percentile(inbound_means, 0.25):.0f} / "
        f"p50 {_percentile(inbound_means, 0.5):.0f} / "
        f"p75 {_percentile(inbound_means, 0.75):.0f} / "
        f"max {inbound_means[-1]:.0f} ms",
        f"  Ordinary jitter   : {min(ordinary_jitter)}-{max(ordinary_jitter)} ms, "
        f"correlation {cfg.jitter_correlation_pct:.0f}%",
        f"  Heavy-tail jitter : {cfg.heavy_tail_jitter_ms} ms, "
        f"correlation {cfg.heavy_tail_jitter_correlation_pct:.0f}%",
        f"  Asymmetry         : mean |A-B - B-A| = {asymmetry_mean:.1f} ms, "
        f"max = {asymmetry_max} ms",
        f"  Triangle violations: {violations}/{triples} ({100 * violation_rate:.1f}%)",
    ]


if __name__ == "__main__":
    import argparse

    parser = argparse.ArgumentParser(description="Inspect the fixed latency model")
    parser.add_argument("-n", "--num-validators", type=int, default=10)
    parser.add_argument(
        "-s",
        "--seed",
        type=int,
        default=42,
        help="accepted for CLI compatibility; ignored by latency generation",
    )
    parser.add_argument(
        "-o",
        "--output",
        type=Path,
        help="write the generated TSV matrix to this path",
    )
    args = parser.parse_args()

    matrix = generate(LatencyConfig(num_validators=args.num_validators, seed=args.seed))
    if args.output is not None:
        write_tsv(matrix, args.output)
    for line in summarize(matrix):
        print(line)
