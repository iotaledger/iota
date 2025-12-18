"""Sync stress scenario with a core/outsider split."""

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


def apply_topology(validators: list[str]) -> None:
    """Apply latency rules for core/outsider topology."""
    core = validators[:7]
    outsiders = validators[7:]

    log.info("Applying topology: core=%d outsiders=%d", len(core), len(outsiders))

    for u in validators:
        if not docker_env.is_container_running(u):
            continue

        for v in validators:
            if u == v:
                continue

            if u in core and v in core:
                lat = random.randint(10, 50)
            else:
                lat = random.randint(50, 100)

            try:
                disruptions.add_latency(u, v, lat, jitter_ms=5)
            except Exception as exc:
                log.debug("Failed to set latency %s->%s: %s", u, v, exc)


def wait_for_sync(validators: list[str], timeout: int = 600) -> None:
    """Wait until all running validators are synchronized (within 5 rounds)."""
    start_time = time.time()
    while time.time() - start_time < timeout:
        rounds = []
        running_validators = []

        for v in validators:
            if not docker_env.is_container_running(v):
                continue

            running_validators.append(v)
            m = metrics.get_consensus_metrics(v)
            if "last_committed_round" in m:
                rounds.append(m["last_committed_round"])

        if not rounds:
            time.sleep(1)
            continue

        max_round = max(rounds)
        min_round = min(rounds)
        diff = max_round - min_round

        log.info(
            "Sync status: max=%s min=%s diff=%s nodes=%d",
            max_round,
            min_round,
            diff,
            len(running_validators),
        )

        if diff <= 5 and len(running_validators) == len(validators):
            log.info("All nodes synchronized!")
            return

        time.sleep(5)

    log.warning("Timeout waiting for synchronization!")


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

    if len(validators) < 10:
        log.error("Need at least 10 validators, found %d", len(validators))
        return

    # Reset network
    disruptions.reset_network(len(validators))
    
    # Start spammer (100 TPS)
    log.info("Starting spammer at 100 TPS...")
    spammer.start_stress_spammer(tps=100)
    collector = start_validator_log_collection(validators, log_path, interval_s=60)

    # Apply Topology
    apply_topology(validators)

    # Initial warm up
    log.info("Warming up for 30s...")
    time.sleep(30)

    # Loop duration from 60s, increasing by 30s
    # We want to run for roughly 30 minutes total.
    start_time_total = time.time()
    MAX_RUNTIME = 30 * 60  # 30 minutes

    for duration in range(60, 310, 60):
        if time.time() - start_time_total > MAX_RUNTIME:
            log.info("Max runtime reached. Stopping test.")
            break

        log.info("Starting iteration with stop duration=%ds", duration)

        # Step 1: Stop Outsiders (Validators 8-10 -> indices 7,8,9)
        outsiders = validators[7:10]
        log.info("Stopping outsiders: %s", outsiders)
        for v in outsiders:
            docker_env.stop_container(v)

        log.info("Sleeping for %ds...", duration)
        time.sleep(duration)

        log.info("Restarting outsiders: %s", outsiders)
        for v in outsiders:
            docker_env.start_container(v)

        # Re-apply topology immediately after restart
        log.info("Re-applying topology...")
        apply_topology(validators)

        # Step 2: Stop Core Subset (Validators 1-3 -> indices 0,1,2)
        core_subset = validators[0:3]
        log.info("Stopping core subset: %s", core_subset)
        for v in core_subset:
            docker_env.stop_container(v)

        log.info("Sleeping for %ds...", duration)
        time.sleep(duration)

        log.info("Restarting core subset: %s", core_subset)
        for v in core_subset:
            docker_env.start_container(v)

        # Re-apply topology immediately after restart
        log.info("Re-applying topology...")
        apply_topology(validators)

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
