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

import json
import re
import selectors
import subprocess
import sys
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


def cache_sudo() -> None:
    """Prompt for sudo once and refresh the timestamp in the background.

    The latency injector, bootstrap, and teardown all need root; caching
    upfront keeps a long run from prompting mid-way. Background-refreshed
    every 240s (ahead of the default 5-minute sudo timeout)."""
    import shutil as _shutil
    if _shutil.which("sudo") is None:
        return
    log("Caching sudo credentials (you may be prompted for your password)...")
    subprocess.run(["sudo", "-v"], check=True)
    subprocess.Popen(
        ["bash", "-c", "while true; do sleep 240; sudo -vn 2>/dev/null || true; done"],
        stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
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


def measure_block_production(num_validators: int, window: int) -> None:
    """Wait *window* seconds, then report per-validator own-block rate
    (min/max/spread) and the averaged block-creation-reason mix."""
    phase_start = time.time()
    log(_phase_banner(f"Measuring block production over {window}s", "BLOCKS"))
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
    log(_phase_complete("Block measurement", time.time() - phase_start))


# ========================= Network phases =========================


def generate_compose_file(
    path: Path,
    *,
    num_validators: int,
    base_image: str,
    chain_override: str,
    network_name: str = "iota-network",
    subnet: str = "10.0.1.0/24",
    ip_base: int = 10,
    image_env_prefix: str | None = None,
    include_fullnode: bool = False,
    fullnode_image: str | None = None,
    header: str = "Auto-generated; do not edit manually.",
) -> None:
    """Write a docker compose file with one service block per validator.

    When *image_env_prefix* is set, each validator image is
    ``${{<prefix><i>_IMAGE:-<base_image>}}`` so individual nodes can be
    overridden via env (used by the rolling-upgrade migration); otherwise all
    validators run *base_image*. A fullnode is appended when *include_fullnode*
    (the load generator's RPC target)."""
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
        lines.append(f"        ipv4_address: 10.0.1.{ip_base + i}")
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
        lines.append("        ipv4_address: 10.0.1.250")
        lines.append("    volumes:")
        lines.append(
            "      - ./configs/fullnodes/fullnode.yaml:/opt/iota/config/fullnode.yaml:ro"
        )
        lines.append(
            "      - ./configs/genesis/genesis.blob:/opt/iota/config/genesis.blob:ro"
        )
        lines.append("      - ./data/fullnode-1:/opt/iota/db:rw")
        lines.append("")

    lines.append("networks:")
    lines.append(f"  {network_name}:")
    lines.append("    driver: bridge")
    lines.append("    ipam:")
    lines.append("      config:")
    lines.append(f"        - subnet: {subnet}")
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
    cmd += ["-f", compose_file, "up", "-d"]
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

    Always `up -d` (never skip): a stack left on a different network by a
    prior run is recreated on drift, so Prometheus lands on the network whose
    validators it must scrape (`iota-network` here; the migration runner
    passes its own override)."""
    cmd = ["docker", "compose", "--ansi", "never", "-f", "docker-compose.yaml"]
    if override_file:
        cmd += ["-f", override_file]
    cmd += ["up", "-d"]
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
