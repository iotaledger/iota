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
import shutil
import signal
import subprocess
import sys
import time
from dataclasses import dataclass, field
from datetime import datetime
from pathlib import Path

import experiment_common as ec
from experiment_common import _C, log, log_status, run

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
            _network_stats(cfg)
        except Exception:
            pass
    run(["docker", "rm", "-f", "stress-benchmark"], check=False, quiet=True)
    # Stop the fuzzer (it runs under sudo internally) and clear its host rules.
    run(["sudo", "rm", "-f", "/tmp/network-fuzz.stop"], check=False, quiet=True)
    run(["sudo", "pkill", "-9", "-f", r"network-fuzz\.sh"], check=False, quiet=True)
    if _fuzz_proc is not None and _fuzz_proc.poll() is None:
        _fuzz_proc.terminate()
    _clear_fuzzdrop_rules()
    ec.compose_down(cfg.compose_file, None, cfg.network_dir)
    log("Cleanup complete.")


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


# ========================= Shared phases =========================


def build_images(cfg: Config) -> None:
    if not cfg.build:
        log("Skipping image builds")
        return
    log(ec._phase_banner("Building docker images", "BUILD"))
    docker_dir = cfg.script_dir.parent.parent.parent / "docker"
    for name in ("iota-node", "iota-tools", "iota-indexer"):
        ec.run_timed(["./build.sh", "-t", name], f"Building {name}", cwd=docker_dir / name)
    print()


def start_spammer(cfg: Config) -> None:
    if not cfg.spammer_enable:
        return
    duration = max(10, cfg.run_duration - 60)
    log(ec._phase_banner(f"Starting {cfg.spammer_type} spammer (tps={cfg.spammer_tps})", "LOAD"))
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
    log(ec._phase_banner(f"Running for {cfg.run_duration}s (logs every {cfg.log_interval}s)", "RUN"))
    end = time.time() + cfg.run_duration
    last_save = 0.0
    while time.time() < end:
        if time.time() - last_save >= cfg.log_interval:
            ec.save_validator_logs(cfg.log_dir, cfg.num_validators, prefix="fuzz")
            last_save = time.time()
        remaining = int(end - time.time())
        done = cfg.run_duration - remaining
        log_status(f"  {ec._progress_bar(done, cfg.run_duration)} {done}s / {cfg.run_duration}s")
        time.sleep(min(5, max(1, remaining)))
    print()
    ts = datetime.now().strftime("%Y%m%d-%H%M%S")
    ec.save_validator_logs(cfg.log_dir, cfg.num_validators, prefix=f"fuzz-{ts}")


# ========================= Main =========================


def parse_args() -> argparse.Namespace:
    p = argparse.ArgumentParser(description="Fuzz runner for the IOTA private network.")
    p.add_argument("-n", "--num-validators", type=int, default=4, metavar="N")
    p.add_argument("-b", "--build", type=lambda v: v.lower() in ("true", "1", "yes"),
                   default=True)
    p.add_argument("-t", "--topology", default="geo-low", choices=TOPOLOGIES)
    p.add_argument("-s", "--seed", type=int, default=42)
    p.add_argument("-x", "--percent-block", type=int, default=0)
    p.add_argument("-l", "--percent-loss", type=int, default=0)
    p.add_argument("-r", "--percent-restart", type=int, default=0)
    p.add_argument("-d", "--run-duration", type=int, default=3600, metavar="SECONDS")
    p.add_argument("--restart-duration", type=int, default=120)
    p.add_argument("--round-span", type=int, default=0,
                   help="fuzz round length in seconds (0 = 2*restart_duration)")
    p.add_argument("--ttl", type=int, default=0, help="fuzz TTL in seconds (0 = none)")
    p.add_argument("--heal-every-round", type=int, default=0)
    p.add_argument("--heal-num-rounds", type=int, default=0)
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
    return p.parse_args()


def main() -> None:
    global _cfg, _fuzz_proc
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
    )
    _cfg = cfg
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

    # Take the lock before the try/finally: if another run is active, its
    # containers must not be torn down by this process's cleanup().
    try:
        ec.acquire_single_run_lock("run-fuzz.py")
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
        log(ec._phase_banner(f"Generating compose file for {cfg.num_validators} validators", "COMPOSE"))
        ec.generate_compose_file(
            cfg.network_dir / cfg.compose_file,
            num_validators=cfg.num_validators,
            base_image=cfg.image,
            chain_override=cfg.chain_override,
            include_fullnode=cfg.spammer_enable,
            fullnode_image=cfg.fullnode_image,
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
        start_spammer(cfg)
        run_loop(cfg)
    finally:
        cleanup(cfg)


if __name__ == "__main__":
    main()
