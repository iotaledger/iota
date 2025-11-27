"""
Non-Triangle Stress Test Scenario.
Three groups of validators with dynamic latencies.
Group 1: Validator 1,2,3
Group 2: Validator 4,5,6,7
Group 3: Validator 8,9,10

Intra-group latency starts at 100ms, decreases by 10ms/min.
Inter-group latency starts at 30ms, decreases by 5ms/min.
Total duration: 5 minutes.
"""

import logging
import time
import random
import sys
from typing import List

from . import docker_env
from . import disruptions
from . import metrics
from . import spammer

# Configure logging
logging.basicConfig(level=logging.INFO, format='%(asctime)s - %(name)s - %(levelname)s - %(message)s')
log = logging.getLogger("non_triangle_stress")

def get_group(validator_name: str) -> int:
    """
    Determines the group ID for a given validator.
    Group 1: 1-3
    Group 2: 4-7
    Group 3: 8-10
    """
    try:
        # Assumes format "validator-N"
        num = int(validator_name.split('-')[1])
        if 1 <= num <= 3:
            return 1
        elif 4 <= num <= 7:
            return 2
        elif 8 <= num <= 10:
            return 3
    except (IndexError, ValueError):
        pass
    return 0  # Unknown or not in range

def apply_topology(validators: List[str], intra_latency: int, inter_latency: int, intra_loss: float = 0.0, inter_jitter: int = 5):
    """
    Applies the non-triangle topology rules with optional loss and jitter.
    """
    log.info(f"Applying Topology: Intra=[{intra_latency}ms, {intra_loss}% loss], Inter=[{inter_latency}ms, {inter_jitter}ms jitter]")
    
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
                jitter = 5 # Low jitter on slow links
            else:
                lat = inter_latency
                loss = 0.0
                jitter = inter_jitter # High jitter on fast links
            
            try:
                # Add small jitter to avoid perfect synchronization artifacts
                disruptions.add_latency(u, v, lat, jitter_ms=jitter, loss_pct=loss)
            except Exception as e:
                # Log debug instead of warning to avoid spamming if a node is temporarily down
                log.debug(f"Failed to set latency {u}->{v}: {e}")

def run():
    # Discover validators
    try:
        v_list = docker_env.list_validator_containers()
        # Natural sort: validator-1, validator-2, ... validator-10
        validators = sorted([v.name for v in v_list], key=lambda x: int(x.split('-')[1]))
    except Exception as e:
        log.error(f"Failed to list validators: {e}")
        return

    if len(validators) < 10:
        log.warning(f"Expected at least 10 validators for full scenario, found {len(validators)}")

    # Reset network to clean state
    log.info("Resetting network...")
    disruptions.reset_network(len(validators))
    
    # Start Spammer
    log.info("Starting Spammer at 100 TPS...")
    spammer.start_stress_spammer(tps=100)
    
    # Desired schedule (matches docstring)
    start_intra = 100   # ms
    start_inter = 30    # ms
    intra_step = 10    # ms per minute
    inter_step = -5     # ms per minute
    total_minutes = 5
    minute_interval = 60  # seconds

    log.info(f"Starting {total_minutes}-minute Non-Triangle run with 1-minute parameter updates.")

    try:
        for minute in range(total_minutes):
            # Compute current latencies
            intra_latency = max(0, start_intra + minute * intra_step)
            inter_latency = max(0, start_inter + minute * inter_step)

            log.info(
                f"Minute {minute+1}/{total_minutes}: "
                f"intra_latency={intra_latency}ms, inter_latency={inter_latency}ms"
            )

            # Keep the non-metric flavour: intra = slow+lossy, inter = fast+jittery
            apply_topology(
                validators,
                intra_latency=intra_latency,
                inter_latency=inter_latency,
                intra_loss=10.0,   # same as before
                inter_jitter=20,   # same as before
            )

            # Topology stays fixed for this whole minute
            time.sleep(minute_interval)

    except KeyboardInterrupt:
        log.info("Interrupted by user.")    

def run_safe():
    try:
        run()
    except KeyboardInterrupt:
        log.info("Interrupted by user.")
    except Exception as e:
        log.error(f"Unexpected error: {e}", exc_info=True)
    finally:
        log.info("Cleaning up...")
        spammer.stop_stress_spammer()
        try:
            v_list = docker_env.list_validator_containers()
            disruptions.reset_network(len(v_list))
        except:
            pass

if __name__ == "__main__":
    run_safe()
