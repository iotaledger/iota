#!/usr/bin/env python3

# Copyright (c) 2026 IOTA Stiftung
# SPDX-License-Identifier: Apache-2.0

"""Rolling migration test: start validators on a release image under the
role-based latency model (network-benchmark.sh), with Grafana/Prometheus
monitoring, checkpoint-liveness tracking, and an optional stress load
generator, then perform a rolling upgrade to a locally-built image. Simple
mode (default) rolls a short fixed offset into the epoch and reports a
pre/post-upgrade stable-window comparison; --mode advanced runs the full
mid-epoch + post-upgrade restart schedule.

Run from: iota/dev-tools/iota-private-network/experiments/
"""

from __future__ import annotations

import argparse
import atexit
import math
import os
import random
import re
import shutil
import signal
import subprocess
import sys
import threading
import time
from concurrent.futures import ThreadPoolExecutor
from dataclasses import dataclass, field
from datetime import datetime, timezone
from pathlib import Path

import experiment_common as ec
from experiment_common import (
    _C,
    _phase_banner,
    _phase_complete,
    _progress_bar,
    countdown as _countdown,
    find_repo_root as _find_repo_root,
    log,
    log_status,
    prometheus_query as _prometheus_query,
    prometheus_scalar as _prometheus_scalar,
    run,
    run_timed,
)



# ========================= Configuration =========================


@dataclass
class Config:
    """All parameters for the migration test."""

    # --- Hardcoded (fixed for every run) ---
    num_validators: int = 10
    epoch_duration_ms: int = 600_000  # 10 minutes
    seed: int = 42
    geodistributed: bool = True
    log_interval: int = 60  # save logs every N seconds
    final_epoch_settle_wait: int = 10  # seconds after the second post-start epoch
    epoch_start_slop_seconds: int = 15  # epoch_0_start is observed after boot

    # Derived from epoch_duration_ms (set in __post_init__)
    mid_epoch_wait: int = field(init=False)
    pre_rolling_wait: int = field(init=False)  # simple mode: wait into epoch 0 before rolling
    upgrade_delay: int = field(init=False)
    protocol_probe_wait: int = field(init=False)
    restart_settle_wait: int = field(init=False)
    restart_pause_keep_db: int = field(init=False)
    restart_pause_wipe_db: int = field(init=False)
    rolling_restart_pause_min: int = field(init=False)
    rolling_restart_pause_max: int = field(init=False)
    fresh_db_restart_pause_min: int = field(init=False)
    fresh_db_restart_pause_max: int = field(init=False)
    phase8_worst_case: int = field(init=False)
    phase8_simple_estimate: int = field(init=False)
    phase9_epoch0_worst_case: int = field(init=False)
    timeline_safety_margin: int = field(init=False)
    # End-of-run stable-window comparison: matched-length windows in epoch 0
    # (pre-rolling, no upgrades started) and epoch 1 (post-migration, after a
    # short settle offset). Window length is capped so total test time stays
    # reasonable when pre_rolling_wait is large.
    stable_window_seconds: int = field(init=False)
    stable_window_settle_seconds: int = 30
    block_measurement_seconds: int = 120
    # Wait inside phase 6 for network-benchmark.sh to apply the matrix.
    # The injector applies the full matrix in a few seconds and its watcher
    # heals any wiped edge within ~2s, so 15s covers apply + consensus settle.
    latency_apply_wait: int = 15

    image_old: str = "iota-node:old"
    image_upgrade: str = "iota-node:upgrade"
    compose_file: str = "docker-compose.migration.yaml"
    env_migration_file: str = ".env.migration"
    grafana_override_file: str = "docker-compose.migration-override.yaml"

    # --- CLI tunables ---
    release_network: str = "testnet"
    # "simple": fast rolling upgrade, no mid-epoch wait, no post-upgrade restarts.
    # "advanced": full schedule with rolling offline windows + keep/wipe-DB restarts.
    mode: str = "simple"
    build: bool = True
    chain_override: str = ""  # empty = Chain::Unknown (devnet-like)
    load_qps: int = 0
    load_in_flight_ratio: int = 5
    load_transfer_objects: int = 100
    load_rpc_address: str = "http://fullnode-1:9000"
    load_tools_image: str = "iotaledger/stress"
    load_primary_gas_owner_id: str = ec.DEFAULT_PRIMARY_GAS_OWNER_ID

    # --- Derived paths (set in __post_init__) ---
    script_dir: Path = field(default_factory=lambda: Path(__file__).resolve().parent)
    network_dir: Path = field(init=False)
    repo_root: Path = field(init=False)
    grafana_dir: Path = field(init=False)
    log_dir: Path = field(init=False)
    log_file: Path = field(init=False)

    def __post_init__(self) -> None:
        ec.validate_num_validators(self.num_validators)
        if self.mode not in ("simple", "advanced"):
            raise ValueError(f"mode must be 'simple' or 'advanced', got {self.mode!r}")
        if self.load_qps < 0:
            raise ValueError("load qps must be >= 0")
        if self.load_in_flight_ratio <= 0:
            raise ValueError("load in-flight ratio must be > 0")
        if self.load_transfer_objects <= 0:
            raise ValueError("load transfer objects must be > 0")
        # Timing derived from epoch duration
        epoch_s = self.epoch_duration_ms // 1000
        # Rolling upgrade timing is derived from epoch length and validator
        # count. For 20 validators and a 1h epoch this gives 90-120s offline
        # per validator plus a tiny separate inter-validator pacing delay.
        n = max(self.num_validators, 1)
        self.rolling_restart_pause_max = max(1, (2 * epoch_s) // (3 * n))
        self.rolling_restart_pause_min = max(1, (self.rolling_restart_pause_max * 3 + 3) // 4)
        self.upgrade_delay = (
            0
            if self.rolling_restart_pause_max <= 1
            else min(5, max(1, self.rolling_restart_pause_max // 120))
        )
        self.fresh_db_restart_pause_min = self.rolling_restart_pause_min
        self.fresh_db_restart_pause_max = self.rolling_restart_pause_max
        # Upper bound on waiting for validator-1's logs to show the new
        # max_protocol_version after restart. The probe polls and exits as
        # soon as the line appears (usually a few seconds); the bound covers
        # slow starts (image load, WAL replay) and is charged to the phase-8
        # estimates below.
        self.protocol_probe_wait = 15
        self.restart_settle_wait = min(10, max(1, self.rolling_restart_pause_max // 3))

        # Keep the post-upgrade restarts inside the same epoch by scaling the
        # pre-restart waits with the rolling offline window instead of using
        # large independent epoch fractions.
        self.restart_pause_keep_db = max(1, min(epoch_s // 30, self.rolling_restart_pause_max // 2))
        self.restart_pause_wipe_db = max(1, min(epoch_s // 20, self.rolling_restart_pause_max))

        self.phase8_worst_case = (
            n * (self.rolling_restart_pause_max + self.upgrade_delay)
            + self.protocol_probe_wait
        )
        self.phase9_epoch0_worst_case = (
            self.restart_pause_keep_db
            + self.restart_settle_wait
            + self.restart_pause_wipe_db
            + self.fresh_db_restart_pause_max
            + self.restart_settle_wait
        )
        self.timeline_safety_margin = min(max(10, epoch_s // 60), max(0, epoch_s // 10))
        self.mid_epoch_wait = (
            epoch_s
            - self.phase8_worst_case
            - self.phase9_epoch0_worst_case
            - self.timeline_safety_margin
            - self.epoch_start_slop_seconds
        )
        if self.mid_epoch_wait < 0:
            # Simple mode skips phases 7 and 9, so the rolling schedule need not
            # fit inside one epoch — only advanced mode requires it.
            if self.mode == "advanced":
                required = (
                    self.phase8_worst_case
                    + self.phase9_epoch0_worst_case
                    + self.timeline_safety_margin
                    + self.epoch_start_slop_seconds
                )
                raise ValueError(
                    "epoch duration is too short for the derived migration schedule: "
                    f"need at least {required}s for {self.num_validators} validators, "
                    f"got {epoch_s}s"
                )
            self.mid_epoch_wait = 0

        # Simple mode: per-validator phase-8 cost is dominated by docker
        # compose CLI overhead (~4-6s parsing the compose YAML + env file)
        # plus container start (~3-5s). With `stop -t 1` and no per-validator
        # log save / `docker ps` check, this lands around 10s. Plus the
        # one-time protocol probe after validator-1 and the final liveness
        # sweep (5s settle + one `docker ps`).
        self.phase8_simple_estimate = n * 10 + self.protocol_probe_wait + 5
        min_stable_window_seconds = 60
        # Phases 5-6B burn epoch-0 time before the fixed pre-rolling wait is
        # checked: the latency-apply wait, the block-production measurement
        # window, plus slack for monitoring setup and matrix generation.
        # pre_rolling_wait must cover them, or phase 7 would start already
        # past its planned offset and abort.
        pre_phase7_overhead = (
            self.latency_apply_wait
            + (
                self.block_measurement_seconds
                if self.block_measurement_enabled()
                else 0
            )
            + 30
        )
        min_pre_rolling_wait = (
            pre_phase7_overhead
            + self.stable_window_settle_seconds
            + min_stable_window_seconds
        )
        self.pre_rolling_wait = (
            epoch_s
            - self.phase8_simple_estimate
            - self.timeline_safety_margin
            - self.epoch_start_slop_seconds
        )
        if self.pre_rolling_wait < min_pre_rolling_wait:
            if self.mode == "simple":
                required = (
                    min_pre_rolling_wait
                    + self.phase8_simple_estimate
                    + self.timeline_safety_margin
                    + self.epoch_start_slop_seconds
                )
                raise ValueError(
                    "epoch duration is too short for the simple migration schedule: "
                    f"need at least {required}s for {self.num_validators} validators "
                    f"({min_pre_rolling_wait}s pre-rolling wait covering "
                    f"{pre_phase7_overhead}s phase 5-6B overhead, "
                    f"{self.stable_window_settle_seconds}s settle, and a "
                    f"{min_stable_window_seconds}s stable window, "
                    f"{self.phase8_simple_estimate}s phase-8 estimate, "
                    f"{self.timeline_safety_margin}s safety, "
                    f"{self.epoch_start_slop_seconds}s epoch-start slop), "
                    f"got {epoch_s}s"
                )
            self.pre_rolling_wait = min_pre_rolling_wait
        # Stable analysis window: same length after setup completes in epoch 0
        # and after the epoch-1 settle offset. Reserve estimated phase 5-6B
        # overhead before deriving the available pre-upgrade window.
        self.stable_window_seconds = max(
            min_stable_window_seconds,
            min(
                180,
                self.pre_rolling_wait
                - pre_phase7_overhead
                - self.stable_window_settle_seconds,
            ),
        )

        self.network_dir = self.script_dir.parent
        self.repo_root = _find_repo_root(self.script_dir)
        self.grafana_dir = self.network_dir / ".." / "grafana-local"
        self.log_dir = self.script_dir / "logs"
        self.log_file = self.log_dir / "migration_script_latest.log"

        # Default chain override from release network
        if not self.chain_override:
            if self.release_network in ("testnet", "mainnet"):
                self.chain_override = self.release_network

    def block_measurement_enabled(self) -> bool:
        """Run the pre-upgrade report only in the simple schedule."""
        return self.mode == "simple" and self.block_measurement_seconds > 0


def _restart_validator_count(n: int) -> int:
    """Return a restart set size strictly below one third of validators."""
    return max(0, (n + 2) // 3 - 1)


def _pick_restart_validators(n: int, epoch: int) -> list[int]:
    """Pick a deterministic pseudo-random restart set for (n, epoch)."""
    count = _restart_validator_count(n)
    if count == 0:
        return []

    validators = list(range(1, n + 1))
    rng = random.Random(n * 1_000_003 + epoch * 97_531 + 0xC0FFEE)
    rng.shuffle(validators)
    return sorted(validators[:count])


# ========================= Globals / State =========================

_cfg: Config | None = None
_cleaning = False
_latency_proc: subprocess.Popen[str] | None = None
_load_logs_proc: subprocess.Popen[str] | None = None
_load_log_archived = False


# ========================= Prometheus / Epoch =========================


def get_current_epoch() -> int | None:
    """Current epoch from Prometheus, or None when the query fails.

    None (unknown) is deliberately distinct from 0 (genesis epoch) so a
    transient Prometheus failure can't masquerade as an epoch reading.
    """
    try:
        value = _prometheus_scalar("max(current_epoch)")
        return int(value) if value is not None else None
    except Exception:
        return None


def get_current_epoch_or_raise(attempts: int = 5, delay: float = 2.0) -> int:
    """get_current_epoch with retries; raises after repeated query failures."""
    for attempt in range(attempts):
        epoch = get_current_epoch()
        if epoch is not None:
            return epoch
        if attempt < attempts - 1:
            time.sleep(delay)
    raise RuntimeError(
        f"could not read current epoch from Prometheus after {attempts} attempts"
    )


def wait_for_epoch_change(cfg: Config, epoch_before: int) -> int:
    """Poll until epoch advances past epoch_before. Returns new epoch."""
    log(f"  Waiting for epoch > {epoch_before}...")
    timeout = cfg.epoch_duration_ms // 1000 * 3 // 2  # 1.5x epoch duration
    start = time.time()

    last_known = epoch_before
    while True:
        epoch_now = get_current_epoch()
        if epoch_now is not None:
            last_known = epoch_now
        if epoch_now is not None and epoch_now > epoch_before:
            print()  # finish status line
            log(f"  {_C.GREEN}Epoch advanced to {epoch_now}{_C.RESET} (was {epoch_before})")
            return epoch_now

        elapsed = int(time.time() - start)
        if elapsed >= timeout:
            print()  # finish status line
            log(f"  {_C.YELLOW}WARNING: Epoch did not advance within {timeout}s — proceeding anyway{_C.RESET}")
            return last_known

        bar = _progress_bar(elapsed, timeout)
        epoch_label = "?" if epoch_now is None else str(epoch_now)
        log_status(f"  Epoch wait: {bar} epoch={epoch_label}, {elapsed}s / {timeout}s")
        time.sleep(30)


class CheckpointMonitor:
    """Background checkpoint liveness monitor.

    Polls ``max(highest_synced_checkpoint)`` from Prometheus every *interval*
    seconds, records samples, and detects stalls (checkpoint not advancing).
    """

    _MS_PER_SECOND = 1000.0

    _COMMIT_LATENCY_QUERIES = ec._commit_latency_queries(60)
    _BLK_P50 = _COMMIT_LATENCY_QUERIES["blk_p50"]
    _BLK_P95 = _COMMIT_LATENCY_QUERIES["blk_p95"]
    _TXN_P50 = _COMMIT_LATENCY_QUERIES["txn_p50"]
    _TXN_P95 = _COMMIT_LATENCY_QUERIES["txn_p95"]

    def __init__(self, interval: int = 10):
        self.interval = interval
        self._samples: list[tuple[float, int, int]] = []  # (ts, checkpoint, epoch)
        # (ts, epoch, blk_p50, blk_p95, txn_p50, txn_p95)
        self._latencies: list[tuple[float, int, float, float, float, float]] = []
        self._stalls: list[tuple[float, float, int]] = []
        self._epoch_regressions: list[tuple[float, int, int, int]] = []
        self._active_epoch_regression: tuple[int, int] | None = None
        self._last_epoch: int | None = None
        self._stop = threading.Event()
        self._thread: threading.Thread | None = None

    def start(self) -> None:
        self._thread = threading.Thread(target=self._run, daemon=True)
        self._thread.start()

    def stop(self) -> None:
        self._stop.set()
        if self._thread:
            self._thread.join(timeout=10)

    def _query_int(self, expr: str) -> int:
        try:
            value = _prometheus_scalar(expr)
            return int(value) if value is not None else -1
        except Exception:
            return -1

    def _query_float(self, expr: str) -> float:
        try:
            value = _prometheus_scalar(expr)
            if value is None:
                return -1.0
            v = float(value)
            return v if v == v else -1.0  # NaN check
        except Exception:
            return -1.0

    def _query_latency_ms(self, expr: str) -> float:
        value = self._query_float(expr)
        return value * self._MS_PER_SECOND if value >= 0 else -1.0

    def _normalize_epoch(self, raw_epoch: int, ts: float, cp: int) -> int:
        if raw_epoch < 0:
            return self._last_epoch if self._last_epoch is not None else -1
        if self._last_epoch is None:
            self._last_epoch = raw_epoch
            return raw_epoch
        if raw_epoch < self._last_epoch:
            regression = (self._last_epoch, raw_epoch)
            if self._active_epoch_regression != regression:
                self._epoch_regressions.append((ts, self._last_epoch, raw_epoch, cp))
                self._active_epoch_regression = regression
            return self._last_epoch
        self._active_epoch_regression = None
        self._last_epoch = raw_epoch
        return raw_epoch

    def _run(self) -> None:
        last_cp = -1
        stall_start: float | None = None
        while not self._stop.is_set():
            cp = self._query_int("max(highest_synced_checkpoint)")
            raw_epoch = self._query_int("max(current_epoch)")
            now = time.time()
            epoch = self._normalize_epoch(raw_epoch, now, cp)
            if cp >= 0:
                self._samples.append((now, cp, epoch))
                if cp > last_cp:
                    if stall_start is not None:
                        self._stalls.append((stall_start, now, last_cp))
                        stall_start = None
                    last_cp = cp
                elif stall_start is None and last_cp >= 0:
                    stall_start = now
            bp50 = self._query_latency_ms(self._BLK_P50)
            bp95 = self._query_latency_ms(self._BLK_P95)
            tp50 = self._query_latency_ms(self._TXN_P50)
            tp95 = self._query_latency_ms(self._TXN_P95)
            if bp50 >= 0:
                self._latencies.append((
                    now, epoch, bp50, bp95, tp50, tp95,
                ))
            self._stop.wait(self.interval)
        if stall_start is not None:
            self._stalls.append((stall_start, time.time(), last_cp))

    @staticmethod
    def _median(vals: list[float]) -> float:
        s = sorted(vals)
        n = len(s)
        if n == 0:
            return 0.0
        if n % 2:
            return s[n // 2]
        return (s[n // 2 - 1] + s[n // 2]) / 2

    def _observed_epoch_changes(self) -> list[tuple[float, int, int, int]]:
        changes: list[tuple[float, int, int, int]] = []
        if len(self._samples) < 2:
            return changes

        prev_epoch = next((ep for _, _, ep in self._samples if ep >= 0), -1)
        if prev_epoch < 0:
            return changes
        for ts, cp, epoch in self._samples[1:]:
            if epoch >= 0 and epoch != prev_epoch:
                changes.append((ts, prev_epoch, epoch, cp))
                prev_epoch = epoch
        return changes

    def _epoch_segments(self) -> list[tuple[int, float, float, int, int]]:
        segments: list[tuple[int, float, float, int, int]] = []
        current_epoch: int | None = None
        start_ts = last_ts = 0.0
        start_cp = last_cp = 0

        for ts, cp, epoch in self._samples:
            if epoch < 0:
                continue
            if current_epoch is None:
                current_epoch = epoch
                start_ts = last_ts = ts
                start_cp = last_cp = cp
                continue
            if epoch != current_epoch:
                segments.append((current_epoch, start_ts, ts, start_cp, cp))
                current_epoch = epoch
                start_ts = ts
                start_cp = cp
            last_ts = ts
            last_cp = cp

        if current_epoch is not None:
            segments.append((current_epoch, start_ts, last_ts, start_cp, last_cp))
        return segments

    def stable_window_report(
        self,
        cfg: "Config",
        pre_upgrade_ready_ts: float,
        epoch_1_start_ts: float,
    ) -> str:
        """Side-by-side metric comparison over equal-length stable windows.

        Pre-upgrade window: [setup_complete + settle, setup_complete + settle
        + window]. Setup is complete only after monitoring, latency, optional
        load, and the block-production measurement are established.

        Epoch 1 window: [epoch_1_start + settle, epoch_1_start + settle + window].
        Skips the reconfig transient; same duration as the pre-upgrade window.

        Queries Prometheus with the `@` modifier so the report stays valid
        regardless of how long the test ran after these windows.
        """
        w = cfg.stable_window_seconds
        settle = cfg.stable_window_settle_seconds
        e0_window_start = pre_upgrade_ready_ts + settle
        e1_window_start = epoch_1_start_ts + settle
        e0_end = int(e0_window_start + w)
        e1_end = int(e1_window_start + w)

        def _eval(expr: str) -> float | None:
            data = _prometheus_query(expr)
            if not data:
                return None
            try:
                r = data["data"]["result"]
                if not r:
                    return None
                v = float(r[0]["value"][1])
                return v if v == v else None  # NaN guard
            except (KeyError, IndexError, TypeError, ValueError):
                return None

        # (label, query_template, unit, value_scale)
        metrics = [
            ("Tx commit p50", "histogram_quantile(0.5, sum by (le) (rate(consensus_transaction_commit_latency_bucket[{w}s] @ {t})))", "ms", 1000.0),
            ("Tx commit p95", "histogram_quantile(0.95, sum by (le) (rate(consensus_transaction_commit_latency_bucket[{w}s] @ {t})))", "ms", 1000.0),
            ("Tx commit p99", "histogram_quantile(0.99, sum by (le) (rate(consensus_transaction_commit_latency_bucket[{w}s] @ {t})))", "ms", 1000.0),
            ("Block commit p50", "histogram_quantile(0.5, sum by (le) (rate(consensus_block_commit_latency_bucket[{w}s] @ {t})) or sum by (le) (rate(consensus_block_header_commit_latency_bucket[{w}s] @ {t})))", "ms", 1000.0),
            ("Block commit p95", "histogram_quantile(0.95, sum by (le) (rate(consensus_block_commit_latency_bucket[{w}s] @ {t})) or sum by (le) (rate(consensus_block_header_commit_latency_bucket[{w}s] @ {t})))", "ms", 1000.0),
            ("Proposed blocks/s", "sum(rate(consensus_proposed_blocks[{w}s] @ {t}))", "blk/s", 1.0),
            ("Commits/s", "sum(rate(consensus_transaction_commit_latency_count[{w}s] @ {t}))", "/s", 1.0),
        ]

        e0_label = datetime.fromtimestamp(e0_window_start, tz=timezone.utc).strftime("%H:%M:%S")
        e1_label = datetime.fromtimestamp(e1_window_start, tz=timezone.utc).strftime("%H:%M:%S")
        lines = [
            f"  Windows ({w}s each): pre-upgrade starts {e0_label} UTC "
            f"(= setup_complete_at + {settle}s settle), "
            f"post-upgrade starts {e1_label} UTC (= next_epoch_at + {settle}s settle)",
            "",
            f"  {'Metric':<24} {'pre-upgrade':>14} {'post-upgrade':>14} {'delta':>14}",
            f"  {'-' * 24} {'-' * 14:>14} {'-' * 14:>14} {'-' * 14:>14}",
        ]
        for label, tpl, unit, scale in metrics:
            v0 = _eval(tpl.format(w=w, t=e0_end))
            v1 = _eval(tpl.format(w=w, t=e1_end))
            s0 = f"{v0 * scale:.1f} {unit}" if v0 is not None else "—"
            s1 = f"{v1 * scale:.1f} {unit}" if v1 is not None else "—"
            if v0 is not None and v1 is not None:
                sd = f"{(v1 - v0) * scale:+.1f} {unit}"
            else:
                sd = "—"
            lines.append(f"  {label:<24} {s0:>14} {s1:>14} {sd:>14}")
        return "\n".join(lines)

    def report(self) -> str:
        if not self._samples:
            return "  No checkpoint samples collected."

        # --- Aggregated summary ---
        first_ts, first_cp, _ = self._samples[0]
        last_ts, last_cp, _ = self._samples[-1]
        duration = last_ts - first_ts
        advanced = last_cp - first_cp
        stall_time = sum(end - start for start, end, _ in self._stalls)
        active_time = duration - stall_time
        cp_rate = advanced / active_time if active_time > 0 else 0

        lines = [
            f"  Checkpoints  : {first_cp} \u2192 {last_cp} (+{advanced} in {int(duration)}s)",
            f"  CP rate      : {cp_rate:.2f}/s",
        ]
        if self._latencies:
            bp50 = self._median([v for _, _, v, _, _, _ in self._latencies])
            bp95 = self._median([v for _, _, _, v, _, _ in self._latencies])
            tp50 = self._median([v for _, _, _, _, v, _ in self._latencies if v >= 0])
            tp95 = self._median([v for _, _, _, _, _, v in self._latencies if v >= 0])
            lines.append(f"  Block  lat   : p50={bp50:.0f}ms  p95={bp95:.0f}ms")
            lines.append(f"  Tx lat       : p50={tp50:.0f}ms  p95={tp95:.0f}ms")
        lines.append(f"  Samples      : {len(self._samples)}")
        epoch_changes = self._observed_epoch_changes()
        lines.append(f"  Stalls       : {len(self._stalls) if self._stalls else 'none'}")
        lines.append(f"  Epoch shifts : {len(epoch_changes) if epoch_changes else 'none'}")
        if self._epoch_regressions:
            lines.append(f"  Epoch regressions ignored: {len(self._epoch_regressions)}")

        events: list[tuple[float, str]] = []
        for start, end, cp in self._stalls:
            dur = int(end - start)
            t = datetime.fromtimestamp(start, tz=timezone.utc).strftime("%H:%M:%S")
            events.append((start, f"    - {t} stuck at checkpoint {cp} for {dur}s"))
        for ts, from_epoch, to_epoch, cp in epoch_changes:
            t = datetime.fromtimestamp(ts, tz=timezone.utc).strftime("%H:%M:%S")
            events.append((ts, f"    - {t} epoch {from_epoch} \u2192 {to_epoch} observed at checkpoint {cp}"))
        for ts, from_epoch, to_epoch, cp in self._epoch_regressions:
            t = datetime.fromtimestamp(ts, tz=timezone.utc).strftime("%H:%M:%S")
            events.append((ts, f"    - {t} ignored epoch regression {from_epoch} \u2192 {to_epoch} at checkpoint {cp}"))
        for _, event_line in sorted(events, key=lambda item: item[0]):
            lines.append(event_line)

        # --- Per-epoch table ---
        lat_by_epoch: dict[int, list[tuple[float, float, float, float]]] = {}
        for _, ep, bp50, bp95, tp50, tp95 in self._latencies:
            if ep < 0:
                continue
            lat_by_epoch.setdefault(ep, []).append((bp50, bp95, tp50, tp95))

        epoch_segments = self._epoch_segments()
        if len(epoch_segments) > 1:
            hdr = (
                f"  {'Epoch':>5}  {'Duration':>8}  {'CP rate':>8}"
                f"  {'Blk p50':>8}  {'Blk p95':>8}"
                f"  {'Tx p50':>8}  {'Tx p95':>8}"
            )
            sep = (
                f"  {'-----':>5}  {'--------':>8}  {'-------':>8}"
                f"  {'-------':>8}  {'-------':>8}"
                f"  {'------':>8}  {'------':>8}"
            )
            lines += ["", hdr, sep]
            for ep, start_ts, end_ts, start_cp, end_cp in epoch_segments:
                ep_dur = end_ts - start_ts
                ep_adv = end_cp - start_cp
                ep_rate = ep_adv / ep_dur if ep_dur > 0 else 0
                dur_s = f"{int(ep_dur)}s" if ep_dur > 0 else "-"
                rate_s = f"{ep_rate:.2f}/s" if ep_dur > 0 else "-"
                # Latencies
                lats = lat_by_epoch.get(ep, [])
                if lats:
                    ebp50 = f"{self._median([v for v, _, _, _ in lats]):.0f}ms"
                    ebp95 = f"{self._median([v for _, v, _, _ in lats]):.0f}ms"
                    etp50 = f"{self._median([v for _, _, v, _ in lats if v >= 0]):.0f}ms"
                    etp95 = f"{self._median([v for _, _, _, v in lats if v >= 0]):.0f}ms"
                else:
                    ebp50 = ebp95 = etp50 = etp95 = "-"
                lines.append(
                    f"  {ep:>5}  {dur_s:>8}  {rate_s:>8}"
                    f"  {ebp50:>8}  {ebp95:>8}"
                    f"  {etp50:>8}  {etp95:>8}"
                )

        return "\n".join(lines)


def docker_compose(
    cfg: Config, args: list[str], *, cwd: Path | None = None, quiet: bool = False
) -> subprocess.CompletedProcess[str]:
    """Run docker compose with the migration env and compose file."""
    cmd = [
        "docker",
        "compose",
        "--ansi",
        "never",
        "--env-file",
        cfg.env_migration_file,
        "-f",
        cfg.compose_file,
        *args,
    ]
    return run(cmd, cwd=cwd or cfg.network_dir, quiet=quiet)


def _migration_network_name(cfg: Config) -> str:
    return f"{cfg.network_dir.name}_migration-network"


def save_validator_logs(cfg: Config, num: int, prefix: str = "exp") -> None:
    for i in range(1, num + 1):
        dest = cfg.log_dir / f"{prefix}-validator-{i}-latest.log"
        with dest.open("w") as fh:
            subprocess.run(
                ["docker", "logs", f"validator-{i}"],
                stdout=fh,
                stderr=subprocess.STDOUT,
                check=False,
            )


def start_load_generator(cfg: Config) -> None:
    """Start optional stress load against the migration network."""
    global _load_logs_proc
    if cfg.load_qps <= 0:
        return

    phase_start = time.time()
    log(_phase_banner(f"Starting load generator ({cfg.load_qps} qps)", "PHASE 6b"))

    # Load was explicitly requested (--load-qps > 0). The image was resolved
    # up front by ensure_stress_image (pull or build); this non-interactive
    # guard fails the run instead of silently measuring an unloaded network
    # if it is somehow still missing.
    if not ec.ensure_image(cfg.load_tools_image):
        raise RuntimeError(
            f"--load-qps {cfg.load_qps} requested but image "
            f"{cfg.load_tools_image} is unavailable — `docker login` to the "
            "registry or pass --load-tools-image"
        )

    for sec in range(30):
        result = run(
            ["docker", "ps", "--format", "{{.Names}}"],
            capture=True,
            quiet=True,
        )
        if "fullnode-1" in set(result.stdout.strip().splitlines()):
            break
        log_status(f"  Waiting for fullnode-1 before starting load... {sec + 1}s")
        time.sleep(1)
    else:
        print()
        raise RuntimeError("fullnode-1 is not running; cannot start load generator")
    print()

    # Shared implementation: writable keystore copy, docker run, and a 5s
    # startup liveness check (raises with the container logs on failure).
    ec.start_stress_container(
        image=cfg.load_tools_image,
        network_name=_migration_network_name(cfg),
        network_dir=cfg.network_dir,
        log_dir=cfg.log_dir,
        rpc_address=cfg.load_rpc_address,
        gas_owner_id=cfg.load_primary_gas_owner_id,
        target_qps=cfg.load_qps,
        in_flight_ratio=cfg.load_in_flight_ratio,
        transfer_objects=cfg.load_transfer_objects,
    )

    # Verify the stress tool actually connected to the fullnode RPC.
    # "Found new state" is emitted after successful system state retrieval.
    for sec in range(30):
        logs = subprocess.run(
            ["docker", "logs", "--tail", "10", "stress-benchmark"],
            capture_output=True, text=True, check=False,
        )
        combined = logs.stdout + logs.stderr
        if "Found new state" in combined:
            log(f"  Load generator connected to RPC after {sec + 6}s")
            break
        log_status(f"  Waiting for load generator to connect... {sec + 6}s")
        time.sleep(1)
    else:
        print()  # finish status line
        fail_logs = subprocess.run(
            ["docker", "logs", "--tail", "20", "stress-benchmark"],
            capture_output=True, text=True, check=False,
        )
        raise RuntimeError(
            f"Load generator started but did not connect to RPC within 30s.\n"
            f"  Last logs:\n{(fail_logs.stdout + fail_logs.stderr).strip()}"
        )
    print()  # finish status line

    load_log = cfg.log_dir / "load-generator-latest.log"
    load_log_fh = load_log.open("w")
    _load_logs_proc = subprocess.Popen(
        ["docker", "logs", "-f", "stress-benchmark"],
        stdout=load_log_fh,
        stderr=subprocess.STDOUT,
    )
    load_log_fh.close()

    log(f"  RPC target: {cfg.load_rpc_address}")
    log(f"  Logs: {load_log}")
    log(_phase_complete("Phase 6b", time.time() - phase_start))


def stop_load_generator(cfg: Config) -> None:
    """Stop load and archive its log once."""
    global _load_logs_proc, _load_log_archived
    if cfg.load_qps <= 0:
        return

    run(["docker", "rm", "-f", "stress-benchmark"], check=False, quiet=True)
    if _load_logs_proc is not None and _load_logs_proc.poll() is None:
        _load_logs_proc.terminate()
        try:
            _load_logs_proc.wait(timeout=5)
        except subprocess.TimeoutExpired:
            _load_logs_proc.kill()
            _load_logs_proc.wait(timeout=5)
    _load_logs_proc = None

    latest = cfg.log_dir / "load-generator-latest.log"
    if latest.exists() and not _load_log_archived:
        ts = datetime.now().strftime("%Y%m%d-%H%M%S")
        archived = cfg.log_dir / f"load-generator-{ts}.log"
        shutil.copy2(latest, archived)
        _load_log_archived = True
        log(f"Saved load generator log to {archived}")


# ========================= Cleanup =========================


def cleanup() -> None:
    global _cleaning
    if _cleaning:
        return
    _cleaning = True

    cfg = _cfg
    if cfg is None:
        return

    log("Cleaning up...")

    stop_load_generator(cfg)

    # Stop the latency injector first so it does not keep mutating the network
    # while the validator and monitoring stacks are being torn down.
    run(["sudo", "pkill", "-f", r"network-benchmark\.sh"], check=False, quiet=True)
    if _latency_proc is not None and _latency_proc.poll() is None:
        _latency_proc.terminate()
        try:
            _latency_proc.wait(timeout=5)
        except subprocess.TimeoutExpired:
            _latency_proc.kill()
            _latency_proc.wait(timeout=5)

    # Leave Grafana/Prometheus running so data and dashboards remain accessible
    # after the test completes.

    compose_path = cfg.network_dir / cfg.compose_file
    if compose_path.exists():
        run(
            [
                "docker",
                "compose",
                "--env-file",
                cfg.env_migration_file,
                "-f",
                cfg.compose_file,
                "down",
                "--remove-orphans",
            ],
            cwd=cfg.network_dir,
            check=False,
            quiet=True,
        )

    # Clean generated files (keep Grafana override so monitoring stays functional)
    for f in (
        compose_path,
        cfg.network_dir / cfg.env_migration_file,
    ):
        f.unlink(missing_ok=True)

    # Clean data directories (may contain root-owned files from bootstrap)
    data_dir = cfg.network_dir / "data"
    if data_dir.exists():
        subprocess.run(["sudo", "rm", "-rf", str(data_dir)], check=False)

    shutil.rmtree(cfg.log_dir / "load-generator-keystore", ignore_errors=True)

    log("Cleanup complete.")
    # Restore terminal to a sane state after subprocess output
    os.system("stty sane 2>/dev/null")
    print("\r\033[K", end="", flush=True)
    ec.close_logging()


def _signal_handler(signum: int, _frame: object) -> None:
    log(f"Received signal {signum}, cleaning up...")
    cleanup()
    sys.exit(0)


# ========================= Phase 1: Docker Images =========================


def phase1_docker_images(cfg: Config) -> tuple[str, str]:
    """Pull old release image and build upgrade image. Returns (old_version, upgrade_version)."""
    phase_start = time.time()
    log(_phase_banner("Preparing Docker images", "PHASE 1"))

    hub_image = f"iotaledger/iota-node:{cfg.release_network}"
    run_timed(["docker", "pull", hub_image], f"Pulling {hub_image}")
    run(["docker", "tag", hub_image, cfg.image_old], quiet=True)
    run(["docker", "tag", cfg.image_old, "iotaledger/iota-node"], quiet=True)

    # Pull tools image for genesis generation
    tools_image = f"iotaledger/iota-tools:{cfg.release_network}"
    result = run_timed(["docker", "pull", tools_image], f"Pulling {tools_image}", check=False)
    if result.returncode != 0:
        run(["docker", "pull", "iotaledger/iota-tools"], check=False, quiet=True)
    else:
        run(["docker", "tag", tools_image, "iotaledger/iota-tools"], quiet=True)

    # Dummy indexer tag
    run(["docker", "tag", cfg.image_old, "iotaledger/iota-indexer"], check=False, quiet=True)

    if cfg.build:
        build_env = {**os.environ, "DOCKER_BUILDKIT": "1"}
        run_timed(
            [
                str(cfg.repo_root / "docker" / "utils" / "build-script.sh"),
                "--image-tag",
                cfg.image_upgrade,
            ],
            "Building upgrade image",
            cwd=cfg.repo_root,
            env=build_env,
        )
    else:
        result = run(
            ["docker", "image", "inspect", cfg.image_upgrade],
            check=False,
            capture=True,
            quiet=True,
        )
        if result.returncode != 0:
            log(f"ERROR: {cfg.image_upgrade} not found and build is disabled")
            sys.exit(1)

    # Get binary versions
    print()  # finish status line
    old_ver = run(
        ["docker", "run", "--rm", cfg.image_old, "iota-node", "--version"],
        capture=True, check=False, quiet=True,
    ).stdout.strip().split("\n")[0]

    upgrade_ver = run(
        ["docker", "run", "--rm", cfg.image_upgrade, "iota-node", "--version"],
        capture=True, check=False, quiet=True,
    ).stdout.strip().split("\n")[0]

    branch = run(
        ["git", "-C", str(cfg.repo_root), "rev-parse", "--abbrev-ref", "HEAD"],
        capture=True, check=False, quiet=True,
    ).stdout.strip()
    commit = run(
        ["git", "-C", str(cfg.repo_root), "rev-parse", "--short", "HEAD"],
        capture=True, check=False, quiet=True,
    ).stdout.strip()

    log(f"  {_C.YELLOW}Old{_C.RESET}     : {old_ver or 'unknown'} ({cfg.release_network})")
    log(f"  {_C.GREEN}Upgrade{_C.RESET} : {upgrade_ver or 'unknown'} ({branch}@{commit})")
    log(_phase_complete("Phase 1", time.time() - phase_start))

    return branch, commit


# ========================= Phase 2: Generate Compose =========================


def phase2_generate_compose(cfg: Config) -> None:
    log(_phase_banner("Generating migration compose file", "PHASE 2"))

    path = cfg.network_dir / cfg.compose_file
    lines: list[str] = [
        "# Auto-generated by run-migration-test.py. Do not edit manually.",
        f"# Rolling migration compose file for {cfg.num_validators} validators.",
        "",
        "services:",
    ]

    for i in range(1, cfg.num_validators + 1):
        ip = 10 + i
        lines.append(f"  validator-{i}:")
        lines.append(f"    image: ${{VALIDATOR_{i}_IMAGE:-{cfg.image_old}}}")
        lines.append(f"    container_name: validator-{i}")
        lines.append(f"    hostname: validator-{i}")
        lines.append("    environment:")
        lines.append("      - RUST_BACKTRACE=1")
        lines.append(
            "      - RUST_LOG=info,iota_core=debug,iota_network=debug,"
            "iota_node=debug,jsonrpsee=error"
        )
        lines.append("      - RPC_WORKER_THREAD=12")
        lines.append("      - NEW_CHECKPOINT_WARNING_TIMEOUT_MS=30000")
        lines.append("      - NEW_CHECKPOINT_PANIC_TIMEOUT_MS=60000")
        lines.append(
            f"      - IOTA_PROTOCOL_CONFIG_CHAIN_OVERRIDE={cfg.chain_override}"
        )
        lines.append("    command:")
        lines.append("      [")
        lines.append('        "/usr/local/bin/iota-node",')
        lines.append('        "--config-path",')
        lines.append('        "/opt/iota/config/validator.yaml",')
        lines.append("      ]")
        lines.append("    restart: on-failure")
        lines.append("    logging:")
        lines.append('      driver: "json-file"')
        lines.append("      options:")
        lines.append('        max-file: "10"')
        lines.append('        max-size: "1g"')
        lines.append("    networks:")
        lines.append("      migration-network:")
        lines.append(f"        ipv4_address: 10.0.1.{ip}")
        lines.append("    volumes:")
        lines.append(
            f"      - ./configs/validators/validator-{i}-8080.yaml:"
            "/opt/iota/config/validator.yaml:ro"
        )
        lines.append(
            "      - ./configs/genesis/genesis.blob:/opt/iota/config/genesis.blob:ro"
        )
        lines.append(f"      - ./data/validator-{i}:/opt/iota/db:rw")
        lines.append("")

    if cfg.load_qps > 0:
        lines.append("  fullnode-1:")
        lines.append(f"    image: {cfg.image_upgrade}")
        lines.append("    container_name: fullnode-1")
        lines.append("    hostname: fullnode-1")
        lines.append("    environment:")
        lines.append("      - RUST_BACKTRACE=1")
        lines.append(
            "      - RUST_LOG=info,iota_core=debug,iota_network=debug,"
            "iota_node=debug,jsonrpsee=error"
        )
        lines.append(
            f"      - IOTA_PROTOCOL_CONFIG_CHAIN_OVERRIDE={cfg.chain_override}"
        )
        lines.append("    command:")
        lines.append("      [")
        lines.append('        "/usr/local/bin/iota-node",')
        lines.append('        "--config-path",')
        lines.append('        "/opt/iota/config/fullnode.yaml",')
        lines.append("      ]")
        lines.append("    restart: on-failure")
        lines.append("    logging:")
        lines.append('      driver: "json-file"')
        lines.append("      options:")
        lines.append('        max-file: "10"')
        lines.append('        max-size: "1g"')
        lines.append("    networks:")
        lines.append("      migration-network:")
        lines.append("        ipv4_address: 10.0.1.250")
        lines.append("    volumes:")
        lines.append(
            "      - ./configs/fullnodes/fullnode.yaml:"
            "/opt/iota/config/fullnode.yaml:ro"
        )
        lines.append(
            "      - ./configs/genesis/genesis.blob:/opt/iota/config/genesis.blob:ro"
        )
        lines.append("      - ./data/fullnode-1:/opt/iota/db:rw")
        lines.append("")

    lines.append("networks:")
    lines.append("  migration-network:")
    lines.append("    driver: bridge")
    lines.append("    ipam:")
    lines.append("      config:")
    lines.append("        - subnet: 10.0.1.0/24")

    path.write_text("\n".join(lines) + "\n")
    log(f"Generated compose file: {path}")
    log(_phase_complete("Phase 2"))


# ========================= Phase 3: Bootstrap Genesis =========================


def phase3_bootstrap_genesis(cfg: Config) -> None:
    phase_start = time.time()
    log(_phase_banner(f"Bootstrapping genesis for {cfg.num_validators} validators", "PHASE 3"))
    run_timed(
        [
            "sudo", "./bootstrap.sh",
            "-n", str(cfg.num_validators),
            "-e", str(cfg.epoch_duration_ms),
        ],
        "Bootstrapping genesis",
        cwd=cfg.network_dir,
    )
    print()  # finish status line
    log(_phase_complete("Phase 3", time.time() - phase_start))


# ========================= Phase 4: Start Validators =========================


def phase4_start_validators(cfg: Config) -> tuple[str, str, float]:
    """Start all validators on old image. Returns (old_max_proto, old_consensus, epoch_0_start)."""
    phase_start = time.time()
    log(_phase_banner(f"Starting all validators on {cfg.image_old}", "PHASE 4"))

    env_path = cfg.network_dir / cfg.env_migration_file
    env_path.write_text("# Migration env file — generated by run-migration-test.py\n")

    docker_compose(cfg, ["up", "-d"], quiet=True)

    for sec in range(10, 0, -1):
        log_status(f"  Waiting for validators to boot... {sec}s")
        time.sleep(1)

    result = run(
        ["docker", "ps", "--filter", "name=validator-", "--format", "{{.Names}}"],
        capture=True, quiet=True,
    )
    running_names = set(result.stdout.strip().splitlines())
    expected_names = {f"validator-{i}" for i in range(1, cfg.num_validators + 1)}
    missing = expected_names - running_names
    print()  # finish status line
    if not missing:
        log(f"  {_C.GREEN}Running validators: {len(running_names)}/{cfg.num_validators}{_C.RESET}")
    else:
        raise RuntimeError(
            f"Missing validators after boot: {sorted(missing)} "
            f"(running: {len(running_names)}/{cfg.num_validators})"
        )

    # Extract protocol info from old image
    old_max_proto, old_consensus = _read_validator_protocol_info("validator-1")

    log(f"  Protocol: {old_consensus or 'unknown'}, max version: {old_max_proto or 'unknown'}")

    epoch_0_start = time.time()
    log(_phase_complete("Phase 4", time.time() - phase_start))

    return old_max_proto, old_consensus, epoch_0_start


def _extract_log_field(logs: str, marker: str, pattern: str, *, last: bool = False) -> str:
    result = ""
    for line in logs.split("\n"):
        if marker in line:
            m = re.search(pattern, line)
            if m:
                if not last:
                    return m.group(1)
                result = m.group(1)
    return result


def _read_validator_protocol_info(validator: str = "validator-1", *, last: bool = False) -> tuple[str, str]:
    result = run(
        ["docker", "logs", validator], capture=True, check=False, quiet=True
    )
    logs = result.stderr + result.stdout
    max_protocol = _extract_log_field(
        logs, "Supported protocol versions", r"max: ProtocolVersion\((\d+)\)", last=last
    )
    consensus = _extract_log_field(
        logs, "Starting consensus protocol", r"Starting consensus protocol (\w+)", last=last
    )
    return max_protocol, consensus


def _probe_protocol_info(validator: str, deadline_s: int) -> tuple[str, str]:
    """Poll a freshly (re)started validator's logs for its protocol info.

    Returns as soon as the max_protocol_version line appears, bounded by
    *deadline_s* so a slow start degrades to 'unknown' instead of stalling
    the schedule.
    """
    deadline = time.time() + deadline_s
    while True:
        proto, consensus = _read_validator_protocol_info(validator, last=True)
        if proto or time.time() >= deadline:
            return proto, consensus
        time.sleep(2)


# ========================= Phase 5: Start Monitoring =========================


def phase5_start_monitoring(cfg: Config) -> None:
    phase_start = time.time()
    log(_phase_banner("Starting Grafana/Prometheus monitoring stack", "PHASE 5"))

    override_path = cfg.grafana_dir / cfg.grafana_override_file
    override_path.write_text(
        "networks:\n"
        "  iota-network:\n"
        "    name: iota-private-network_migration-network\n"
        "    external: true\n"
    )

    # start_grafana force-recreates: a monitoring container left over from a
    # prior run still references that run's (now deleted) network ID and
    # otherwise fails `up` with "network ... not found".
    ec.start_grafana(cfg.grafana_dir, cfg.grafana_override_file)
    log(_phase_complete("Phase 5", time.time() - phase_start))


# ========================= Phase 6: Apply Latency =========================


def _generate_latency_matrix(cfg: Config) -> Path:
    """Dump the effective latency matrix to ``logs/latency-matrix.tsv``.

    network-benchmark.sh natively computes the role-based model (the single
    source of truth) for any validator count; ``-D`` writes the matrix it
    would apply without touching docker or netem state. The dump serves as
    the run's audit artifact and feeds the logged summary.
    """
    matrix_path = cfg.log_dir / "latency-matrix.tsv"
    run(
        [
            "./network-benchmark.sh",
            "-n",
            str(cfg.num_validators),
            "-g",
            str(cfg.geodistributed).lower(),
            "-o",
            str(cfg.log_file.resolve()),
            "-D",
            str(matrix_path.resolve()),
        ],
        cwd=cfg.script_dir,
        quiet=True,
    )

    rows = [
        line.split("\t")
        for line in matrix_path.read_text().splitlines()
        if line and not line.startswith("#")
    ]
    delays = [int(row[2]) for row in rows]
    slot_edges = sum(1 for row in rows if len(row) > 7 and int(row[7]) > 0)
    log(f"  {_C.BOLD}Latency matrix{_C.RESET}    : {matrix_path}")
    log(
        f"  Edges: {len(rows)}, delay mean/max: "
        f"{sum(delays) / len(delays):.1f}/{max(delays)} ms, "
        f"slot-burst edges: {slot_edges}"
    )
    return matrix_path


def phase6_apply_latency(cfg: Config) -> subprocess.Popen[str]:
    global _latency_proc
    geo_label = "geo-high" if cfg.geodistributed else "geo-low"
    log(
        _phase_banner(
            f"Applying basic latency ({geo_label})",
            "PHASE 6",
        )
    )

    # Kill stale network-benchmark.sh from a previous run (may be owned by root).
    # The script sweeps its own lock directory at startup, so no extra cleanup here.
    run(["sudo", "pkill", "-f", r"network-benchmark\.sh"], check=False, quiet=True)

    # Avoid confusion from a stale default benchmark log; this migration run writes
    # all latency-script output into the main migration log instead.
    stale_fuzz_log = cfg.script_dir / "logs" / "fuzz_script.log"
    stale_fuzz_log.unlink(missing_ok=True)

    # Dump the effective matrix for the log; the injector below computes the
    # same role-based model natively, so no -L override is passed.
    _generate_latency_matrix(cfg)

    latency_output = cfg.log_file.open("a")

    proc = subprocess.Popen(
        [
            "sudo",
            "./network-benchmark.sh",
            "-n",
            str(cfg.num_validators),
            "-s",
            str(cfg.seed),
            "-b",
            "0",
            "-l",
            "0",
            "-r",
            "0",
            "-g",
            str(cfg.geodistributed).lower(),
            "-o",
            str(cfg.log_file.resolve()),
        ],
        cwd=cfg.script_dir,
        stdout=latency_output,
        stderr=subprocess.STDOUT,
    )
    latency_output.close()
    _latency_proc = proc

    # network-benchmark.sh emits no readiness marker, so wait a fixed window
    # for the matrix to apply and consensus to settle.
    latency_wait = cfg.latency_apply_wait
    for sec in range(latency_wait):
        if proc.poll() is not None:
            raise RuntimeError(
                f"network-benchmark.sh exited early with code {proc.returncode}"
            )
        log_status(f"  Waiting for latency application... {sec + 1}s")
        time.sleep(1)
    print()  # finish status line
    log(f"  Latency applied after {latency_wait}s wait")

    log(_phase_complete("Phase 6"))
    return proc


def measure_block_production(cfg: Config) -> None:
    if cfg.mode == "advanced":
        # The advanced schedule must fit phases 7-9 inside epoch 0; the
        # measurement window does not, so it is simple-mode only.
        log("  Block-production measurement skipped in advanced mode")
        return
    if not cfg.block_measurement_enabled():
        log("  Block-production measurement disabled")
        return

    # Shared implementation: per-validator own-block rates, block-creation
    # reasons, and block/transaction commit latencies over the window.
    ec.measure_block_production(
        cfg.num_validators, cfg.block_measurement_seconds, phase="PHASE 6B",
    )


# ========================= Phase 7: Wait Mid-Epoch =========================


def phase7_wait_mid_epoch(cfg: Config, epoch_0_start: float) -> None:
    phase_start = time.time()
    epoch_s = cfg.epoch_duration_ms // 1000
    elapsed_since_epoch_start = int(time.time() - epoch_0_start)
    required_after_phase7 = cfg.phase8_worst_case + cfg.phase9_epoch0_worst_case
    remaining_epoch = (
        epoch_s - elapsed_since_epoch_start - cfg.epoch_start_slop_seconds
    )
    if remaining_epoch < required_after_phase7:
        raise RuntimeError(
            "not enough epoch time left for migration schedule: "
            f"remaining={remaining_epoch}s, required={required_after_phase7}s "
            "(Phase 8 worst-case + Phase 9a/9b worst-case)"
        )

    wait_s = max(0, cfg.mid_epoch_wait - elapsed_since_epoch_start)
    log(_phase_banner(f"Waiting {wait_s}s before rolling upgrade", "PHASE 7"))
    log(
        f"  Epoch elapsed={elapsed_since_epoch_start}s, "
        f"reserved after wait={required_after_phase7}s, "
        f"epoch-start slop={cfg.epoch_start_slop_seconds}s, "
        f"safety={max(0, remaining_epoch - wait_s - required_after_phase7)}s"
    )

    start = time.time()
    last_log_save = start
    while time.time() < start + wait_s:
        elapsed = int(time.time() - start)
        bar = _progress_bar(elapsed, wait_s)
        log_status(f"  {bar} {elapsed}s / {wait_s}s")
        if time.time() - last_log_save >= cfg.log_interval:
            save_validator_logs(cfg, cfg.num_validators)
            last_log_save = time.time()
        time.sleep(1)

    print()  # finish status line
    log(_phase_complete("Phase 7", time.time() - phase_start))


def phase7_wait_fixed(
    cfg: Config, epoch_0_start: float, stable_window_complete_at: float
) -> None:
    """Simple mode: wait a short fixed offset into epoch 0 before rolling.

    Unlike the advanced schedule, simple mode does not reserve epoch time for
    post-upgrade restarts, so it just gives the network a brief warm-up before
    the upgrade rather than aiming for mid-epoch. No in-loop log save: each
    `docker logs` over 10 validators with multi-minute accumulated state takes
    tens of seconds and was making phase 7 overshoot its budget; the post-run
    archive captures everything we need.
    """
    phase_start = time.time()
    planned_start = epoch_0_start + cfg.pre_rolling_wait
    if stable_window_complete_at > planned_start:
        overrun = math.ceil(stable_window_complete_at - planned_start)
        raise RuntimeError(
            "pre-upgrade stable window does not fit before the planned rolling "
            f"upgrade ({overrun}s over budget). Increase --epoch-duration or "
            "reduce --num-validators/--block-measurement-seconds."
        )
    elapsed = int(time.time() - epoch_0_start)
    if elapsed > cfg.pre_rolling_wait:
        raise RuntimeError(
            "simple migration schedule missed the planned rolling-upgrade start: "
            f"elapsed={elapsed}s, planned={cfg.pre_rolling_wait}s from epoch start. "
            "Increase --epoch-duration or reduce --num-validators."
        )
    wait_s = max(0, cfg.pre_rolling_wait - elapsed)
    log(_phase_banner(f"Waiting {wait_s}s before rolling upgrade", "PHASE 7"))

    start = time.time()
    while time.time() < start + wait_s:
        e = int(time.time() - start)
        bar = _progress_bar(e, wait_s)
        log_status(f"  {bar} {e}s / {wait_s}s")
        time.sleep(1)

    print()  # finish status line
    log(_phase_complete("Phase 7", time.time() - phase_start))


# ========================= Phase 8: Rolling Upgrade =========================


def phase8_rolling_upgrade(
    cfg: Config,
    old_max_proto: str,
    old_consensus: str,
    local_branch: str,
    local_commit: str,
) -> tuple[str, str]:
    log(_phase_banner("Starting rolling upgrade", "PHASE 8"))

    upgrade_start = time.time()
    env_path = cfg.network_dir / cfg.env_migration_file
    upgrade_proto = ""
    upgrade_consensus = ""
    rng = random.Random(cfg.seed)

    for i in range(1, cfg.num_validators + 1):
        v = f"validator-{i}"
        bar = _progress_bar(i - 1, cfg.num_validators)
        log_status(f"  {bar} Upgrading {_C.BOLD}{v}{_C.RESET}...")

        # Advanced mode snapshots per-validator pre-upgrade logs for debugging
        # rolling-window scheduling. Simple mode skips it: each `docker logs`
        # over a multi-minute-old validator takes 5–10s, and the final archive
        # at phase 10 captures the same state.
        if cfg.mode == "advanced":
            with (cfg.log_dir / f"pre-upgrade-{v}.log").open("w") as fh:
                subprocess.run(
                    ["docker", "logs", v],
                    stdout=fh,
                    stderr=subprocess.STDOUT,
                    check=False,
                )

        # Append image override to env file
        with env_path.open("a") as f:
            f.write(f"VALIDATOR_{i}_IMAGE={cfg.image_upgrade}\n")

        # Stop old container, start it back on the upgrade image. Advanced mode
        # keeps each validator offline for a randomized rolling window (and paces
        # between validators); simple mode swaps back-to-back with a 1s SIGTERM
        # grace, so only one validator is briefly down at a time. RocksDB WAL
        # makes the short grace safe (replay on restart restores last state).
        stop_args = ["stop", v] if cfg.mode == "advanced" else ["stop", "-t", "1", v]
        docker_compose(cfg, stop_args, quiet=True)
        if cfg.mode == "advanced":
            restart_pause = rng.randint(
                cfg.rolling_restart_pause_min,
                cfg.rolling_restart_pause_max,
            )
            log_status(f"  {bar} {v} stopped — restarting in {restart_pause}s...")
            time.sleep(restart_pause)
        docker_compose(cfg, ["up", "-d", "--no-deps", v], quiet=True)

        if cfg.mode == "advanced":
            time.sleep(cfg.upgrade_delay)
            # `docker ps` validation is advanced-mode only because the
            # randomized restart pause introduces a non-trivial window where
            # the container could legitimately fail to come up. Simple mode
            # trusts `docker compose up`'s exit code (raised on failure).
            result = run(
                ["docker", "ps", "--format", "{{.Names}}"],
                capture=True,
                quiet=True,
            )
            running_names = set(result.stdout.strip().splitlines())
            if v not in running_names:
                print()  # newline before error
                raise RuntimeError(f"{v} failed to start after upgrade!")
        bar = _progress_bar(i, cfg.num_validators)
        log_status(f"  {bar} {_C.GREEN}✔{_C.RESET} {v} upgraded")

        # After first validator, extract upgrade protocol info
        if i == 1:
            print()  # finish status line
            upgrade_proto, upgrade_consensus = _probe_protocol_info(
                "validator-1", cfg.protocol_probe_wait
            )
            log(f"  {_C.BOLD}Protocol Version Comparison{_C.RESET}")
            log(
                f"  {_C.YELLOW}Old{_C.RESET}     ({cfg.release_network:>8s})            : "
                f"max_protocol={old_max_proto or 'unknown'}, "
                f"consensus={old_consensus or 'unknown'}"
            )
            log(
                f"  {_C.GREEN}Upgrade{_C.RESET} ({local_branch}@{local_commit}) : "
                f"max_protocol={upgrade_proto or 'unknown'}, "
                f"consensus={upgrade_consensus or 'unknown'}"
            )

    if cfg.mode == "simple":
        # `docker compose up -d` exits 0 even if the node crashes right after
        # start, so sweep once at the end: every upgraded validator must
        # still be running. (Advanced mode checks per validator above.)
        time.sleep(5)
        result = run(
            ["docker", "ps", "--format", "{{.Names}}"], capture=True, quiet=True
        )
        running_names = set(result.stdout.strip().splitlines())
        missing = [
            f"validator-{i}"
            for i in range(1, cfg.num_validators + 1)
            if f"validator-{i}" not in running_names
        ]
        if missing:
            print()  # newline before error
            raise RuntimeError(
                "validators not running after rolling upgrade: "
                + ", ".join(missing)
            )

    duration = time.time() - upgrade_start
    print()  # finish status line
    log(_phase_complete("Phase 8", duration))
    return upgrade_proto, upgrade_consensus


# ========================= Phase 9: Post-Upgrade Restarts =========================


def restart_validators(cfg: Config, *, wipe_db: bool, epoch: int) -> None:
    """Restart selected validators, optionally wiping their DB."""
    mode = "wipe DB" if wipe_db else "keep DB"
    selected = _pick_restart_validators(cfg.num_validators, epoch)
    names = [f"validator-{i}" for i in selected]
    log(
        f"  Restart set for epoch {epoch}: {selected} "
        f"({len(selected)}/{cfg.num_validators}, ceil(n/3)-1)"
    )
    if not selected:
        log(f"  {_C.YELLOW}No validators selected for {mode} restart; skipping.{_C.RESET}")
        return

    def stop_selected(label: str) -> None:
        log_status(f"  Stopping {len(names)} validators ({label})...")
        with ThreadPoolExecutor(max_workers=len(names)) as pool:
            list(pool.map(lambda v: run(["docker", "stop", v], check=False, quiet=True), names))

    def wipe_selected_dbs() -> None:
        for idx in selected:
            base_path = cfg.network_dir / "data" / f"validator-{idx}"
            if base_path.exists():
                subprocess.run(["sudo", "rm", "-rf", str(base_path)], check=True)
                subprocess.run(["sudo", "mkdir", "-p", str(base_path)], check=True)
                subprocess.run(["sudo", "chown", "-R", f"{os.getuid()}:{os.getgid()}", str(base_path)], check=True)

    def start_selected(*, force_recreate: bool, label: str) -> None:
        log_status(f"  Starting {len(names)} validators ({label})...")

        def start_one(idx: int) -> None:
            if force_recreate:
                docker_compose(
                    cfg, ["up", "-d", "--no-deps", "--force-recreate", f"validator-{idx}"],
                    quiet=True,
                )
            else:
                run(["docker", "start", f"validator-{idx}"], check=False, quiet=True)

        with ThreadPoolExecutor(max_workers=len(selected)) as pool:
            list(pool.map(start_one, selected))

    def wait_and_report(label: str, wait: int) -> None:
        for sec in range(wait, 0, -1):
            log_status(f"  Waiting for validators to start... {sec}s")
            time.sleep(1)

        print()  # finish status line
        result = run(["docker", "ps", "--format", "{{.Names}}"], capture=True, quiet=True)
        running_names = set(result.stdout.strip().splitlines())
        failed = []
        for idx in selected:
            v = f"validator-{idx}"
            if v in running_names:
                log(f"  {_C.GREEN}✔{_C.RESET} {v} restarted ({label}, epoch {epoch})")
            else:
                log(f"  {_C.RED}✘{_C.RESET} {v} failed to start after {label} restart!")
                failed.append(v)
        if failed:
            raise RuntimeError(f"Validators failed to restart: {failed}")

    stop_selected(mode)
    if wipe_db:
        wipe_selected_dbs()
        start_selected(force_recreate=True, label="fresh DB")
        rng = random.Random(cfg.seed + cfg.num_validators * 1_000_003 + epoch * 97_531)
        fresh_db_restart_pause = rng.randint(
            cfg.fresh_db_restart_pause_min,
            cfg.fresh_db_restart_pause_max,
        )
        log(f"  Waiting {fresh_db_restart_pause}s before fresh DB follow-up restart...")
        wait_and_report("fresh DB", fresh_db_restart_pause)

        # Exercise fast-sync restart behavior after the fresh DB has been created.
        stop_selected("fresh DB follow-up, keep DB")
        start_selected(force_recreate=False, label="fresh DB follow-up, keep DB")
        wait_and_report("fresh DB follow-up, keep DB", cfg.restart_settle_wait)
        return

    start_selected(force_recreate=False, label=mode)
    wait_and_report(mode, cfg.restart_settle_wait)


def phase9_post_upgrade_restarts(
    cfg: Config,
    epoch_0_start: float,
    old_max_proto: str,
    old_consensus: str,
    upgrade_label: str,
    upgrade_proto: str,
    upgrade_consensus: str,
) -> int:
    # --- 9a: Epoch 0 — restart with DB intact ---
    epoch_0 = get_current_epoch_or_raise()
    phase_start = time.time()
    log(_phase_banner(f"Epoch {epoch_0} — restart with DB intact", "PHASE 9a"))
    log(f"  Waiting {cfg.restart_pause_keep_db}s before restart...")
    _countdown(cfg.restart_pause_keep_db)
    restart_validators(cfg, wipe_db=False, epoch=epoch_0)
    log(_phase_complete("Phase 9a", time.time() - phase_start))

    # --- 9b: Epoch 0 — restart with DB wipe ---
    phase_start = time.time()
    log(_phase_banner(f"Epoch {epoch_0} — restart with DB wipe", "PHASE 9b"))
    log(f"  Waiting {cfg.restart_pause_wipe_db}s before restart...")
    _countdown(cfg.restart_pause_wipe_db)
    epoch_0_wipe_time = time.time()
    wipe_offset = int(epoch_0_wipe_time - epoch_0_start)
    restart_validators(cfg, wipe_db=True, epoch=epoch_0)
    log(_phase_complete("Phase 9b", time.time() - phase_start))

    # --- Wait for epoch change ---
    log(f"  Waiting for epoch to advance past {epoch_0}...")
    epoch_1 = wait_for_epoch_change(cfg, epoch_0)
    if epoch_1 <= epoch_0:
        raise RuntimeError(
            f"Epoch did not advance past {epoch_0}; aborting epoch 1 restart checks"
        )
    epoch_1_start = time.time()
    log(f"  Epoch advanced to {epoch_1}")
    log("  Collecting validator-1 protocol/consensus info for the new epoch...")
    time.sleep(cfg.protocol_probe_wait)
    epoch_1_proto, epoch_1_consensus = _read_validator_protocol_info("validator-1", last=True)
    log(f"  {_C.BOLD}Protocol Version Comparison (Epoch {epoch_1}){_C.RESET}")
    log(
        f"  {_C.YELLOW}Old{_C.RESET}     ({cfg.release_network:>8s})            : "
        f"max_protocol={old_max_proto or 'unknown'}, "
        f"consensus={old_consensus or 'unknown'}"
    )
    log(
        f"  {_C.GREEN}Upgrade{_C.RESET} ({upgrade_label}) : "
        f"max_protocol={upgrade_proto or 'unknown'}, "
        f"consensus={upgrade_consensus or 'unknown'}"
    )
    log(
        f"  {_C.CYAN}Epoch {epoch_1}{_C.RESET} (validator-1 latest)  : "
        f"max_protocol={epoch_1_proto or 'unknown'}, "
        f"consensus={epoch_1_consensus or 'unknown'}"
    )

    # --- 9c: Epoch 1 — restart with DB intact ---
    phase_start = time.time()
    log(_phase_banner(f"Epoch {epoch_1} — restart with DB intact", "PHASE 9c"))
    log(f"  Waiting {cfg.restart_pause_keep_db}s before restart...")
    _countdown(cfg.restart_pause_keep_db)
    restart_validators(cfg, wipe_db=False, epoch=epoch_1)
    log(_phase_complete("Phase 9c", time.time() - phase_start))

    # --- 9d: Epoch 1 — restart with DB wipe (aligned to epoch 0 offset) ---
    phase_start = time.time()
    log(_phase_banner(f"Epoch {epoch_1} — restart with DB wipe", "PHASE 9d"))
    elapsed_since_epoch1 = int(time.time() - epoch_1_start)
    sleep_for_wipe = wipe_offset - elapsed_since_epoch1
    if sleep_for_wipe > 0:
        log(f"  Aligning wipe to epoch offset {wipe_offset}s (waiting {sleep_for_wipe}s)...")
        _countdown(sleep_for_wipe)

    restart_validators(cfg, wipe_db=True, epoch=epoch_1)
    log(_phase_complete("Phase 9d", time.time() - phase_start))
    return epoch_1


# ========================= Phase 10: Observation =========================


def phase10_observe_stable_window(cfg: Config, epoch_0_at_phase8_end: int) -> int:
    """Simple-mode post-upgrade observation.

    Waits for the next epoch to start (proves the upgrade vote landed), then
    sleeps `stable_window_settle_seconds + stable_window_seconds` so the
    end-of-run report has a clean stable window in the post-upgrade epoch that
    matches the pre-rolling window. The precise next-epoch start timestamp is
    read from the CheckpointMonitor's higher-resolution polling, not from here.
    """
    phase_start = time.time()
    total_wait = cfg.stable_window_settle_seconds + cfg.stable_window_seconds
    log(
        _phase_banner(
            f"Waiting for epoch > {epoch_0_at_phase8_end}, then {total_wait}s "
            f"of stable epoch-1 observation",
            "PHASE 10",
        )
    )

    epoch_1 = wait_for_epoch_change(cfg, epoch_0_at_phase8_end)
    if epoch_1 <= epoch_0_at_phase8_end:
        raise RuntimeError(
            f"Epoch did not advance past {epoch_0_at_phase8_end}; "
            "aborting final observation"
        )
    log(
        f"  Epoch advanced to {epoch_1}; observing {total_wait}s "
        f"({cfg.stable_window_settle_seconds}s settle + "
        f"{cfg.stable_window_seconds}s window)"
    )

    obs_start = time.time()
    last_log_save = obs_start
    while time.time() < obs_start + total_wait:
        elapsed = int(time.time() - obs_start)
        bar = _progress_bar(elapsed, total_wait)
        log_status(f"  {bar} {elapsed}s / {total_wait}s")
        if time.time() - last_log_save >= cfg.log_interval:
            save_validator_logs(cfg, cfg.num_validators)
            last_log_save = time.time()
        time.sleep(1)
    print()

    _archive_final_logs(cfg)
    log(_phase_complete("Phase 10", time.time() - phase_start))
    log(f"\n{_C.GREEN}{_C.BOLD}All phases completed. Cleanup will run on script exit.{_C.RESET}")
    return epoch_1


def phase10_observation(cfg: Config, epoch_1: int) -> None:
    phase_start = time.time()
    log(
        _phase_banner(
            f"Waiting for epoch > {epoch_1}, then observing {cfg.final_epoch_settle_wait}s",
            "PHASE 10",
        )
    )

    epoch_2 = wait_for_epoch_change(cfg, epoch_1)
    if epoch_2 <= epoch_1:
        raise RuntimeError(
            f"Epoch did not advance past {epoch_1}; aborting final observation"
        )
    log(f"  Epoch advanced to {epoch_2}; final observation for {cfg.final_epoch_settle_wait}s")

    obs_start = time.time()
    last_log_save = obs_start

    while time.time() < obs_start + cfg.final_epoch_settle_wait:
        elapsed = int(time.time() - obs_start)
        bar = _progress_bar(elapsed, cfg.final_epoch_settle_wait)
        log_status(f"  {bar} {elapsed}s / {cfg.final_epoch_settle_wait}s")
        if time.time() - last_log_save >= cfg.log_interval:
            save_validator_logs(cfg, cfg.num_validators)
            last_log_save = time.time()
        time.sleep(1)

    print()  # finish status line
    _archive_final_logs(cfg)
    log(_phase_complete("Phase 10", time.time() - phase_start))
    log(f"\n{_C.GREEN}{_C.BOLD}All phases completed. Cleanup will run on script exit.{_C.RESET}")


def _archive_final_logs(cfg: Config) -> None:
    ts = datetime.now().strftime("%Y%m%d-%H%M%S")
    for i in range(1, cfg.num_validators + 1):
        v = f"validator-{i}"
        dest = cfg.log_dir / f"migration-{v}-{ts}.log"
        with dest.open("w") as fh:
            subprocess.run(
                ["docker", "logs", v],
                stdout=fh,
                stderr=subprocess.STDOUT,
                check=False,
            )
        shutil.copy2(dest, cfg.log_dir / f"migration-{v}-latest.log")

    if cfg.load_qps > 0:
        dest = cfg.log_dir / f"migration-fullnode-1-{ts}.log"
        with dest.open("w") as fh:
            subprocess.run(
                ["docker", "logs", "fullnode-1"],
                stdout=fh,
                stderr=subprocess.STDOUT,
                check=False,
            )
        shutil.copy2(dest, cfg.log_dir / "migration-fullnode-1-latest.log")

    shutil.copy2(cfg.log_file, cfg.log_dir / f"migration_script_{ts}.log")


# ========================= Main =========================


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Rolling migration test for IOTA validators.",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog=(
            "Defaults: simple mode (--mode advanced for the full restart "
            "schedule), testnet release image, 10 validators (-n to change), "
            "10min epoch (-e to change), geodistributed latency, and a "
            "rolling upgrade scheduled to finish before the epoch boundary."
        ),
    )
    parser.add_argument(
        "-r",
        "--release-network",
        default="testnet",
        choices=("devnet", "testnet", "mainnet", "alphanet"),
        help="Release network to pull the old image from Docker Hub (default: testnet)",
    )
    parser.add_argument(
        "--mode",
        default="simple",
        choices=("simple", "advanced"),
        help=(
            "simple: fast back-to-back rolling upgrade after a short fixed "
            "warm-up, no post-upgrade restarts (default). advanced: full "
            "schedule with a mid-epoch wait, rolling offline windows, and "
            "keep-DB/wipe-DB restart torture across two epochs."
        ),
    )
    parser.add_argument(
        "-b",
        "--build",
        default=True,
        type=lambda v: v.lower() in ("true", "1", "yes"),
        help="Whether to build the local upgrade image (default: true)",
    )
    parser.add_argument(
        "-n",
        "--num-validators",
        default=10,
        type=int,
        choices=range(ec.MIN_VALIDATORS, ec.MAX_VALIDATORS + 1),
        metavar="N",
        help="Number of validators to run (2-30, default: 10)",
    )
    parser.add_argument(
        "-c",
        "--chain-override",
        default="",
        choices=("", "testnet", "mainnet"),
        help=(
            "Chain override for protocol feature flags. Default: empty, which "
            "inherits from --release-network (testnet/mainnet set the matching "
            "override; devnet/alphanet = none). Controls which features are "
            "enabled at each protocol version."
        ),
    )
    parser.add_argument(
        "-e",
        "--epoch-duration",
        default=10,
        type=int,
        metavar="MINUTES",
        help="Epoch duration in minutes (default: 10)",
    )
    parser.add_argument(
        "--geodistributed",
        default=True,
        type=lambda v: v.lower() in ("true", "1", "yes"),
        help="Use large geodistributed latencies (default: true)",
    )
    parser.add_argument(
        "--load-qps",
        default=0,
        type=int,
        metavar="QPS",
        help="Start stress load generator at target QPS (default: 0 = disabled)",
    )
    parser.add_argument(
        "--load-in-flight-ratio",
        default=5,
        type=int,
        help="Stress load in-flight ratio (default: 5)",
    )
    parser.add_argument(
        "--load-transfer-objects",
        default=100,
        type=int,
        help="Stress load --transfer-object value (default: 100)",
    )
    parser.add_argument(
        "--load-rpc-address",
        default="http://fullnode-1:9000",
        help="RPC address used by stress load generator (default: http://fullnode-1:9000)",
    )
    parser.add_argument(
        "--load-tools-image",
        default="iotaledger/stress",
        help="Docker image containing /usr/local/bin/stress (default: iotaledger/stress)",
    )
    parser.add_argument(
        "--block-measurement-seconds",
        "--block-validation-seconds",
        dest="block_measurement_seconds",
        default=120,
        type=int,
        help=(
            "Seconds to measure pre-upgrade block production after latency "
            "is applied (0 disables, default: 120)"
        ),
    )
    return parser.parse_args()


def main() -> None:
    global _cfg

    args = parse_args()

    # Single-run guard: a concurrent benchmark/fuzz/migration run shares
    # container names and tc/iptables state — its cleanup would tear this
    # run's network down mid-flight.
    try:
        ec.acquire_single_run_lock("run-migration-test.py")
    except RuntimeError as err:
        print(f"ERROR: {err}")
        sys.exit(1)

    # Cache sudo credentials first so the password prompt is immediately visible
    print("Caching sudo credentials (you may be prompted for your password)...")
    subprocess.run(["sudo", "-v"], check=True)

    # Keep sudo alive in the background (refreshes every 4 minutes)
    def _sudo_keepalive() -> None:
        while True:
            time.sleep(240)
            subprocess.run(["sudo", "-vn"], check=False, capture_output=True)

    threading.Thread(target=_sudo_keepalive, daemon=True).start()

    try:
        cfg = Config(
            release_network=args.release_network,
            mode=args.mode,
            build=args.build,
            chain_override=args.chain_override,
            num_validators=args.num_validators,
            geodistributed=args.geodistributed,
            load_qps=args.load_qps,
            load_in_flight_ratio=args.load_in_flight_ratio,
            load_transfer_objects=args.load_transfer_objects,
            load_rpc_address=args.load_rpc_address,
            load_tools_image=args.load_tools_image,
            block_measurement_seconds=args.block_measurement_seconds,
            epoch_duration_ms=args.epoch_duration * 60_000,
        )
    except ValueError as err:
        print(f"Configuration error: {err}", file=sys.stderr)
        sys.exit(2)
    _cfg = cfg

    # Ensure correct directory
    if cfg.script_dir.name != "experiments":
        log("Error: run from experiments/")
        sys.exit(1)

    # Setup logging (truncate, then O_APPEND — see experiment_common).
    ec.setup_logging(cfg.log_file)

    # Register cleanup
    atexit.register(cleanup)
    signal.signal(signal.SIGINT, _signal_handler)
    signal.signal(signal.SIGTERM, _signal_handler)

    # Summary
    log(_phase_banner("Migration Test Configuration"))
    log(f"  {_C.BOLD}Mode{_C.RESET}                 : {cfg.mode}")
    log(f"  {_C.BOLD}Validators{_C.RESET}           : {cfg.num_validators}")
    log(f"  {_C.BOLD}Consensus protocol{_C.RESET}   : auto-detected from protocol config")
    log(f"  {_C.BOLD}Epoch duration{_C.RESET}       : {cfg.epoch_duration_ms}ms ({cfg.epoch_duration_ms // 60_000} min)")
    log(f"  {_C.BOLD}Release network{_C.RESET}      : {cfg.release_network}")
    log(f"  {_C.BOLD}Chain override{_C.RESET}       : {cfg.chain_override or 'none (devnet-like)'}")
    log(f"  {_C.BOLD}Build local image{_C.RESET}    : {cfg.build}")
    log(
        f"  {_C.BOLD}Latency model{_C.RESET}        : "
        "role-based, built into network-benchmark.sh"
    )
    if cfg.load_qps > 0:
        log(
            f"  {_C.BOLD}Load generator{_C.RESET}       : "
            f"{cfg.load_qps} qps, in-flight ratio {cfg.load_in_flight_ratio}, "
            f"transfer-object {cfg.load_transfer_objects}, rpc {cfg.load_rpc_address}"
        )
    else:
        log(f"  {_C.BOLD}Load generator{_C.RESET}       : disabled")
    log(f"  {_C.BOLD}Protocol probe wait{_C.RESET}  : {cfg.protocol_probe_wait}s")
    if cfg.mode == "advanced":
        log(f"  {_C.BOLD}Rolling start offset{_C.RESET} : <= {cfg.mid_epoch_wait}s from epoch start")
        log(f"  {_C.BOLD}Next-validator pause{_C.RESET} : {cfg.upgrade_delay}s")
        log(
            f"  {_C.BOLD}Epoch-0 schedule cap{_C.RESET} : "
            f"phase8 <= {cfg.phase8_worst_case}s, "
            f"phase9a/9b <= {cfg.phase9_epoch0_worst_case}s, "
            f"safety {cfg.timeline_safety_margin}s, "
            f"epoch-start slop {cfg.epoch_start_slop_seconds}s"
        )
        log(
            f"  {_C.BOLD}Rolling offline pause{_C.RESET}: "
            f"{cfg.rolling_restart_pause_min}-{cfg.rolling_restart_pause_max}s per validator"
        )
        log(
            f"  {_C.BOLD}Restart validators{_C.RESET}   : "
            f"{_restart_validator_count(cfg.num_validators)} per epoch "
            f"(ceil(n/3)-1, deterministic by epoch)"
        )
        log(f"    keep-DB after {cfg.restart_pause_keep_db}s, wipe-DB after {cfg.restart_pause_wipe_db}s")
        log(f"    restart settle wait {cfg.restart_settle_wait}s")
        log(
            f"    fresh DB follow-up restart pause "
            f"{cfg.fresh_db_restart_pause_min}-{cfg.fresh_db_restart_pause_max}s"
        )
        log(f"    epoch 1 wipe-DB aligned to same offset as epoch 0")
        log(
            f"  {_C.BOLD}Stop condition{_C.RESET}       : "
            f"second epoch observed + {cfg.final_epoch_settle_wait}s"
        )
    else:
        log(
            f"  {_C.BOLD}Rolling upgrade{_C.RESET}      : "
            f"back-to-back, no offline pause, no post-upgrade restarts"
        )
        log(
            f"  {_C.BOLD}Pre-rolling wait{_C.RESET}     : {cfg.pre_rolling_wait}s from epoch start "
            f"(phase8 estimate {cfg.phase8_simple_estimate}s, "
            f"safety {cfg.timeline_safety_margin}s, "
            f"epoch-start slop {cfg.epoch_start_slop_seconds}s)"
        )
        log(
            f"  {_C.BOLD}Stop condition{_C.RESET}       : "
            f"one epoch boundary after upgrade + "
            f"{cfg.stable_window_settle_seconds + cfg.stable_window_seconds}s"
        )

    # Run all phases
    local_branch, local_commit = phase1_docker_images(cfg)
    if cfg.load_qps > 0:
        # Resolve the load image up front (pull, else build from the
        # network-benchmark clone) instead of surprising phase 6b mid-run.
        ec.ensure_stress_image(cfg.load_tools_image)
    phase2_generate_compose(cfg)
    phase3_bootstrap_genesis(cfg)
    old_max_proto, old_consensus, epoch_0_start = phase4_start_validators(cfg)
    phase5_start_monitoring(cfg)
    cp_monitor = CheckpointMonitor(interval=10)
    cp_monitor.start()
    latency_proc = phase6_apply_latency(cfg)
    start_load_generator(cfg)
    measure_block_production(cfg)
    pre_upgrade_ready_ts = time.time()

    if cfg.mode == "advanced":
        phase7_wait_mid_epoch(cfg, epoch_0_start)
        simple_upgrade_epoch = None
    else:
        stable_window_complete_at = (
            pre_upgrade_ready_ts
            + cfg.stable_window_settle_seconds
            + cfg.stable_window_seconds
        )
        phase7_wait_fixed(cfg, epoch_0_start, stable_window_complete_at)
        simple_upgrade_epoch = get_current_epoch_or_raise()
    upgrade_proto, upgrade_consensus = phase8_rolling_upgrade(
        cfg, old_max_proto, old_consensus, local_branch, local_commit
    )
    if cfg.mode == "advanced":
        epoch_1 = phase9_post_upgrade_restarts(
            cfg,
            epoch_0_start,
            old_max_proto,
            old_consensus,
            f"{local_branch}@{local_commit}",
            upgrade_proto,
            upgrade_consensus,
        )
        phase10_observation(cfg, epoch_1)
        simple_observed_epoch = None
    else:
        epoch_after_upgrade = get_current_epoch_or_raise()
        if (
            simple_upgrade_epoch is not None
            and epoch_after_upgrade != simple_upgrade_epoch
        ):
            raise RuntimeError(
                "simple rolling upgrade crossed an epoch boundary: "
                f"started in epoch {simple_upgrade_epoch}, ended in epoch "
                f"{epoch_after_upgrade}. Increase --epoch-duration or reduce "
                "--num-validators."
            )
        # Simple mode: no post-upgrade restarts. Wait for the next epoch to
        # start and then hold long enough for the stable-window comparison.
        simple_observed_epoch = phase10_observe_stable_window(
            cfg, epoch_after_upgrade
        )
    stop_load_generator(cfg)

    cp_monitor.stop()
    log(_phase_banner("Checkpoint Liveness Report"))
    for line in cp_monitor.report().split("\n"):
        log(line)
    if cfg.mode == "simple" and simple_observed_epoch is not None:
        # Pull the precise post-upgrade epoch start from the monitor's
        # 10s-resolution polling rather than from the 30s wait_for_epoch_change
        # loop.
        epoch_1_start_ts = next(
            (
                ts
                for ts, _, to_ep, _ in cp_monitor._observed_epoch_changes()
                if to_ep == simple_observed_epoch
            ),
            None,
        )
        if epoch_1_start_ts is None:
            raise RuntimeError(
                "checkpoint monitor did not observe the post-upgrade epoch "
                "transition; cannot produce the required stable-window comparison"
            )
        log(_phase_banner("Stable-Window Comparison"))
        for line in cp_monitor.stable_window_report(
            cfg, pre_upgrade_ready_ts, epoch_1_start_ts
        ).split("\n"):
            log(line)

    # Kill latency background process (runs under sudo, so use sudo pkill)
    run(["sudo", "pkill", "-f", r"network-benchmark\.sh"], check=False, quiet=True)
    if latency_proc.poll() is None:
        latency_proc.terminate()

    # Re-snapshot the script log so the timestamped archive includes the
    # liveness and stable-window reports logged after _archive_final_logs.
    ts = datetime.now().strftime("%Y%m%d-%H%M%S")
    shutil.copy2(cfg.log_file, cfg.log_dir / f"migration_script_{ts}.log")


if __name__ == "__main__":
    main()
