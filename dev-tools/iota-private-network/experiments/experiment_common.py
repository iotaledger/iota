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

The compose generator emits one service block per validator, so a network of
any size is produced from ``num_validators`` alone (the static
``docker-compose.yaml`` caps the legacy bash path at its hand-written 19
services; this path has no such limit beyond the /24 subnet).
"""

from __future__ import annotations

import fcntl
import json
import math
import os
import re
import selectors
import shutil
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
    print(f"\r\033[K{colored}", flush=True)
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
        fh.close()
        raise RuntimeError(
            f"another experiment run is already active ({holder}; lock: "
            f"{lock_path}) — wait for it to finish or kill it first"
        )
    fh.seek(0)
    fh.truncate()
    fh.write(f"{runner} pid {os.getpid()} since {datetime.now(timezone.utc):%Y-%m-%d %H:%M:%S} UTC\n")
    fh.flush()
    _run_lock_fh = fh  # keep the fd open: the flock dies with the process


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

    Each query carries `or` fallbacks across the two block-latency metric
    naming conventions, and the transaction queries additionally fall back to
    block latency when the transaction histogram is unavailable."""
    r = f"{range_s}s"
    return {
        "blk_p50": (
            "quantile(0.5,"
            f" rate(consensus_block_commit_latency_sum[{r}])"
            f" / rate(consensus_block_commit_latency_count[{r}])"
            f" or rate(consensus_block_header_commit_latency_sum[{r}])"
            f" / rate(consensus_block_header_commit_latency_count[{r}]))"
        ),
        "blk_p95": (
            "histogram_quantile(0.95,"
            f" sum(rate(consensus_block_commit_latency_bucket[{r}])) by (le)"
            f" or sum(rate(consensus_block_header_commit_latency_bucket[{r}])) by (le))"
        ),
        "txn_p50": (
            "quantile(0.5,"
            f" rate(consensus_transaction_commit_latency_sum[{r}])"
            f" / rate(consensus_transaction_commit_latency_count[{r}])"
            f" or rate(consensus_block_commit_latency_sum[{r}])"
            f" / rate(consensus_block_commit_latency_count[{r}])"
            f" or rate(consensus_block_header_commit_latency_sum[{r}])"
            f" / rate(consensus_block_header_commit_latency_count[{r}]))"
        ),
        "txn_p95": (
            "histogram_quantile(0.95,"
            f" sum(rate(consensus_transaction_commit_latency_bucket[{r}])) by (le)"
            f" or sum(rate(consensus_block_commit_latency_bucket[{r}])) by (le)"
            f" or sum(rate(consensus_block_header_commit_latency_bucket[{r}])) by (le))"
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


def ensure_image(image: str) -> bool:
    """Return True if *image* is available locally, pulling it if missing.

    On pull failure (typically a private registry needing credentials) logs an
    actionable hint and returns False — callers that *require* the image should
    treat that as fatal. Deliberately non-interactive: a mid-run prompt can
    hang forever when stdin has a pty but no keyboard behind it, so
    authentication happens out of band."""
    present = subprocess.run(
        ["docker", "image", "inspect", image],
        stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
    ).returncode == 0
    if present:
        return True
    log(f"  Image {image} not present locally; pulling...")
    if run(["docker", "pull", image], check=False, quiet=True).returncode == 0:
        return True
    log(f"  Could not pull {image} — the registry likely needs credentials.")
    log("  Fix (any one): `docker login` and re-run, build it from the")
    log("  network-benchmark repo, or pass --spammer-image.")
    return False


# Faucet account from the bootstrap genesis templates; owns the gas the
# `stress` load generator spends.
DEFAULT_PRIMARY_GAS_OWNER_ID = (
    "0x7cc6ff19b379d305b8363d9549269e388b8c1515772253ed4c868ee80b149ca0"
)


def build_images(script_dir: Path, build: bool) -> None:
    """Rebuild the local iota-node / iota-tools / iota-indexer images."""
    if not build:
        log("Skipping image builds")
        return
    log(_phase_banner("Building docker images", "BUILD"))
    docker_dir = script_dir.parent.parent.parent / "docker"
    for name in ("iota-node", "iota-tools", "iota-indexer"):
        run_timed(["./build.sh", "-t", name], f"Building {name}", cwd=docker_dir / name)
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


def start_spammer(cfg) -> None:
    """Start the configured transaction spammer for the benchmark/fuzz
    runners (duck-typed over their Config fields)."""
    if not cfg.spammer_enable:
        return
    duration = max(10, cfg.run_duration - 60)
    log(_phase_banner(
        f"Starting {cfg.spammer_type} spammer (tps={cfg.spammer_tps})", "LOAD",
    ))

    if cfg.spammer_type == "stress":
        # The `stress` load tool is the iota-benchmark binary, shipped as the
        # iotaledger/stress image. ensure_image pulls it, offering an
        # interactive `docker login` on auth failures; load was explicitly
        # requested, so a still-missing image fails the run.
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
        log(f"  stress-benchmark started (~{duration}s); logs: docker logs stress-benchmark")
    else:  # iota-spammer
        home = Path.home()
        sudo_user = os.environ.get("SUDO_USER")
        script = home / "iota-spammer" / "scripts" / "spamming_fuzz_test.sh"
        if not script.is_file():
            log(f"  Skipping spammer: iota-spammer script not at {script} "
                "(clone github.com/iotaledger/iota-spammer); run continues without load.")
            return
        spam_log = (cfg.log_dir / "spammer.log").open("w")
        cmd = ["bash", str(script), "-T", str(cfg.spammer_tps),
               "-s", cfg.spammer_size, "-d", f"{duration}s"]
        if sudo_user:
            cmd = ["sudo", "-u", sudo_user, "-H", *cmd]
        subprocess.Popen(cmd, stdout=spam_log, stderr=subprocess.STDOUT)
        log(f"  iota-spammer started (~{duration}s); logs: {cfg.log_dir / 'spammer.log'}")


def run_loop(cfg, prefix: str) -> None:
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
    save_validator_logs(cfg.log_dir, cfg.num_validators, prefix=f"{prefix}-{ts}")


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


def save_validator_logs(log_dir: Path, num: int, prefix: str = "exp") -> None:
    for i in range(1, num + 1):
        dest = log_dir / f"{prefix}-validator-{i}-latest.log"
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
