#!/usr/bin/env python3

# Copyright (c) 2026 IOTA Stiftung
# SPDX-License-Identifier: Apache-2.0

"""Rolling migration test: start validators on a release image, then
perform a mid-epoch rolling upgrade to a locally-built image.

Run from: iota/dev-tools/iota-private-network/experiments/
"""

from __future__ import annotations

import argparse
import atexit
import json
import os
import random
import re
import selectors
import shutil
import signal
import subprocess
import sys
import threading
import time
import urllib.parse
import urllib.request
from concurrent.futures import ThreadPoolExecutor
from dataclasses import dataclass, field
from datetime import datetime, timezone
from pathlib import Path


# ========================= Configuration =========================


@dataclass
class Config:
    """All parameters for the migration test."""

    # --- Hardcoded (fixed for every run) ---
    num_validators: int = 20
    epoch_duration_ms: int = 900_000  # 15 minutes
    seed: int = 42
    geodistributed: bool = True
    log_interval: int = 60  # save logs every N seconds
    final_epoch_settle_wait: int = 10  # seconds after the second post-start epoch

    # Derived from epoch_duration_ms (set in __post_init__)
    mid_epoch_wait: int = field(init=False)
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
    phase9_epoch0_worst_case: int = field(init=False)
    timeline_safety_margin: int = field(init=False)

    image_old: str = "iota-node:old"
    image_upgrade: str = "iota-node:upgrade"
    compose_file: str = "docker-compose.migration.yaml"
    env_migration_file: str = ".env.migration"
    grafana_override_file: str = "docker-compose.migration-override.yaml"

    # --- CLI tunables ---
    release_network: str = "devnet"
    build: bool = True
    chain_override: str = ""  # empty = Chain::Unknown (devnet-like)
    load_qps: int = 0
    load_in_flight_ratio: int = 5
    load_transfer_objects: int = 100
    load_rpc_address: str = "http://fullnode-1:9000"
    load_tools_image: str = "iotaledger/iota-tools"
    load_primary_gas_owner_id: str = (
        "0x7cc6ff19b379d305b8363d9549269e388b8c1515772253ed4c868ee80b149ca0"
    )

    # --- Derived paths (set in __post_init__) ---
    script_dir: Path = field(default_factory=lambda: Path(__file__).resolve().parent)
    network_dir: Path = field(init=False)
    repo_root: Path = field(init=False)
    grafana_dir: Path = field(init=False)
    log_dir: Path = field(init=False)
    log_file: Path = field(init=False)

    def __post_init__(self) -> None:
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
        self.protocol_probe_wait = min(15, max(1, self.rolling_restart_pause_max // 2))
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
        )
        if self.mid_epoch_wait < 0:
            required = (
                self.phase8_worst_case
                + self.phase9_epoch0_worst_case
                + self.timeline_safety_margin
            )
            raise ValueError(
                "epoch duration is too short for the derived migration schedule: "
                f"need at least {required}s for {self.num_validators} validators, "
                f"got {epoch_s}s"
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


def _find_repo_root(start: Path) -> Path:
    try:
        out = subprocess.check_output(
            ["git", "rev-parse", "--show-toplevel"],
            cwd=start,
            text=True,
            stderr=subprocess.DEVNULL,
        )
        return Path(out.strip())
    except (subprocess.CalledProcessError, FileNotFoundError):
        return start.parent.parent.parent


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
_log_fh = None  # file handle for log file
_cleaning = False
_latency_proc: subprocess.Popen[str] | None = None
_load_logs_proc: subprocess.Popen[str] | None = None
_load_log_archived = False
_ANSI_RE = re.compile(r"\x1b\[[0-?]*[ -/]*[@-~]")


# ========================= Colors / Formatting =========================


class _C:
    """ANSI color codes, disabled when not writing to a terminal."""

    RESET = "\033[0m"
    BOLD = "\033[1m"
    DIM = "\033[2m"
    RED = "\033[31m"
    GREEN = "\033[32m"
    YELLOW = "\033[33m"
    BLUE = "\033[34m"
    MAGENTA = "\033[35m"
    CYAN = "\033[36m"
    WHITE = "\033[37m"

    @classmethod
    def disable(cls) -> None:
        for attr in ("RESET", "BOLD", "DIM", "RED", "GREEN", "YELLOW",
                      "BLUE", "MAGENTA", "CYAN", "WHITE"):
            setattr(cls, attr, "")


if not sys.stdout.isatty():
    _C.disable()


def _phase_banner(title: str, phase: str = "") -> str:
    """Return a decorated phase header."""
    c = _C
    label = f"{phase}: " if phase else ""
    return f"\n{c.BOLD}{c.CYAN}▶ {label}{title}{c.RESET}"


def _phase_complete(phase: str, duration: float | None = None) -> str:
    c = _C
    dur = f" ({int(duration)}s)" if duration is not None else ""
    return f"{c.GREEN}✔ {phase} complete{dur}{c.RESET}"


def _progress_bar(current: int, total: int, width: int = 30) -> str:
    frac = min(current / total, 1.0) if total else 0
    filled = int(width * frac)
    bar = "█" * filled + "░" * (width - filled)
    pct = int(frac * 100)
    return f"[{bar}] {pct:3d}%"


def _countdown(seconds: int) -> None:
    """Sleep for *seconds* with a live progress bar."""
    start = time.time()
    while time.time() < start + seconds:
        elapsed = int(time.time() - start)
        bar = _progress_bar(elapsed, seconds)
        log_status(f"  {bar} {elapsed}s / {seconds}s")
        time.sleep(1)
    print()  # finish status line


# ========================= Helpers =========================


def log(msg: str) -> None:
    ts = datetime.now(timezone.utc).strftime("%H:%M:%S")
    plain_msg = _ANSI_RE.sub("", msg).replace("\r", "")
    colored = f"{_C.DIM}{ts}{_C.RESET} {msg}"
    print(f"\r\033[K{colored}", flush=True)
    if _log_fh is not None:
        timestamp = datetime.now(timezone.utc).isoformat()
        for line in plain_msg.split("\n"):
            _log_fh.write(f"{timestamp} {line}\n")
        _log_fh.flush()


def log_status(msg: str) -> None:
    """Overwrite the current terminal line with a status message (no newline).

    The message is still written to the log file normally.
    """
    ts = datetime.now(timezone.utc).strftime("%H:%M:%S")
    plain_msg = _ANSI_RE.sub("", msg).replace("\r", "")
    colored = f"{_C.DIM}{ts}{_C.RESET} {msg}"
    print(f"\r\033[K{colored}", end="", flush=True)
    if _log_fh is not None:
        timestamp = datetime.now(timezone.utc).isoformat()
        for line in plain_msg.split("\n"):
            _log_fh.write(f"{timestamp} {line}\n")
        _log_fh.flush()


def run_timed(
    cmd: list[str],
    label: str,
    *,
    cwd: Path | None = None,
    env: dict[str, str] | None = None,
    check: bool = True,
) -> subprocess.CompletedProcess[str]:
    """Run a command quietly, showing *label* with a live elapsed timer.

    On success the timer line is overwritten by the next output.
    On failure the full buffered output is dumped.
    """
    start = time.time()

    proc = subprocess.Popen(
        cmd,
        cwd=cwd,
        env=env,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        bufsize=1,
    )
    output_lines: list[str] = []

    # Log the command to the file
    if _log_fh is not None:
        timestamp = datetime.now(timezone.utc).isoformat()
        _log_fh.write(f"{timestamp}   $ {' '.join(cmd)}\n")
        _log_fh.flush()

    assert proc.stdout is not None
    sel = selectors.DefaultSelector()
    sel.register(proc.stdout, selectors.EVENT_READ)

    while proc.poll() is None:
        elapsed = int(time.time() - start)
        log_status(f"  {label}... {_C.DIM}{elapsed}s{_C.RESET}")
        ready = sel.select(timeout=1.0)
        if ready:
            raw_line = proc.stdout.readline()
            if raw_line:
                clean = _ANSI_RE.sub("", raw_line).replace("\r", "\n")
                for line in clean.splitlines():
                    output_lines.append(line)
                    if _log_fh is not None:
                        _log_fh.write(f"{datetime.now(timezone.utc).isoformat()}     {line}\n")

    # Drain remaining output
    for raw_line in proc.stdout:
        clean = _ANSI_RE.sub("", raw_line).replace("\r", "\n")
        for line in clean.splitlines():
            output_lines.append(line)
            if _log_fh is not None:
                _log_fh.write(f"{datetime.now(timezone.utc).isoformat()}     {line}\n")
    if _log_fh is not None:
        _log_fh.flush()

    sel.close()
    returncode = proc.wait()
    elapsed = int(time.time() - start)
    result = subprocess.CompletedProcess(cmd, returncode, stdout="\n".join(output_lines), stderr="")

    if check and returncode != 0:
        print()  # finish status line
        log(f"  {_C.RED}✘ {label} failed ({elapsed}s){_C.RESET}")
        for line in output_lines:
            if line:
                log(f"    {line}")
        raise subprocess.CalledProcessError(returncode, cmd, output=result.stdout)

    # Show completion on status line (will be overwritten by next log/log_status)
    log_status(f"  {label} {_C.DIM}{elapsed}s{_C.RESET}")
    return result


def run(
    cmd: list[str],
    *,
    cwd: Path | None = None,
    check: bool = True,
    capture: bool = False,
    env: dict[str, str] | None = None,
    verbose: bool = False,
    quiet: bool = False,
) -> subprocess.CompletedProcess[str]:
    """Run a subprocess with logging.

    By default output is buffered silently. On failure the full output is
    printed so the error context is visible. Pass ``verbose=True`` to stream
    every line as it arrives (useful for long-running commands where progress
    feedback matters). Pass ``quiet=True`` to also suppress the ``$ command``
    echo (the command is still written to the log file).
    """
    if quiet:
        # Write to log file only, not to terminal
        if _log_fh is not None:
            timestamp = datetime.now(timezone.utc).isoformat()
            plain = " ".join(cmd)
            _log_fh.write(f"{timestamp}   $ {plain}\n")
            _log_fh.flush()
    else:
        log(f"  $ {' '.join(cmd)}")
    if capture:
        return subprocess.run(
            cmd,
            cwd=cwd,
            check=check,
            text=True,
            capture_output=True,
            env=env,
        )

    proc = subprocess.Popen(
        cmd,
        cwd=cwd,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        bufsize=1,
        env=env,
    )
    output_lines: list[str] = []

    assert proc.stdout is not None
    for raw_line in proc.stdout:
        clean = _ANSI_RE.sub("", raw_line).replace("\r", "\n")
        for line in clean.splitlines():
            output_lines.append(line)
            if verbose and line:
                log(f"    {line}")

    returncode = proc.wait()
    result = subprocess.CompletedProcess(
        cmd,
        returncode,
        stdout="\n".join(output_lines),
        stderr="",
    )
    if check and returncode != 0:
        # Dump buffered output so the failure is diagnosable
        if not verbose:
            for line in output_lines:
                if line:
                    log(f"    {line}")
        raise subprocess.CalledProcessError(returncode, cmd, output=result.stdout)
    return result


def _prometheus_query(expr: str) -> dict[str, object] | None:
    try:
        query = urllib.parse.urlencode({"query": expr})
        with urllib.request.urlopen(
            f"http://localhost:9090/api/v1/query?{query}", timeout=5
        ) as resp:
            return json.loads(resp.read())
    except Exception:
        return None


def _prometheus_scalar(expr: str) -> str | None:
    data = _prometheus_query(expr)
    if not data:
        return None
    try:
        result = data["data"]["result"]
        if not result:
            return None
        return str(result[0]["value"][1])
    except (KeyError, IndexError, TypeError):
        return None


def get_current_epoch() -> int:
    try:
        value = _prometheus_scalar("max(current_epoch)")
        return int(value) if value is not None else 0
    except Exception:
        return 0


def wait_for_epoch_change(cfg: Config, epoch_before: int) -> int:
    """Poll until epoch advances past epoch_before. Returns new epoch."""
    log(f"  Waiting for epoch > {epoch_before}...")
    timeout = cfg.epoch_duration_ms // 1000 * 3 // 2  # 1.5x epoch duration
    start = time.time()

    while True:
        epoch_now = get_current_epoch()
        if epoch_now > epoch_before:
            print()  # finish status line
            log(f"  {_C.GREEN}Epoch advanced to {epoch_now}{_C.RESET} (was {epoch_before})")
            return epoch_now

        elapsed = int(time.time() - start)
        if elapsed >= timeout:
            print()  # finish status line
            log(f"  {_C.YELLOW}WARNING: Epoch did not advance within {timeout}s — proceeding anyway{_C.RESET}")
            return epoch_now

        bar = _progress_bar(elapsed, timeout)
        log_status(f"  Epoch wait: {bar} epoch={epoch_now}, {elapsed}s / {timeout}s")
        time.sleep(30)


class CheckpointMonitor:
    """Background checkpoint liveness monitor.

    Polls ``max(highest_synced_checkpoint)`` from Prometheus every *interval*
    seconds, records samples, and detects stalls (checkpoint not advancing).
    """

    _MS_PER_SECOND = 1000.0

    # Block commit latency (covers both metric naming conventions)
    _BLK_P50 = (
        "quantile(0.5,"
        " rate(consensus_block_commit_latency_sum[1m])"
        " / rate(consensus_block_commit_latency_count[1m])"
        " or rate(consensus_block_header_commit_latency_sum[1m])"
        " / rate(consensus_block_header_commit_latency_count[1m]))"
    )
    _BLK_P90 = (
        "histogram_quantile(0.9,"
        " sum(rate(consensus_block_commit_latency_bucket[1m])) by (le)"
        " or sum(rate(consensus_block_header_commit_latency_bucket[1m])) by (le))"
    )
    # Transaction commit latency (falls back to block latency if unavailable)
    _TXN_P50 = (
        "quantile(0.5,"
        " rate(consensus_transaction_commit_latency_sum[1m])"
        " / rate(consensus_transaction_commit_latency_count[1m])"
        " or rate(consensus_block_commit_latency_sum[1m])"
        " / rate(consensus_block_commit_latency_count[1m])"
        " or rate(consensus_block_header_commit_latency_sum[1m])"
        " / rate(consensus_block_header_commit_latency_count[1m]))"
    )
    _TXN_P90 = (
        "histogram_quantile(0.9,"
        " sum(rate(consensus_transaction_commit_latency_bucket[1m])) by (le)"
        " or sum(rate(consensus_block_commit_latency_bucket[1m])) by (le)"
        " or sum(rate(consensus_block_header_commit_latency_bucket[1m])) by (le))"
    )

    def __init__(self, interval: int = 10):
        self.interval = interval
        self._samples: list[tuple[float, int, int]] = []  # (ts, checkpoint, epoch)
        # (ts, epoch, blk_p50, blk_p90, txn_p50, txn_p90)
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
            bp90 = self._query_latency_ms(self._BLK_P90)
            tp50 = self._query_latency_ms(self._TXN_P50)
            tp90 = self._query_latency_ms(self._TXN_P90)
            if bp50 >= 0:
                self._latencies.append((
                    now, epoch, bp50, bp90, tp50, tp90,
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
            bp90 = self._median([v for _, _, _, v, _, _ in self._latencies])
            tp50 = self._median([v for _, _, _, _, v, _ in self._latencies if v >= 0])
            tp90 = self._median([v for _, _, _, _, _, v in self._latencies if v >= 0])
            lines.append(f"  Block  lat   : p50={bp50:.0f}ms  p90={bp90:.0f}ms")
            lines.append(f"  Tx lat       : p50={tp50:.0f}ms  p90={tp90:.0f}ms")
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
        for _, ep, bp50, bp90, tp50, tp90 in self._latencies:
            if ep < 0:
                continue
            lat_by_epoch.setdefault(ep, []).append((bp50, bp90, tp50, tp90))

        epoch_segments = self._epoch_segments()
        if len(epoch_segments) > 1:
            hdr = (
                f"  {'Epoch':>5}  {'Duration':>8}  {'CP rate':>8}"
                f"  {'Blk p50':>8}  {'Blk p90':>8}"
                f"  {'Tx p50':>8}  {'Tx p90':>8}"
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
                    ebp90 = f"{self._median([v for _, v, _, _ in lats]):.0f}ms"
                    etp50 = f"{self._median([v for _, _, v, _ in lats if v >= 0]):.0f}ms"
                    etp90 = f"{self._median([v for _, _, _, v in lats if v >= 0]):.0f}ms"
                else:
                    ebp50 = ebp90 = etp50 = etp90 = "-"
                lines.append(
                    f"  {ep:>5}  {dur_s:>8}  {rate_s:>8}"
                    f"  {ebp50:>8}  {ebp90:>8}"
                    f"  {etp50:>8}  {etp90:>8}"
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

    genesis_blob = cfg.network_dir / "configs" / "genesis" / "genesis.blob"
    faucet_keystore = cfg.network_dir / "configs" / "faucet" / "iota.keystore"
    load_keystore_dir = cfg.log_dir / "load-generator-keystore"
    load_keystore = load_keystore_dir / "iota.keystore"
    if not genesis_blob.exists():
        raise FileNotFoundError(f"genesis blob not found: {genesis_blob}")
    if not faucet_keystore.exists():
        raise FileNotFoundError(f"faucet keystore not found: {faucet_keystore}")
    shutil.rmtree(load_keystore_dir, ignore_errors=True)
    load_keystore_dir.mkdir(parents=True, exist_ok=True)
    shutil.copy2(faucet_keystore, load_keystore)

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

    run(["docker", "rm", "-f", "stress-benchmark"], check=False, quiet=True)
    result = run(
        [
            "docker",
            "run",
            "-d",
            "--rm",
            "--name",
            "stress-benchmark",
            "--network",
            _migration_network_name(cfg),
            "-v",
            f"{genesis_blob.resolve()}:/opt/iota/config/genesis.blob:ro",
            "-v",
            f"{load_keystore_dir.resolve()}:/opt/iota/config:rw",
            cfg.load_tools_image,
            "/usr/local/bin/stress",
            "--local",
            "false",
            "--use-fullnode-for-execution",
            "true",
            "--fullnode-rpc-addresses",
            cfg.load_rpc_address,
            "--genesis-blob-path",
            "/opt/iota/config/genesis.blob",
            "--keystore-path",
            "/opt/iota/config/iota.keystore",
            "--primary-gas-owner-id",
            cfg.load_primary_gas_owner_id,
            "bench",
            "--target-qps",
            str(cfg.load_qps),
            "--in-flight-ratio",
            str(cfg.load_in_flight_ratio),
            "--transfer-object",
            str(cfg.load_transfer_objects),
        ],
        capture=True,
        quiet=True,
    )
    container_id = result.stdout.strip()[:12] or "unknown"

    # Health check: verify the container is still running after a short startup period
    time.sleep(5)
    health = run(
        ["docker", "inspect", "-f", "{{.State.Running}}", "stress-benchmark"],
        capture=True, check=False, quiet=True,
    )
    if health.returncode != 0 or health.stdout.strip() != "true":
        fail_logs = run(
            ["docker", "logs", "--tail", "20", "stress-benchmark"],
            capture=True, check=False, quiet=True,
        )
        raise RuntimeError(
            f"Load generator exited immediately after start.\n"
            f"  Last logs:\n{fail_logs.stdout.strip()}"
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

    log(f"  Load generator container: {container_id}")
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
    global _cleaning, _log_fh
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

    lock_dir = cfg.script_dir / "logs" / "network-benchmark-locks"
    shutil.rmtree(lock_dir, ignore_errors=True)
    shutil.rmtree(cfg.log_dir / "load-generator-keystore", ignore_errors=True)

    log("Cleanup complete.")
    # Restore terminal to a sane state after subprocess output
    os.system("stty sane 2>/dev/null")
    print("\r\033[K", end="", flush=True)
    if _log_fh is not None:
        _log_fh.close()
        _log_fh = None


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

    run_timed(
        [
            "docker",
            "compose",
            "--ansi",
            "never",
            "-f",
            "docker-compose.yaml",
            "-f",
            cfg.grafana_override_file,
            "up",
            "-d",
        ],
        "Starting/reusing monitoring containers",
        cwd=cfg.grafana_dir,
    )

    print()  # finish status line
    log(f"  Grafana: {_C.CYAN}http://localhost:3000/dashboards{_C.RESET}")
    log(f"  Prometheus: {_C.CYAN}http://localhost:9090/targets{_C.RESET}")
    log(_phase_complete("Phase 5", time.time() - phase_start))


# ========================= Phase 6: Apply Latency =========================


def phase6_apply_latency(cfg: Config) -> subprocess.Popen[str]:
    global _latency_proc
    geo_label = "geo-high" if cfg.geodistributed else "geo-low"
    log(
        _phase_banner(
            f"Applying basic latency ({geo_label})",
            "PHASE 6",
        )
    )

    # Kill stale network-benchmark.sh from a previous run (may be owned by root)
    run(["sudo", "pkill", "-f", r"network-benchmark\.sh"], check=False, quiet=True)

    # Avoid confusion from a stale default benchmark log; this migration run writes
    # all latency-script output into the main migration log instead.
    stale_fuzz_log = cfg.script_dir / "logs" / "fuzz_script.log"
    stale_fuzz_log.unlink(missing_ok=True)

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

    # Wait for latency application (no readiness marker on develop version)
    latency_wait = 30
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


# ========================= Phase 7: Wait Mid-Epoch =========================


def phase7_wait_mid_epoch(cfg: Config, epoch_0_start: float) -> None:
    phase_start = time.time()
    epoch_s = cfg.epoch_duration_ms // 1000
    elapsed_since_epoch_start = int(time.time() - epoch_0_start)
    required_after_phase7 = cfg.phase8_worst_case + cfg.phase9_epoch0_worst_case
    remaining_epoch = epoch_s - elapsed_since_epoch_start
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

        # Save pre-upgrade logs
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

        # Stop old container, pause (simulating real-world upgrade delay), start new
        docker_compose(cfg, ["stop", v], quiet=True)
        restart_pause = rng.randint(
            cfg.rolling_restart_pause_min,
            cfg.rolling_restart_pause_max,
        )
        log_status(f"  {bar} {v} stopped — restarting in {restart_pause}s...")
        time.sleep(restart_pause)
        docker_compose(cfg, ["up", "-d", "--no-deps", v], quiet=True)

        time.sleep(cfg.upgrade_delay)

        result = run(
            ["docker", "ps", "--format", "{{.Names}}"],
            capture=True,
            quiet=True,
        )
        running_names = set(result.stdout.strip().splitlines())
        if v in running_names:
            bar = _progress_bar(i, cfg.num_validators)
            log_status(f"  {bar} {_C.GREEN}✔{_C.RESET} {v} upgraded")
        else:
            print()  # newline before error
            raise RuntimeError(f"{v} failed to start after upgrade!")

        # After first validator, extract upgrade protocol info
        if i == 1:
            print()  # finish status line
            time.sleep(cfg.protocol_probe_wait)
            upgrade_proto, upgrade_consensus = _read_validator_protocol_info("validator-1", last=True)
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
    epoch_0 = get_current_epoch()
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
    # Final log save with timestamp
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

    log(_phase_complete("Phase 10", time.time() - phase_start))
    log(f"\n{_C.GREEN}{_C.BOLD}All phases completed. Cleanup will run on script exit.{_C.RESET}")


# ========================= Main =========================


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Rolling migration test for IOTA validators.",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog=(
            "Defaults: 20 validators (-n to change), 15min epoch (-e to change), "
            "geodistributed latency, mid-epoch rolling upgrade."
        ),
    )
    parser.add_argument(
        "-r",
        "--release-network",
        default="devnet",
        choices=("devnet", "testnet", "mainnet", "alphanet"),
        help="Release network to pull the old image from Docker Hub (default: devnet)",
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
        default=20,
        type=int,
        choices=range(2, 101),
        metavar="N",
        help="Number of validators to run (2-100, default: 20)",
    )
    parser.add_argument(
        "-c",
        "--chain-override",
        default="",
        choices=("", "testnet", "mainnet"),
        help=(
            "Chain override for protocol feature flags (default: none = devnet-like). "
            "Controls which features are enabled at each protocol version."
        ),
    )
    parser.add_argument(
        "-e",
        "--epoch-duration",
        default=15,
        type=int,
        metavar="MINUTES",
        help="Epoch duration in minutes (default: 15)",
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
    return parser.parse_args()


def main() -> None:
    global _cfg, _log_fh

    args = parse_args()

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
            build=args.build,
            chain_override=args.chain_override,
            num_validators=args.num_validators,
            geodistributed=args.geodistributed,
            load_qps=args.load_qps,
            load_in_flight_ratio=args.load_in_flight_ratio,
            load_transfer_objects=args.load_transfer_objects,
            load_rpc_address=args.load_rpc_address,
            load_tools_image=args.load_tools_image,
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

    # Setup logging
    cfg.log_dir.mkdir(parents=True, exist_ok=True)
    _log_fh = cfg.log_file.open("w")

    # Register cleanup
    atexit.register(cleanup)
    signal.signal(signal.SIGINT, _signal_handler)
    signal.signal(signal.SIGTERM, _signal_handler)

    # Summary
    log(_phase_banner("Migration Test Configuration"))
    log(f"  {_C.BOLD}Validators{_C.RESET}           : {cfg.num_validators}")
    log(f"  {_C.BOLD}Consensus protocol{_C.RESET}   : auto-detected from protocol config")
    log(f"  {_C.BOLD}Epoch duration{_C.RESET}       : {cfg.epoch_duration_ms}ms ({cfg.epoch_duration_ms // 60_000} min)")
    log(f"  {_C.BOLD}Release network{_C.RESET}      : {cfg.release_network}")
    log(f"  {_C.BOLD}Chain override{_C.RESET}       : {cfg.chain_override or 'none (devnet-like)'}")
    log(f"  {_C.BOLD}Build local image{_C.RESET}    : {cfg.build}")
    log(f"  {_C.BOLD}Geodistributed{_C.RESET}      : {cfg.geodistributed}")
    if cfg.load_qps > 0:
        log(
            f"  {_C.BOLD}Load generator{_C.RESET}      : "
            f"{cfg.load_qps} qps, in-flight ratio {cfg.load_in_flight_ratio}, "
            f"transfer-object {cfg.load_transfer_objects}, rpc {cfg.load_rpc_address}"
        )
    else:
        log(f"  {_C.BOLD}Load generator{_C.RESET}      : disabled")
    log(f"  {_C.BOLD}Rolling start offset{_C.RESET}: <= {cfg.mid_epoch_wait}s from epoch start")
    log(f"  {_C.BOLD}Next-validator pause{_C.RESET} : {cfg.upgrade_delay}s")
    log(f"  {_C.BOLD}Protocol probe wait{_C.RESET}  : {cfg.protocol_probe_wait}s")
    log(
        f"  {_C.BOLD}Epoch-0 schedule cap{_C.RESET} : "
        f"phase8 <= {cfg.phase8_worst_case}s, "
        f"phase9a/9b <= {cfg.phase9_epoch0_worst_case}s, "
        f"safety {cfg.timeline_safety_margin}s"
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

    # Run all phases
    local_branch, local_commit = phase1_docker_images(cfg)
    phase2_generate_compose(cfg)
    phase3_bootstrap_genesis(cfg)
    old_max_proto, old_consensus, epoch_0_start = phase4_start_validators(cfg)
    phase5_start_monitoring(cfg)
    cp_monitor = CheckpointMonitor(interval=10)
    cp_monitor.start()
    latency_proc = phase6_apply_latency(cfg)
    start_load_generator(cfg)

    phase7_wait_mid_epoch(cfg, epoch_0_start)
    upgrade_proto, upgrade_consensus = phase8_rolling_upgrade(
        cfg, old_max_proto, old_consensus, local_branch, local_commit
    )
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
    stop_load_generator(cfg)

    cp_monitor.stop()
    log(_phase_banner("Checkpoint Liveness Report"))
    for line in cp_monitor.report().split("\n"):
        log(line)

    # Kill latency background process (runs under sudo, so use sudo pkill)
    run(["sudo", "pkill", "-f", r"network-benchmark\.sh"], check=False, quiet=True)
    if latency_proc.poll() is None:
        latency_proc.terminate()


if __name__ == "__main__":
    main()
