#!/usr/bin/env python3

# Copyright (c) 2026 IOTA Stiftung
# SPDX-License-Identifier: Apache-2.0

"""Deterministic role-based latency model for private-network experiments.

Validators are assigned one of four roles, repeating every ten validators:

- ``hub`` (validator 1): ordinary band member with slightly fast inbound
  spokes (47-49 ms), so its quorum occasionally completes just under the
  50 ms minimum block delay (small ``MinBlockDelayTimeout`` share).
- ``band`` (validators 2-8): a narrow asymmetric 48-54 ms mesh. Quorums
  complete via direct full blocks (``AddBlock`` dominates globally).
- ``relay follower`` (validator 9): one fast 22 ms spoke from the hub plus
  slow 88-96 ms direct edges. Every hub block arrives one round ahead of the
  direct mesh and completes the round via its embedded headers, so this
  validator proposes almost exclusively on ``AddBlockHeader`` while staying
  at the global pace.
- ``heavy tail`` (validator 10): large fluctuating direct inbound
  (350-375 ms +/- 50 ms) with a single decent route from the hub (60 ms)
  whose delivery is bursty (netem ``slot 98-142 ms``, i.e. effective
  per-packet latency swinging 60-200 ms). Bursts carry ~2 rounds each; the
  50 ms min-block-delay deferral converts part of them into round skips,
  which pushes this validator ~1.3 blk/s below the pace (the required
  block-rate spread) and keeps a visible ``AddBlockHeader`` +
  ``MinBlockDelayTimeout`` mix. Outbound stays moderate (70-95 ms) so its
  stale leader blocks never stall the healthy quorum.

The expected pre-upgrade signature for 10 validators (testnet image,
measured in epoch 0 over 120 s) is a 16.5-19.5 blk/s band with >= 1 blk/s
spread and block-creation reasons ordered AddBlock >> AddBlockHeader >
MinBlockDelayTimeout.

The model is fully deterministic; the seed argument is accepted for CLI
compatibility but does not affect latency generation. Larger validator sets
repeat the ten roles per decade; the n=10 matrix is the validated
configuration.
"""

from __future__ import annotations

from dataclasses import dataclass
from pathlib import Path


ROLE_NAMES: tuple[str, ...] = (
    "hub",
    "band-b",
    "band-c",
    "band-d",
    "band-e",
    "band-f",
    "band-g",
    "band-h",
    "relay-follower",
    "heavy-tail",
)

HUB_ROLE = 0
FOLLOWER_ROLE = 8
HEAVY_ROLE = 9


@dataclass
class LatencyConfig:
    """Knobs for the role-based latency model."""

    num_validators: int
    # Accepted for compatibility with scripts that also seed disruptions.
    seed: int = 42

    # Ordinary mesh: delay = band_lo_ms + (3*src + 5*dst) % 7.
    band_lo_ms: int = 48
    # Inbound spokes to the hub: fast_in_ms + src % 3.
    fast_in_ms: int = 47
    # Relay follower: hub spoke and slow direct mesh.
    follower_relay_ms: int = 22
    follower_in_ms: int = 88
    follower_out_ms: int = 58
    follower_jitter_ms: int = 8
    # Heavy tail: bursty hub spoke, deep fluctuating directs, moderate out.
    heavy_relay_ms: int = 60
    heavy_relay_slot_min_ms: int = 98
    heavy_relay_slot_max_ms: int = 142
    heavy_in_ms: int = 350
    heavy_in_jitter_ms: int = 50
    heavy_in_corr_pct: float = 70.0
    heavy_out_ms: int = 70
    heavy_out_jitter_ms: int = 25
    jitter_correlation_pct: float = 30.0

    def __post_init__(self) -> None:
        if self.num_validators < 2:
            raise ValueError("num_validators must be >= 2")
        if min(
            self.band_lo_ms,
            self.fast_in_ms,
            self.follower_relay_ms,
            self.follower_in_ms,
            self.follower_out_ms,
            self.heavy_relay_ms,
            self.heavy_in_ms,
            self.heavy_out_ms,
        ) <= 0:
            raise ValueError("delays must be > 0")
        if not (
            0 < self.heavy_relay_slot_min_ms <= self.heavy_relay_slot_max_ms
        ):
            raise ValueError("require 0 < slot_min <= slot_max")


@dataclass
class LatencyMatrix:
    """Output of `generate`.

    Validator indices in the maps are 1-based to match container names.
    """

    cfg: LatencyConfig
    role_of: dict[int, str]
    rtt_ms: dict[tuple[int, int], int]
    jitter_ms: dict[tuple[int, int], int]
    correlation_pct: dict[tuple[int, int], float]
    loss_pct: dict[tuple[int, int], float]
    slot_min_ms: dict[tuple[int, int], int]
    slot_max_ms: dict[tuple[int, int], int]


def _role(validator: int) -> int:
    return (validator - 1) % len(ROLE_NAMES)


def _hub_of(validator: int) -> int:
    """Hub validator index of `validator`'s decade."""
    return validator - _role(validator)


# Edge tuple: (delay_ms, jitter_ms, corr_pct, loss_pct, slot_min, slot_max)
def _edge(cfg: LatencyConfig, i: int, j: int) -> tuple[int, int, float, float, int, int]:
    corr = cfg.jitter_correlation_pct
    # heavy tail inbound: bursty hub spoke, deep fluctuating directs
    if _role(j) == HEAVY_ROLE:
        if i == _hub_of(j):
            return (
                cfg.heavy_relay_ms,
                3,
                0.0,
                0.0,
                cfg.heavy_relay_slot_min_ms,
                cfg.heavy_relay_slot_max_ms,
            )
        return (
            cfg.heavy_in_ms + (7 * i) % 26,
            cfg.heavy_in_jitter_ms,
            cfg.heavy_in_corr_pct,
            0.0,
            0,
            0,
        )
    # heavy tail outbound: moderate, never stalls healthy quorums
    if _role(i) == HEAVY_ROLE:
        return cfg.heavy_out_ms + (9 * j) % 26, cfg.heavy_out_jitter_ms, corr, 0.0, 0, 0
    # relay follower inbound: hub spoke wins every round
    if _role(j) == FOLLOWER_ROLE:
        if i == _hub_of(j):
            return cfg.follower_relay_ms, 2, corr, 0.0, 0, 0
        return cfg.follower_in_ms + (3 * i) % 9, cfg.follower_jitter_ms, corr, 0.0, 0, 0
    if _role(i) == FOLLOWER_ROLE:
        return cfg.follower_out_ms + (5 * j) % 9, cfg.follower_jitter_ms, corr, 0.0, 0, 0
    # fast inbound spokes to the hub
    if _role(j) == HUB_ROLE:
        return cfg.fast_in_ms + i % 3, 3, corr, 0.0, 0, 0
    # ordinary band mesh
    delay = cfg.band_lo_ms + (3 * i + 5 * j) % 7
    return delay, 3 + delay % 3, corr, 0.0, 0, 0


def generate(cfg: LatencyConfig) -> LatencyMatrix:
    """Expand the role table to the requested validator count."""
    role_of = {
        validator: ROLE_NAMES[_role(validator)]
        for validator in range(1, cfg.num_validators + 1)
    }
    rtt: dict[tuple[int, int], int] = {}
    jitter: dict[tuple[int, int], int] = {}
    correlation: dict[tuple[int, int], float] = {}
    loss: dict[tuple[int, int], float] = {}
    slot_min: dict[tuple[int, int], int] = {}
    slot_max: dict[tuple[int, int], int] = {}

    for src in range(1, cfg.num_validators + 1):
        for dst in range(1, cfg.num_validators + 1):
            if src == dst:
                continue
            edge = (src, dst)
            (
                rtt[edge],
                jitter[edge],
                correlation[edge],
                loss[edge],
                slot_min[edge],
                slot_max[edge],
            ) = _edge(cfg, src, dst)

    return LatencyMatrix(
        cfg=cfg,
        role_of=role_of,
        rtt_ms=rtt,
        jitter_ms=jitter,
        correlation_pct=correlation,
        loss_pct=loss,
        slot_min_ms=slot_min,
        slot_max_ms=slot_max,
    )


def write_tsv(matrix: LatencyMatrix, path: Path) -> None:
    """Write `src dst delay jitter loss corr slot_min slot_max` TSV rows.

    The slot columns are consumed by network-benchmark.sh as netem
    ``slot <min>ms <max>ms`` (bursty delivery); zeros mean no slot clause.
    """
    cfg = matrix.cfg
    lines = [
        f"# latency-matrix n={cfg.num_validators} model=role-based",
        "# seed is intentionally ignored by latency generation",
        f"# roles repeat every {len(ROLE_NAMES)} validators: "
        "hub / band x7 / relay-follower / heavy-tail",
        f"# heavy-tail relay slot {cfg.heavy_relay_slot_min_ms}-"
        f"{cfg.heavy_relay_slot_max_ms} ms",
        "# src\tdst\tdelay_ms\tjitter_ms\tloss_pct\tcorr_pct\tslot_min_ms\tslot_max_ms",
    ]
    for (src, dst), delay in sorted(matrix.rtt_ms.items()):
        lines.append(
            f"{src}\t{dst}\t{delay}\t"
            f"{matrix.jitter_ms[(src, dst)]}\t"
            f"{matrix.loss_pct[(src, dst)]:.2f}\t"
            f"{matrix.correlation_pct[(src, dst)]:.0f}\t"
            f"{matrix.slot_min_ms[(src, dst)]}\t"
            f"{matrix.slot_max_ms[(src, dst)]}"
        )
    path.write_text("\n".join(lines) + "\n")


def _percentile(values: list[float], q: float) -> float:
    if not values:
        return 0
    sorted_values = sorted(values)
    index = max(0, min(len(sorted_values) - 1, round(q * (len(sorted_values) - 1))))
    return sorted_values[index]


def summarize(matrix: LatencyMatrix) -> list[str]:
    """Return human-readable summary lines."""
    cfg = matrix.cfg
    n = cfg.num_validators
    all_rtt = list(matrix.rtt_ms.values())
    mean = sum(all_rtt) / len(all_rtt) if all_rtt else 0.0
    inbound_means = sorted(
        sum(matrix.rtt_ms[(src, dst)] for src in range(1, n + 1) if src != dst)
        / (n - 1)
        for dst in range(1, n + 1)
    )
    heavies = [v for v in range(1, n + 1) if _role(v) == HEAVY_ROLE]
    followers = [v for v in range(1, n + 1) if _role(v) == FOLLOWER_ROLE]

    return [
        f"  Validators        : {n}",
        "  Model             : role-based (hub / band / relay-follower / heavy-tail)",
        f"  Delay mean / p50 / p90 / max : "
        f"{mean:.1f} / {_percentile(all_rtt, 0.5)} / {_percentile(all_rtt, 0.9)} / "
        f"{max(all_rtt)} ms",
        f"  Per-validator inbound mean delay spread: "
        f"min {inbound_means[0]:.0f} / p50 {_percentile(inbound_means, 0.5):.0f} / "
        f"max {inbound_means[-1]:.0f} ms",
        f"  Relay followers   : {followers or '-'} (hub spoke "
        f"{cfg.follower_relay_ms} ms, directs {cfg.follower_in_ms}+ ms)",
        f"  Heavy tails       : {heavies or '-'} (directs {cfg.heavy_in_ms}+ ms "
        f"±{cfg.heavy_in_jitter_ms} ms, hub spoke {cfg.heavy_relay_ms} ms "
        f"slot {cfg.heavy_relay_slot_min_ms}-{cfg.heavy_relay_slot_max_ms} ms)",
    ]


if __name__ == "__main__":
    import argparse

    parser = argparse.ArgumentParser(description="Inspect the role-based latency model")
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
