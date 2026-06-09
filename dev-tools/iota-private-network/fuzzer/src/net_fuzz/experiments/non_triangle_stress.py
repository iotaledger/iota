"""Non-triangle stress scenario with dynamic validator groups.

For ``n = 3f + 1`` validators we partition them into three clusters sized
``t = f``, ``f``, and ``f + 1``. Validators within the same cluster experience
high latency and some packet loss, while validators in different clusters
experience lower latency with some jitter.
"""

from __future__ import annotations

import logging
import time

from . import (
    ValidatorLogCollector,
    configure_experiment_logging,
    start_validator_log_collection,
)
from .. import docker_env, disruptions, spammer

log = logging.getLogger(__name__)


def _assign_groups(validators: list[str]) -> tuple[dict[str, str], dict[str, int], int]:
    """Assign validators to t/f/f+1 groups for ``n = 3f + 1``."""
    num_validators = len(validators)
    if num_validators < 4:
        raise RuntimeError("non_triangle_stress requires at least 4 validators")
    if (num_validators - 1) % 3 != 0:
        raise RuntimeError(
            f"non_triangle_stress requires n = 3f + 1 validators; got {num_validators}"
        )

    f = (num_validators - 1) // 3
    group_specs = (("t", f), ("f", f), ("f+1", f + 1))
    assignments: dict[str, str] = {}
    offset = 0

    for label, size in group_specs:
        end = offset + size
        for validator in validators[offset:end]:
            assignments[validator] = label
        offset = end

    if offset != num_validators:
        raise RuntimeError(
            f"Group assignment mismatch: assigned {offset} validators, expected {num_validators}"
        )

    counts = {label: size for label, size in group_specs}
    return assignments, counts, f


def apply_topology(
    validators: list[str],
    groups: dict[str, str],
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

        group_u = groups.get(u)
        if not group_u:
            continue

        for v in validators:
            if u == v:
                continue

            group_v = groups.get(v)
            if not group_v:
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


def run() -> tuple[list[str], ValidatorLogCollector | None]:
    log_path = configure_experiment_logging("non_triangle_stress")
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
        groups, group_sizes, f = _assign_groups(validators)
    except RuntimeError as exc:
        log.error("%s", exc)
        return validators, collector

    log.info(
        "Group sizes (n=%d, f=%d): t=%d, f=%d, f+1=%d",
        len(validators),
        f,
        group_sizes["t"],
        group_sizes["f"],
        group_sizes["f+1"],
    )

    # Reset network to clean state
    log.info("Resetting network...")
    disruptions.reset_network(len(validators))

    # Start spammer
    log.info("Starting spammer at 100 TPS...")
    spammer.start_stress_spammer(tps=100)
    collector = start_validator_log_collection(validators, log_path, interval_s=60)

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
                groups,
                intra_latency=intra_latency,
                inter_latency=inter_latency,
                intra_loss=10.0,
                inter_jitter=20,
            )

            # Topology stays fixed for this whole minute
            time.sleep(minute_interval)

    except KeyboardInterrupt:
        log.info("Interrupted by user.")
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
