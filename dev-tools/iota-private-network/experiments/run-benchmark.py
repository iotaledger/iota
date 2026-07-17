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
import signal
import subprocess
import sys
from dataclasses import dataclass, field
from pathlib import Path

import experiment_common as ec
from experiment_common import log, run


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
    load_primary_gas_owner_id: str = ec.DEFAULT_PRIMARY_GAS_OWNER_ID

    script_dir: Path = field(default_factory=lambda: Path(__file__).resolve().parent)
    network_dir: Path = field(init=False)
    grafana_dir: Path = field(init=False)
    log_dir: Path = field(init=False)
    log_file: Path = field(init=False)
    network_name: str = field(init=False)

    def __post_init__(self) -> None:
        ec.validate_num_validators(self.num_validators)
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
_spammer_proc: subprocess.Popen[str] | None = None


# ========================= Teardown =========================


def cleanup(cfg: Config) -> None:
    global _cleaning
    if _cleaning:
        return
    _cleaning = True
    log(_phase("Cleaning up"))
    if cfg.network_metric:
        try:
            ec.network_stats(cfg.num_validators)
        except Exception:
            pass
    ec.stop_spammer(cfg, _spammer_proc)
    # Kill the latency injector (runs under sudo; escaped dot avoids matching
    # this pkill's own argv).
    run(["sudo", "pkill", "-f", r"network-benchmark\.sh"], check=False, quiet=True)
    if _latency_proc is not None and _latency_proc.poll() is None:
        _latency_proc.terminate()
    ec.compose_down(cfg.compose_file, None, cfg.network_dir)
    log("Cleanup complete.")
    archived = ec.archive_run_log(cfg.log_file, "experiment_script")
    if archived is not None:
        print(f"Coordinator log archived at {archived}")
    ec.close_logging()


def _phase(title: str, phase: str = "") -> str:
    return ec._phase_banner(title, phase)


# ========================= Phases =========================


# ========================= Main =========================


def parse_args() -> argparse.Namespace:
    p = argparse.ArgumentParser(
        description="Benchmark runner for the IOTA private network.",
        epilog=(
            "Defaults: 4 validators, build the local image, geodistributed "
            "latency, 3600s run, no block/loss/restart disruption, no spammer. "
            "Disruption percents (-x/-l/-r) drive network-benchmark.sh."
        ),
    )
    p.add_argument(
        "-n", "--num-validators", type=int, default=4, metavar="N",
        choices=range(ec.MIN_VALIDATORS, ec.MAX_VALIDATORS + 1),
        help="Number of validators to run (2-30, default: 4)",
    )
    p.add_argument("-b", "--build", type=lambda v: v.lower() in ("true", "1", "yes"),
                   default=True,
                   help="Build the local iota-node image before the run (default: true)")
    p.add_argument("-g", "--geodistributed",
                   type=lambda v: v.lower() in ("true", "1", "yes"), default=True,
                   help="Large geodistributed latencies (true) or small ones (false) "
                        "(default: true)")
    p.add_argument("-s", "--seed", type=int, default=42,
                   help="Seed for the deterministic disruption schedule (default: 42)")
    p.add_argument("-x", "--percent-block", type=int, default=0,
                   help="Percent chance to block a connection (default: 0)")
    p.add_argument("-l", "--percent-loss", type=int, default=0,
                   help="Percent chance to apply packet loss (default: 0)")
    p.add_argument("-r", "--percent-restart", type=int, default=0,
                   help="Percent of validators to periodically restart (default: 0)")
    p.add_argument("-t", "--run-duration", type=int, default=3600, metavar="SECONDS",
                   help="Total run duration in seconds (default: 3600)")
    p.add_argument("-d", "--restart-duration", type=int, default=120,
                   help="Seconds a validator remains stopped (default: 120)")
    p.add_argument("-w", "--restart-timeout", type=int, default=60,
                   help="Seconds to verify a restarted validator is running "
                        "(default: 60)")
    p.add_argument("-M", "--restart-mode", default="preserve-consensus",
                   choices=("preserve-consensus", "full-reset", "simple-restart"),
                   help="Database handling on restart (default: preserve-consensus)")
    p.add_argument("-E", "--epoch-duration-ms", type=int, default=1_200_000,
                   help="Epoch duration in milliseconds (default: 1200000 = 20 min)")
    p.add_argument("-m", "--network-metric", action="store_true",
                   help="Collect per-validator network stats at teardown")
    p.add_argument("-S", "--spammer", type=lambda v: v.lower() in ("true", "1", "yes"),
                   default=False, dest="spammer_enable",
                   help="Run a transaction spammer for load (default: false)")
    p.add_argument("-T", "--spammer-tps", type=int, default=10,
                   help="Target transactions per second for the spammer (default: 10)")
    p.add_argument("-Z", "--spammer-size", default="10KiB",
                   help="Transaction payload size, iota-spammer only (default: 10KiB)")
    p.add_argument("-C", "--spammer-type", default="stress",
                   choices=("stress", "iota-spammer"),
                   help="Spammer backend: stress (container) or iota-spammer "
                        "(host clone) (default: stress)")
    p.add_argument("--spammer-image", default="iotaledger/stress",
                   help="Docker image for the stress spammer (pulled if missing, "
                        "else built from the network-benchmark clone)")
    p.add_argument("-c", "--chain-override", default="", choices=("", "testnet", "mainnet"),
                   help="Protocol feature-flag override; empty defaults to testnet "
                        "for the local image")
    p.add_argument("--block-measurement-seconds", type=int, default=90, metavar="S",
                   help="Block-production measurement window under the applied "
                        "latency/disruption/load (0 disables, default: 90)")
    return p.parse_args()


def main() -> None:
    global _cfg, _latency_proc, _spammer_proc
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

    # Take the lock before setup_logging (which truncates the shared log file
    # of the active run) and before the try/finally (whose cleanup() would
    # tear down the active run's containers).
    try:
        ec.acquire_single_run_lock("run-benchmark.py")
    except RuntimeError as err:
        print(f"ERROR: {err}")
        sys.exit(1)

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

    try:
        ec.cache_sudo()
        ec.build_images(cfg.script_dir, cfg.build)
        if cfg.spammer_enable and cfg.spammer_type == "stress":
            # Resolve the load image up front (pull, else build from the
            # network-benchmark clone) instead of surprising the run mid-way.
            ec.ensure_stress_image(cfg.spammer_image)
        if not cfg.build:
            ec.require_local_image(
                cfg.image,
                "run with -b true to build it, or tag an existing build "
                f"(e.g. `docker tag iotaledger/iota-node:latest {cfg.image}`)",
            )
            if cfg.spammer_enable and cfg.spammer_type == "iota-spammer":
                # The generated faucet service runs on the local iota-tools tag.
                ec.require_local_image(
                    "iota-tools",
                    "run with -b true to build it, or tag an existing build "
                    "(e.g. `docker tag iotaledger/iota-tools:latest iota-tools`)",
                )
        log(_phase(f"Generating compose file for {cfg.num_validators} validators", "COMPOSE"))
        ec.generate_compose_file(
            cfg.network_dir / cfg.compose_file,
            num_validators=cfg.num_validators,
            base_image=cfg.image,
            chain_override=cfg.chain_override,
            include_fullnode=cfg.spammer_enable,
            fullnode_image=cfg.fullnode_image,
            # iota-spammer runs on the host: it needs the published fullnode
            # RPC and a faucet for gas.
            include_faucet=(
                cfg.spammer_enable and cfg.spammer_type == "iota-spammer"
            ),
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
        # Start load as soon as the network is up (validators running, latency
        # applied) so the block-production measurement runs under load — matching
        # the migration runner. Previously the spammer started only after the
        # measurement window, leaving it idle.
        _spammer_proc = ec.start_spammer(cfg)
        if cfg.block_measurement_enabled():
            ec.measure_block_production(cfg.num_validators, cfg.block_measurement_seconds)
        ec.run_loop(cfg, "exp", "experiment")
    finally:
        cleanup(cfg)


if __name__ == "__main__":
    main()
