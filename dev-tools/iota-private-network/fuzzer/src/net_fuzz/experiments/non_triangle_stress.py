"""Non-triangle stress scenario with three validator groups.

Group 1: validators 1-3
Group 2: validators 4-7
Group 3: validators 8-10
"""

from __future__ import annotations

import logging
import time

from .. import configure_logging, docker_env, disruptions, spammer

log = logging.getLogger(__name__)


def get_group(validator_name: str) -> int:
    """Return the group ID for a validator name (1..3), or 0 if unknown."""
    try:
        num = int(validator_name.split("-")[1])
        if 1 <= num <= 3:
            return 1
        if 4 <= num <= 7:
            return 2
        if 8 <= num <= 10:
            return 3
    except (IndexError, ValueError):
        pass
    return 0  # Unknown or not in range


def apply_topology(
    validators: list[str],
    intra_latency: int,
    inter_latency: int,
    *,
    intra_loss: float = 0.0,
    inter_jitter: int = 5,
) -> None:
    """Apply the non-triangle topology rules with optional loss and jitter."""
    log.info(
        "Applying topology: intra=%dms loss=%.2f%% inter=%dms jitter=%dms",
        intra_latency,
        intra_loss,
        inter_latency,
        inter_jitter,
    )

    for u in validators:
        if not docker_env.is_container_running(u):
            continue

        group_u = get_group(u)
        if group_u == 0:
            continue

        for v in validators:
            if u == v:
                continue

            group_v = get_group(v)
            if group_v == 0:
                continue

            # Determine latency based on group membership
            if group_u == group_v:
                lat = intra_latency
                loss = intra_loss
                jitter = 5
            else:
                lat = inter_latency
                loss = 0.0
                jitter = inter_jitter

            try:
                disruptions.add_latency(u, v, lat, jitter_ms=jitter, loss_pct=loss)
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

    if len(validators) < 10:
        log.warning("Expected at least 10 validators, found %d", len(validators))

    # Reset network to clean state
    log.info("Resetting network...")
    disruptions.reset_network(len(validators))

    # Start spammer
    log.info("Starting spammer at 100 TPS...")
    spammer.start_stress_spammer(tps=100)

    # Desired schedule (matches docstring)
    start_intra = 100  # ms
    start_inter = 30  # ms
    intra_step = 10  # ms per minute
    inter_step = -5  # ms per minute
    total_minutes = 5
    minute_interval = 60  # seconds

    log.info("Starting %d-minute non-triangle run", total_minutes)

    try:
        for minute in range(total_minutes):
            # Compute current latencies
            intra_latency = max(0, start_intra + minute * intra_step)
            inter_latency = max(0, start_inter + minute * inter_step)

            log.info(
                "Minute %d/%d intra_latency=%dms inter_latency=%dms",
                minute + 1,
                total_minutes,
                intra_latency,
                inter_latency,
            )

            # Keep the non-metric flavour: intra = slow+lossy, inter = fast+jittery
            apply_topology(
                validators,
                intra_latency=intra_latency,
                inter_latency=inter_latency,
                intra_loss=10.0,
                inter_jitter=20,
            )

            # Topology stays fixed for this whole minute
            time.sleep(minute_interval)

    except KeyboardInterrupt:
        log.info("Interrupted by user.")


def run_safe() -> None:
    try:
        run()
    except KeyboardInterrupt:
        log.info("Interrupted by user.")
    except Exception as exc:
        log.error("Unexpected error: %s", exc, exc_info=True)
    finally:
        log.info("Cleaning up...")
        spammer.stop_stress_spammer()
        try:
            v_list = docker_env.list_validator_containers()
            disruptions.reset_network(len(v_list))
        except Exception:
            return

if __name__ == "__main__":
    run_safe()
