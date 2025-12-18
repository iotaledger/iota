"""Block stress scenario with symmetric neighbor blocking.

For n = 3f + 1 validators, each node applies high latency to f peers:
- if f is even: f/2 neighbors to the left and f/2 to the right
- if f is odd: (f-1)/2 neighbors to each side plus the antipode
"""

from __future__ import annotations

import logging
import random
import time

from . import configure_experiment_logging, start_validator_log_collection
from .. import checks, docker_env, disruptions, spammer

log = logging.getLogger(__name__)

# Store latencies globally to keep them constant across re-applications.
LATENCIES: dict[tuple[str, str], int] = {}


def _blocked_indices(index: int, num_validators: int) -> set[int]:
    """Return a symmetric blocked set for a given node."""
    if num_validators < 4:
        return set()
    f = max(0, (num_validators - 1) // 3)
    if f == 0:
        return set()
    if f > num_validators - 1:
        raise RuntimeError(f"Invalid block count f={f} for n={num_validators}")

    half = f // 2
    blocked = {(index - offset) % num_validators for offset in range(1, half + 1)}
    blocked.update({(index + offset) % num_validators for offset in range(1, half + 1)})

    if f % 2 == 1:
        if num_validators % 2 != 0:
            raise RuntimeError(
                "Antipodal blocking requires an even validator count when f is odd."
            )
        blocked.add((index + num_validators // 2) % num_validators)

    if len(blocked) != f:
        raise RuntimeError(
            f"Blocked set size mismatch for node {index}: expected {f}, got {len(blocked)}"
        )
    return blocked


def _validate_blocking_scheme(num_validators: int) -> None:
    """Ensure the blocking scheme is symmetric and size-correct."""
    for i in range(num_validators):
        blocked = _blocked_indices(i, num_validators)
        for j in blocked:
            if i not in _blocked_indices(j, num_validators):
                raise RuntimeError(f"Blocking is not symmetric between {i} and {j}")


def apply_topology(validators: list[str], block_latency_ms: int) -> None:
    """Apply the sparse topology using latency-based blocking."""
    num_validators = len(validators)
    _validate_blocking_scheme(num_validators)
    f = max(0, (num_validators - 1) // 3)
    log.info(
        "Enforcing block topology: validators=%d blocked_per_node=%d block_latency_ms=%d",
        num_validators,
        f,
        block_latency_ms,
    )
    
    for i, u in enumerate(validators):
        if not docker_env.is_container_running(u):
            continue

        blocked_indices = _blocked_indices(i, num_validators)

        for j, v in enumerate(validators):
            if i == j:
                continue

            if j in blocked_indices:
                # Simulated block via high latency.
                lat = block_latency_ms
                jitter = 0
            else:
                # Connected: apply a stable random latency for the run.
                if (u, v) not in LATENCIES:
                    LATENCIES[(u, v)] = random.randint(30, 150)
                lat = LATENCIES[(u, v)]
                jitter = 5

            try:
                disruptions.unblock_connection(u, v)
                disruptions.add_latency(u, v, lat, jitter_ms=jitter)
            except Exception as exc:
                log.debug("Failed to set latency %s->%s: %s", u, v, exc)


def verify_topology(validators: list[str], block_latency_ms: int) -> None:
    """Sample edges to verify that blocked links have the expected latency."""
    log.info("Verifying topology enforcement...")
    num_validators = len(validators)
    _validate_blocking_scheme(num_validators)

    # Check a random subset of edges to avoid long runtimes.
    checked = 0
    violations = 0

    for i, u in enumerate(validators):
        if not docker_env.is_container_running(u):
            continue

        blocked_indices = _blocked_indices(i, num_validators)

        for j, v in enumerate(validators):
            if i == j:
                continue

            # Sample a subset to keep the check lightweight.
            if random.random() > 0.2:
                continue

            should_block = j in blocked_indices
            dst_ip = docker_env.get_container_ip(v)

            if not dst_ip:
                continue

            if not should_block:
                continue
            try:
                tolerance_ms = max(10, block_latency_ms // 10)
                if not checks.check_latency(
                    u,
                    v,
                    block_latency_ms - tolerance_ms,
                    block_latency_ms + tolerance_ms,
                    emit_latency=True,
                ):
                    violations += 1
                checked += 1
            except Exception as exc:
                log.debug("Verification failed for %s->%s: %s", u, v, exc)

    log.info("Topology verification complete: checked=%d violations=%d", checked, violations)


def run() -> None:
    log_path = configure_experiment_logging("block_stress")
    # Set fixed seed for reproducibility across different runs (e.g. Mysticeti vs Starfish)
    random.seed(42)
    validators: list[str] = []
    collector = None

    # Discover validators
    try:
        v_list = docker_env.list_validator_containers()
        # Natural sort: validator-1, validator-2, ... validator-10
        validators = sorted([v.name for v in v_list], key=lambda x: int(x.split('-')[1]))
    except Exception as exc:
        log.error("Failed to list validators: %s", exc)
        return

    if len(validators) < 4:
        log.warning("Expected at least 4 validators, found %d", len(validators))

    # Reset network to clean state
    log.info("Resetting network...")
    disruptions.reset_network(len(validators))

    # Start spammer
    log.info("Starting spammer at 150 TPS...")
    spammer.start_stress_spammer(tps=150)

    duration_seconds = 1800  # 30 minutes
    update_interval = 60  # Update latencies every minute
    current_block_latency = 200  # Start with 200ms for blocked links

    log.info("Starting block stress run (ramping latency 200ms -> 2000ms).")

    try:
        collector = start_validator_log_collection(validators, log_path, interval_s=60)
        start_time = time.time()
        
        while time.time() - start_time < duration_seconds:
            elapsed = int(time.time() - start_time)
            log.info(
                "Time: %ds/%ds block_latency_ms=%d",
                elapsed,
                duration_seconds,
                current_block_latency,
            )

            # Re-apply topology to enforce rules (idempotent)
            apply_topology(validators, current_block_latency)

            # Verify topology
            verify_topology(validators, current_block_latency)

            time.sleep(update_interval)

            # Increase block latency by 100ms, cap at 1000ms (1s)
            current_block_latency += 100

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
