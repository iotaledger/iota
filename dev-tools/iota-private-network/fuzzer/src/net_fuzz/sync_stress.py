"""
Sync Stress Test Scenario.
Cluster of 7 nodes (Core) + 3 Outsiders.
Dynamic latency and node restarts to stress synchronization.
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
log = logging.getLogger("sync_stress")

def apply_topology(validators: List[str]):
    """
    Apply latency rules:
    - Core (0-6): 10-50ms between each other.
    - Outsiders (7-9): 100-200ms to everyone.
    """
    core = validators[:7]
    outsiders = validators[7:]
    
    log.info(f"Applying Topology: Core={len(core)}, Outsiders={len(outsiders)}")
    
    for u in validators:
        # If container is not running, we can't apply rules (and don't need to)
        if not docker_env.is_container_running(u):
            continue

        for v in validators:
            if u == v: continue
            
            if u in core and v in core:
                # Core-Core: 10-50ms
                lat = random.randint(10, 50)
            else:
                # Outsider involved: 50-100ms
                lat = random.randint(50, 100)
            
            # Apply latency
            # Note: add_latency handles the case where target container v might be down 
            # (it just sets up the rule on u to delay packets to v's IP).
            # However, we need v's IP. If v is down, get_container_ip might fail or return None.
            try:
                disruptions.add_latency(u, v, lat, jitter_ms=5)
            except Exception as e:
                # log.warning(f"Failed to set latency {u}->{v}: {e}")
                pass

def wait_for_sync(validators: List[str], timeout: int = 600):
    """
    Wait until all running validators are synchronized (within 5 rounds of each other).
    """
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
        
        log.info(f"Sync Status: Max={max_round}, Min={min_round}, Diff={diff}, Nodes={len(running_validators)}")
        
        if diff <= 5 and len(running_validators) == len(validators):
            log.info("All nodes synchronized!")
            return
        
        time.sleep(5)
    
    log.warning("Timeout waiting for synchronization!")

def run():
    # Discover validators
    try:
        v_list = docker_env.list_validator_containers()
        # Natural sort: validator-1, validator-2, ... validator-10
        validators = sorted([v.name for v in v_list], key=lambda x: int(x.split('-')[1]))
    except Exception:
        log.error("Failed to list validators")
        return

    if len(validators) < 10:
        log.error(f"Need at least 10 validators, found {len(validators)}")
        return

    # Reset network
    disruptions.reset_network(len(validators))
    
    # Start Spammer (100 TPS)
    log.info("Starting Spammer at 100 TPS...")
    spammer.start_stress_spammer(tps=100)
    
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

        log.info(f"=== Starting Iteration with Stop Duration x=z={duration}s ===")
        
        # Step 1: Stop Outsiders (Validators 8-10 -> indices 7,8,9)
        # Indices 7,8,9 correspond to validator-8, validator-9, validator-10
        outsiders = validators[7:10]
        log.info(f"Stopping Outsiders: {outsiders}")
        for v in outsiders:
            docker_env.stop_container(v)
            
        log.info(f"Sleeping for x={duration}s...")
        time.sleep(duration)
        
        log.info(f"Restarting Outsiders: {outsiders}")
        for v in outsiders:
            docker_env.start_container(v)
        
        # Re-apply topology immediately after restart
        log.info("Re-applying topology...")
        apply_topology(validators)
            
        # Step 2: Stop Core Subset (Validators 1-3 -> indices 0,1,2)
        core_subset = validators[0:3]
        log.info(f"Stopping Core Subset: {core_subset}")
        for v in core_subset:
            docker_env.stop_container(v)
            
        log.info(f"Sleeping for z={duration}s...")
        time.sleep(duration)
        
        log.info(f"Restarting Core Subset: {core_subset}")
        for v in core_subset:
            docker_env.start_container(v)
            
        # Re-apply topology immediately after restart
        log.info("Re-applying topology...")
        apply_topology(validators)
            
        # Wait for synchronization before next level
        #log.info("Waiting for network to synchronize...")
        #wait_for_sync(validators)

    log.info("Test Complete.")

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
        # We need to know how many validators to reset, but if we failed early we might not know.
        # We can try to list them again or just use a safe default/max.
        try:
            v_list = docker_env.list_validator_containers()
            disruptions.reset_network(len(v_list))
        except:
            pass

if __name__ == "__main__":
    run_safe()
