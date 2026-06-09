#!/usr/bin/env python3

# Copyright (c) 2026 IOTA Stiftung
# SPDX-License-Identifier: Apache-2.0

"""Shared infrastructure for the private-network experiment runners.

Both ``run-migration-test.py`` and ``run-benchmark.py`` import from here:
terminal/file logging, subprocess helpers, Prometheus queries, and the
generic network phases (compose generation, genesis bootstrap, validator
startup, monitoring, latency injection, log capture, block-production
measurement, teardown). Anything specific to one runner — the rolling
upgrade and epoch schedule for migration, the fuzz/spammer matrix for the
benchmark — stays in that runner.

The compose generator emits one service block per validator. Experiment
runners support 2-30 validators, matching the Prometheus scrape configuration.
"""

from __future__ import annotations

import fcntl
import getpass
import json
import math
import os
import pwd
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
from datetime import datetime, timezone
from pathlib import Path


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


_ANSI_RE = re.compile(r"\x1b\[[0-?]*[ -/]*[@-~]")
_log_fh = None  # file handle for the run log, set by setup_logging()
MIN_VALIDATORS = 2
MAX_VALIDATORS = 30


def setup_logging(log_file: Path) -> None:
    """Open *log_file* in append mode as the shared log sink.

    Append (not truncate) so a sudo'd child process writing to the same path
    and this parent process do not clobber each other.
    """
    global _log_fh
    log_file.parent.mkdir(parents=True, exist_ok=True)
    # Truncate once for a fresh run, then reopen with O_APPEND so this process
    # and any sudo'd child writing to the same path both land at end-of-file
    # instead of overwriting each other.
    log_file.write_text("")
    _log_fh = log_file.open("a")


def close_logging() -> None:
    global _log_fh
    if _log_fh is not None:
        _log_fh.close()
        _log_fh = None


def archive_run_log(log_file: Path, prefix: str) -> Path | None:
    """Copy the latest coordinator log to a timestamped archive."""
    if not log_file.exists():
        return None
    if _log_fh is not None:
        _log_fh.flush()
    ts = datetime.now().strftime("%Y%m%d-%H%M%S")
    destination = log_file.parent / f"{prefix}_{ts}.log"
    shutil.copy2(log_file, destination)
    return destination


def _phase_banner(title: str, phase: str = "") -> str:
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


def log(msg: str) -> None:
    ts = datetime.now(timezone.utc).strftime("%H:%M:%S")
    plain_msg = _ANSI_RE.sub("", msg).replace("\r", "")
    colored = f"{_C.DIM}{ts}{_C.RESET} {msg}"
    # Each visual line needs its own carriage return + clear: a pty that does
    # not translate LF to CR+LF keeps the column across embedded newlines,
    # which used to indent banner lines under the timestamp.
    print(f"\r\033[K{colored}".replace("\n", "\n\r\033[K"), flush=True)
    if _log_fh is not None:
        timestamp = datetime.now(timezone.utc).isoformat()
        for line in plain_msg.split("\n"):
            _log_fh.write(f"{timestamp} {line}\n")
        _log_fh.flush()


def log_status(msg: str) -> None:
    """Overwrite the current terminal line (no newline); still logged to file."""
    ts = datetime.now(timezone.utc).strftime("%H:%M:%S")
    plain_msg = _ANSI_RE.sub("", msg).replace("\r", "")
    colored = f"{_C.DIM}{ts}{_C.RESET} {msg}"
    print(f"\r\033[K{colored}", end="", flush=True)
    if _log_fh is not None:
        timestamp = datetime.now(timezone.utc).isoformat()
        for line in plain_msg.split("\n"):
            _log_fh.write(f"{timestamp} {line}\n")
        _log_fh.flush()


def countdown(seconds: int) -> None:
    """Sleep for *seconds* with a live progress bar."""
    start = time.time()
    while time.time() < start + seconds:
        elapsed = int(time.time() - start)
        log_status(f"  {_progress_bar(elapsed, seconds)} {elapsed}s / {seconds}s")
        time.sleep(1)
    print()  # finish status line


# ========================= Subprocess helpers =========================


def run_timed(
    cmd: list[str],
    label: str,
    *,
    cwd: Path | None = None,
    env: dict[str, str] | None = None,
    check: bool = True,
) -> subprocess.CompletedProcess[str]:
    """Run a command quietly, showing *label* with a live elapsed timer."""
    start = time.time()
    proc = subprocess.Popen(
        cmd, cwd=cwd, env=env, text=True,
        stdout=subprocess.PIPE, stderr=subprocess.STDOUT, bufsize=1,
    )
    output_lines: list[str] = []
    if _log_fh is not None:
        _log_fh.write(f"{datetime.now(timezone.utc).isoformat()}   $ {' '.join(cmd)}\n")
        _log_fh.flush()

    assert proc.stdout is not None
    sel = selectors.DefaultSelector()
    sel.register(proc.stdout, selectors.EVENT_READ)
    while proc.poll() is None:
        elapsed = int(time.time() - start)
        log_status(f"  {label}... {_C.DIM}{elapsed}s{_C.RESET}")
        if sel.select(timeout=1.0):
            raw_line = proc.stdout.readline()
            if raw_line:
                for line in _ANSI_RE.sub("", raw_line).replace("\r", "\n").splitlines():
                    output_lines.append(line)
                    if _log_fh is not None:
                        _log_fh.write(f"{datetime.now(timezone.utc).isoformat()}     {line}\n")
    for raw_line in proc.stdout:
        for line in _ANSI_RE.sub("", raw_line).replace("\r", "\n").splitlines():
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
        print()
        log(f"  {_C.RED}✘ {label} failed ({elapsed}s){_C.RESET}")
        for line in output_lines:
            if line:
                log(f"    {line}")
        raise subprocess.CalledProcessError(returncode, cmd, output=result.stdout)
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
    """Run a subprocess with logging. See run-migration-test.py for the
    verbose/quiet/capture semantics (kept identical)."""
    if quiet:
        if _log_fh is not None:
            _log_fh.write(f"{datetime.now(timezone.utc).isoformat()}   $ {' '.join(cmd)}\n")
            _log_fh.flush()
    else:
        log(f"  $ {' '.join(cmd)}")
    if capture:
        return subprocess.run(
            cmd, cwd=cwd, check=check, text=True, capture_output=True, env=env,
        )
    proc = subprocess.Popen(
        cmd, cwd=cwd, text=True,
        stdout=subprocess.PIPE, stderr=subprocess.STDOUT, bufsize=1, env=env,
    )
    output_lines: list[str] = []
    assert proc.stdout is not None
    for raw_line in proc.stdout:
        for line in _ANSI_RE.sub("", raw_line).replace("\r", "\n").splitlines():
            output_lines.append(line)
            if verbose and line:
                log(f"    {line}")
    returncode = proc.wait()
    result = subprocess.CompletedProcess(cmd, returncode, stdout="\n".join(output_lines), stderr="")
    if check and returncode != 0:
        if not verbose:
            for line in output_lines:
                if line:
                    log(f"    {line}")
        raise subprocess.CalledProcessError(returncode, cmd, output=result.stdout)
    return result


def find_repo_root(start: Path) -> Path:
    try:
        out = subprocess.check_output(
            ["git", "rev-parse", "--show-toplevel"],
            cwd=start, text=True, stderr=subprocess.DEVNULL,
        )
        return Path(out.strip())
    except (subprocess.CalledProcessError, FileNotFoundError):
        return start.parent.parent.parent


_run_lock_fh = None  # held for the process lifetime by acquire_single_run_lock()


def acquire_single_run_lock(runner: str) -> None:
    """Take the cross-runner single-run lock (released when the process dies).

    The benchmark/fuzz/migration runners share container names, the docker
    networks, and the tc/iptables state on the validators — two concurrent
    runs silently corrupt each other (one run's cleanup tears down the other
    run's network mid-flight while it keeps "succeeding" with no validators).
    Fail fast instead of letting that happen."""
    global _run_lock_fh
    # Fixed /tmp path on purpose: TMPDIR can differ between shells, and the
    # lock must be shared by every process on the host.
    lock_path = Path("/tmp/iota-experiments.lock")
    try:
        # "r+" (no O_CREAT) first: fs.protected_regular forbids O_CREAT opens
        # of another user's pre-existing file in sticky /tmp, even for root.
        fh = lock_path.open("r+")
    except FileNotFoundError:
        try:
            fh = lock_path.open("x+")
        except FileExistsError:  # raced another starting run
            fh = lock_path.open("r+")
    except PermissionError as err:
        raise RuntimeError(
            f"cannot open {lock_path} (created by another user with an older "
            f"version of this script?): {err} — remove it (sudo rm "
            f"{lock_path}) and retry"
        ) from err
    # flock() itself is cross-user (it locks the inode), but the file must be
    # openable by every user for that to matter; otherwise a stale lock file
    # from another user blocks runs at open() instead of with the clean
    # holder message below.
    try:
        os.chmod(lock_path, 0o666)
    except OSError:
        pass  # not the owner — the current mode already let us open it
    try:
        fcntl.flock(fh, fcntl.LOCK_EX | fcntl.LOCK_NB)
    except OSError:
        fh.seek(0)
        holder = fh.read().strip() or "holder unknown"
        owner = _lock_holder_owner(holder)
        if owner:
            holder = f"{holder}, running as {owner}"
        if not _offer_to_kill_lock_holder(fh, holder):
            fh.close()
            raise RuntimeError(
                f"another experiment run is already active ({holder}; lock: "
                f"{lock_path}) — wait for it to finish or kill it first"
            )
    fh.seek(0)
    fh.truncate()
    user = os.environ.get("SUDO_USER") or getpass.getuser()
    fh.write(
        f"{runner} pid {os.getpid()} user {user} "
        f"since {datetime.now(timezone.utc):%Y-%m-%d %H:%M:%S} UTC\n"
    )
    fh.flush()
    _run_lock_fh = fh  # keep the fd open: the flock dies with the process


def _lock_holder_pid(holder: str) -> int | None:
    match = re.search(r"\bpid (\d+)\b", holder)
    return int(match.group(1)) if match else None


def _lock_holder_owner(holder: str) -> str | None:
    """Resolve the user actually running the lock-holding pid (via /proc, so
    it also works for lock lines written before the user field existed)."""
    pid = _lock_holder_pid(holder)
    if pid is None:
        return None
    try:
        uid = os.stat(f"/proc/{pid}").st_uid
        return pwd.getpwuid(uid).pw_name
    except (OSError, KeyError):
        return None


def _offer_to_kill_lock_holder(fh, holder: str) -> bool:
    """Interactively offer to stop the active run and take over its lock.

    Only asks on a TTY (non-interactive callers keep the fail-fast error) and
    defaults to no. On yes, SIGINTs the holder — its signal handler runs the
    full cleanup — and waits for the flock to be released. Returns True once
    this process holds the lock."""
    if not (sys.stdin.isatty() and sys.stdout.isatty()):
        return False
    pid = _lock_holder_pid(holder)
    if pid is None:
        return False
    try:
        answer = input(
            f"Another experiment run is active ({holder}).\n"
            "Kill it and continue? [y/N] "
        )
    except EOFError:
        return False
    if answer.strip().lower() not in ("y", "yes"):
        return False
    try:
        os.kill(pid, signal.SIGINT)
    except ProcessLookupError:
        pass  # exited in the meantime; the flock may already be free
    except PermissionError:  # held by another user
        run(["sudo", "kill", "-INT", str(pid)], check=False, quiet=True)
    deadline = time.time() + 180
    while time.time() < deadline:
        try:
            fcntl.flock(fh, fcntl.LOCK_EX | fcntl.LOCK_NB)
            print()
            return True
        except OSError:
            log_status(f"  Waiting for pid {pid} to finish its cleanup...")
            time.sleep(2)
    print()
    log(f"  Run {pid} did not release the lock within 180s; giving up.")
    return False


def require_local_image(image: str, hint: str) -> None:
    """Fail fast with a clear message when a locally-built image is absent.

    Used for images that must NOT be pulled (bare local tags like
    ``iota-node`` would otherwise hit Docker Hub and die with an opaque
    `pull access denied` mid-compose)."""
    present = subprocess.run(
        ["docker", "image", "inspect", image],
        stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
    ).returncode == 0
    if not present:
        raise RuntimeError(f"local docker image '{image}' not found — {hint}")


def cache_sudo() -> None:
    """Prompt for sudo once and refresh the timestamp in the background.

    The latency injector, bootstrap, and teardown all need root; caching
    upfront keeps a long run from prompting mid-way. Refreshed every 240s by a
    daemon thread (ahead of the default 5-minute sudo timeout); the thread dies
    with the process, so no keepalive subprocess leaks across runs."""
    if shutil.which("sudo") is None:
        return
    log("Caching sudo credentials (you may be prompted for your password)...")
    subprocess.run(["sudo", "-v"], check=True)

    def _refresh() -> None:
        while True:
            time.sleep(240)
            subprocess.run(["sudo", "-vn"], check=False, capture_output=True)

    threading.Thread(target=_refresh, daemon=True).start()


def validate_num_validators(num_validators: int) -> None:
    """Validate the range supported by compose addressing and monitoring."""
    if not MIN_VALIDATORS <= num_validators <= MAX_VALIDATORS:
        raise ValueError(
            f"num_validators must be in [{MIN_VALIDATORS}, {MAX_VALIDATORS}], "
            f"got {num_validators}"
        )


# ========================= Prometheus =========================


def prometheus_query(expr: str) -> dict[str, object] | None:
    try:
        query = urllib.parse.urlencode({"query": expr})
        with urllib.request.urlopen(
            f"http://localhost:9090/api/v1/query?{query}", timeout=5
        ) as resp:
            return json.loads(resp.read())
    except Exception:
        return None


def prometheus_scalar(expr: str) -> str | None:
    data = prometheus_query(expr)
    if not data:
        return None
    try:
        result = data["data"]["result"]
        if not result:
            return None
        return str(result[0]["value"][1])
    except (KeyError, IndexError, TypeError):
        return None


def prometheus_vector(expr: str) -> list[tuple[dict[str, str], float]]:
    data = prometheus_query(expr)
    if not data:
        return []
    rows: list[tuple[dict[str, str], float]] = []
    try:
        for result in data["data"]["result"]:
            value = float(result["value"][1])
            if value == value:  # NaN guard
                rows.append((dict(result["metric"]), value))
    except (KeyError, TypeError, ValueError):
        return []
    return rows


def _commit_latency_queries(range_s: int) -> dict[str, str]:
    """PromQL for block/transaction commit latency over a *range_s* window.

    Block queries carry `or` fallbacks across the two block-latency metric
    naming conventions. Transaction queries intentionally have no block
    fallback: unavailable transaction latency must be reported as n/a rather
    than mislabeled block latency."""
    r = f"{range_s}s"
    return {
        "blk_p50": (
            "histogram_quantile(0.5,"
            f" sum(rate(consensus_block_commit_latency_bucket[{r}])) by (le)"
            f" or sum(rate(consensus_block_header_commit_latency_bucket[{r}])) by (le))"
        ),
        "blk_p95": (
            "histogram_quantile(0.95,"
            f" sum(rate(consensus_block_commit_latency_bucket[{r}])) by (le)"
            f" or sum(rate(consensus_block_header_commit_latency_bucket[{r}])) by (le))"
        ),
        "txn_p50": (
            "histogram_quantile(0.5,"
            f" sum(rate(consensus_transaction_commit_latency_bucket[{r}])) by (le))"
        ),
        "txn_p95": (
            "histogram_quantile(0.95,"
            f" sum(rate(consensus_transaction_commit_latency_bucket[{r}])) by (le))"
        ),
    }


def measure_block_production(
    num_validators: int, window: int, phase: str = "BLOCKS",
) -> None:
    """Wait *window* seconds, then report per-validator own-block rate
    (min/max/spread), the averaged block-creation-reason mix, and block /
    transaction commit latencies (p50/p95) over the same window."""
    phase_start = time.time()
    log(_phase_banner(f"Measuring block production over {window}s", phase))
    countdown(window)

    rate_rows = prometheus_vector(
        f'sum by(host)(rate(consensus_accepted_block_headers{{source="own"}}[{window}s]))'
    )
    rates = {m.get("host", "<unknown>"): v for m, v in rate_rows}
    expected = {f"validator-{i}" for i in range(1, num_validators + 1)}
    missing = sorted(expected - rates.keys())
    if missing:
        log("  WARNING: missing block-rate metrics for: " + ", ".join(missing))
    measured = {h: rates[h] for h in sorted(expected) if h in rates}

    reason_rows = prometheus_vector(
        f"avg by(reason)(rate(consensus_proposed_blocks[{window}s]))"
    )
    reasons = {m.get("reason", "<unknown>"): v for m, v in reason_rows}

    if measured:
        vals = list(measured.values())
        log(
            f"  Block rate min/max/spread: {min(vals):.2f} / {max(vals):.2f} / "
            f"{max(vals) - min(vals):.2f} blk/s"
        )
        for host, v in sorted(measured.items(), key=lambda kv: kv[1]):
            log(f"    {host:<14} {v:5.2f} blk/s")
    else:
        log("  WARNING: no block-rate metrics available")

    log("  Block creation reasons (avg by validator):")
    if reasons:
        for reason, v in sorted(reasons.items(), key=lambda kv: kv[1], reverse=True):
            log(f"    {reason:<24} {v:5.2f} /s")
    else:
        log("    WARNING: no block-creation-reason metrics available")

    # Commit latencies over the same window. The query range is floored at
    # 60s: on shorter windows histogram_quantile inputs are statistical noise
    # (at the cost of including a little pre-window data).
    queries = _commit_latency_queries(max(60, window))
    lat: dict[str, float | None] = {}
    for name, q in queries.items():
        raw = prometheus_scalar(q)
        try:
            val = float(raw) if raw is not None else None
        except ValueError:
            val = None
        lat[name] = None if val is None or math.isnan(val) else val

    def _ms(v: float | None) -> str:
        return f"{v * 1000.0:6.0f} ms" if v is not None else "    n/a"

    log("  Commit latency (across validators):")
    if any(v is not None for v in lat.values()):
        log(f"    block p50/p95: {_ms(lat['blk_p50'])} / {_ms(lat['blk_p95'])}")
        log(f"    txn   p50/p95: {_ms(lat['txn_p50'])} / {_ms(lat['txn_p95'])}")
    else:
        log("    WARNING: no commit-latency metrics available")
    log(_phase_complete("Block measurement", time.time() - phase_start))


# ========================= Network phases =========================


def generate_compose_file(
    path: Path,
    *,
    num_validators: int,
    base_image: str,
    chain_override: str,
    network_name: str = "iota-network",
    ip_prefix: str = "10.0.1",
    ip_base: int = 10,
    image_env_prefix: str | None = None,
    include_fullnode: bool = False,
    fullnode_image: str | None = None,
    include_faucet: bool = False,
    faucet_image: str = "iota-tools",
    header: str = "Auto-generated; do not edit manually.",
) -> None:
    """Write a docker compose file with one service block per validator.

    When *image_env_prefix* is set, each validator image is
    ``${{<prefix><i>_IMAGE:-<base_image>}}`` so individual nodes can be
    overridden via env (used by the rolling-upgrade migration); otherwise all
    validators run *base_image*. A fullnode is appended when *include_fullnode*
    (the load generator's RPC target); *include_faucet* additionally appends a
    faucet and publishes the fullnode RPC (127.0.0.1:9000) and faucet
    (127.0.0.1:5003) to the host — host-side load tools (iota-spammer) need
    both."""
    validate_num_validators(num_validators)
    lines = [f"# {header}", f"# {num_validators} validators.", "", "services:"]

    for i in range(1, num_validators + 1):
        image = (
            f"${{{image_env_prefix}{i}_IMAGE:-{base_image}}}"
            if image_env_prefix
            else base_image
        )
        lines.append(f"  validator-{i}:")
        lines.append(f"    image: {image}")
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
        lines.append(f"      - IOTA_PROTOCOL_CONFIG_CHAIN_OVERRIDE={chain_override}")
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
        lines.append(f"      {network_name}:")
        lines.append(f"        ipv4_address: {ip_prefix}.{ip_base + i}")
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

    if include_fullnode:
        fn_image = fullnode_image or base_image
        lines.append("  fullnode-1:")
        lines.append(f"    image: {fn_image}")
        lines.append("    container_name: fullnode-1")
        lines.append("    hostname: fullnode-1")
        lines.append("    environment:")
        lines.append("      - RUST_BACKTRACE=1")
        lines.append(
            "      - RUST_LOG=info,iota_core=debug,iota_network=debug,"
            "iota_node=debug,jsonrpsee=error"
        )
        lines.append(f"      - IOTA_PROTOCOL_CONFIG_CHAIN_OVERRIDE={chain_override}")
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
        lines.append(f"      {network_name}:")
        lines.append(f"        ipv4_address: {ip_prefix}.250")
        if include_faucet:
            # Host-side load tools talk to the fullnode RPC via localhost.
            lines.append("    ports:")
            lines.append('      - "127.0.0.1:9000:9000/tcp"')
        lines.append("    volumes:")
        lines.append(
            "      - ./configs/fullnodes/fullnode.yaml:/opt/iota/config/fullnode.yaml:ro"
        )
        lines.append(
            "      - ./configs/genesis/genesis.blob:/opt/iota/config/genesis.blob:ro"
        )
        lines.append("      - ./data/fullnode-1:/opt/iota/db:rw")
        lines.append("")

    if include_faucet:
        lines.append("  faucet-1:")
        lines.append(f"    image: {faucet_image}")
        lines.append("    container_name: faucet-1")
        lines.append("    hostname: faucet-1")
        lines.append("    restart: on-failure")
        lines.append("    environment:")
        lines.append("      - RUST_BACKTRACE=1")
        lines.append("      - RUST_LOG=info")
        lines.append("    command:")
        lines.append("      - /usr/local/bin/iota-faucet")
        lines.append("      - --port=5003")
        lines.append("      - --host-ip=0.0.0.0")
        lines.append("      - --write-ahead-log=/wal/faucet.wal")
        lines.append("      - --num-coins=10")
        lines.append("      - --amount=200000000000")
        lines.append("      - --max-request-per-second=50")
        lines.append("      - --ttl-expiration=150")
        lines.append("    ports:")
        lines.append('      - "127.0.0.1:5003:5003/tcp"')
        lines.append("    networks:")
        lines.append(f"      {network_name}:")
        lines.append(f"        ipv4_address: {ip_prefix}.251")
        lines.append("    volumes:")
        lines.append("      - ./configs/faucet:/root/.iota/iota_config")
        lines.append("      - ./data/faucet-1:/wal")
        lines.append("    depends_on:")
        lines.append("      - fullnode-1")
        lines.append("")

    lines.append("networks:")
    lines.append(f"  {network_name}:")
    lines.append("    driver: bridge")
    lines.append("    ipam:")
    lines.append("      config:")
    lines.append(f"        - subnet: {ip_prefix}.0/24")
    path.write_text("\n".join(lines) + "\n")


def bootstrap_genesis(network_dir: Path, num_validators: int, epoch_ms: int) -> None:
    """Run bootstrap.sh under sudo (writes the root-owned data dir)."""
    run_timed(
        ["sudo", "./bootstrap.sh", "-n", str(num_validators), "-e", str(epoch_ms)],
        "Bootstrapping genesis",
        cwd=network_dir,
    )
    print()


def compose_up_validators(
    compose_file: str, env_file: str | None, network_dir: Path, num_validators: int,
    boot_wait: int = 10,
) -> None:
    """Bring up validator-1..N from the generated compose and verify they run."""
    cmd = ["docker", "compose", "--ansi", "never"]
    if env_file:
        cmd += ["--env-file", env_file]
    # --remove-orphans: an interrupted prior run (e.g. kill -9 mid-cleanup)
    # can leave same-project validators from a larger -n running; they hold
    # a stale genesis and would pollute the new network.
    cmd += ["-f", compose_file, "up", "-d", "--remove-orphans"]
    run(cmd, cwd=network_dir, quiet=True)

    for sec in range(boot_wait, 0, -1):
        log_status(f"  Waiting for validators to boot... {sec}s")
        time.sleep(1)
    result = run(
        ["docker", "ps", "--filter", "name=validator-", "--format", "{{.Names}}"],
        capture=True, quiet=True,
    )
    running = set(result.stdout.strip().splitlines())
    expected = {f"validator-{i}" for i in range(1, num_validators + 1)}
    missing = expected - running
    print()
    if missing:
        raise RuntimeError(
            f"Missing validators after boot: {sorted(missing)} "
            f"(running: {len(running & expected)}/{num_validators})"
        )
    log(f"  {_C.GREEN}Running validators: {len(running & expected)}/{num_validators}{_C.RESET}")


def start_grafana(grafana_dir: Path, override_file: str | None = None) -> None:
    """(Re)create the Grafana/Prometheus stack on the experiment network.

    `--force-recreate` (never skip): the experiment network is torn down and
    recreated between runs, so a monitoring container left over from a prior
    run still references the old network ID and fails to start with
    "network ... not found". Force-recreating rebinds the whole stack to the
    current network — the one whose validators Prometheus must scrape
    (`iota-network` here; the migration runner passes its own override)."""
    cmd = ["docker", "compose", "--ansi", "never", "-f", "docker-compose.yaml"]
    if override_file:
        cmd += ["-f", override_file]
    cmd += ["up", "-d", "--force-recreate", "--remove-orphans"]
    run_timed(cmd, "Starting monitoring stack", cwd=grafana_dir)
    print()
    log(f"  Grafana: {_C.CYAN}http://localhost:3000/dashboards{_C.RESET}")
    log(f"  Prometheus: {_C.CYAN}http://localhost:9090/targets{_C.RESET}")


def dump_latency_matrix(
    script_dir: Path, num_validators: int, geodistributed: bool, log_file: Path,
    out_path: Path,
) -> None:
    """Write the effective role-based matrix without touching docker/netem."""
    run(
        [
            "./network-benchmark.sh",
            "-n", str(num_validators),
            "-g", str(geodistributed).lower(),
            "-o", str(log_file.resolve()),
            "-D", str(out_path.resolve()),
        ],
        cwd=script_dir, quiet=True,
    )
    rows = [
        line.split("\t")
        for line in out_path.read_text().splitlines()
        if line and not line.startswith("#")
    ]
    delays = [int(r[2]) for r in rows]
    slots = sum(1 for r in rows if len(r) > 7 and int(r[7]) > 0)
    log(f"  {_C.BOLD}Latency matrix{_C.RESET}    : {out_path}")
    if delays:
        log(
            f"  Edges: {len(rows)}, delay mean/max: "
            f"{sum(delays) / len(delays):.1f}/{max(delays)} ms, slot-burst edges: {slots}"
        )


def apply_latency(
    script_dir: Path, num_validators: int, seed: int, geodistributed: bool,
    log_file: Path, apply_wait: int,
    *, percent_block: int = 0, percent_loss: int = 0, percent_restart: int = 0,
    restart_duration: int = 120, restart_timeout: int = 60,
    restart_mode: str = "preserve-consensus",
) -> subprocess.Popen[str]:
    """Launch network-benchmark.sh under sudo to inject the role-based matrix
    (plus optional block/loss/restart fuzz). Returns the running process."""
    out = log_file.open("a")
    proc = subprocess.Popen(
        [
            "sudo", "./network-benchmark.sh",
            "-n", str(num_validators),
            "-s", str(seed),
            "-b", str(percent_block),
            "-l", str(percent_loss),
            "-r", str(percent_restart),
            "-d", str(restart_duration),
            "-w", str(restart_timeout),
            "-M", restart_mode,
            "-g", str(geodistributed).lower(),
            "-o", str(log_file.resolve()),
        ],
        cwd=script_dir, stdout=out, stderr=subprocess.STDOUT,
    )
    out.close()
    for sec in range(apply_wait):
        if proc.poll() is not None:
            raise RuntimeError(
                f"network-benchmark.sh exited early with code {proc.returncode}"
            )
        log_status(f"  Waiting for latency application... {sec + 1}s")
        time.sleep(1)
    print()
    log(f"  Latency applied after {apply_wait}s wait")
    return proc


def _image_present(image: str) -> bool:
    return subprocess.run(
        ["docker", "image", "inspect", image],
        stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
    ).returncode == 0


def ensure_image(image: str) -> bool:
    """Return True if *image* is available locally, pulling it if missing.

    On pull failure (typically a private registry needing credentials) logs an
    actionable hint and returns False — callers that *require* the image should
    treat that as fatal. Deliberately non-interactive: a mid-run prompt can
    hang forever when stdin has a pty but no keyboard behind it, so
    authentication happens out of band."""
    if _image_present(image):
        return True
    log(f"  Image {image} not present locally; pulling...")
    if run(["docker", "pull", image], check=False, quiet=True).returncode == 0:
        return True
    log(f"  Could not pull {image} — the registry likely needs credentials.")
    log("  Fix (any one): `docker login` and re-run, build it from the")
    log("  network-benchmark repo, or pass --spammer-image.")
    return False


# Source of the `stress` load generator; docker/stress/build.sh in it builds
# and tags the image as iotaledger/stress.
NETWORK_BENCHMARK_REPO = "git@github.com:iotaledger/network-benchmark.git"
NETWORK_BENCHMARK_DIR = Path.home() / "network-benchmark"
STRESS_BUILD_SCRIPT = NETWORK_BENCHMARK_DIR / "docker" / "stress" / "build.sh"


def _prompt_yes_no(question: str, timeout: int = 30) -> bool:
    """Ask a y/N question that can never hang the run: wait for an answer on
    stdin for at most *timeout* seconds, defaulting to No. This covers ptys
    with no keyboard behind them, where a plain input() blocks forever."""
    log(f"  {question} [y/N] (auto-No in {timeout}s)")
    try:
        sel = selectors.DefaultSelector()
        sel.register(sys.stdin, selectors.EVENT_READ)
        ready = sel.select(timeout)
        sel.close()
    except (OSError, ValueError):
        return False
    if not ready:
        log("  No answer — continuing with No.")
        return False
    return sys.stdin.readline().strip().lower() in ("y", "yes")


def _update_or_clone_benchmark_repo() -> None:
    """Best-effort: keep ~/network-benchmark buildable. An existing clone is
    ff-only updated (never fatal — offline or credential-less environments
    build the existing checkout); a missing clone is fetched only after an
    explicit, timeout-guarded user confirmation."""
    env = {**os.environ, "GIT_TERMINAL_PROMPT": "0"}
    if NETWORK_BENCHMARK_DIR.is_dir():
        res = run(
            ["git", "-C", str(NETWORK_BENCHMARK_DIR), "pull", "--ff-only"],
            check=False, quiet=True, env=env,
        )
        if res.returncode != 0:
            log("  Could not update ~/network-benchmark — building the existing checkout.")
        return
    if not _prompt_yes_no(
        f"Clone {NETWORK_BENCHMARK_REPO} to ~/network-benchmark for the build?"
    ):
        return
    run_timed(
        ["git", "clone", NETWORK_BENCHMARK_REPO, str(NETWORK_BENCHMARK_DIR)],
        "Cloning network-benchmark", check=False,
    )


def ensure_stress_image(image: str) -> None:
    """Make the stress load image available BEFORE the network comes up:
    local copy, else registry pull, else build it from the ~/network-benchmark
    clone (cloned on user confirmation when absent). Raises when the image
    cannot be obtained — load was explicitly requested, so starting without it
    would be misleading.

    Resolving this up front keeps a (potentially ~30 min, first-time) build
    out of the run itself, where validators would sit idle under latency."""
    if _image_present(image):
        return
    log(f"  Image {image} not present locally; pulling...")
    if run(["docker", "pull", image], check=False, quiet=True).returncode == 0:
        return
    # build.sh tags exactly iotaledger/stress, so only that name can be
    # satisfied by building.
    if image.split(":")[0] == "iotaledger/stress":
        _update_or_clone_benchmark_repo()
        if STRESS_BUILD_SCRIPT.is_file():
            log("  Pull failed; building it from ~/network-benchmark instead")
            log("  (cached after the first build, which can take ~30 min)...")
            run_timed(
                ["bash", str(STRESS_BUILD_SCRIPT)], f"Building {image}",
                cwd=NETWORK_BENCHMARK_DIR,
            )
            if _image_present(image):
                return
    raise RuntimeError(
        f"spammer image {image} is unavailable — `docker login` to the "
        "registry, clone github.com/iotaledger/network-benchmark to "
        "~/network-benchmark, or pass --spammer-image"
    )


# Faucet account from the bootstrap genesis templates; owns the gas the
# `stress` load generator spends.
DEFAULT_PRIMARY_GAS_OWNER_ID = (
    "0x7cc6ff19b379d305b8363d9549269e388b8c1515772253ed4c868ee80b149ca0"
)


def build_images(script_dir: Path, build: bool) -> None:
    """Rebuild the local iota-node / iota-tools / iota-indexer images.

    Each build.sh ignores its arguments and always tags
    ``iotaledger/<name>:latest``, while the generated compose files run the
    bare local tags (``iota-node``, ``iota-tools``) so docker can never
    silently pull them. Retag after every build — otherwise a stale bare tag
    from an earlier build keeps running old binaries against a genesis
    generated by the fresh tools image (e.g. a protocol-version mismatch that
    crash-loops every validator)."""
    if not build:
        log("Skipping image builds")
        return
    log(_phase_banner("Building docker images", "BUILD"))
    docker_dir = script_dir.parent.parent.parent / "docker"
    for name in ("iota-node", "iota-tools", "iota-indexer"):
        run_timed(["./build.sh"], f"Building {name}", cwd=docker_dir / name)
        run(["docker", "tag", f"iotaledger/{name}:latest", name], quiet=True)
    print()


def start_stress_container(
    *,
    image: str,
    network_name: str,
    network_dir: Path,
    log_dir: Path,
    rpc_address: str,
    gas_owner_id: str,
    target_qps: int,
    in_flight_ratio: int,
    transfer_objects: int,
) -> None:
    """Start the `stress` load container (`stress-benchmark`) against
    *network_name* and verify it survives startup; raises RuntimeError when
    it cannot run."""
    genesis_blob = network_dir / "configs" / "genesis" / "genesis.blob"
    faucet_keystore = network_dir / "configs" / "faucet" / "iota.keystore"
    # stress migrates old-format keystores in place (a rename, which fails
    # with EBUSY on a read-only single-file bind mount and kills the container
    # instantly) — hand it a writable copy in its own directory.
    keystore_dir = log_dir / "load-generator-keystore"
    shutil.rmtree(keystore_dir, ignore_errors=True)
    keystore_dir.mkdir(parents=True, exist_ok=True)
    shutil.copy2(faucet_keystore, keystore_dir / "iota.keystore")
    run(["docker", "rm", "-f", "stress-benchmark"], check=False, quiet=True)
    # No --rm: if stress crashes at startup its logs must survive for the
    # liveness check below (cleanup force-removes the container anyway).
    res = run(
        [
            "docker", "run", "-d", "--name", "stress-benchmark",
            "--network", network_name,
            "-v", f"{genesis_blob.resolve()}:/opt/iota/config/genesis.blob:ro",
            "-v", f"{keystore_dir.resolve()}:/opt/iota/config:rw",
            image, "/usr/local/bin/stress",
            "--local", "false",
            "--use-fullnode-for-execution", "true",
            "--fullnode-rpc-addresses", rpc_address,
            "--genesis-blob-path", "/opt/iota/config/genesis.blob",
            "--keystore-path", "/opt/iota/config/iota.keystore",
            "--primary-gas-owner-id", gas_owner_id,
            "bench",
            "--target-qps", str(target_qps),
            "--in-flight-ratio", str(in_flight_ratio),
            "--transfer-object", str(transfer_objects),
        ],
        check=False, quiet=True,
    )
    if res.returncode != 0:
        raise RuntimeError("stress load container failed to start")
    # `docker run -d` succeeding only means the container was created — a
    # startup crash (bad keystore, unreachable fullnode) shows up within
    # seconds, so re-check liveness instead of silently running unloaded.
    time.sleep(5)
    alive = run(
        ["docker", "ps", "-q", "--filter", "name=^stress-benchmark$"],
        capture=True, quiet=True,
    ).stdout.strip()
    if not alive:
        run(["docker", "logs", "stress-benchmark"], check=False)
        raise RuntimeError("stress load container exited right after start (logs above)")


def start_spammer(cfg) -> subprocess.Popen[str] | None:
    """Start the configured transaction spammer for the benchmark/fuzz
    runners and return its host process, if any."""
    if not cfg.spammer_enable:
        return None
    duration = (
        cfg.run_duration
        + max(0, getattr(cfg, "block_measurement_seconds", 0))
        + 60
    )
    log(_phase_banner(
        f"Starting {cfg.spammer_type} spammer (tps={cfg.spammer_tps})", "LOAD",
    ))

    if cfg.spammer_type == "stress":
        # The `stress` load tool is the iota-benchmark binary, shipped as the
        # iotaledger/stress image. The runners resolve it up front via
        # ensure_stress_image (local copy, else pull, else build); this
        # non-interactive guard fails the run if it is somehow still missing —
        # load was explicitly requested.
        if not ensure_image(cfg.spammer_image):
            raise RuntimeError(
                f"spammer requested (-S true) but image {cfg.spammer_image} is "
                "unavailable — `docker login` to the registry or pass "
                "--spammer-image"
            )
        start_stress_container(
            image=cfg.spammer_image,
            network_name=cfg.network_name,
            network_dir=cfg.network_dir,
            log_dir=cfg.log_dir,
            rpc_address=cfg.load_rpc_address,
            gas_owner_id=cfg.load_primary_gas_owner_id,
            target_qps=cfg.spammer_tps,
            in_flight_ratio=cfg.load_in_flight_ratio,
            transfer_objects=cfg.load_transfer_objects,
        )
        log("  stress-benchmark started; logs: docker logs stress-benchmark")
        return None
    else:  # iota-spammer
        home = Path.home()
        sudo_user = os.environ.get("SUDO_USER")
        script = home / "iota-spammer" / "scripts" / "spamming_fuzz_test.sh"
        if not script.is_file():
            raise RuntimeError(
                f"iota-spammer requested but script not found at {script}; "
                "clone github.com/iotaledger/iota-spammer or select the stress backend"
            )
        spam_log = (cfg.log_dir / "spammer.log").open("w")
        cmd = ["bash", str(script), "-T", str(cfg.spammer_tps),
               "-s", cfg.spammer_size, "-d", f"{duration}s"]
        if sudo_user:
            cmd = ["sudo", "-u", sudo_user, "-H", *cmd]
        proc = subprocess.Popen(
            cmd,
            stdout=spam_log,
            stderr=subprocess.STDOUT,
            text=True,
            start_new_session=True,
        )
        spam_log.close()
        time.sleep(2)
        if proc.poll() is not None:
            raise RuntimeError(
                f"iota-spammer exited during startup with code {proc.returncode}; "
                f"see {cfg.log_dir / 'spammer.log'}"
            )
        log(f"  iota-spammer started (~{duration}s); logs: {cfg.log_dir / 'spammer.log'}")
        return proc


def stop_spammer(cfg, proc: subprocess.Popen[str] | None) -> None:
    """Stop the configured spammer and retain its logs."""
    if not cfg.spammer_enable:
        return

    if cfg.spammer_type == "stress":
        latest = cfg.log_dir / "stress-benchmark-latest.log"
        with latest.open("w") as fh:
            subprocess.run(
                ["docker", "logs", "stress-benchmark"],
                stdout=fh,
                stderr=subprocess.STDOUT,
                check=False,
            )
        if latest.stat().st_size > 0:
            ts = datetime.now().strftime("%Y%m%d-%H%M%S")
            shutil.copy2(latest, cfg.log_dir / f"stress-benchmark-{ts}.log")
        run(["docker", "rm", "-f", "stress-benchmark"], check=False, quiet=True)
        return

    if proc is None:
        return
    try:
        os.killpg(proc.pid, signal.SIGTERM)
        proc.wait(timeout=10)
    except ProcessLookupError:
        proc.poll()
        return
    except subprocess.TimeoutExpired:
        try:
            os.killpg(proc.pid, signal.SIGKILL)
        except ProcessLookupError:
            pass
        proc.wait(timeout=5)


def run_loop(cfg, prefix: str, final_prefix: str) -> None:
    """Sleep for cfg.run_duration with a progress bar, saving validator logs
    every cfg.log_interval seconds and once more at the end."""
    log(_phase_banner(
        f"Running for {cfg.run_duration}s (logs every {cfg.log_interval}s)", "RUN",
    ))
    end = time.time() + cfg.run_duration
    last_save = 0.0
    while time.time() < end:
        if time.time() - last_save >= cfg.log_interval:
            save_validator_logs(cfg.log_dir, cfg.num_validators, prefix=prefix)
            last_save = time.time()
        remaining = int(end - time.time())
        done = cfg.run_duration - remaining
        log_status(f"  {_progress_bar(done, cfg.run_duration)} {done}s / {cfg.run_duration}s")
        time.sleep(min(5, max(1, remaining)))
    print()
    # Final timestamped snapshot.
    ts = datetime.now().strftime("%Y%m%d-%H%M%S")
    save_validator_logs(
        cfg.log_dir,
        cfg.num_validators,
        prefix=f"{final_prefix}-{ts}",
        latest=False,
    )


def network_stats(num_validators: int) -> None:
    """Log per-validator TX/RX packet and byte counters."""
    log(_C.BOLD + "Final network stats per validator:" + _C.RESET)
    for i in range(1, num_validators + 1):
        v = f"validator-{i}"
        try:
            stats = {}
            for key in ("tx_bytes", "rx_bytes", "tx_packets", "rx_packets"):
                r = run(
                    ["docker", "exec", v, "cat", f"/sys/class/net/eth0/statistics/{key}"],
                    capture=True, check=False, quiet=True,
                )
                stats[key] = int(r.stdout.strip() or 0)
            log(
                f"  {v}: TX {stats['tx_packets']:,} pkts / "
                f"{stats['tx_bytes'] / 1048576:.2f} MB, "
                f"RX {stats['rx_packets']:,} pkts / {stats['rx_bytes'] / 1048576:.2f} MB"
            )
        except Exception:
            log(f"  {v}: stats unavailable")


def save_validator_logs(
    log_dir: Path, num: int, prefix: str = "exp", *, latest: bool = True
) -> None:
    for i in range(1, num + 1):
        suffix = "-latest" if latest else ""
        dest = log_dir / f"{prefix}-validator-{i}{suffix}.log"
        with dest.open("w") as fh:
            subprocess.run(
                ["docker", "logs", f"validator-{i}"],
                stdout=fh, stderr=subprocess.STDOUT, check=False,
            )


def compose_down(compose_file: str, env_file: str | None, network_dir: Path) -> None:
    """Tear down the generated compose project."""
    cmd = ["docker", "compose", "--ansi", "never"]
    if env_file:
        cmd += ["--env-file", env_file]
    cmd += ["-f", compose_file, "down", "--remove-orphans"]
    run(cmd, cwd=network_dir, check=False, quiet=True)


# ========================= Self-tests =========================
# Run with: python3 experiment_common.py


if __name__ == "__main__":
    import runpy
    import tempfile
    import unittest
    from types import SimpleNamespace
    from unittest import mock

    class ExperimentCommonTests(unittest.TestCase):
        def test_validator_count_bounds(self) -> None:
            validate_num_validators(2)
            validate_num_validators(30)
            for invalid in (1, 31):
                with self.subTest(invalid=invalid):
                    with self.assertRaises(ValueError):
                        validate_num_validators(invalid)

        def test_commit_latency_percentiles_use_histograms(self) -> None:
            queries = _commit_latency_queries(90)
            self.assertIn("histogram_quantile(0.5", queries["blk_p50"])
            self.assertIn("histogram_quantile(0.5", queries["txn_p50"])
            self.assertNotIn("_sum", queries["blk_p50"])
            self.assertNotIn("_sum", queries["txn_p50"])
            self.assertNotIn("block_commit", queries["txn_p50"])

        def test_validator_log_snapshot_names(self) -> None:
            with mock.patch.object(subprocess, "run") as run_mock:
                run_mock.return_value = subprocess.CompletedProcess([], 0)
                with tempfile.TemporaryDirectory() as tmp:
                    log_dir = Path(tmp)
                    save_validator_logs(log_dir, 2, prefix="exp")
                    save_validator_logs(
                        log_dir, 2, prefix="experiment-20260605-120000",
                        latest=False,
                    )
                    self.assertTrue(
                        (log_dir / "exp-validator-1-latest.log").exists()
                    )
                    self.assertTrue(
                        (log_dir / "experiment-20260605-120000-validator-2.log")
                        .exists()
                    )

        def test_requested_iota_spammer_must_exist(self) -> None:
            with tempfile.TemporaryDirectory() as tmp:
                cfg = SimpleNamespace(
                    spammer_enable=True,
                    spammer_type="iota-spammer",
                    spammer_tps=10,
                    spammer_size="10KiB",
                    run_duration=60,
                    block_measurement_seconds=0,
                    log_dir=Path(tmp),
                )
                with mock.patch.object(Path, "home", return_value=Path(tmp)):
                    with self.assertRaisesRegex(RuntimeError, "script not found"):
                        start_spammer(cfg)

        def test_host_spammer_cleanup_signals_process_group(self) -> None:
            cfg = SimpleNamespace(
                spammer_enable=True,
                spammer_type="iota-spammer",
            )
            proc = mock.Mock(pid=1234)
            with mock.patch.object(os, "killpg") as killpg:
                stop_spammer(cfg, proc)
            killpg.assert_called_once_with(1234, signal.SIGTERM)
            proc.wait.assert_called_once_with(timeout=10)

        def test_lock_contention_is_fatal_without_tty(self) -> None:
            # Non-interactive callers must keep the fail-fast error: the
            # kill-offer only ever engages on a TTY.
            with mock.patch.object(sys.stdin, "isatty", return_value=False):
                self.assertFalse(
                    _offer_to_kill_lock_holder(None, "runner pid 1 user x")
                )

        def test_lock_holder_pid_parsing(self) -> None:
            self.assertEqual(
                _lock_holder_pid("run-benchmark.py pid 4242 user nikita"), 4242
            )
            self.assertIsNone(_lock_holder_pid("holder unknown"))

        def test_migration_reserves_stable_window_after_setup(self) -> None:
            migration = runpy.run_path(
                Path(__file__).with_name("run-migration-test.py"),
                run_name="migration_test_module",
            )
            config = migration["Config"](num_validators=30)
            self.assertEqual(config.stable_window_seconds, 60)
            self.assertFalse(
                migration["Config"](mode="advanced").block_measurement_enabled()
            )
            monitor = migration["CheckpointMonitor"]
            self.assertIn("histogram_quantile(0.5", monitor._BLK_P50)
            self.assertNotIn("block_commit", monitor._TXN_P50)
            planned_start = 1_000.0 + config.pre_rolling_wait
            with self.assertRaisesRegex(RuntimeError, "stable window does not fit"):
                migration["phase7_wait_fixed"](
                    config,
                    1_000.0,
                    planned_start + 1,
                )

    unittest.main()
