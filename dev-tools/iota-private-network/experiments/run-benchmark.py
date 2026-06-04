#!/usr/bin/env python3

# Copyright (c) 2026 IOTA Stiftung
# SPDX-License-Identifier: Apache-2.0

"""Benchmark runner for the IOTA private network.

Replaces the former run-all-benchmark.sh. Generates its docker compose file
per run (one service block per validator), so it scales past the 19 services
hand-written in the static docker-compose.yaml — the same approach the
migration runner uses. Brings up N validators on a locally built image,
applies the role-based latency model (network-benchmark.sh, optionally with
block/loss/restart fuzz), optionally drives a transaction spammer, runs for a
fixed duration while collecting logs, and tears everything down cleanly.

Shared infrastructure lives in experiment_common.py.

Run from: iota/dev-tools/iota-private-network/experiments/
"""

from __future__ import annotations

import argparse
import os
import shutil
import signal
import subprocess
import sys
import time
from dataclasses import dataclass, field
from datetime import datetime
from pathlib import Path

import experiment_common as ec
from experiment_common import _C, log, log_status, run, run_timed


@dataclass
class Config:
    """All parameters for a benchmark run."""

    num_validators: int = 4
    build: bool = True
    geodistributed: bool = True
    seed: int = 42
    percent_block: int = 0
    percent_loss: int = 0
    percent_restart: int = 0
    run_duration: int = 3600
    restart_duration: int = 120
    restart_timeout: int = 60
    restart_mode: str = "preserve-consensus"
    epoch_duration_ms: int = 1_200_000
    network_metric: bool = False
    spammer_enable: bool = False
    spammer_tps: int = 10
    spammer_size: str = "10KiB"
    spammer_type: str = "stress"
    chain_override: str = ""

    image: str = "iota-node"
    fullnode_image: str = "iota-node"
    spammer_image: str = "iotaledger/stress"
    compose_file: str = "docker-compose.benchmark.yaml"
    log_interval: int = 60
    latency_apply_wait: int = 15
    block_measurement_seconds: int = 90
    load_in_flight_ratio: int = 5
    load_transfer_objects: int = 100
    load_rpc_address: str = "http://fullnode-1:9000"
    load_primary_gas_owner_id: str = (
        "0x7cc6ff19b379d305b8363d9549269e388b8c1515772253ed4c868ee80b149ca0"
    )

    script_dir: Path = field(default_factory=lambda: Path(__file__).resolve().parent)
    network_dir: Path = field(init=False)
    grafana_dir: Path = field(init=False)
    log_dir: Path = field(init=False)
    log_file: Path = field(init=False)
    network_name: str = field(init=False)

    def __post_init__(self) -> None:
        if self.restart_mode not in (
            "preserve-consensus", "full-reset", "simple-restart"
        ):
            raise ValueError(f"invalid restart mode: {self.restart_mode!r}")
        if self.spammer_type not in ("stress", "iota-spammer"):
            raise ValueError(f"invalid spammer type: {self.spammer_type!r}")
        for pct in (self.percent_block, self.percent_loss, self.percent_restart):
            if not 0 <= pct <= 100:
                raise ValueError("percentages must be in [0, 100]")
        self.network_dir = self.script_dir.parent
        self.grafana_dir = self.network_dir / ".." / "grafana-local"
        self.log_dir = self.script_dir / "logs"
        self.log_file = self.log_dir / "experiment_script_latest.log"
        # docker compose derives the project from the directory it runs in
        # (network_dir); the compose network "iota-network" therefore becomes
        # "<dir>_iota-network", which the base Grafana compose already targets.
        self.network_name = f"{self.network_dir.name}_iota-network"
        if not self.chain_override:
            # Local benchmark image is testnet-derived; keep testnet flags by
            # default so the network matches the migration test's defaults.
            self.chain_override = "testnet"

    def block_measurement_enabled(self) -> bool:
        return self.block_measurement_seconds > 0


_cfg: Config | None = None
_cleaning = False
_latency_proc: subprocess.Popen[str] | None = None


# ========================= Teardown =========================


def _network_stats(cfg: Config) -> None:
    log(_C.BOLD + "Final network stats per validator:" + _C.RESET)
    for i in range(1, cfg.num_validators + 1):
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


def cleanup(cfg: Config) -> None:
    global _cleaning
    if _cleaning:
        return
    _cleaning = True
    log(_phase("Cleaning up"))
    if cfg.network_metric:
        try:
            _network_stats(cfg)
        except Exception:
            pass
    # Stop spammer container if present.
    run(["docker", "rm", "-f", "stress-benchmark"], check=False, quiet=True)
    # Kill the latency injector (runs under sudo; escaped dot avoids matching
    # this pkill's own argv).
    run(["sudo", "pkill", "-f", r"network-benchmark\.sh"], check=False, quiet=True)
    if _latency_proc is not None and _latency_proc.poll() is None:
        _latency_proc.terminate()
    ec.compose_down(cfg.compose_file, None, cfg.network_dir)
    log("Cleanup complete.")


def _phase(title: str, phase: str = "") -> str:
    return ec._phase_banner(title, phase)


# ========================= Phases =========================


def build_images(cfg: Config) -> None:
    if not cfg.build:
        log("Skipping image builds")
        return
    log(_phase("Building docker images", "BUILD"))
    docker_dir = cfg.script_dir.parent.parent.parent / "docker"
    for name in ("iota-node", "iota-tools", "iota-indexer"):
        run_timed(
            ["./build.sh", "-t", name], f"Building {name}", cwd=docker_dir / name,
        )
    print()


def start_spammer(cfg: Config) -> None:
    if not cfg.spammer_enable:
        return
    duration = max(10, cfg.run_duration - 60)
    log(_phase(f"Starting {cfg.spammer_type} spammer (tps={cfg.spammer_tps})", "LOAD"))

    if cfg.spammer_type == "stress":
        # The `stress` load tool is the iota-benchmark binary, shipped as the
        # iotaledger/stress image. ensure_image pulls it, offering an
        # interactive `docker login` on auth failures; load was explicitly
        # requested, so a still-missing image fails the run.
        if not ec.ensure_image(cfg.spammer_image):
            raise RuntimeError(
                f"spammer requested (-S true) but image {cfg.spammer_image} is "
                "unavailable — `docker login` to the registry or pass "
                "--spammer-image"
            )
        genesis_blob = cfg.network_dir / "configs" / "genesis" / "genesis.blob"
        faucet_keystore = cfg.network_dir / "configs" / "faucet" / "iota.keystore"
        # stress migrates old-format keystores in place (a rename, which fails
        # with EBUSY on a read-only single-file bind mount and kills the
        # container instantly) — hand it a writable copy in its own directory,
        # mirroring run-migration-test.py.
        keystore_dir = cfg.log_dir / "load-generator-keystore"
        shutil.rmtree(keystore_dir, ignore_errors=True)
        keystore_dir.mkdir(parents=True, exist_ok=True)
        shutil.copy2(faucet_keystore, keystore_dir / "iota.keystore")
        run(["docker", "rm", "-f", "stress-benchmark"], check=False, quiet=True)
        # No --rm: if stress crashes at startup its logs must survive for the
        # liveness check below (cleanup force-removes the container anyway).
        res = run(
            [
                "docker", "run", "-d", "--name", "stress-benchmark",
                "--network", cfg.network_name,
                "-v", f"{genesis_blob.resolve()}:/opt/iota/config/genesis.blob:ro",
                "-v", f"{keystore_dir.resolve()}:/opt/iota/config:rw",
                cfg.spammer_image, "/usr/local/bin/stress",
                "--local", "false",
                "--use-fullnode-for-execution", "true",
                "--fullnode-rpc-addresses", cfg.load_rpc_address,
                "--genesis-blob-path", "/opt/iota/config/genesis.blob",
                "--keystore-path", "/opt/iota/config/iota.keystore",
                "--primary-gas-owner-id", cfg.load_primary_gas_owner_id,
                "bench",
                "--target-qps", str(cfg.spammer_tps),
                "--in-flight-ratio", str(cfg.load_in_flight_ratio),
                "--transfer-object", str(cfg.load_transfer_objects),
            ],
            check=False, quiet=True,
        )
        if res.returncode != 0:
            raise RuntimeError("stress spammer failed to start")
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
            raise RuntimeError(
                "stress spammer exited right after start (logs above)"
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


def run_loop(cfg: Config) -> None:
    log(_phase(f"Running for {cfg.run_duration}s (logs every {cfg.log_interval}s)", "RUN"))
    end = time.time() + cfg.run_duration
    last_save = 0.0
    while time.time() < end:
        if time.time() - last_save >= cfg.log_interval:
            ec.save_validator_logs(cfg.log_dir, cfg.num_validators, prefix="exp")
            last_save = time.time()
        remaining = int(end - time.time())
        log_status(f"  {ec._progress_bar(cfg.run_duration - remaining, cfg.run_duration)} "
                   f"{cfg.run_duration - remaining}s / {cfg.run_duration}s")
        time.sleep(min(5, max(1, remaining)))
    print()
    # Final timestamped snapshot.
    ts = datetime.now().strftime("%Y%m%d-%H%M%S")
    ec.save_validator_logs(cfg.log_dir, cfg.num_validators, prefix=f"experiment-{ts}")


# ========================= Main =========================


def parse_args() -> argparse.Namespace:
    p = argparse.ArgumentParser(description="Benchmark runner for the IOTA private network.")
    p.add_argument("-n", "--num-validators", type=int, default=4, metavar="N")
    p.add_argument("-b", "--build", type=lambda v: v.lower() in ("true", "1", "yes"),
                   default=True)
    p.add_argument("-g", "--geodistributed",
                   type=lambda v: v.lower() in ("true", "1", "yes"), default=True)
    p.add_argument("-s", "--seed", type=int, default=42)
    p.add_argument("-x", "--percent-block", type=int, default=0)
    p.add_argument("-l", "--percent-loss", type=int, default=0)
    p.add_argument("-r", "--percent-restart", type=int, default=0)
    p.add_argument("-t", "--run-duration", type=int, default=3600, metavar="SECONDS")
    p.add_argument("-d", "--restart-duration", type=int, default=120)
    p.add_argument("-w", "--restart-timeout", type=int, default=60)
    p.add_argument("-M", "--restart-mode", default="preserve-consensus",
                   choices=("preserve-consensus", "full-reset", "simple-restart"))
    p.add_argument("-E", "--epoch-duration-ms", type=int, default=1_200_000)
    p.add_argument("-m", "--network-metric", action="store_true")
    p.add_argument("-S", "--spammer", type=lambda v: v.lower() in ("true", "1", "yes"),
                   default=False, dest="spammer_enable")
    p.add_argument("-T", "--spammer-tps", type=int, default=10)
    p.add_argument("-Z", "--spammer-size", default="10KiB")
    p.add_argument("-C", "--spammer-type", default="stress",
                   choices=("stress", "iota-spammer"))
    p.add_argument("--spammer-image", default="iotaledger/stress",
                   help="Docker image for the stress spammer (auto-pulled if missing)")
    p.add_argument("-c", "--chain-override", default="", choices=("", "testnet", "mainnet"))
    p.add_argument("--block-measurement-seconds", type=int, default=90, metavar="S",
                   help="pre-disruption block-production window (0 disables)")
    return p.parse_args()


def main() -> None:
    global _cfg, _latency_proc
    args = parse_args()
    cfg = Config(
        num_validators=args.num_validators, build=args.build,
        geodistributed=args.geodistributed, seed=args.seed,
        percent_block=args.percent_block, percent_loss=args.percent_loss,
        percent_restart=args.percent_restart, run_duration=args.run_duration,
        restart_duration=args.restart_duration, restart_timeout=args.restart_timeout,
        restart_mode=args.restart_mode, epoch_duration_ms=args.epoch_duration_ms,
        network_metric=args.network_metric, spammer_enable=args.spammer_enable,
        spammer_tps=args.spammer_tps, spammer_size=args.spammer_size,
        spammer_type=args.spammer_type, chain_override=args.chain_override,
        block_measurement_seconds=args.block_measurement_seconds,
        spammer_image=args.spammer_image,
    )
    _cfg = cfg
    ec.setup_logging(cfg.log_file)

    def _on_signal(signum: int, _frame: object) -> None:
        cleanup(cfg)
        sys.exit(128 + signum)

    signal.signal(signal.SIGINT, _on_signal)
    signal.signal(signal.SIGTERM, _on_signal)

    log(_phase("Benchmark Configuration"))
    log(f"  Validators        : {cfg.num_validators}")
    log(f"  Build images      : {cfg.build}")
    log(f"  Geodistributed    : {cfg.geodistributed}")
    log(f"  Block / loss / restart %: {cfg.percent_block} / {cfg.percent_loss} / "
        f"{cfg.percent_restart}")
    log(f"  Run duration      : {cfg.run_duration}s")
    log(f"  Spammer           : {cfg.spammer_enable} ({cfg.spammer_type}, "
        f"tps={cfg.spammer_tps})")

    # Take the lock before the try/finally: if another run is active, its
    # containers must not be torn down by this process's cleanup().
    try:
        ec.acquire_single_run_lock("run-benchmark.py")
    except RuntimeError as err:
        log(f"ERROR: {err}")
        sys.exit(1)

    try:
        ec.cache_sudo()
        build_images(cfg)
        if not cfg.build:
            ec.require_local_image(
                cfg.image,
                "run with -b true to build it, or tag an existing build "
                f"(e.g. `docker tag iotaledger/iota-node:latest {cfg.image}`)",
            )
        log(_phase(f"Generating compose file for {cfg.num_validators} validators", "COMPOSE"))
        ec.generate_compose_file(
            cfg.network_dir / cfg.compose_file,
            num_validators=cfg.num_validators,
            base_image=cfg.image,
            chain_override=cfg.chain_override,
            include_fullnode=cfg.spammer_enable,
            fullnode_image=cfg.fullnode_image,
            ip_prefix="10.0.2",  # 10.0.1.x belongs to the migration network
            header="Auto-generated by run-benchmark.py. Do not edit manually.",
        )
        log(_phase(f"Bootstrapping genesis for {cfg.num_validators} validators", "GENESIS"))
        ec.bootstrap_genesis(cfg.network_dir, cfg.num_validators, cfg.epoch_duration_ms)
        log(_phase(f"Starting {cfg.num_validators} validators on {cfg.image}", "START"))
        ec.compose_up_validators(
            cfg.compose_file, None, cfg.network_dir, cfg.num_validators,
        )
        log(_phase("Starting Grafana/Prometheus", "MONITOR"))
        ec.start_grafana(cfg.grafana_dir)
        log(_phase(f"Applying latency ({'geo-high' if cfg.geodistributed else 'geo-low'})",
                   "LATENCY"))
        ec.dump_latency_matrix(
            cfg.script_dir, cfg.num_validators, cfg.geodistributed, cfg.log_file,
            cfg.log_dir / "latency-matrix.tsv",
        )
        _latency_proc = ec.apply_latency(
            cfg.script_dir, cfg.num_validators, cfg.seed, cfg.geodistributed,
            cfg.log_file, cfg.latency_apply_wait,
            percent_block=cfg.percent_block, percent_loss=cfg.percent_loss,
            percent_restart=cfg.percent_restart, restart_duration=cfg.restart_duration,
            restart_timeout=cfg.restart_timeout, restart_mode=cfg.restart_mode,
        )
        if cfg.block_measurement_enabled():
            ec.measure_block_production(cfg.num_validators, cfg.block_measurement_seconds)
        start_spammer(cfg)
        run_loop(cfg)
    finally:
        cleanup(cfg)


if __name__ == "__main__":
    main()
