#!/usr/bin/env python3

# Copyright (c) 2026 IOTA Stiftung
# SPDX-License-Identifier: Apache-2.0

"""Half-network upgrade test.

Brings up all validators on a released image, then mid-epoch-0 rolls the
locally-built upgrade image out to a deterministic random half of the
network. The mixed-binary network holds the lower of the two halves'
MAX_PROTOCOL_VERSION values (the new-binary half is below 2f+1) for
`--mid-observation-epochs` full epochs (epoch 1 by default). At the start of
the following epoch (epoch 2 by default), upgrades the rest; after the next
boundary the whole network supports (and, if HEAD's version is higher,
advances to) the new protocol. Observes one final epoch.

Run from: iota/dev-tools/iota-private-network/experiments/
"""

from __future__ import annotations

import argparse
import atexit
import importlib.util
import random
import signal
import subprocess
import sys
import time
from pathlib import Path


# ========================= Load migration_test as a module =========================
#
# run-migration-test.py holds the shared machinery (Config, phases, log
# helpers, cleanup). Its hyphenated filename blocks a normal import, so load
# it via importlib.

_HERE = Path(__file__).resolve().parent
_MIGRATION_PATH = _HERE / "run-migration-test.py"
if not _MIGRATION_PATH.exists():
    print(f"FATAL: cannot find {_MIGRATION_PATH}", file=sys.stderr)
    sys.exit(1)

_spec = importlib.util.spec_from_file_location("migration_test", _MIGRATION_PATH)
mt = importlib.util.module_from_spec(_spec)  # type: ignore[arg-type]
sys.modules["migration_test"] = mt
_spec.loader.exec_module(mt)  # type: ignore[union-attr]

# Re-bind frequently used names so the rest of this file reads cleanly.
Config = mt.Config
log = mt.log
log_status = mt.log_status
run = mt.run
docker_compose = mt.docker_compose
save_validator_logs = mt.save_validator_logs
get_current_epoch = mt.get_current_epoch
wait_for_epoch_change = mt.wait_for_epoch_change
_phase_banner = mt._phase_banner
_phase_complete = mt._phase_complete
_progress_bar = mt._progress_bar
_read_validator_protocol_info = mt._read_validator_protocol_info
phase1_docker_images = mt.phase1_docker_images
phase2_generate_compose = mt.phase2_generate_compose
phase3_bootstrap_genesis = mt.phase3_bootstrap_genesis
phase4_start_validators = mt.phase4_start_validators
phase5_start_monitoring = mt.phase5_start_monitoring
phase6_apply_latency = mt.phase6_apply_latency
cleanup = mt.cleanup
_signal_handler = mt._signal_handler
_C = mt._C


# ========================= Patch Config for half-upgrade timing =========================
#
# Override migration_test's __post_init__ to drop the phase-9 (restart-stress)
# epoch budget this test never uses, and force advanced mode. Simple-mode-only
# fields get placeholder values so the dataclass stays complete.


def _half_upgrade_post_init(self: Config) -> None:
    # Input validation.
    mt.ec.validate_num_validators(self.num_validators)
    if self.load_qps < 0:
        raise ValueError("load qps must be >= 0")
    if self.load_in_flight_ratio <= 0:
        raise ValueError("load in-flight ratio must be > 0")
    if self.load_transfer_objects <= 0:
        raise ValueError("load transfer objects must be > 0")
    # Force advanced mode: phase 7 wait, no simple-mode measurement.
    self.mode = "advanced"

    epoch_s = self.epoch_duration_ms // 1000
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
    # 15s, matching the base config.
    self.protocol_probe_wait = 15
    self.restart_settle_wait = min(10, max(1, self.rolling_restart_pause_max // 3))
    self.restart_pause_keep_db = max(
        1, min(epoch_s // 30, self.rolling_restart_pause_max // 2)
    )
    self.restart_pause_wipe_db = max(1, min(epoch_s // 20, self.rolling_restart_pause_max))

    self.phase8_worst_case = (
        n * (self.rolling_restart_pause_max + self.upgrade_delay)
        + self.protocol_probe_wait
    )
    # Half-upgrade-specific: no restart-stress phase.
    self.phase9_epoch0_worst_case = 0
    # Simple-mode-only placeholders (unused in this test).
    self.phase8_simple_estimate = n * 10 + self.protocol_probe_wait + 5
    self.pre_rolling_wait = 0
    self.stable_window_seconds = 0

    self.timeline_safety_margin = min(max(10, epoch_s // 60), max(0, epoch_s // 10))
    self.mid_epoch_wait = (
        epoch_s
        - self.phase8_worst_case
        - self.timeline_safety_margin
        - self.epoch_start_slop_seconds
    )
    if self.mid_epoch_wait < 0:
        required = (
            self.phase8_worst_case
            + self.timeline_safety_margin
            + self.epoch_start_slop_seconds
        )
        raise ValueError(
            "epoch duration is too short for the half-upgrade schedule: "
            f"need at least {required}s for {self.num_validators} validators, "
            f"got {epoch_s}s"
        )

    self.network_dir = self.script_dir.parent
    self.repo_root = mt._find_repo_root(self.script_dir)
    self.grafana_dir = self.network_dir / ".." / "grafana-local"
    self.log_dir = self.script_dir / "logs"
    self.log_file = self.log_dir / "migration_script_latest.log"

    if not self.chain_override:
        if self.release_network in ("testnet", "mainnet"):
            self.chain_override = self.release_network


Config.__post_init__ = _half_upgrade_post_init


# ========================= Local helpers =========================


def split_halves(num_validators: int, seed: int) -> tuple[list[int], list[int]]:
    """Deterministic random partition of [1..num_validators] into two sorted
    halves: floor(N/2) upgraded first (mid-epoch 0), the rest second."""
    indices = list(range(1, num_validators + 1))
    random.Random(seed).shuffle(indices)
    half = num_validators // 2
    return sorted(indices[:half]), sorted(indices[half:])


def rolling_upgrade(cfg: Config, indices: list[int], label: str) -> None:
    """Roll the given validator subset forward to cfg.image_upgrade, one at a
    time with a randomized offline pause. Called once per half."""
    env_path = cfg.network_dir / cfg.env_migration_file
    rng = random.Random(cfg.seed + sum(indices))  # tied to the subset for determinism

    total = len(indices)
    upgrade_start = time.time()
    for i_pos, i in enumerate(indices):
        v = f"validator-{i}"
        bar = _progress_bar(i_pos, total)
        log_status(f"  {bar} {label}: upgrading {_C.BOLD}{v}{_C.RESET}...")

        # Snapshot pre-upgrade logs so the old-image side of the comparison survives
        with (cfg.log_dir / f"pre-upgrade-{v}.log").open("w") as fh:
            subprocess.run(
                ["docker", "logs", v],
                stdout=fh,
                stderr=subprocess.STDOUT,
                check=False,
            )

        with env_path.open("a") as f:
            f.write(f"VALIDATOR_{i}_IMAGE={cfg.image_upgrade}\n")

        docker_compose(cfg, ["stop", v], quiet=True)
        restart_pause = rng.randint(
            cfg.rolling_restart_pause_min,
            cfg.rolling_restart_pause_max,
        )
        log_status(f"  {bar} {label}: {v} stopped — restarting in {restart_pause}s...")
        time.sleep(restart_pause)
        docker_compose(cfg, ["up", "-d", "--no-deps", v], quiet=True)
        time.sleep(cfg.upgrade_delay)

        result = run(
            ["docker", "ps", "--format", "{{.Names}}"],
            capture=True,
            quiet=True,
        )
        running = set(result.stdout.strip().splitlines())
        if v in running:
            bar = _progress_bar(i_pos + 1, total)
            log_status(f"  {bar} {_C.GREEN}✔{_C.RESET} {label}: {v} upgraded")
        else:
            print()  # newline before error
            raise RuntimeError(f"{v} failed to start after upgrade!")

    print()  # finish status line
    log(
        f"  {label}: rolled {total} validator(s) "
        f"({indices[0]}..{indices[-1]} subset) in {time.time() - upgrade_start:.1f}s"
    )


def _prometheus_int(expr: str) -> int | None:
    value = mt.ec.prometheus_scalar(expr)
    return int(float(value)) if value is not None else None


def read_onchain_protocol_version(attempts: int = 6, delay: float = 5.0) -> int:
    """Network-wide on-chain protocol version from Prometheus. Raises if the
    metric stays missing or validators keep disagreeing."""
    last = "no data"
    for attempt in range(attempts):
        lo = _prometheus_int("min(iota_current_protocol_version)")
        hi = _prometheus_int("max(iota_current_protocol_version)")
        if lo is not None and hi is not None:
            if lo == hi:
                return lo
            last = f"validators disagree: min={lo}, max={hi}"
        if attempt < attempts - 1:
            time.sleep(delay)
    raise RuntimeError(
        "could not read a consistent iota_current_protocol_version from "
        f"Prometheus after {attempts} attempts ({last}); old release images "
        "may not export this metric"
    )


# ========================= Phase wrappers tailored to half-upgrade =========================


def _wait_for_epoch_with_log_save(cfg: Config, epoch_before: int) -> int:
    """Wait for the epoch to advance past `epoch_before`, saving validator
    logs every cfg.log_interval seconds during the wait. Returns the new
    epoch (or the current one if the timeout elapses)."""
    timeout = cfg.epoch_duration_ms // 1000 * 3 // 2  # 1.5x epoch duration
    start = time.time()
    last_log_save = start
    log(f"  Waiting for epoch > {epoch_before}...")
    while True:
        epoch_now = get_current_epoch()
        if epoch_now > epoch_before:
            print()  # finish status line
            log(f"  {_C.GREEN}Epoch advanced to {epoch_now}{_C.RESET} (was {epoch_before})")
            return epoch_now

        elapsed = int(time.time() - start)
        if elapsed >= timeout:
            print()
            log(
                f"  {_C.YELLOW}WARNING: Epoch did not advance within {timeout}s "
                f"— proceeding anyway{_C.RESET}"
            )
            return epoch_now

        if time.time() - last_log_save >= cfg.log_interval:
            save_validator_logs(cfg, cfg.num_validators)
            last_log_save = time.time()

        bar = _progress_bar(elapsed, timeout)
        log_status(f"  Epoch wait: {bar} epoch={epoch_now}, {elapsed}s / {timeout}s")
        time.sleep(10)


def _wait_mid_epoch(cfg: Config, epoch_start: float, phase_label: str) -> None:
    """Wait until mid-epoch before the rolling upgrade, with a caller-supplied
    phase-banner label."""
    phase_start = time.time()
    epoch_s = cfg.epoch_duration_ms // 1000
    elapsed = int(time.time() - epoch_start)
    required_after = cfg.phase8_worst_case
    remaining = epoch_s - elapsed
    if remaining < required_after:
        raise RuntimeError(
            "not enough epoch time left for half-upgrade schedule: "
            f"remaining={remaining}s, required={required_after}s (Phase 8 worst-case)"
        )

    wait_s = max(0, cfg.mid_epoch_wait - elapsed)
    log(_phase_banner(f"Waiting {wait_s}s before rolling upgrade", phase_label))
    log(
        f"  Epoch elapsed={elapsed}s, reserved after wait={required_after}s, "
        f"safety={max(0, remaining - wait_s - required_after)}s"
    )

    start = time.time()
    last_log_save = start
    while time.time() < start + wait_s:
        if time.time() - last_log_save >= cfg.log_interval:
            save_validator_logs(cfg, cfg.num_validators)
            last_log_save = time.time()
        time.sleep(1)

    log(_phase_complete(phase_label, time.time() - phase_start))


def phase8_first_half(cfg: Config, first_half: list[int]) -> None:
    phase_start = time.time()
    log(_phase_banner(f"Rolling upgrade — first half ({len(first_half)} validators)", "PHASE 8"))
    log(f"  Upgrading: {first_half}")
    rolling_upgrade(cfg, first_half, label="first half")
    log(_phase_complete("Phase 8", time.time() - phase_start))


def phase11_second_half(cfg: Config, second_half: list[int]) -> None:
    phase_start = time.time()
    log(_phase_banner(f"Rolling upgrade — second half ({len(second_half)} validators)", "PHASE 11"))
    log(f"  Upgrading: {second_half}")
    rolling_upgrade(cfg, second_half, label="second half")
    log(_phase_complete("Phase 11", time.time() - phase_start))


def phase9_observe_mixed(
    cfg: Config, epoch_0: int, initial_proto_version: int
) -> int:
    """Hold the mixed-binary network from the 0→1 boundary through
    `cfg.mid_observation_epochs` extra epochs. Returns the epoch it ended on.

    Raises if the on-chain protocol version moves while the binaries are
    mixed (the upgraded half is below 2f+1, so it must not)."""
    phase_start = time.time()
    log(_phase_banner("Waiting for epoch 0→1 transition (mixed binaries)", "PHASE 9"))

    def check_protocol_unchanged() -> None:
        onchain = read_onchain_protocol_version()
        if cfg.head_only:
            log(f"  On-chain protocol version {onchain} (head-only mode: an "
                "advance at any boundary is legitimate; not asserted)")
        elif onchain != initial_proto_version:
            raise RuntimeError(
                f"on-chain protocol version moved from {initial_proto_version} "
                f"to {onchain} in the mixed-binary state; the upgraded half is "
                "below 2f+1 and must not carry a version change"
            )
        else:
            log(f"  On-chain protocol version {onchain} — unchanged, as expected "
                "(upgraded half below 2f+1).")

    current_epoch = _wait_for_epoch_with_log_save(cfg, epoch_0)
    if current_epoch <= epoch_0:
        raise RuntimeError(f"Epoch did not advance past {epoch_0}")

    log(f"  Reading validator-1 protocol info...")
    time.sleep(cfg.protocol_probe_wait)
    proto, consensus = _read_validator_protocol_info("validator-1", last=True)
    log(
        f"  {_C.CYAN}Epoch {current_epoch}{_C.RESET} (mixed binaries) — "
        f"max_protocol={proto or 'unknown'}, consensus={consensus or 'unknown'}"
    )
    check_protocol_unchanged()

    extra = getattr(cfg, "mid_observation_epochs", 0)
    for i in range(extra):
        log(f"  [{i + 1}/{extra}] Holding mixed-binary state for one more epoch...")
        new_epoch = _wait_for_epoch_with_log_save(cfg, current_epoch)
        if new_epoch <= current_epoch:
            log(f"  {_C.YELLOW}WARN: epoch did not advance further; stopping mixed-state observation{_C.RESET}")
            break
        current_epoch = new_epoch
        log(f"  Mixed-binary epoch advanced to {current_epoch} (extra observation)")
        check_protocol_unchanged()

    log(_phase_complete("Phase 9", time.time() - phase_start))
    return current_epoch


def phase12_observe_upgraded(
    cfg: Config, second_half_upgrade_epoch: int, initial_proto_version: int
) -> int:
    """Cross the post-upgrade epoch boundary (protocol advances here if HEAD's
    version is higher), then hold `cfg.post_observation_epochs` extra epochs.
    Returns the final epoch.

    Raises if the on-chain protocol version does not land on the version the
    whole (now all-upgraded) committee supports, or moves off it later."""
    phase_start = time.time()
    log(_phase_banner("Waiting for post-second-half-upgrade epoch boundary", "PHASE 12"))

    current_epoch = _wait_for_epoch_with_log_save(cfg, second_half_upgrade_epoch)
    if current_epoch <= second_half_upgrade_epoch:
        raise RuntimeError(f"Epoch did not advance past {second_half_upgrade_epoch}")

    log(f"  Reading validator-1 protocol info...")
    time.sleep(cfg.protocol_probe_wait)
    proto, consensus = _read_validator_protocol_info("validator-1", last=True)
    log(
        f"  {_C.GREEN}Epoch {current_epoch}{_C.RESET} (all upgraded) — "
        f"max_protocol={proto or 'unknown'}, consensus={consensus or 'unknown'}"
    )

    # Every validator now runs HEAD, so the committee-wide supported maximum
    # is the lowest configured max across the network; the chain must sit on
    # it (== the initial version when HEAD brings no protocol bump).
    supported = _prometheus_int("min(iota_configured_max_protocol_version)")
    if supported is None:
        raise RuntimeError(
            "could not read iota_configured_max_protocol_version from Prometheus"
        )
    expected = max(initial_proto_version, supported)

    def check_protocol_settled() -> None:
        onchain = read_onchain_protocol_version()
        if onchain != expected:
            raise RuntimeError(
                f"on-chain protocol version is {onchain} after the all-upgraded "
                f"boundary; expected {expected} (started at {initial_proto_version}, "
                f"every validator supports up to {supported})"
            )
        log(f"  On-chain protocol version {onchain} — as expected (started at "
            f"{initial_proto_version}, supported max {supported}).")

    check_protocol_settled()

    extra = getattr(cfg, "post_observation_epochs", 0)
    for i in range(extra):
        log(f"  [{i + 1}/{extra}] Observing post-upgrade steady-state for one more epoch...")
        new_epoch = _wait_for_epoch_with_log_save(cfg, current_epoch)
        if new_epoch <= current_epoch:
            log(f"  {_C.YELLOW}WARN: epoch did not advance further; stopping post-upgrade observation{_C.RESET}")
            break
        current_epoch = new_epoch
        log(f"  Post-upgrade epoch advanced to {current_epoch}")
        check_protocol_settled()

    log(_phase_complete("Phase 12", time.time() - phase_start))
    return current_epoch


# ========================= CLI =========================


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Half-network upgrade test for IOTA validators.",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog=(
            "Defaults: 19 validators, 15-min epoch, geodistributed latency, "
            "two rolling-upgrade phases (first half mid-epoch-0, second half "
            "at the start of epoch 2) with one full mixed-binary epoch "
            "between them."
        ),
    )
    parser.add_argument(
        "-r", "--release-network",
        default="devnet",
        choices=("devnet", "testnet", "mainnet", "alphanet"),
        help="Release network to pull the old image from (default: devnet)",
    )
    parser.add_argument(
        "-b", "--build",
        default=True,
        type=lambda v: v.lower() in ("true", "1", "yes"),
        help="Whether to build the local upgrade image (default: true)",
    )
    parser.add_argument(
        "-n", "--num-validators",
        default=19,
        type=int,
        choices=range(mt.ec.MIN_VALIDATORS, mt.ec.MAX_VALIDATORS + 1),
        metavar="N",
        help="Number of validators to run (2-30, default: 19)",
    )
    parser.add_argument(
        "-c", "--chain-override",
        default="",
        choices=("", "testnet", "mainnet"),
        help="Chain override for protocol feature flags (default: none = devnet-like)",
    )
    parser.add_argument(
        "-e", "--epoch-duration",
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
        "--seed",
        default=42,
        type=int,
        help="Seed for the deterministic split into halves (default: 42)",
    )
    parser.add_argument(
        "--mid-observation-epochs",
        default=1,
        type=int,
        metavar="N",
        help=(
            "Additional full epochs to hold the mixed-binary state between "
            "first-half and second-half upgrades (default: 1). 0 = behaviour "
            "of the original test (move on as soon as the 0->1 boundary fires)."
        ),
    )
    parser.add_argument(
        "--post-observation-epochs",
        default=1,
        type=int,
        metavar="N",
        help=(
            "Full epochs of post-upgrade steady-state observation after the "
            "all-upgraded epoch boundary (default: 1). 0 = no extra "
            "observation past the boundary itself."
        ),
    )
    parser.add_argument(
        "--head-only",
        action="store_true",
        help=(
            "Start every validator on the locally-built HEAD image from "
            "genesis (rolling upgrades become same-binary restarts). Isolates "
            "orchestrator overhead from binary-version effects."
        ),
    )
    return parser.parse_args()


# ========================= Main =========================


def main() -> None:
    args = parse_args()

    # Shared with the other orchestrators via /tmp/iota-experiments.lock: they
    # share container names, the docker network, and validator tc/iptables
    # state, so concurrent runs corrupt each other. Acquire it here too.
    mt.ec.acquire_single_run_lock("run-half-upgrade-test.py")

    try:
        cfg = Config(
            release_network=args.release_network,
            build=args.build,
            chain_override=args.chain_override,
            num_validators=args.num_validators,
            geodistributed=args.geodistributed,
            seed=args.seed,
            epoch_duration_ms=args.epoch_duration * 60_000,
        )
    except ValueError as err:
        print(f"Configuration error: {err}", file=sys.stderr)
        sys.exit(2)

    # Distinct log file from the migration test so transcripts don't get clobbered
    cfg.log_file = cfg.log_dir / "half_upgrade_script_latest.log"
    # Extra observation knobs (Config is a non-frozen dataclass).
    cfg.mid_observation_epochs = max(0, args.mid_observation_epochs)
    cfg.post_observation_epochs = max(0, args.post_observation_epochs)
    cfg.head_only = bool(args.head_only)
    mt._cfg = cfg

    if cfg.script_dir.name != "experiments":
        print("Error: run from experiments/", file=sys.stderr)
        sys.exit(1)

    # Route log() to the half-upgrade log file.
    mt.ec.setup_logging(cfg.log_file)

    atexit.register(cleanup)
    signal.signal(signal.SIGINT, _signal_handler)
    signal.signal(signal.SIGTERM, _signal_handler)

    first_half, second_half = split_halves(cfg.num_validators, cfg.seed)

    log(_phase_banner("Half-Network Upgrade Test Configuration"))
    log(f"  {_C.BOLD}Validators{_C.RESET}           : {cfg.num_validators}")
    log(f"  {_C.BOLD}Epoch duration{_C.RESET}       : {cfg.epoch_duration_ms // 60_000} min")
    log(f"  {_C.BOLD}Release network{_C.RESET}      : {cfg.release_network}")
    log(f"  {_C.BOLD}Chain override{_C.RESET}       : {cfg.chain_override or 'none (devnet-like)'}")
    log(f"  {_C.BOLD}Build local image{_C.RESET}    : {cfg.build}")
    log(f"  {_C.BOLD}Geodistributed{_C.RESET}      : {cfg.geodistributed}")
    log(f"  {_C.BOLD}Split seed{_C.RESET}          : {cfg.seed}")
    log(f"  {_C.BOLD}First half (mid-epoch 0){_C.RESET} : {first_half}  ({len(first_half)} vals)")
    log(f"  {_C.BOLD}Second half (start of epoch {1 + cfg.mid_observation_epochs}){_C.RESET}: {second_half}  ({len(second_half)} vals)")
    log(f"  {_C.BOLD}Mid-epoch wait{_C.RESET}      : {cfg.mid_epoch_wait}s")
    log(f"  {_C.BOLD}Rolling offline pause{_C.RESET}: {cfg.rolling_restart_pause_min}-{cfg.rolling_restart_pause_max}s per validator")
    log(f"  {_C.BOLD}Mixed-binary observation{_C.RESET}: {cfg.mid_observation_epochs} extra epoch(s) of mixed-binary state between halves")
    log(f"  {_C.BOLD}Post-upgrade observation{_C.RESET} : {cfg.post_observation_epochs} extra epoch(s) after all validators on HEAD")
    if cfg.head_only:
        log(f"  {_C.BOLD}Head-only mode{_C.RESET}      : ON — all validators start on HEAD "
            "(rolling 'upgrades' will be same-binary restarts)")

    # --- Setup phases (reused from migration test) ---
    local_branch, local_commit = phase1_docker_images(cfg)

    # head-only: every validator starts on the HEAD image. Do this after
    # phase1 so its (cached) release-image pull still runs unchanged.
    if cfg.head_only:
        log(
            f"  {_C.BOLD}--head-only{_C.RESET}: overriding image_old "
            f"({cfg.image_old}) := image_upgrade ({cfg.image_upgrade})"
        )
        cfg.image_old = cfg.image_upgrade

    phase2_generate_compose(cfg)
    phase3_bootstrap_genesis(cfg)
    old_max_proto, old_consensus, epoch_0_start = phase4_start_validators(cfg)
    phase5_start_monitoring(cfg)
    latency_proc = phase6_apply_latency(cfg)

    log(
        f"  {_C.BOLD}Old release{_C.RESET}  ({cfg.release_network:>8s}) : "
        f"max_protocol={old_max_proto or 'unknown'}, "
        f"consensus={old_consensus or 'unknown'}"
    )
    log(
        f"  {_C.BOLD}Local build{_C.RESET}  ({local_branch}@{local_commit}) : "
        "(version will be confirmed after first-half upgrade)"
    )
    initial_proto_version = read_onchain_protocol_version()
    log(f"  {_C.BOLD}On-chain protocol{_C.RESET}     : version {initial_proto_version} at start")

    # --- First half upgrade in epoch 0 ---
    _wait_mid_epoch(cfg, epoch_0_start, "PHASE 7")
    phase8_first_half(cfg, first_half)

    # --- Mixed-binary observation in epoch 1 ---
    epoch_0 = get_current_epoch()
    second_half_upgrade_epoch = phase9_observe_mixed(
        cfg, epoch_0, initial_proto_version
    )

    # --- Second half upgrade — rolls at the start of the epoch, so most of
    # it runs as all-on-HEAD steady state before the next boundary fires the
    # protocol advance.
    phase11_second_half(cfg, second_half)

    # --- Cross the post-upgrade epoch boundary (protocol advance if applicable) ---
    final_epoch = phase12_observe_upgraded(
        cfg, second_half_upgrade_epoch, initial_proto_version
    )

    log(_phase_banner("Half-Network Upgrade Test Complete"))
    log(f"  Final epoch: {final_epoch}")
    log(f"  First-half (upgraded mid-epoch 0): {first_half}")
    log(f"  Second-half (upgraded at start of epoch {second_half_upgrade_epoch}): {second_half}")
    log("  Validator log archives are under experiments/logs/")

    # Best-effort latency teardown (cleanup() also does this on exit)
    run(["sudo", "pkill", "-f", r"network-benchmark\.sh"], check=False, quiet=True)
    if latency_proc.poll() is None:
        latency_proc.terminate()


if __name__ == "__main__":
    main()
