#!/usr/bin/env python3

# Copyright (c) 2026 IOTA Stiftung
# SPDX-License-Identifier: Apache-2.0

"""Summarize latency-matrix TSVs produced by network-benchmark.sh.

The latency model itself lives in network-benchmark.sh (see its "Built-in
role-based latency model" section): roles repeat every ten validators as
hub / band x7 / relay-follower / heavy-tail, and the script can dump the
effective matrix with ``network-benchmark.sh -n N -g BOOL -D <path>``.

This module only parses such a TSV (also accepted by ``-L``) and renders the
human-readable summary used in benchmark and migration-test logs, so there is
a single source of truth for the matrix values.
"""

from __future__ import annotations

from dataclasses import dataclass
from pathlib import Path


ROLE_PERIOD = 10
FOLLOWER_ROLE = 8
HEAVY_ROLE = 9


@dataclass(frozen=True)
class Edge:
    delay_ms: int
    jitter_ms: int
    loss_pct: float
    corr_pct: float
    slot_min_ms: int
    slot_max_ms: int


def read_tsv(path: Path) -> dict[tuple[int, int], Edge]:
    """Parse `src dst delay jitter [loss corr slot_min slot_max]` rows.

    Comment lines starting with ``#`` and blank lines are skipped; the
    optional columns default to zero, mirroring the bash reader.
    """
    edges: dict[tuple[int, int], Edge] = {}
    for line in Path(path).read_text().splitlines():
        line = line.strip()
        if not line or line.startswith("#"):
            continue
        fields = line.split()
        if len(fields) < 4:
            raise ValueError(f"malformed latency-matrix row: {line!r}")
        src, dst = int(fields[0]), int(fields[1])
        edges[(src, dst)] = Edge(
            delay_ms=int(fields[2]),
            jitter_ms=int(fields[3]),
            loss_pct=float(fields[4]) if len(fields) > 4 else 0.0,
            corr_pct=float(fields[5]) if len(fields) > 5 else 0.0,
            slot_min_ms=int(fields[6]) if len(fields) > 6 else 0,
            slot_max_ms=int(fields[7]) if len(fields) > 7 else 0,
        )
    if not edges:
        raise ValueError(f"no edges found in latency matrix {path}")
    return edges


def _percentile(values: list[float], q: float) -> float:
    sorted_values = sorted(values)
    index = max(0, min(len(sorted_values) - 1, round(q * (len(sorted_values) - 1))))
    return sorted_values[index]


def _role(validator: int) -> int:
    return (validator - 1) % ROLE_PERIOD


def summarize(edges: dict[tuple[int, int], Edge]) -> list[str]:
    """Return human-readable summary lines for a parsed matrix."""
    n = max(max(src, dst) for src, dst in edges)
    delays = [edge.delay_ms for edge in edges.values()]
    mean = sum(delays) / len(delays)
    # Average over the edges actually present, so partial -L matrices
    # summarize instead of raising KeyError.
    inbound: dict[int, list[int]] = {}
    for (_, dst), edge in edges.items():
        inbound.setdefault(dst, []).append(edge.delay_ms)
    inbound_means = sorted(sum(values) / len(values) for values in inbound.values())
    slot_edges = [
        (src, dst, edge.slot_min_ms, edge.slot_max_ms)
        for (src, dst), edge in sorted(edges.items())
        if edge.slot_max_ms > 0
    ]
    lossy_edges = sum(1 for edge in edges.values() if edge.loss_pct > 0)
    followers = [v for v in range(1, n + 1) if _role(v) == FOLLOWER_ROLE]
    heavies = [v for v in range(1, n + 1) if _role(v) == HEAVY_ROLE]

    lines = [
        f"  Validators        : {n}",
        "  Model             : role-based (hub / band / relay-follower / heavy-tail)",
        f"  Delay mean / p50 / p90 / max : "
        f"{mean:.1f} / {_percentile(delays, 0.5):.0f} / {_percentile(delays, 0.9):.0f} / "
        f"{max(delays)} ms",
        f"  Per-validator inbound mean delay spread: "
        f"min {inbound_means[0]:.0f} / p50 {_percentile(inbound_means, 0.5):.0f} / "
        f"max {inbound_means[-1]:.0f} ms",
        f"  Relay followers   : {followers or '-'}",
        f"  Heavy tails       : {heavies or '-'}",
    ]
    if slot_edges:
        rendered = ", ".join(
            f"{src}->{dst} {smin}-{smax}ms" for src, dst, smin, smax in slot_edges
        )
        lines.append(f"  Slot-burst edges  : {rendered}")
    if lossy_edges:
        lines.append(f"  Lossy edges       : {lossy_edges}")
    return lines


if __name__ == "__main__":
    import argparse

    parser = argparse.ArgumentParser(
        description="Summarize a latency-matrix TSV dumped by network-benchmark.sh"
    )
    parser.add_argument("matrix", type=Path, help="path to the TSV file")
    args = parser.parse_args()

    for line in summarize(read_tsv(args.matrix)):
        print(line)
