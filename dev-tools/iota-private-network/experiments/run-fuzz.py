#!/usr/bin/env python3

# Copyright (c) 2026 IOTA Stiftung
# SPDX-License-Identifier: Apache-2.0

"""Fuzz runner for the IOTA private network.

Replaces the former run-all-fuzz.sh. Generates its docker compose file per
run (one service block per validator), so it scales past the 19 services
hand-written in the static docker-compose.yaml. Brings up N validators on a
locally built image, then drives network-fuzz.sh to apply a topology latency
profile (ring / star / non-triangle / random / geo-high / geo-low) plus
packet loss, host-level connection blocking, periodic validator restarts, and
optional heal rounds / TTL. Optionally runs a transaction spammer, runs for a
fixed duration while collecting logs, and tears everything down cleanly.

Shared infrastructure lives in experiment_common.py; only the fuzz injection
and its teardown are fuzz-specific here.

Run from: iota/dev-tools/iota-private-network/experiments/
"""

from __future__ import annotations

import argparse
import os
import signal
import subprocess
import sys
import time
from dataclasses import dataclass, field
from datetime import datetime
from pathlib import Path

import experiment_common as ec
from experiment_common import log, log_status, run

TOPOLOGIES = ("ring", "star", "non-triangle", "random", "geo-high", "geo-low")


@dataclass
class Config:
    """All parameters for a fuzz run."""

    num_validators: int = 4
    build: bool = True
    topology: str = "geo-low"
    seed: int = 42
    percent_block: int = 0
    percent_loss: int = 0
    percent_restart: int = 0
    run_duration: int = 3600
    restart_duration: int = 120
    round_span: int = 0       # 0 = network-fuzz.sh default (2*restart_duration)
    ttl: int = 0              # 0 = no TTL
    heal_every_round: int = 0
    heal_num_rounds: int = 0
    epoch_duration_ms: int = 1_200_000
    network_metric: bool = False
    block_measurement_seconds: int = 90
    spammer_enable: bool = False
    spammer_tps: int = 10
    spammer_size: str = "10KiB"
    spammer_type: str = "stress"
    chain_override: str = ""

    image: str = "iota-node"
    fullnode_image: str = "iota-node"
    spammer_image: str = "iotaledger/stress"
    compose_file: str = "docker-compose.fuzz.yaml"
    log_interval: int = 60
    fuzz_apply_wait: int = 15
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
        if self.topology not in TOPOLOGIES:
            raise ValueError(f"topology must be one of {TOPOLOGIES}, got {self.topology!r}")
        if self.spammer_type not in ("stress", "iota-spammer"):
            raise ValueError(f"invalid spammer type: {self.spammer_type!r}")
        for pct in (self.percent_block, self.percent_loss, self.percent_restart):
            if not 0 <= pct <= 100:
                raise ValueError("percentages must be in [0, 100]")
        self.network_dir = self.script_dir.parent
        self.grafana_dir = self.network_dir / ".." / "grafana-local"
        self.log_dir = self.script_dir / "logs"
        self.log_file = self.log_dir / "experiment_script_latest.log"
        self.network_name = f"{self.network_dir.name}_iota-network"
        if not self.chain_override:
            self.chain_override = "testnet"


_cfg: Config | None = None
_cleaning = False
_fuzz_proc: subprocess.Popen[str] | None = None
_spammer_proc: subprocess.Popen[str] | None = None


# ========================= Fuzz-specific phases =========================


def apply_fuzz(cfg: Config) -> subprocess.Popen[str]:
    """Launch network-fuzz.sh (it self-sudos for tc/iptables). Returns the
    running process. A separate timestamped fuzz log keeps its per-round
    output out of the main script log."""
    ts = datetime.now().strftime("%Y%m%d-%H%M%S")
    fuzz_log = cfg.log_dir / f"fuzz_{ts}.log"
    # Clear any leftover stop/lock files from a previous fuzz run.
    run(["sudo", "rm", "-f", "/tmp/network-fuzz.stop",
         "/tmp/network-fuzz-single.lock"], check=False, quiet=True)
    env = dict(os.environ)
    env["HEAL_EVERY_ROUND"] = str(cfg.heal_every_round)
    env["HEAL_NUM_ROUNDS"] = str(cfg.heal_num_rounds)
    out = cfg.log_file.open("a")
    proc = subprocess.Popen(
        [
            "./network-fuzz.sh",
            "-n", str(cfg.num_validators),
            "-s", str(cfg.seed),
            "-b", str(cfg.percent_block),
            "-l", str(cfg.percent_loss),
            "-r", str(cfg.percent_restart),
            "-t", cfg.topology,
            "-d", str(cfg.restart_duration),
            "--round-span", str(cfg.round_span),
            "--ttl", str(cfg.ttl),
            "-o", str(fuzz_log.resolve()),
        ],
        cwd=cfg.script_dir, env=env, stdout=out, stderr=subprocess.STDOUT,
    )
    out.close()
    for sec in range(cfg.fuzz_apply_wait):
        if proc.poll() is not None:
            raise RuntimeError(
                f"network-fuzz.sh exited early with code {proc.returncode}"
            )
        log_status(f"  Waiting for fuzz application... {sec + 1}s")
        time.sleep(1)
    print()
    log(f"  Fuzz ({cfg.topology}) applied; log: {fuzz_log}")
    return proc


def _clear_fuzzdrop_rules() -> None:
    """Remove leftover fuzzdrop iptables rules in the host DOCKER-USER chain."""
    listing = run(
        ["sudo", "iptables", "-L", "DOCKER-USER", "-n", "--line-numbers"],
        capture=True, check=False, quiet=True,
    )
    nums = [
        line.split()[0]
        for line in listing.stdout.splitlines()
        if "fuzzdrop:" in line and line.split() and line.split()[0].isdigit()
    ]
    for num in sorted(nums, key=int, reverse=True):
        run(["sudo", "iptables", "-D", "DOCKER-USER", num], check=False, quiet=True)


# ========================= Teardown =========================


def cleanup(cfg: Config) -> None:
    global _cleaning
    if _cleaning:
        return
    _cleaning = True
    log(ec._phase_banner("Cleaning up"))
    if cfg.network_metric:
        try:
            ec.network_stats(cfg.num_validators)
        except Exception:
            pass
    ec.stop_spammer(cfg, _spammer_proc)
    # Stop the fuzzer (it runs under sudo internally) and clear its host rules.
    run(["sudo", "rm", "-f", "/tmp/network-fuzz.stop"], check=False, quiet=True)
    run(["sudo", "pkill", "-9", "-f", r"network-fuzz\.sh"], check=False, quiet=True)
    if _fuzz_proc is not None and _fuzz_proc.poll() is None:
        _fuzz_proc.terminate()
    _clear_fuzzdrop_rules()
    ec.compose_down(cfg.compose_file, None, cfg.network_dir)
    log("Cleanup complete.")
    archived = ec.archive_run_log(cfg.log_file, "experiment_script")
    if archived is not None:
        print(f"Coordinator log archived at {archived}")
    ec.close_logging()



# ========================= Shared phases =========================



# ========================= Main =========================


def parse_args() -> argparse.Namespace:
    p = argparse.ArgumentParser(
        description="Fuzz runner for the IOTA private network.",
        epilog=(
            "Defaults: 4 validators, build the local image, geo-low topology, "
            "3600s run, no block/loss/restart fuzz, no spammer. Topology and "
            "disruptions are applied by network-fuzz.sh."
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
    p.add_argument("-t", "--topology", default="geo-low", choices=TOPOLOGIES,
                   help="Latency topology profile (default: geo-low)")
    p.add_argument("-s", "--seed", type=int, default=42,
                   help="Seed for the deterministic fuzz schedule (default: 42)")
    p.add_argument("-x", "--percent-block", type=int, default=0,
                   help="Percent of validator pairs to block bidirectionally "
                        "(default: 0)")
    p.add_argument("-l", "--percent-loss", type=int, default=0,
                   help="Percent of validators to apply packet loss to (default: 0)")
    p.add_argument("-r", "--percent-restart", type=int, default=0,
                   help="Percent of validators to restart per fuzz round (default: 0)")
    p.add_argument("-d", "--run-duration", type=int, default=3600, metavar="SECONDS",
                   help="Total run duration in seconds (default: 3600)")
    p.add_argument("--restart-duration", type=int, default=120,
                   help="Seconds validators remain stopped in restart rounds "
                        "(default: 120)")
    p.add_argument("--round-span", type=int, default=0,
                   help="fuzz round length in seconds (0 = 2*restart_duration)")
    p.add_argument("--ttl", type=int, default=0, help="fuzz TTL in seconds (0 = none)")
    p.add_argument("--heal-every-round", type=int, default=0,
                   help="Heal (clear drops) every N fuzz rounds (0 = disabled)")
    p.add_argument("--heal-num-rounds", type=int, default=0,
                   help="Consecutive heal rounds per heal window (default: 0)")
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
                   help="block-production measurement window after fuzz is applied "
                        "(0 disables, default: 90)")
    return p.parse_args()


def main() -> None:
    global _cfg, _fuzz_proc, _spammer_proc
    args = parse_args()
    cfg = Config(
        num_validators=args.num_validators, build=args.build, topology=args.topology,
        seed=args.seed, percent_block=args.percent_block, percent_loss=args.percent_loss,
        percent_restart=args.percent_restart, run_duration=args.run_duration,
        restart_duration=args.restart_duration, round_span=args.round_span, ttl=args.ttl,
        heal_every_round=args.heal_every_round, heal_num_rounds=args.heal_num_rounds,
        epoch_duration_ms=args.epoch_duration_ms, network_metric=args.network_metric,
        spammer_enable=args.spammer_enable, spammer_tps=args.spammer_tps,
        spammer_size=args.spammer_size, spammer_type=args.spammer_type,
        spammer_image=args.spammer_image, chain_override=args.chain_override,
        block_measurement_seconds=args.block_measurement_seconds,
    )
    _cfg = cfg

    # Take the lock before setup_logging (which truncates the shared log file
    # of the active run) and before the try/finally (whose cleanup() would
    # tear down the active run's containers).
    try:
        ec.acquire_single_run_lock("run-fuzz.py")
    except RuntimeError as err:
        print(f"ERROR: {err}")
        sys.exit(1)

    ec.setup_logging(cfg.log_file)

    def _on_signal(signum: int, _frame: object) -> None:
        cleanup(cfg)
        sys.exit(128 + signum)

    signal.signal(signal.SIGINT, _on_signal)
    signal.signal(signal.SIGTERM, _on_signal)

    log(ec._phase_banner("Fuzz Configuration"))
    log(f"  Validators        : {cfg.num_validators}")
    log(f"  Topology          : {cfg.topology}")
    log(f"  Block / loss / restart %: {cfg.percent_block} / {cfg.percent_loss} / "
        f"{cfg.percent_restart}")
    log(f"  Round span / TTL  : {cfg.round_span}s / {cfg.ttl}s")
    log(f"  Heal every/num    : {cfg.heal_every_round} / {cfg.heal_num_rounds}")
    log(f"  Run duration      : {cfg.run_duration}s")
    log(f"  Spammer           : {cfg.spammer_enable} ({cfg.spammer_type}, tps={cfg.spammer_tps})")

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
        log(ec._phase_banner(f"Generating compose file for {cfg.num_validators} validators", "COMPOSE"))
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
            header="Auto-generated by run-fuzz.py. Do not edit manually.",
        )
        log(ec._phase_banner(f"Bootstrapping genesis for {cfg.num_validators} validators", "GENESIS"))
        ec.bootstrap_genesis(cfg.network_dir, cfg.num_validators, cfg.epoch_duration_ms)
        log(ec._phase_banner(f"Starting {cfg.num_validators} validators on {cfg.image}", "START"))
        ec.compose_up_validators(cfg.compose_file, None, cfg.network_dir, cfg.num_validators)
        log(ec._phase_banner("Starting Grafana/Prometheus", "MONITOR"))
        ec.start_grafana(cfg.grafana_dir)
        log(ec._phase_banner(f"Applying fuzz ({cfg.topology})", "FUZZ"))
        _fuzz_proc = apply_fuzz(cfg)
        # Start load as soon as the network is up (validators running, fuzz
        # applied) so the block-production measurement runs under load — matching
        # the migration runner. Previously the spammer started only after the
        # measurement window, leaving it idle.
        _spammer_proc = ec.start_spammer(cfg)
        if cfg.block_measurement_seconds > 0:
            ec.measure_block_production(
                cfg.num_validators, cfg.block_measurement_seconds,
            )
        ec.run_loop(cfg, "fuzz", "fuzz")
    finally:
        cleanup(cfg)


if __name__ == "__main__":
    main()
