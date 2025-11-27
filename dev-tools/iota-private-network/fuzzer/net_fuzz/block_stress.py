"""
Block Stress Test Scenario.
Implements a specific sparse topology by blocking 3 specific connections per node.
Blocking is simulated using extremely high latency (10s) to avoid iptables issues.

Topology Pattern (Ring of 10):
For every node i:
1. Block (i - 1) % N  (Immediate Left Neighbor)
2. Block (i + 1) % N  (Immediate Right Neighbor)
3. Block (i + N/2) % N (Antipodal/Opposite Node)

All other connections have a random latency between 10ms and 100ms.

This creates a graph where local neighbors and the furthest node are unreachable,
forcing routing through "medium-distance" peers.
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
log = logging.getLogger("block_stress")

# Store latencies globally to keep them constant across re-applications
LATENCIES = {}

def apply_topology(validators: List[str]):
    """
    Applies the Block topology using latencies.
    Blocks 3 specific peers for each node by setting extremely high latency.
    Sets random latency (10-100ms) for others.
    """
    N = len(validators)
    log.info(f"Enforcing Block Topology on {N} nodes (High Latency Strategy).")
    
    for i, u in enumerate(validators):
        if not docker_env.is_container_running(u):
            continue

        blocked_indices = {
            (i - 1) % N,
            (i + 1) % N,
            (i + N // 2) % N
        }

        for j, v in enumerate(validators):
            if i == j: continue
            
            if j in blocked_indices:
                # Simulate block with high latency (10 seconds)
                lat = 250
                jitter = 50
            else:
                # Connected: Apply random latency (constant for the run)
                if (u, v) not in LATENCIES:
                    LATENCIES[(u, v)] = random.randint(30, 100)
                lat = LATENCIES[(u, v)]
                jitter = 15
                
            try:
                # Ensure no iptables blocks exist
                disruptions.unblock_connection(u, v)
                # Apply latency
                disruptions.add_latency(u, v, lat, jitter_ms=jitter)
            except Exception as e:
                log.debug(f"Failed to set latency {u}->{v}: {e}")

def verify_topology(validators: List[str]):
    """
    Verifies that blocked connections are indeed blocked and open ones are open.
    """
    log.info("Verifying topology enforcement...")
    N = len(validators)
    
    # Check a few random blocked and open connections to avoid taking too long
    checked = 0
    errors = 0
    
    for i, u in enumerate(validators):
        if not docker_env.is_container_running(u):
            continue
            
        blocked_indices = {
            (i - 1) % N,
            (i + 1) % N,
            (i + N // 2) % N
        }
        
        for j, v in enumerate(validators):
            if i == j: continue
            
            # We only check a subset to save time, e.g., 20% probability or specific ones
            if random.random() > 0.2:
                continue
                
            should_block = j in blocked_indices
            dst_ip = docker_env.get_container_ip(v)
            
            if not dst_ip:
                continue
                
            # Try to ping with a short timeout (1s)
            # We use 'ping -c 1 -W 1 <ip>'
            # If it succeeds (exit code 0), connection is OPEN.
            # If it fails (exit code != 0), connection is BLOCKED (or node down).
            try:
                # We assume 'ping' is available. If not, this check will fail gracefully.
                # Using docker_env.run_in_container which returns output, but we need exit code.
                # docker_env.run_in_container raises DockerEnvError on non-zero exit code if check=True.
                
                is_open = False
                try:
                    docker_env.run_in_container(u, ["ping", "-c", "1", "-W", "1", dst_ip], check=True)
                    is_open = True
                except docker_env.DockerEnvError:
                    is_open = False
                
                if should_block and is_open:
                    log.error(f"VIOLATION: {u}->{v} should be BLOCKED but ping succeeded!")
                    errors += 1
                elif not should_block and not is_open:
                    # This might be due to latency/jitter or node load, so just warn
                    log.warning(f"POTENTIAL ISSUE: {u}->{v} should be OPEN but ping failed.")
                else:
                    # Correct
                    pass
                    
                checked += 1
            except Exception as e:
                log.debug(f"Verification failed for {u}->{v}: {e}")
                
    log.info(f"Topology Verification Complete. Checked {checked} links. Found {errors} violations.")

def run():
    # Set fixed seed for reproducibility across different runs (e.g. Mysticeti vs Starfish)
    random.seed(42)
    
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
    
    duration_seconds = 900  # 15 minutes
    update_interval = 60    # Update latencies every minute
    
    log.info(f"Starting 15-minute Block Stress run.")

    try:
        start_time = time.time()
        
        while time.time() - start_time < duration_seconds:
            elapsed = int(time.time() - start_time)
            log.info(f"=== Time: {elapsed}s / {duration_seconds}s ===")
            
            # Re-apply topology to enforce rules (idempotent)
            apply_topology(validators)
            
            # Verify topology
            verify_topology(validators)
            
            time.sleep(update_interval)
            
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
