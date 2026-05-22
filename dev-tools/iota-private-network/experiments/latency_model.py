#!/usr/bin/env python3

# Copyright (c) 2026 IOTA Stiftung
# SPDX-License-Identifier: Apache-2.0

"""Deterministic, region-based latency model for the migration test.

Generates a directed N x N RTT matrix with realistic pathologies that the
flat 10x10 RTT table in network-benchmark.sh doesn't produce:

* Validators are assigned to ~5 geographic regions; intra-region RTT is small,
  inter-region RTT follows a fixed (AWS-ish, scaled-down) base table.
* A small set of "heavy-tail" validators carry a per-validator handicap that
  is added to every edge incident to them — models peers with bad uplinks or
  sub-optimal peering.
* Each *directed* edge gets independent log-normal perturbation. Drawing
  i->j and j->i separately produces both asymmetric paths and a non-trivial
  fraction of triangle-inequality violations.

The model is fully seeded. For small networks it draws a canonical
20-validator table first and emits the requested prefix, so a 10-validator run
uses the top-left submatrix of the 20-validator run for the same seed.
"""

from __future__ import annotations

import math
import random
from dataclasses import dataclass
from pathlib import Path


# Five regions, deliberately a small fixed set so the matrix exposes clear
# intra- vs inter-region structure even at small N.
REGIONS: tuple[str, ...] = ("na-east", "na-west", "eu", "ap-east", "ap-south")

# Symmetric base RTT (ms). Scaled down from real WAN numbers so the overall
# matrix p50 lands a touch under 50 ms after the per-validator uplink
# handicap is added on top.
#
#            na-east  na-west  eu   ap-east  ap-south
_BASE_RTT_MS: tuple[tuple[int, ...], ...] = (
    (      2,      18,  22,    42,      48),  # na-east
    (     18,       2,  42,    30,      38),  # na-west
    (     22,      42,   2,    52,      34),  # eu
    (     42,      30,  52,     2,      22),  # ap-east
    (     48,      38,  34,    22,       2),  # ap-south
)


@dataclass
class LatencyConfig:
    """Knobs for the latency model.

    *Good-to-good* edges follow the regional base table plus small log-normal
    noise — these dominate the matrix and keep the **median** RTT near 50 ms.

    *Edges incident to a heavy-tail validator* follow a much harsher model:
    each heavy-tail validator has a small fixed set of `heavy_tail_fast_peers`
    "fast" peers (default 4) — bidirectional edges to those peers are drawn
    uniformly on `[heavy_tail_fast_floor_ms, heavy_tail_fast_ceiling_ms]`
    (default 100–200 ms), modelling the few peers the bad node still reaches
    over a working path. Every *other* edge incident to a heavy-tail node is
    drawn log-uniformly on `[heavy_tail_floor_ms, heavy_tail_ceiling_ms]` —
    a heavy tail in the statistical sense, sitting mostly near the floor with
    a few values stretching to the ceiling.
    """

    num_validators: int
    seed: int = 42

    # The seeded default table is 20 validators. Smaller runs use its
    # top-left submatrix, so changing -n from 20 to 10 keeps validators 1..10
    # on exactly the same edges instead of drawing a separate small-network
    # topology. Larger runs extend the same deterministic model.
    default_matrix_validators: int = 20

    # Block-based heavy-tail selection. Each consecutive run of
    # `heavy_tail_block_size` validators contains exactly one heavy-tail,
    # at a position determined by a stable hash of `(seed, block_index)`.
    # That gives the user-cited "1 bad per 10" rate (N=10→1, N=20→2,
    # N=30→3, N=60→6) and — crucially — makes heavy-tail membership
    # *monotonic in N*: a validator's heavy-tail status is fixed once and
    # for all by `(seed, k)`, so adding more validators only adds heavies
    # in *their* new blocks and never reshuffles existing ones.
    heavy_tail_block_size: int = 10

    # Each heavy-tail validator gets exactly this many fast bidirectional
    # peers, deterministically chosen from the non-heavy validators. The
    # edges to/from those peers are drawn uniformly on
    # [heavy_tail_fast_floor_ms, heavy_tail_fast_ceiling_ms].
    heavy_tail_fast_peers: int = 4
    heavy_tail_fast_floor_ms: int = 100
    heavy_tail_fast_ceiling_ms: int = 200

    # Heavy-tail (slow) edges: log-uniform on [floor, ceiling] ms.
    heavy_tail_floor_ms: int = 150
    heavy_tail_ceiling_ms: int = 1500

    # Per-edge log-normal perturbation for non-heavy edges:
    # noise = (LogNormal(0, sigma) - exp(sigma^2 / 2)) * scale, i.e. the
    # mean of the multiplicative term is exactly subtracted so the noise
    # is zero-mean and the matrix p50 lands at the base-table median.
    noise_sigma: float = 0.75
    noise_scale_ms: float = 12.0

    # --- Per-validator outbound quality ---
    # Every validator k gets a stable quality `q_k ∈ [0, 1]` (lower = worse
    # uplink). Two effects apply to every edge *out* of k:
    #   - additive RTT handicap: `(1 - q_k)^uplink_handicap_skew * uplink_handicap_max_ms`
    #   - jitter base = rtt * (non_heavy_jitter_base_frac + (1 - q_k) *
    #     uplink_jitter_extra_frac)
    # The handicap is *skewed* so the median validator barely sees any
    # slowdown (only the bottom quartile carries meaningful handicap).
    # That preserves the per-validator throughput gradient (worst nodes
    # still see ~50 ms) without inflating the matrix median.
    uplink_handicap_max_ms: float = 50.0
    uplink_handicap_skew: float = 3.0
    non_heavy_jitter_base_frac: float = 0.05
    uplink_jitter_extra_frac: float = 0.10

    # Floor for jitter (ms) so very-low-RTT edges still have a couple ms
    # of variation. The proportional jitter computed above is taken as the
    # max(floor, value).
    jitter_max_ms: int = 3

    # Slow sub-edges of a heavy-tail validator get jitter = round(rtt * frac)
    # instead of the small `jitter_max_ms` uniform draw, so a 1000 ms edge
    # observes ~±200 ms one-way variation per packet. Set 0 to disable.
    heavy_tail_jitter_frac: float = 0.2

    # Default realistic latency should be latency/jitter-driven only. Packet
    # loss is deliberately disabled here because even sub-1% loss on high-RTT
    # TCP streams dominates the effect and pushes block production below the
    # intended ~19 blocks/s band. Set these non-zero for explicit loss tests.
    heavy_tail_loss_min_pct: float = 0.0
    heavy_tail_loss_max_pct: float = 0.0

    min_rtt_ms: int = 1

    def __post_init__(self) -> None:
        if self.num_validators < 2:
            raise ValueError("num_validators must be >= 2")
        if self.default_matrix_validators < 2:
            raise ValueError("default_matrix_validators must be >= 2")
        if self.heavy_tail_block_size < 2:
            raise ValueError("heavy_tail_block_size must be >= 2")
        if self.heavy_tail_fast_peers < 0:
            raise ValueError("heavy_tail_fast_peers must be >= 0")
        if self.uplink_handicap_max_ms < 0:
            raise ValueError("uplink_handicap_max_ms must be >= 0")
        if self.uplink_handicap_skew <= 0:
            raise ValueError("uplink_handicap_skew must be > 0")
        if self.non_heavy_jitter_base_frac < 0 \
                or self.uplink_jitter_extra_frac < 0:
            raise ValueError("jitter fractions must be >= 0")
        if self.heavy_tail_fast_floor_ms <= 0 \
                or self.heavy_tail_fast_ceiling_ms < self.heavy_tail_fast_floor_ms:
            raise ValueError(
                "require 0 < heavy_tail_fast_floor_ms <= heavy_tail_fast_ceiling_ms"
            )
        if self.heavy_tail_floor_ms <= 0 or self.heavy_tail_ceiling_ms <= self.heavy_tail_floor_ms:
            raise ValueError("require 0 < heavy_tail_floor_ms < heavy_tail_ceiling_ms")
        # heavy_tail_fast_peers vs. available-non-heavy is checked at
        # generate() time because the heavy-tail count depends on N and
        # block alignment, not on a stored config value.
        if not (0.0 <= self.heavy_tail_jitter_frac <= 1.0):
            raise ValueError("heavy_tail_jitter_frac must be in [0, 1]")
        if not (0.0 <= self.heavy_tail_loss_min_pct
                <= self.heavy_tail_loss_max_pct <= 100.0):
            raise ValueError(
                "require 0 <= heavy_tail_loss_min_pct "
                "<= heavy_tail_loss_max_pct <= 100"
            )


@dataclass
class LatencyMatrix:
    """Output of `generate`. Indices in rtt_ms / jitter_ms are 1-based to match
    container names (validator-1, ...).
    """

    cfg: LatencyConfig
    region_of: list[str]                  # length n, indexed 0..n-1
    heavy_tail: list[int]                 # 1-based validator indices, sorted
    # Per heavy-tail validator (1-based): the small set of "fast" peers it
    # still reaches at moderate RTT. Edges in this set bypass the slow
    # log-uniform draw and use the fast-band uniform draw.
    fast_peers: dict[int, list[int]]
    rtt_ms: dict[tuple[int, int], int]    # (src, dst) -> ms (1-based, src != dst)
    jitter_ms: dict[tuple[int, int], int]
    loss_pct: dict[tuple[int, int], float]  # 0.0 on non-heavy-tail edges


_MASK64 = 0xFFFF_FFFF_FFFF_FFFF


def _rng_for(cfg: LatencyConfig, *indices: int, salt: int) -> random.Random:
    """Return a Random instance seeded by `(cfg.seed, indices..., salt)`.

    Crucially the seed does **not** include `cfg.num_validators` and the
    indices fully determine the state — so a draw keyed by `(seed, k=3, salt=1)`
    is identical at N=10 and at N=30, and likewise a per-edge draw keyed by
    `(seed, i, j, salt=5)` is identical regardless of the surrounding loop
    bounds. That gives the matrix the "monotonic extension" property: adding
    validators only adds rows / columns; existing edges keep their values.

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
    matrix_n = max(n, cfg.default_matrix_validators)
    num_regions = len(REGIONS)

    # Per-validator region: derived purely from (seed, k). Validator-k is in
    # the same region at any N, so the base RTT between two specific
    # validators doesn't shift when the network grows.
    region_idx = [
        _rng_for(cfg, k, salt=1).randrange(num_regions)
        for k in range(matrix_n)
    ]
    region_of = [REGIONS[r] for r in region_idx[:n]]

    # Block-based heavy-tail selection. Each block of `block_size` validators
    # contains exactly one heavy-tail at a position determined by a stable
    # `(seed, block_index)` hash. Result: val-k's heavy-tail status depends
    # only on `(seed, k)`, never on N, so the matrix is monotone — adding
    # validators only adds heavies in their *own* (new) blocks; existing
    # validators keep their heavy-tail status and their edges keep their
    # draws.
    block_size = cfg.heavy_tail_block_size
    heavy_tail_set: set[int] = set()
    for block_idx in range((matrix_n + block_size - 1) // block_size):
        pos = _rng_for(cfg, block_idx, salt=2).randrange(block_size)
        k = block_idx * block_size + pos
        if k < matrix_n:
            heavy_tail_set.add(k)

    # For each heavy-tail validator, pick fast peers from validators in
    # *its own block* — that pool is fixed once the block is fully present,
    # so a heavy-tail v's fast-peer set is stable across N (as long as N is
    # at least past v's block end). Picking from inside the block also
    # gives the fast peers a "same rack / same exchange" flavor, which is
    # the realistic shape: a bad-uplink validator still reaches its
    # immediate neighbors over a working short-haul path.
    fast_peers: dict[int, set[int]] = {}
    for v in heavy_tail_set:
        block_idx = v // cfg.heavy_tail_block_size
        block_start = block_idx * cfg.heavy_tail_block_size
        block_end = min(block_start + cfg.heavy_tail_block_size, matrix_n)
        candidates = [
            p for p in range(block_start, block_end)
            if p != v and p not in heavy_tail_set
        ]
        candidates.sort(key=lambda p: _rng_for(cfg, v, p, salt=7).random())
        fast_peers[v] = set(
            candidates[: min(cfg.heavy_tail_fast_peers, len(candidates))]
        )
    # Per-validator outbound quality, stable across N.
    uplink_q = [
        _rng_for(cfg, k, salt=8).random() for k in range(matrix_n)
    ]

    def _is_fast_edge(a: int, b: int) -> bool:
        return (a in heavy_tail_set and b in fast_peers[a]) or (
            b in heavy_tail_set and a in fast_peers[b]
        )

    rtt: dict[tuple[int, int], int] = {}
    jitter: dict[tuple[int, int], int] = {}
    loss: dict[tuple[int, int], float] = {}

    # Precompute: log-uniform map exponent for heavy-tail slow draws, the
    # mean of LogNormal(0, sigma) so the non-heavy noise term is zero-mean,
    # and the fast-band span.
    log_ratio = math.log(cfg.heavy_tail_ceiling_ms / cfg.heavy_tail_floor_ms)
    noise_mean = math.exp(cfg.noise_sigma ** 2 / 2.0)
    loss_span = cfg.heavy_tail_loss_max_pct - cfg.heavy_tail_loss_min_pct
    fast_span = cfg.heavy_tail_fast_ceiling_ms - cfg.heavy_tail_fast_floor_ms

    for i in range(n):
        for j in range(n):
            if i == j:
                continue
            # Per-edge RNG state — each property uses its own salt to keep
            # the streams orthogonal. Because the seed depends only on
            # `(seed, i, j, salt)` and not on N or iteration order, this
            # edge's draws are identical at any N (cross-N stable matrix).
            u_ht = _rng_for(cfg, i, j, salt=5).random()
            ln_noise = _rng_for(cfg, i, j, salt=3).lognormvariate(
                0.0, cfg.noise_sigma
            )
            j_small = _rng_for(cfg, i, j, salt=4).randint(
                0, cfg.jitter_max_ms
            )
            u_loss = _rng_for(cfg, i, j, salt=6).random()

            base = _BASE_RTT_MS[region_idx[i]][region_idx[j]]
            ht_incident = (i in heavy_tail_set) or (j in heavy_tail_set)
            fast_edge = ht_incident and _is_fast_edge(i, j)
            slow_sub_edge = ht_incident and not fast_edge

            if fast_edge:
                # Heavy-tail's fast peer: uniform on [fast_floor, fast_ceiling].
                value = cfg.heavy_tail_fast_floor_ms + u_ht * fast_span
            elif slow_sub_edge:
                # Heavy-tail slow edge: log-uniform on [floor, ceiling].
                value = cfg.heavy_tail_floor_ms * math.exp(u_ht * log_ratio)
            else:
                # Healthy core: base regional RTT + zero-mean log-normal noise.
                value = base + (ln_noise - noise_mean) * cfg.noise_scale_ms

            # Sender's outbound uplink handicap: a slow-quality validator
            # adds latency to *every* edge it sends on. Applied universally
            # so heavy-tail and fast-peer edges also see the asymmetry,
            # which produces per-validator throughput dispersion in
            # consensus rather than every node looking identical.
            #
            # Skewed by `uplink_handicap_skew` so the *median* validator
            # barely sees a handicap (only the bottom quartile pays the
            # full cost). At skew=2, median handicap = max/4; at skew=3,
            # max/8 — knobs to trade off matrix p50 against per-validator
            # spread.
            handicap_i = (
                (1.0 - uplink_q[i]) ** cfg.uplink_handicap_skew
                * cfg.uplink_handicap_max_ms
            )
            value += handicap_i

            rtt_val = max(cfg.min_rtt_ms, int(round(value)))
            rtt[(i + 1, j + 1)] = rtt_val

            # Jitter:
            #   - slow heavy-tail edges keep their existing proportional
            #     jitter (big swings, breaks TCP RTT estimation);
            #   - everything else (healthy core, heavy-tail fast peers)
            #     gets RTT-proportional jitter scaled by the sender's
            #     uplink quality, with `jitter_max_ms` as a floor so very
            #     low-RTT intra-region edges still see a couple ms.
            if slow_sub_edge and cfg.heavy_tail_jitter_frac > 0.0:
                jitter[(i + 1, j + 1)] = int(
                    round(rtt_val * cfg.heavy_tail_jitter_frac)
                )
            else:
                jitter_frac = (
                    cfg.non_heavy_jitter_base_frac
                    + (1.0 - uplink_q[i]) * cfg.uplink_jitter_extra_frac
                )
                jitter[(i + 1, j + 1)] = max(
                    j_small, int(round(rtt_val * jitter_frac))
                )

            # Loss only applies on slow heavy-tail edges — the fast peers
            # are explicitly the "working" connections the bad validator
            # still has, so they should look like a (slower) healthy edge.
            if slow_sub_edge and cfg.heavy_tail_loss_max_pct > 0.0:
                loss[(i + 1, j + 1)] = (
                    cfg.heavy_tail_loss_min_pct + u_loss * loss_span
                )
            else:
                loss[(i + 1, j + 1)] = 0.0

    # Persist the per-validator fast-peer sets (1-based) on the matrix so
    # `summarize` / TSV header can show which peers a bad node still reaches.
    fast_peers_1b = {
        v + 1: sorted(p + 1 for p in fast_peers[v] if p < n)
        for v in heavy_tail_set
        if v < n
    }

    return LatencyMatrix(
        cfg=cfg,
        region_of=region_of,
        heavy_tail=sorted(v + 1 for v in heavy_tail_set if v < n),
        fast_peers=fast_peers_1b,
        rtt_ms=rtt,
        jitter_ms=jitter,
        loss_pct=loss,
    )


def write_tsv(matrix: LatencyMatrix, path: Path) -> None:
    """Write `src \\t dst \\t rtt_ms \\t jitter_ms \\t loss_pct` rows
    (1-based indices). The bash reader treats the 5th column as optional so
    older TSVs without `loss_pct` still load (loss falls back to 0).

    Header comment lines (starting with `#`) are skipped by the bash reader.
    """
    cfg = matrix.cfg
    fast_peer_lines = [
        f"# fast_peers[{v}]: {matrix.fast_peers[v]}"
        for v in matrix.heavy_tail
    ]
    lines = [
        f"# latency-matrix n={cfg.num_validators} seed={cfg.seed} "
        f"default_matrix_validators={cfg.default_matrix_validators}",
        f"# regions: {', '.join(f'{i+1}:{r}' for i, r in enumerate(matrix.region_of))}",
        f"# heavy_tail: {matrix.heavy_tail}",
        *fast_peer_lines,
        "# src\tdst\trtt_ms\tjitter_ms\tloss_pct",
    ]
    for (src, dst), rtt in sorted(matrix.rtt_ms.items()):
        lines.append(
            f"{src}\t{dst}\t{rtt}\t"
            f"{matrix.jitter_ms[(src, dst)]}\t"
            f"{matrix.loss_pct[(src, dst)]:.2f}"
        )
    path.write_text("\n".join(lines) + "\n")


def _percentile(values: list[int], q: float) -> int:
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

    # Per-validator outbound mean RTT — shows the per-node uplink spread.
    out_means = [
        sum(rtt[(v, j)] for j in range(1, n + 1) if j != v) / (n - 1)
        for v in range(1, n + 1)
    ]
    out_means.sort()
    lines = [
        f"  Validators        : {n} across {len(REGIONS)} regions",
        f"  RTT mean / p50 / p90 / p99 / max : "
        f"{mean:.1f} / {_percentile(all_rtt, 0.5)} / {_percentile(all_rtt, 0.9)} / "
        f"{_percentile(all_rtt, 0.99)} / {max(all_rtt)} ms",
        f"  Per-validator outbound mean RTT spread: "
        f"min {out_means[0]:.0f} / p25 {out_means[max(0, len(out_means)//4)]:.0f} / "
        f"p50 {out_means[len(out_means)//2]:.0f} / "
        f"p75 {out_means[min(len(out_means)-1, 3*len(out_means)//4)]:.0f} / "
        f"max {out_means[-1]:.0f} ms",
    ]

    if matrix.heavy_tail:
        lines.append(f"  Heavy-tail validators ({len(matrix.heavy_tail)}):")
        for v in matrix.heavy_tail:
            fast = set(matrix.fast_peers.get(v, []))
            out_slow = [rtt[(v, j)] for j in range(1, n + 1)
                        if j != v and j not in fast]
            in_slow = [rtt[(j, v)] for j in range(1, n + 1)
                       if j != v and j not in fast]
            out_total = sum(1 for j in range(1, n + 1) if j != v)
            # Loss + jitter spread across this validator's *slow* edges only —
            # fast peers are explicitly 0% loss / small jitter by construction.
            edge_loss = [
                matrix.loss_pct[(v, j)] for j in range(1, n + 1)
                if j != v and j not in fast
            ] + [
                matrix.loss_pct[(j, v)] for j in range(1, n + 1)
                if j != v and j not in fast
            ]
            edge_jit = [
                matrix.jitter_ms[(v, j)] for j in range(1, n + 1)
                if j != v and j not in fast
            ] + [
                matrix.jitter_ms[(j, v)] for j in range(1, n + 1)
                if j != v and j not in fast
            ]
            lines.append(
                f"    validator-{v}: "
                f"out {len(out_slow)}/{out_total} slow "
                f"(median {_percentile(out_slow, 0.5)}ms, "
                f"max {max(out_slow) if out_slow else 0}ms); "
                f"in {len(in_slow)}/{out_total} slow "
                f"(median {_percentile(in_slow, 0.5)}ms, "
                f"max {max(in_slow) if in_slow else 0}ms)"
            )
            sorted_fast = sorted(fast)
            fast_rtts = [rtt[(v, p)] for p in sorted_fast]
            if fast_rtts:
                lines.append(
                    f"      fast peers {sorted_fast} "
                    f"(rtt {min(fast_rtts)}–{max(fast_rtts)} ms)"
                )
            if edge_loss and max(edge_loss) > 0.0:
                lines.append(
                    f"      slow-edge loss "
                    f"{min(edge_loss):.1f}%–{max(edge_loss):.1f}%, "
                    f"jitter {min(edge_jit)}–{max(edge_jit)} ms"
                )
            elif edge_jit:
                lines.append(
                    f"      slow-edge jitter {min(edge_jit)}–{max(edge_jit)} ms "
                    "(loss disabled)"
                )
    else:
        lines.append("  Heavy-tail validators: (none)")

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
        help="canonical seeded table size used for smaller submatrix runs",
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
