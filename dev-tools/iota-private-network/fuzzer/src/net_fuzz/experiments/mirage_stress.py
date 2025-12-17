"""Mirage stress scenario with low-latency/high-jitter links.

All pairs receive low base latency with high jitter to produce
statistically attractive but operationally unstable paths.
"""

from __future__ import annotations

import logging
import time

from .. import configure_logging, docker_env, disruptions, spammer

log = logging.getLogger(__name__)


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
    configure_logging()
    # Discover validators
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
    log.info("Starting spammer at 100 TPS...")
    spammer.start_stress_spammer(tps=100)
    
    # Initial parameters
    base_latency = 10  # Base latency for all edges.
    current_jitter = 50  # Starting jitter
    max_jitter = 500  # Extreme jitter

    duration_seconds = 1800  # 30 minutes
    update_interval = 120  # 2 minutes

    log.info("Starting 30-minute mirage run.")

    try:
        start_time = time.time()
        
        while time.time() - start_time < duration_seconds:
            elapsed = int(time.time() - start_time)
            log.info(
                "Time: %ds/%ds jitter_ms=%d",
                elapsed,
                duration_seconds,
                current_jitter,
            )

            apply_mirage_topology(validators, base_latency, current_jitter)

            time.sleep(update_interval)

            # Increase jitter to make the mirage worse
            if current_jitter < max_jitter:
                current_jitter += 50

    except KeyboardInterrupt:
        log.info("Interrupted by user.")
    except Exception as exc:
        log.error("Unexpected error: %s", exc, exc_info=True)
    finally:
        log.info("Test complete. Cleaning up...")
        spammer.stop_stress_spammer()
        disruptions.reset_network(len(validators))

if __name__ == "__main__":
    run()
