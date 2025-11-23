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
    
    log.info(f"Starting 30-minute run with aggressive disruption.")

    try:
        start_time = time.time()
        duration_seconds = 360  # 6 minutes
        iteration = 0
        
        while time.time() - start_time < duration_seconds:
            iteration += 1
            log.info(f"=== Iteration {iteration} (Time: {int(time.time() - start_time)}s) ===")
            
            # Oscillate between 3 modes to prevent protocol stabilization
            mode = iteration % 3
            
            if mode == 1:
                # Mode 1: Extreme Triangle Violation
                # Direct: Very slow + Lossy (forces routing via indirect)
                # Indirect: Fast + High Jitter (unstable preferred path)
                apply_topology(validators, intra_latency=200, inter_latency=20, intra_loss=10.0, inter_jitter=20)
            elif mode == 2:
                # Mode 2: Inverted (Direct is fast, Indirect is slow)
                # Confuses routing tables built in Mode 1
                apply_topology(validators, intra_latency=20, inter_latency=200, intra_loss=0.0, inter_jitter=5)
            else:
                # Mode 0: High Latency Everywhere (Congestion simulation)
                apply_topology(validators, intra_latency=150, inter_latency=150, intra_loss=5.0, inter_jitter=10)
            
            # Faster updates (20s) to force constant adaptation
            time.sleep(20)
            
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
