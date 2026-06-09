"""Mirage stress scenario with low-latency/high-jitter links.

All pairs receive low base latency with high jitter to produce
statistically attractive but operationally unstable paths.
"""

from __future__ import annotations

import logging
import time

from . import configure_experiment_logging, start_validator_log_collection
from .. import docker_env, disruptions, spammer

log = logging.getLogger(__name__)

BASE_LATENCY_MS = 10
INITIAL_JITTER_MS = 50
MAX_JITTER_MS = 500
JITTER_STEP_MS = 50
UPDATE_INTERVAL_S = 120
DURATION_S = 1800
SPAMMER_TPS = 100


def apply_mirage_topology(validators: list[str], base_latency: int, jitter: int) -> None:
    """Apply low base latency with high jitter to all edges."""
    log.info("Applying mirage topology: base=%dms jitter=%dms", base_latency, jitter)
    
    for u in validators:
        if not docker_env.is_container_running(u):
            continue

        for v in validators:
            if u == v:
                continue

            try:
                disruptions.add_latency(u, v, base_latency, jitter_ms=jitter)
            except Exception as exc:
                log.debug("Failed to set latency %s->%s: %s", u, v, exc)


def run() -> None:
    log_path = configure_experiment_logging("mirage_stress")
    # Discover validators
    collector = None
    try:
        v_list = docker_env.list_validator_containers()
        # Natural sort: validator-1, validator-2, ... validator-10
        validators = sorted([v.name for v in v_list], key=lambda x: int(x.split("-")[1]))
    except Exception as exc:
        log.error("Failed to list validators: %s", exc)
        return

    if len(validators) < 4:
        log.warning("Expected at least 4 validators, found %d", len(validators))

    # Reset network to clean state
    log.info("Resetting network...")
    disruptions.reset_network(len(validators))
    
    # Start spammer
    log.info("Starting spammer at %d TPS...", SPAMMER_TPS)
    spammer.start_stress_spammer(tps=SPAMMER_TPS)

    base_latency = BASE_LATENCY_MS
    current_jitter = INITIAL_JITTER_MS

    log.info(
        "Starting mirage run: base=%dms jitter_start=%dms step=%dms max=%dms interval=%ds duration=%ds",
        base_latency,
        current_jitter,
        JITTER_STEP_MS,
        MAX_JITTER_MS,
        UPDATE_INTERVAL_S,
        DURATION_S,
    )

    try:
        collector = start_validator_log_collection(validators, log_path, interval_s=60)
        start_time = time.monotonic()
        deadline = start_time + DURATION_S

        while True:
            now = time.monotonic()
            if now >= deadline:
                break
            elapsed = int(now - start_time)
            log.info(
                "Time: %ds/%ds jitter_ms=%d",
                elapsed,
                DURATION_S,
                current_jitter,
            )

            apply_mirage_topology(validators, base_latency, current_jitter)

            sleep_for = min(UPDATE_INTERVAL_S, max(0.0, deadline - time.monotonic()))
            if sleep_for > 0:
                time.sleep(sleep_for)

            # Increase jitter to make the mirage worse
            if current_jitter < MAX_JITTER_MS:
                current_jitter = min(current_jitter + JITTER_STEP_MS, MAX_JITTER_MS)

    except KeyboardInterrupt:
        log.info("Interrupted by user.")
    except Exception as exc:
        log.error("Unexpected error: %s", exc, exc_info=True)
    finally:
        log.info("Test complete. Cleaning up...")
        if collector:
            collector.stop()
        spammer.stop_stress_spammer()
        disruptions.reset_network(len(validators))

if __name__ == "__main__":
    run()
