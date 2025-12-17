"""
Mirage Stress Test Scenario.
Creates a "Mirage" topology where direct links have low average latency but extreme jitter,
while indirect links are slower but stable.

This traps protocols into selecting "fast" direct links that fail at critical moments due to variance.

Topology:
- All nodes are connected to all other nodes.
- Direct Links: 10ms latency, but +/- 200ms jitter (Variance Attack).
- Indirect Links (simulated by not using direct): 50ms latency, 0ms jitter.

Since we can't easily force "indirect" routing at the network layer without blocking direct,
we simulate this by making the direct link *statistically* attractive (low mean) but *operationally* fatal (high variance).

Progression:
- Runs for 30 minutes.
- Every 2 minutes, the jitter increases, making the "Mirage" more unstable.
"""

import logging
import time
import random
import sys
from typing import List

from . import docker_env
from . import disruptions
from . import spammer

# Configure logging
logging.basicConfig(level=logging.INFO, format='%(asctime)s - %(name)s - %(levelname)s - %(message)s')
log = logging.getLogger("mirage_stress")

def apply_mirage_topology(validators: List[str], base_latency: int, jitter: int):
    """
    Applies the Mirage topology:
    - Base Latency: Low (e.g., 10ms)
    - Jitter: High (e.g., 200ms)
    - Distribution: Normal/Pareto (simulated by 'tc' netem if supported, else uniform)
    """
    log.info(f"Applying Mirage Topology: Base={base_latency}ms, Jitter={jitter}ms")
    
    for u in validators:
        if not docker_env.is_container_running(u):
            continue

        for v in validators:
            if u == v:
                continue
            
            # We apply this to ALL links to create a chaotic environment where
            # no link is truly reliable, but they all "look" fast.
            try:
                # tc netem delay {base}ms {jitter}ms distribution normal
                # Note: disruptions.add_latency currently supports simple jitter.
                # We might need to enhance it for distribution if needed, but uniform is often enough.
                disruptions.add_latency(u, v, base_latency, jitter_ms=jitter)
            except Exception as e:
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

    if len(validators) < 4:
        log.warning(f"Expected at least 4 validators, found {len(validators)}")

    # Reset network to clean state
    log.info("Resetting network...")
    disruptions.reset_network(len(validators))
    
    # Start Spammer
    log.info("Starting Spammer at 100 TPS...")
    spammer.start_stress_spammer(tps=100)
    
    # Initial parameters
    base_latency = 10  # Looks fast!
    current_jitter = 50 # Starting jitter
    max_jitter = 500    # Extreme jitter
    
    duration_seconds = 1800 # 30 minutes
    update_interval = 120   # 2 minutes
    
    log.info(f"Starting 30-minute Mirage run.")

    try:
        start_time = time.time()
        
        while time.time() - start_time < duration_seconds:
            elapsed = int(time.time() - start_time)
            log.info(f"=== Time: {elapsed}s / {duration_seconds}s | Jitter: {current_jitter}ms ===")
            
            apply_mirage_topology(validators, base_latency, current_jitter)
            
            time.sleep(update_interval)
            
            # Increase jitter to make the mirage worse
            if current_jitter < max_jitter:
                current_jitter += 50
            
    except KeyboardInterrupt:
        log.info("Interrupted by user.")
    except Exception as e:
        log.error(f"Unexpected error: {e}", exc_info=True)
    finally:
        log.info("Test Complete. Cleaning up...")
        spammer.stop_stress_spammer()
        disruptions.reset_network(len(validators))

if __name__ == "__main__":
    run()
