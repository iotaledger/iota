#!/usr/bin/env python3

# Copyright (c) 2026 IOTA Stiftung
# SPDX-License-Identifier: Apache-2.0

"""Half-network upgrade test.

Brings up all validators on a released image, then mid-epoch-0 rolls out the
locally-built upgrade image to a deterministic random half of the network.
Observes the resulting mixed-binary network for one full epoch (protocol
stays at the lower-of-the-two MAX_PROTOCOL_VERSION values because the
new-binary half doesn't reach 2f+1 supermajority). Mid-epoch-1, upgrades the
remaining half. After the next epoch boundary the network has unanimous
support for the higher protocol version (if HEAD's MAX_PROTOCOL_VERSION is
higher than the release image's), advances, and any new feature flags
become active. Observes one final epoch under the new protocol.

When HEAD's MAX_PROTOCOL_VERSION equals the release image's, no protocol
advance occurs at any boundary — the test still exercises the heterogeneous
binary path but only verifies stability, not feature-flag activation.

Reuses the setup / latency / monitoring / phase plumbing from
run-migration-test.py via importlib because hyphenated module names cannot
be imported the normal way. The only orchestration-specific helpers we
define locally are `split_halves` and `rolling_upgrade(indices)`.

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
# `run-migration-test.py` contains all the heavy lifting (Config, docker_compose,
# get_current_epoch / wait_for_epoch_change, phase1..phase7, phase10, log helpers,
# cleanup, _read_validator_protocol_info, etc.). Hyphens block normal `import`,
# so we load it via importlib. A future cleanup is to refactor those helpers
# into a hyphen-free `migration_lib.py`; until then this is the pragmatic path.

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
phase7_wait_mid_epoch = mt.phase7_wait_mid_epoch
phase10_observation = mt.phase10_observation
cleanup = mt.cleanup
_signal_handler = mt._signal_handler
_C = mt._C


# ========================= Patch Config for half-upgrade timing =========================
#
# Migration test's Config.__post_init__ reserves an epoch budget for both phase
# 8 (rolling upgrade) AND phase 9 (restart-stress with/without DB wipe). The
# half-upgrade test doesn't run phase 9 at all, so we override __post_init__
# to zero the phase-9 budget and recompute mid_epoch_wait without it.
#
# Mode is forced to "advanced" because we use phase 7's mid-epoch wait and do
# NOT use simple-mode block-production measurement. The new dataclass has a
# few simple-mode-only fields (phase8_simple_estimate, pre_rolling_wait,
# stable_window_seconds) that we set to safe placeholders for dataclass
# completeness even though the half-upgrade test's flow does not consult them.


def _half_upgrade_post_init(self: Config) -> None:
    # Validation mirrored from migration_test.Config.__post_init__
    mt.ec.validate_num_validators(self.num_validators)
    if self.load_qps < 0:
        raise ValueError("load qps must be >= 0")
    if self.load_in_flight_ratio <= 0:
        raise ValueError("load in-flight ratio must be > 0")
    if self.load_transfer_objects <= 0:
        raise ValueError("load transfer objects must be > 0")
    # Force advanced-mode semantics: we use phase 7 and skip simple-mode's
    # block-production measurement.
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
    # Migration test pins this to 15s. Match that.
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
    # *** Half-upgrade-specific: no restart-stress phase ***
    self.phase9_epoch0_worst_case = 0
    # Simple-mode-only fields. Set safe placeholders for dataclass completeness.
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
    """Deterministic random partition of [1..num_validators] into two halves.

    First returned list has floor(N/2) entries (upgraded first, mid-epoch 0).
    Second list has the remaining ceil(N/2) (upgraded mid-epoch 1).
    Both lists are sorted for readability; the seeded shuffle determines
    which validators go in which half, not the iteration order within.
    """
    indices = list(range(1, num_validators + 1))
    random.Random(seed).shuffle(indices)
    half = num_validators // 2
    return sorted(indices[:half]), sorted(indices[half:])


def rolling_upgrade(cfg: Config, indices: list[int], label: str) -> None:
    """Roll the given subset of validators forward to cfg.image_upgrade.

    Mirrors the per-validator mechanism of run-migration-test.py's
    phase8_rolling_upgrade but takes an explicit list of validator indices
    so it can be called twice (once per half) instead of iterating all.
    """
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


# ========================= Phase wrappers tailored to half-upgrade =========================


def _wait_for_epoch_with_log_save(cfg: Config, epoch_before: int) -> int:
    """Like migration_test.wait_for_epoch_change but also saves validator logs
    every cfg.log_interval seconds during the wait. The migration test's wait
    polls every 30 s without saving logs, so for long waits (e.g. a full epoch)
    we lose the live-log snapshot of that whole window. This version preserves
    it.
    """
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
    """Local copy of migration test's phase7_wait_mid_epoch with a customizable
    banner label so the half-upgrade test's two mid-epoch waits get monotonic
    phase numbers (7a, 7b) instead of both reading "PHASE 7".
    """
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


def phase9_observe_mixed(cfg: Config, epoch_0_start: float, epoch_0: int) -> tuple[int, float]:
    """Wait through the 0->1 boundary, then hold the mixed-binary network
    through `cfg.mid_observation_epochs` extra full epochs. Protocol version
    is expected to stay at the lower of the two halves' MAX_PROTOCOL_VERSION
    values because the new-binary half is below 2f+1. Returns
    (current_epoch, current_epoch_start_time) — the most recent epoch and
    when it started, used to anchor the next mid-epoch wait.
    """
    phase_start = time.time()
    log(_phase_banner("Waiting for epoch 0→1 transition (mixed binaries)", "PHASE 9"))

    current_epoch = _wait_for_epoch_with_log_save(cfg, epoch_0)
    if current_epoch <= epoch_0:
        raise RuntimeError(f"Epoch did not advance past {epoch_0}")
    current_start = time.time()

    log(f"  Reading validator-1 protocol info...")
    time.sleep(cfg.protocol_probe_wait)
    proto, consensus = _read_validator_protocol_info("validator-1", last=True)
    log(
        f"  {_C.CYAN}Epoch {current_epoch}{_C.RESET} (mixed binaries) — "
        f"max_protocol={proto or 'unknown'}, consensus={consensus or 'unknown'}"
    )
    log(
        "  Expectation: protocol version unchanged (supermajority of upgrade-side "
        "not reached; old binary still in the committee)."
    )

    extra = getattr(cfg, "mid_observation_epochs", 0)
    for i in range(extra):
        log(f"  [{i + 1}/{extra}] Holding mixed-binary state for one more epoch...")
        new_epoch = _wait_for_epoch_with_log_save(cfg, current_epoch)
        if new_epoch <= current_epoch:
            log(f"  {_C.YELLOW}WARN: epoch did not advance further; stopping mixed-state observation{_C.RESET}")
            break
        current_epoch = new_epoch
        current_start = time.time()
        log(f"  Mixed-binary epoch advanced to {current_epoch} (extra observation)")

    log(_phase_complete("Phase 9", time.time() - phase_start))
    return current_epoch, current_start


def phase12_observe_upgraded(cfg: Config, epoch_1: int) -> int:
    """Wait through the next epoch boundary. If HEAD's MAX_PROTOCOL_VERSION
    is higher than the release image's, the protocol advances here (whole
    network now supports the new version) and any new feature flags become
    active; otherwise the protocol stays put and this is just steady-state
    observation. Holds the resulting state for `cfg.post_observation_epochs`
    additional full epochs. Periodically saves validator logs throughout.
    """
    phase_start = time.time()
    log(_phase_banner("Waiting for post-second-half-upgrade epoch boundary", "PHASE 12"))

    current_epoch = _wait_for_epoch_with_log_save(cfg, epoch_1)
    if current_epoch <= epoch_1:
        raise RuntimeError(f"Epoch did not advance past {epoch_1}")

    log(f"  Reading validator-1 protocol info...")
    time.sleep(cfg.protocol_probe_wait)
    proto, consensus = _read_validator_protocol_info("validator-1", last=True)
    log(
        f"  {_C.GREEN}Epoch {current_epoch}{_C.RESET} (all upgraded) — "
        f"max_protocol={proto or 'unknown'}, consensus={consensus or 'unknown'}"
    )
    log(
        "  Note: protocol advances here only if HEAD's MAX_PROTOCOL_VERSION "
        "exceeds the release image's. Otherwise this is steady-state "
        "observation on the same protocol the whole run used."
    )

    extra = getattr(cfg, "post_observation_epochs", 0)
    for i in range(extra):
        log(f"  [{i + 1}/{extra}] Observing post-upgrade steady-state for one more epoch...")
        new_epoch = _wait_for_epoch_with_log_save(cfg, current_epoch)
        if new_epoch <= current_epoch:
            log(f"  {_C.YELLOW}WARN: epoch did not advance further; stopping post-upgrade observation{_C.RESET}")
            break
        current_epoch = new_epoch
        log(f"  Post-upgrade epoch advanced to {current_epoch}")

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
            "mid-epoch-1) with one full mixed-binary epoch between them."
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
        choices=range(2, 101),
        metavar="N",
        help="Number of validators to run (2-100, default: 19)",
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
            "Start every validator on the locally-built HEAD image from the "
            "very beginning (image_old := image_upgrade) instead of on the "
            "release image. All phases still run; the rolling 'upgrades' "
            "in phases 8 and 11 become same-binary restarts. Used to isolate "
            "orchestrator overhead (compose file, network-benchmark.sh) from "
            "binary-version effects: compare the pre-phase-8 window of a "
            "--head-only run against the same window of a normal run, where "
            "the only difference is release vs HEAD binary."
        ),
    )
    return parser.parse_args()


# ========================= Main =========================


def main() -> None:
    args = parse_args()

    # The other orchestrators (run-fuzz, run-benchmark, run-migration-test)
    # all acquire /tmp/iota-experiments.lock at startup. They share container
    # names, the docker network, and the tc/iptables state on validators —
    # concurrent runs silently corrupt each other (one run's cleanup tears
    # down the other run's network mid-flight). Acquire it here too so the
    # half-upgrade test plays nicely alongside them.
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
    # Attach half-upgrade-specific observation knobs (extra attrs on the Config
    # instance — Config is a non-frozen dataclass so this is allowed).
    cfg.mid_observation_epochs = max(0, args.mid_observation_epochs)
    cfg.post_observation_epochs = max(0, args.post_observation_epochs)
    cfg.head_only = bool(args.head_only)
    mt._cfg = cfg

    if cfg.script_dir.name != "experiments":
        print("Error: run from experiments/", file=sys.stderr)
        sys.exit(1)

    # Routes future log() calls to the half-upgrade-specific log file. The
    # function also mkdir -p's log_dir; migration test's cleanup() will close
    # the handle via ec.close_logging().
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
    log(f"  {_C.BOLD}First half (mid-ep-0){_C.RESET} : {first_half}  ({len(first_half)} vals)")
    log(f"  {_C.BOLD}Second half (mid-ep-1){_C.RESET}: {second_half}  ({len(second_half)} vals)")
    log(f"  {_C.BOLD}Mid-epoch wait{_C.RESET}      : {cfg.mid_epoch_wait}s")
    log(f"  {_C.BOLD}Rolling offline pause{_C.RESET}: {cfg.rolling_restart_pause_min}-{cfg.rolling_restart_pause_max}s per validator")
    log(f"  {_C.BOLD}Mid-binary observation{_C.RESET}: {cfg.mid_observation_epochs} extra epoch(s) of mixed-binary state between halves")
    log(f"  {_C.BOLD}Post-upgrade observation{_C.RESET} : {cfg.post_observation_epochs} extra epoch(s) after all validators on HEAD")
    if cfg.head_only:
        log(f"  {_C.BOLD}Head-only mode{_C.RESET}      : ON — all validators start on HEAD "
            "(rolling 'upgrades' will be same-binary restarts)")

    # --- Setup phases (reused from migration test) ---
    local_branch, local_commit = phase1_docker_images(cfg)

    # In head-only mode, every validator starts directly on the HEAD-built
    # image. Overriding image_old here means phase2's compose template, phase4's
    # boot, and the env-file substitution all see the upgrade image as the
    # default. We deliberately do this AFTER phase1 so phase1's release-image
    # pull/tag still runs (it's cached anyway, and keeping the unconditional
    # pull avoids forking the phase1 logic just for this mode).
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

    # --- First half upgrade in epoch 0 ---
    _wait_mid_epoch(cfg, epoch_0_start, "PHASE 7")
    phase8_first_half(cfg, first_half)

    # --- Mixed-binary observation in epoch 1 ---
    epoch_0 = get_current_epoch()
    epoch_1, epoch_1_start = phase9_observe_mixed(cfg, epoch_0_start, epoch_0)

    # --- Second half upgrade — skip the mid-epoch wait so Phase 11 starts at
    # the beginning of the current (mixed-binary) epoch. That leaves most of
    # the epoch for an "all-on-HEAD on the still-old protocol" steady-state
    # window before the next epoch boundary fires the protocol advance (if
    # any). With -e 15 this is ~9 min of steady state instead of just the
    # tail of the rolling-upgrade recovery.
    phase11_second_half(cfg, second_half)

    # --- Cross the post-upgrade epoch boundary (protocol advance if applicable) ---
    final_epoch = phase12_observe_upgraded(cfg, epoch_1)

    log(_phase_banner("Half-Network Upgrade Test Complete"))
    log(f"  Final epoch: {final_epoch}")
    log(f"  First-half (upgraded mid-ep-0): {first_half}")
    log(f"  Second-half (upgraded mid-ep-1): {second_half}")
    log("  Validator log archives are under experiments/logs/")

    # Best-effort latency teardown (cleanup() also does this on exit)
    run(["sudo", "pkill", "-f", r"network-benchmark\.sh"], check=False, quiet=True)
    if latency_proc.poll() is None:
        latency_proc.terminate()


if __name__ == "__main__":
    main()
