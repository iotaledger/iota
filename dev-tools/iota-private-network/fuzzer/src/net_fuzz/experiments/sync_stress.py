"""Sync stress scenario with a core/outsider split and measured recovery."""

from __future__ import annotations

import logging
import random
import time

from . import (
    ValidatorLogCollector,
    configure_experiment_logging,
    start_validator_log_collection,
)
from .. import docker_env, disruptions, metrics, spammer

log = logging.getLogger(__name__)

SYNC_CHECKPOINT_DELTA = 10
SYNC_SPREAD_TOLERANCE = 10
SYNC_TIMEOUT_S = 900
SYNC_POLL_INTERVAL_S = 5


def _split_validators(validators: list[str]) -> tuple[list[str], list[str], int]:
    """Return (core, outsiders, f) for ``n = 3f + 1`` validators."""
    num_validators = len(validators)
    if num_validators < 4:
        raise RuntimeError("sync_stress requires at least 4 validators")
    if (num_validators - 1) % 3 != 0:
        raise RuntimeError(
            f"sync_stress requires n = 3f + 1 validators; got {num_validators}"
        )
    f = (num_validators - 1) // 3
    core_size = 2 * f + 1
    core = validators[:core_size]
    outsiders = validators[core_size:]
    return core, outsiders, f


def _stop_containers(names: list[str]) -> None:
    for name in names:
        if docker_env.is_container_running(name):
            docker_env.stop_container(name)


def _start_containers(names: list[str]) -> None:
    for name in names:
        if not docker_env.is_container_running(name):
            docker_env.start_container(name)


def _swap_outage(stop_group: list[str], start_group: list[str]) -> None:
    if len(stop_group) != len(start_group):
        raise RuntimeError(
            "Swap groups must be the same size to preserve quorum during handoff."
        )
    for stop_name, start_name in zip(stop_group, start_group):
        if docker_env.is_container_running(stop_name):
            docker_env.stop_container(stop_name)
        if not docker_env.is_container_running(start_name):
            docker_env.start_container(start_name)


def apply_topology(validators: list[str], core: list[str]) -> None:
    """Apply latency rules for core/outsider topology."""
    outsiders = [v for v in validators if v not in core]
    core_set = set(core)

    log.info("Applying topology: core=%d outsiders=%d", len(core), len(outsiders))

    for u in validators:
        if not docker_env.is_container_running(u):
            continue

        for v in validators:
            if u == v:
                continue

            if u in core_set and v in core_set:
                lat = random.randint(10, 50)
            else:
                lat = random.randint(50, 100)

            try:
                disruptions.add_latency(u, v, lat, jitter_ms=5)
            except Exception as exc:
                log.debug("Failed to set latency %s->%s: %s", u, v, exc)


def _progress_value(metrics_data: dict[str, float]) -> tuple[int | None, str]:
    value = metrics_data.get("last_executed_checkpoint")
    if value is None:
        return None, "checkpoint"
    return int(value), "checkpoint"


def _collect_progress(validators: list[str]) -> tuple[dict[str, int], str]:
    progress: dict[str, int] = {}
    label = "progress"

    for v in validators:
        if not docker_env.is_container_running(v):
            continue
        m = metrics.get_consensus_metrics(v)
        value, metric_label = _progress_value(m)
        if value is None:
            continue
        progress[v] = value
        if metric_label:
            label = metric_label

    return progress, label


def wait_for_sync(
    validators: list[str],
    *,
    min_progress_delta: int = SYNC_CHECKPOINT_DELTA,
    spread_tolerance: int = SYNC_SPREAD_TOLERANCE,
    timeout_s: int = SYNC_TIMEOUT_S,
) -> float | None:
    """Wait until validators are synchronized and have advanced by a delta."""
    start_time = time.monotonic()
    progress, label = _collect_progress(validators)
    if not progress:
        log.warning("No progress metrics available; cannot verify synchronization.")
        return None

    baseline = min(progress.values())
    while time.monotonic() - start_time < timeout_s:
        progress, label = _collect_progress(validators)
        if not progress:
            time.sleep(1)
            continue

        max_val = max(progress.values())
        min_val = min(progress.values())
        spread = max_val - min_val
        advanced = min_val - baseline

        log.info(
            "Sync status (%s): min=%d max=%d spread=%d advanced=%d nodes=%d",
            label,
            min_val,
            max_val,
            spread,
            advanced,
            len(progress),
        )

        if spread <= spread_tolerance and advanced >= min_progress_delta:
            duration = time.monotonic() - start_time
            log.info(
                "Synchronized after %.1fs (min advanced %d %s)",
                duration,
                advanced,
                label,
            )
            return duration

        time.sleep(SYNC_POLL_INTERVAL_S)

    log.warning("Timeout waiting for synchronization.")
    return None


def run() -> tuple[list[str], ValidatorLogCollector | None]:
    log_path = configure_experiment_logging("sync_stress")
    # Discover validators
    validators: list[str] = []
    collector = None
    try:
        v_list = docker_env.list_validator_containers()
        # Natural sort: validator-1, validator-2, ... validator-10
        validators = sorted([v.name for v in v_list], key=lambda x: int(x.split("-")[1]))
    except Exception as exc:
        log.error("Failed to list validators: %s", exc)
        return validators, collector

    try:
        core, outsiders, f = _split_validators(validators)
    except RuntimeError as exc:
        log.error("%s", exc)
        return validators, collector

    log.info(
        "Validator split (n=%d, f=%d): core=%d outsiders=%d",
        len(validators),
        f,
        len(core),
        len(outsiders),
    )

    # Reset network
    disruptions.reset_network(len(validators))
    
    # Start spammer (100 TPS)
    log.info("Starting spammer at 100 TPS...")
    spammer.start_stress_spammer(tps=100)
    collector = start_validator_log_collection(validators, log_path, interval_s=60)

    # Apply Topology
    apply_topology(validators, core)

    # Initial warm up
    log.info("Warming up for 30s...")
    time.sleep(30)

    # Loop duration from 60s, increasing by 30s
    # We want to run for roughly 30 minutes total.
    start_time_total = time.time()
    MAX_RUNTIME = 30 * 60  # 30 minutes

    log.info("Stopping outsiders to start core-only phase...")
    _stop_containers(outsiders)

    sync_records: list[tuple[str, int, float]] = []

    for duration in range(60, 310, 60):
        if time.time() - start_time_total > MAX_RUNTIME:
            log.info("Max runtime reached. Stopping test.")
            break

        log.info("Holding core-only quorum for %ds...", duration)
        time.sleep(duration)

        # Step 1: Swap outages - stop f core validators and restart outsiders.
        core_subset = random.sample(core, f)
        log.info("Swapping outages: core subset down, outsiders up")
        _swap_outage(core_subset, outsiders)

        # Re-apply topology after swapping outages.
        log.info("Re-applying topology...")
        apply_topology(validators, core)

        duration_s = wait_for_sync(validators)
        if duration_s is not None:
            log.info(
                "Sync duration (core subset stop=%ds): %.1fs",
                duration,
                duration_s,
            )
            sync_records.append(("core", duration, duration_s))

        # Step 2: Swap back so outsiders are down for the next iteration.
        log.info("Swapping outages: outsiders down, core subset up")
        _swap_outage(outsiders, core_subset)

        log.info("Re-applying topology...")
        apply_topology(validators, core)

    if sync_records:
        durations = [entry[2] for entry in sync_records]
        avg = sum(durations) / len(durations)
        log.info(
            "Sync durations: count=%d avg=%.1fs max=%.1fs",
            len(durations),
            avg,
            max(durations),
        )

    log.info("Test Complete.")
    return validators, collector


def run_safe() -> None:
    validators: list[str] = []
    collector = None
    try:
        validators, collector = run()
    except KeyboardInterrupt:
        log.info("Interrupted by user.")
    except Exception as exc:
        log.error("Unexpected error: %s", exc, exc_info=True)
    finally:
        log.info("Cleaning up...")
        if collector:
            collector.stop()
        spammer.stop_stress_spammer()
        # We need to know how many validators to reset, but if we failed early we might not know.
        # We can try to list them again or just use a safe default/max.
        try:
            if not validators:
                v_list = docker_env.list_validator_containers()
                validators = [v.name for v in v_list]
            if validators:
                disruptions.reset_network(len(validators))
        except Exception:
            return

if __name__ == "__main__":
    run_safe()
