#!/usr/bin/env python3

# Copyright (c) 2026 IOTA Stiftung
# SPDX-License-Identifier: Apache-2.0

"""Deterministic latency model for the migration test.

The matrix intentionally sits just above Starfish's 50 ms minimum block delay.
Each validator gets a small inbound delay offset, so every validator remains in
the same broad production band while still creating a visible fastest-to-slowest
spread. The model is deliberately simple: per-destination delay bias, tiny
per-edge noise, and bounded jitter.
"""

from __future__ import annotations

import random
from dataclasses import dataclass
from pathlib import Path


@dataclass
class LatencyConfig:
    """Knobs for a simple near-threshold latency model."""

    num_validators: int
    seed: int = 42

    # Kept for CLI compatibility. The active validator set is ranked directly
    # so every N in the migration-test range gets the full delay spread.
    default_matrix_validators: int = 20

    # The fastest validator receives enough prior-round blocks shortly after
    # the min delay; the slowest is only a few ms behind. This keeps the target
    # interval around 53-57 ms, i.e. about 17.5-19.3 blocks/s.
    base_delay_ms: int = 53
    validator_delay_spread_ms: int = 4
    edge_noise_ms: int = 1
    jitter_base_ms: int = 4
    jitter_spread_ms: int = 3
    jitter_correlation_pct: float = 30.0

    min_rtt_ms: int = 1

    def __post_init__(self) -> None:
        if self.num_validators < 2:
            raise ValueError("num_validators must be >= 2")
        if self.default_matrix_validators < 2:
            raise ValueError("default_matrix_validators must be >= 2")
        if self.base_delay_ms <= 0:
            raise ValueError("base_delay_ms must be > 0")
        if self.validator_delay_spread_ms < 0 or self.edge_noise_ms < 0:
            raise ValueError("delay spread and edge noise must be >= 0")
        if self.jitter_base_ms < 0 or self.jitter_spread_ms < 0:
            raise ValueError("jitter values must be >= 0")
        if not (0.0 <= self.jitter_correlation_pct <= 100.0):
            raise ValueError("jitter_correlation_pct must be in [0, 100]")


@dataclass
class LatencyMatrix:
    """Output of `generate`.

    Indices in the maps are 1-based to match container names
    (validator-1, ...).
    """

    cfg: LatencyConfig
    delay_bias_ms: dict[int, float]       # validator -> inbound delay offset
    rtt_ms: dict[tuple[int, int], int]    # (src, dst) -> ms (1-based, src != dst)
    jitter_ms: dict[tuple[int, int], int]
    correlation_pct: dict[tuple[int, int], float]
    loss_pct: dict[tuple[int, int], float]


_MASK64 = 0xFFFF_FFFF_FFFF_FFFF


def _rng_for(cfg: LatencyConfig, *indices: int, salt: int) -> random.Random:
    """Return a Random instance seeded by `(cfg.seed, indices..., salt)`.

    Different salts give independent streams; different `indices` tuples give
    different streams within a salt.
    """
    h = (cfg.seed & 0xFFFF_FFFF) * 1_000_003 + salt * 7_919
    for idx in indices:
        # Each index is mixed in with a 1-shift so swapping (i, j) → (j, i)
        # produces a different state (directed edges need to differ).
        h = ((h * 65_537) + (idx * 7_919) + 1) & _MASK64
    return random.Random(h)


def generate(cfg: LatencyConfig) -> LatencyMatrix:
    n = cfg.num_validators
    ranked = sorted(range(n), key=lambda k: _rng_for(cfg, k, salt=1).random())
    rank_of = {validator: rank for rank, validator in enumerate(ranked)}
    denom = max(1, n - 1)
    delay_bias = {
        validator + 1: cfg.validator_delay_spread_ms * rank_of[validator] / denom
        for validator in range(n)
    }

    rtt: dict[tuple[int, int], int] = {}
    jitter: dict[tuple[int, int], int] = {}
    correlation: dict[tuple[int, int], float] = {}
    loss: dict[tuple[int, int], float] = {}

    for i in range(n):
        for j in range(n):
            if i == j:
                continue
            edge_noise = (
                _rng_for(cfg, i, j, salt=2).randint(
                    -cfg.edge_noise_ms, cfg.edge_noise_ms
                )
            )
            dst_bias = delay_bias[j + 1]
            value = cfg.base_delay_ms + dst_bias + edge_noise

            rtt_val = max(cfg.min_rtt_ms, int(round(value)))
            rtt[(i + 1, j + 1)] = rtt_val

            rank_frac = rank_of[j] / denom
            jitter[(i + 1, j + 1)] = int(
                round(cfg.jitter_base_ms + cfg.jitter_spread_ms * rank_frac)
            )
            correlation[(i + 1, j + 1)] = cfg.jitter_correlation_pct
            loss[(i + 1, j + 1)] = 0.0

    return LatencyMatrix(
        cfg=cfg,
        delay_bias_ms=delay_bias,
        rtt_ms=rtt,
        jitter_ms=jitter,
        correlation_pct=correlation,
        loss_pct=loss,
    )


def write_tsv(matrix: LatencyMatrix, path: Path) -> None:
    """Write `src \\t dst \\t rtt_ms \\t jitter_ms \\t loss_pct \\t corr_pct`
    rows (1-based indices). The bash reader treats the 5th and 6th columns as
    optional so older TSVs without `loss_pct` or `corr_pct` still load
    (missing values fall back to 0).

    Header comment lines (starting with `#`) are skipped by the bash reader.
    """
    cfg = matrix.cfg
    bias_lines = [
        f"# validator-{v} inbound_bias_ms={bias:.1f}"
        for v, bias in sorted(matrix.delay_bias_ms.items())
    ]
    lines = [
        f"# latency-matrix n={cfg.num_validators} seed={cfg.seed} "
        f"default_matrix_validators={cfg.default_matrix_validators} "
        f"base_delay_ms={cfg.base_delay_ms} "
        f"validator_delay_spread_ms={cfg.validator_delay_spread_ms} "
        f"edge_noise_ms={cfg.edge_noise_ms}",
        f"# jitter={cfg.jitter_base_ms}-{cfg.jitter_base_ms + cfg.jitter_spread_ms}ms "
        f"corr={cfg.jitter_correlation_pct:.0f}%",
        *bias_lines,
        "# src\tdst\trtt_ms\tjitter_ms\tloss_pct\tcorr_pct",
    ]
    for (src, dst), rtt in sorted(matrix.rtt_ms.items()):
        lines.append(
            f"{src}\t{dst}\t{rtt}\t"
            f"{matrix.jitter_ms[(src, dst)]}\t"
            f"{matrix.loss_pct[(src, dst)]:.2f}\t"
            f"{matrix.correlation_pct[(src, dst)]:.0f}"
        )
    path.write_text("\n".join(lines) + "\n")


def _percentile(values: list[float], q: float) -> float:
    if not values:
        return 0
    s = sorted(values)
    k = max(0, min(len(s) - 1, int(round(q * (len(s) - 1)))))
    return s[k]


def _triangle_violations(matrix: LatencyMatrix) -> tuple[int, int]:
    """Count ordered triples (i, j, k) with rtt[i,k] > rtt[i,j] + rtt[j,k]."""
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
    """Returns (mean abs diff, max abs diff) over unordered pairs in ms."""
    n = matrix.cfg.num_validators
    diffs: list[int] = []
    for i in range(1, n + 1):
        for j in range(i + 1, n + 1):
            diffs.append(abs(matrix.rtt_ms[(i, j)] - matrix.rtt_ms[(j, i)]))
    if not diffs:
        return 0.0, 0
    return sum(diffs) / len(diffs), max(diffs)


def summarize(matrix: LatencyMatrix) -> list[str]:
    """Human-readable summary lines (one per line, no trailing newline)."""
    cfg = matrix.cfg
    n = cfg.num_validators
    rtt = matrix.rtt_ms
    all_rtt = list(rtt.values())
    mean = sum(all_rtt) / len(all_rtt) if all_rtt else 0.0
    v_count, v_total = _triangle_violations(matrix)
    v_rate = (v_count / v_total) if v_total else 0.0
    asym_mean, asym_max = _asymmetry(matrix)

    # Per-validator inbound mean delay drives local block-production spread.
    in_means = [
        sum(rtt[(i, v)] for i in range(1, n + 1) if i != v) / (n - 1)
        for v in range(1, n + 1)
    ]
    in_means.sort()
    biases = sorted(matrix.delay_bias_ms.values())
    lines = [
        f"  Validators        : {n}",
        f"  RTT mean / p50 / p90 / p99 / max : "
        f"{mean:.1f} / {_percentile(all_rtt, 0.5)} / {_percentile(all_rtt, 0.9)} / "
        f"{_percentile(all_rtt, 0.99)} / {max(all_rtt)} ms",
        f"  Per-validator inbound mean delay spread: "
        f"min {in_means[0]:.0f} / p25 {_percentile(in_means, 0.25):.0f} / "
        f"p50 {_percentile(in_means, 0.5):.0f} / "
        f"p75 {_percentile(in_means, 0.75):.0f} / "
        f"max {in_means[-1]:.0f} ms",
        f"  Inbound bias spread: "
        f"min {biases[0]:.1f} / p50 {_percentile(biases, 0.5):.1f} / "
        f"max {biases[-1]:.1f} ms",
        f"  Jitter             : {cfg.jitter_base_ms}-"
        f"{cfg.jitter_base_ms + cfg.jitter_spread_ms} ms, "
        f"correlation {cfg.jitter_correlation_pct:.0f}%",
    ]

    lines.extend([
        f"  Asymmetry         : mean |A→B−B→A| = {asym_mean:.1f} ms, "
        f"max = {asym_max} ms",
        f"  Triangle violations: {v_count}/{v_total} ({100 * v_rate:.1f}%)",
    ])
    return lines


# Small self-check when run directly. Useful for tuning the defaults.
if __name__ == "__main__":
    import argparse

    parser = argparse.ArgumentParser(description="Inspect the latency model output")
    parser.add_argument("-n", "--num-validators", type=int, default=30)
    parser.add_argument("-s", "--seed", type=int, default=42)
    parser.add_argument(
        "--default-matrix-validators",
        type=int,
        default=20,
        help="kept for compatibility; the simple model ranks the active set",
    )
    parser.add_argument(
        "-o",
        "--output",
        type=Path,
        help="write the generated TSV matrix to this path",
    )
    args = parser.parse_args()

    cfg = LatencyConfig(
        num_validators=args.num_validators,
        seed=args.seed,
        default_matrix_validators=args.default_matrix_validators,
    )
    matrix = generate(cfg)
    if args.output is not None:
        write_tsv(matrix, args.output)
    for line in summarize(matrix):
        print(line)
