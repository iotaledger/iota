"""Adaptive fuzzing driver that searches for disruptive configurations."""

from __future__ import annotations

import copy
import csv
import logging
import math
import random
import time
from dataclasses import dataclass, field
from datetime import datetime

from .. import configure_logging, docker_env, disruptions, metrics

log = logging.getLogger(__name__)

@dataclass
class FuzzParams:
    strategy: str = "core_minority"  # or "triangle_violation"
    topology_seed: int = 0
    fringe_latency_mean: int = 50
    core_latency_mean: int = 5
    jitter: int = 5
    packet_loss: float = 0.0
    
    # Momentum for gradient-like updates: (param_name -> direction)
    # direction: 1 (increase), -1 (decrease), 0 (random)
    momentum: dict[str, int] = field(default_factory=dict)

    def mutate(self, aggressive: bool = False, feedback: float = 0.0) -> None:
        """Mutate parameters to explore the search space."""
        # Chance to change topology structure (rare event)
        if aggressive or random.random() < 0.05:
            self.topology_seed = random.randint(0, 100000)
            log.info("Mutating topology seed -> %d", self.topology_seed)

        if aggressive or random.random() < 0.1:
            # Switch strategy
            self.strategy = "triangle_violation" if self.strategy == "core_minority" else "core_minority"

        # Helper to update a param with momentum
        def update_param(name, current_val, min_val, max_val, step_size):
            direction = self.momentum.get(name, 0)

            if feedback > 0 and direction != 0:
                pass
            elif feedback < 0 and direction != 0:
                direction = -direction
            else:
                direction = random.choice([-1, 1])

            self.momentum[name] = direction
            new_val = current_val + (direction * step_size)

            # Add some noise
            if random.random() < 0.2:
                new_val += random.randint(-step_size, step_size)

            return max(min_val, min(max_val, new_val))

        self.fringe_latency_mean = int(update_param("fringe", self.fringe_latency_mean, 10, 200, 25))
        self.core_latency_mean = int(update_param("core", self.core_latency_mean, 0, 50, 5))
        self.jitter = int(update_param("jitter", self.jitter, 0, 50, 5))
        
        # Packet loss
        direction = self.momentum.get("loss", 0)
        if feedback > 0 and direction != 0:
            pass
        elif feedback < 0 and direction != 0:
            direction = -direction
        else:
            direction = random.choice([-1, 1])
        self.momentum["loss"] = direction

        self.packet_loss = max(0.0, min(5.0, self.packet_loss + (direction * 0.5)))

@dataclass
class NodeState:
    name: str
    last_committed_round: float = 0
    highest_accepted_round: float = 0
    timeouts: float = 0
    latency_sum: float = 0
    latency_count: float = 0
    sync_requests: float = 0
    is_running: bool = True
    panic_detected: bool = False

@dataclass
class NetworkState:
    nodes: dict[str, NodeState] = field(default_factory=dict)
    # Adjacency list: src -> set of dst (allowed connections)
    # If A->B is in this set, traffic is ALLOWED. If not, it is BLOCKED.
    topology: dict[str, set[str]] = field(default_factory=dict)

    def get_avg_round(self) -> float:
        rounds = [n.last_committed_round for n in self.nodes.values() if n.is_running]
        if not rounds:
            return 0
        return sum(rounds) / len(rounds)

    def get_total_timeouts(self) -> float:
        return sum(n.timeouts for n in self.nodes.values())

class AdaptiveFuzzer:
    def __init__(self, validators: list[str]):
        self.validators = validators
        self.state = NetworkState()
        for v in validators:
            self.state.nodes[v] = NodeState(name=v)
            # Initially fully connected
            self.state.topology[v] = set(validators) - {v}
        
        self.iteration = 0
        self.start_time = time.time()
        
        # Hill Climbing State
        self.current_params = FuzzParams()
        self.best_params = copy.deepcopy(self.current_params)
        self.best_score = -1.0

        self.last_total_timeouts = 0.0
        self.last_avg_round = 0.0
        self.last_latency_sum = 0.0
        self.last_latency_count = 0.0
        
        # CSV logging
        self.csv_file = open("fuzz_results.csv", "w", newline="")
        self.csv_writer = csv.writer(self.csv_file)
        self.csv_writer.writerow(
            [
                "Iteration",
                "Timestamp",
                "Strategy",
                "FringeLat",
                "CoreLat",
                "Jitter",
                "Loss",
                "Score",
                "AvgRound",
                "TotalTimeouts",
                "AvgLatency",
                "AvgSyncRequests",
            ]
        )

    def update_metrics(self) -> None:
        """Fetch metrics from all validators and update state."""
        for v in self.validators:
            if not docker_env.is_container_running(v):
                self.state.nodes[v].is_running = False
                # Check logs for panic
                try:
                    logs = docker_env.run_in_container(v, ["tail", "-n", "20"], check=False)
                    if "panic" in logs.lower() or "thread 'main' panicked" in logs:
                        self.state.nodes[v].panic_detected = True
                        log.critical("Panic detected in %s", v)
                except Exception as exc:
                    log.debug("Failed to read logs from %s: %s", v, exc)
                continue

            self.state.nodes[v].is_running = True
            m = metrics.get_consensus_metrics(v)
            if "last_committed_round" in m:
                self.state.nodes[v].last_committed_round = m["last_committed_round"]
            if "highest_accepted_round" in m:
                self.state.nodes[v].highest_accepted_round = m["highest_accepted_round"]
            if "timeouts" in m:
                self.state.nodes[v].timeouts = m["timeouts"]
            if "block_commit_latency_sum" in m:
                self.state.nodes[v].latency_sum = m["block_commit_latency_sum"]
            if "block_commit_latency_count" in m:
                self.state.nodes[v].latency_count = m["block_commit_latency_count"]
            if "sync_concurrent_requests" in m:
                self.state.nodes[v].sync_requests = m["sync_concurrent_requests"]

    def enforce_topology(self) -> None:
        """Apply iptables rules to match self.state.topology (placeholder)."""
        # Intentionally left as a placeholder for future diff-based updates.
        return

    def check_liveness(self) -> bool:
        """Check if rounds are advancing."""
        # Simple check: if avg round hasn't increased in X seconds.
        # We rely on the loop to track this.
        return True

    def run(self) -> None:
        configure_logging()
        log.info("Starting adaptive fuzz run with %d validators", len(self.validators))

        # Initial reset
        disruptions.reset_network(len(self.validators))

        # Initialize baseline metrics
        self.update_metrics()
        self.last_total_timeouts = self.state.get_total_timeouts()
        self.last_avg_round = self.state.get_avg_round()
        self.last_latency_sum = sum(n.latency_sum for n in self.state.nodes.values())
        self.last_latency_count = sum(n.latency_count for n in self.state.nodes.values())

        while True:
            self.iteration += 1
            self.update_metrics()

            avg_round = self.state.get_avg_round()
            total_timeouts = self.state.get_total_timeouts()

            log.info(
                "Iter %d: avg_round=%.1f timeouts=%s",
                self.iteration,
                avg_round,
                total_timeouts,
            )

            if any(n.panic_detected for n in self.state.nodes.values()):
                log.critical("Stopping due to panic.")
                break

            # Strategy Step
            self.hill_climbing_step()

            time.sleep(30)

    def calculate_score(self) -> float:
        """Calculate a 'badness' score (higher is worse)."""
        # Timeouts
        current_timeouts = self.state.get_total_timeouts()
        delta_timeouts = current_timeouts - self.last_total_timeouts
        self.last_total_timeouts = current_timeouts

        # Round Progress
        current_avg_round = self.state.get_avg_round()
        # round_progress = current_avg_round - self.last_avg_round # Unused now
        self.last_avg_round = current_avg_round

        # Latency
        total_latency_sum = sum(n.latency_sum for n in self.state.nodes.values())
        total_latency_count = sum(n.latency_count for n in self.state.nodes.values())

        delta_sum = total_latency_sum - self.last_latency_sum
        delta_count = total_latency_count - self.last_latency_count
        self.last_latency_sum = total_latency_sum
        self.last_latency_count = total_latency_count

        avg_latency = 0.0
        if delta_count > 0:
            avg_latency = delta_sum / delta_count

        # Congestion (Sync Requests)
        avg_sync_requests = sum(n.sync_requests for n in self.state.nodes.values()) / max(1, len(self.state.nodes))

        # Score Calculation
        # Weights:
        # Timeouts: High importance (1.0)
        # Latency: Medium importance (10.0 scale factor to make ms comparable to counts)
        # Sync Requests: Low importance (0.5)

        score = (delta_timeouts * 1.0) + (avg_latency * 10.0) + (avg_sync_requests * 0.5)

        # Removed Stall Penalty as requested

        log.info(
            "Metrics: d_timeouts=%s avg_latency=%.4f avg_sync=%.2f",
            delta_timeouts,
            avg_latency,
            avg_sync_requests,
        )

        # Log to CSV
        self.csv_writer.writerow(
            [
                self.iteration,
                datetime.now().isoformat(),
                self.current_params.strategy,
                self.current_params.fringe_latency_mean,
                self.current_params.core_latency_mean,
                self.current_params.jitter,
                self.current_params.packet_loss,
                score,
                current_avg_round,
                current_timeouts,
                avg_latency,
                avg_sync_requests,
            ]
        )
        self.csv_file.flush()

        return score

    def hill_climbing_step(self) -> None:
        """Modify network conditions using hill-climbing with momentum."""
        score = self.calculate_score()
        log.info(
            "Score: %.2f (best: %.2f) params=%s",
            score,
            self.best_score,
            self.current_params,
        )

        feedback = 0.0
        if self.best_score > 0:
            feedback = score - self.best_score

        if score >= self.best_score:
            # We found a new best (or equal) badness. Keep these params as baseline.
            self.best_score = score
            self.best_params = copy.deepcopy(self.current_params)
            # Explore neighbors with positive feedback
            self.current_params.mutate(aggressive=False, feedback=1.0)
            log.info("New best found! Refining parameters...")
        else:
            # We got worse (network recovered). Revert to best and try a bigger jump.
            self.current_params = copy.deepcopy(self.best_params)
            # Negative feedback to reverse direction
            self.current_params.mutate(aggressive=True, feedback=-1.0)
            log.info("Score dropped. Reverting and trying aggressive mutation...")

        self.apply_params(self.current_params)

    def apply_params(self, params: FuzzParams) -> None:
        if params.strategy == "core_minority":
            self.strategy_core_minority(params)
        else:
            self.strategy_triangle_violation(params)

    def strategy_core_minority(self, params: FuzzParams) -> None:
        """Split nodes into a core supermajority and a fringe minority."""
        # Use a deterministic RNG based on topology_seed so the split is stable
        rng = random.Random(params.topology_seed)

        num_validators = len(self.validators)
        # Core size: 2f+1. f = (N-1)//3. So 2*((N-1)//3) + 1.
        # Or just ceil(2N/3).
        core_size = math.ceil(2 * num_validators / 3)

        shuffled = sorted(self.validators[:])  # Ensure stable input
        rng.shuffle(shuffled)

        core = set(shuffled[:core_size])
        fringe = set(shuffled[core_size:])

        log.info(
            "Strategy core/minority (seed=%d): core=%d fringe=%d",
            params.topology_seed,
            len(core),
            len(fringe),
        )

        # Build symmetric target topology
        target_topology = {v: set() for v in self.validators}
        
        # 1. Core fully connected
        for u in core:
            for v in core:
                if u != v:
                    target_topology[u].add(v)
                    target_topology[v].add(u)
        
        # 2. Fringe connects to subset of Core
        limit = math.floor(2 * num_validators / 3)
        for u in fringe:
            # Sample core nodes to connect to
            connected_core = rng.sample(sorted(list(core)), min(len(core), limit))
            for v in connected_core:
                target_topology[u].add(v)
                target_topology[v].add(u)
        
        for u in self.validators:
            peers = target_topology[u]
            current_peers = self.state.topology[u]
            
            # Block unwanted
            to_block = current_peers - peers
            for p in to_block:
                if p in self.state.topology[u]:
                    disruptions.block_connection(u, p)
                    self.state.topology[u].remove(p)
                    if u in self.state.topology[p]:
                        self.state.topology[p].remove(u)

            # Connect wanted
            to_connect = peers - current_peers
            for p in to_connect:
                if p not in self.state.topology[u]:
                    disruptions.unblock_connection(u, p)
                    self.state.topology[u].add(p)
                    if u not in self.state.topology[p]:
                        self.state.topology[p].add(u)
            
            # Apply latency and loss
            for p in peers:
                if u in fringe or p in fringe:
                    delay = random.randint(params.fringe_latency_mean - 10, params.fringe_latency_mean + 10)
                else:
                    delay = random.randint(params.core_latency_mean, params.core_latency_mean + 5)

                disruptions.add_latency(u, p, max(0, delay), jitter_ms=params.jitter, loss_pct=params.packet_loss)

    def strategy_triangle_violation(self, params: FuzzParams) -> None:
        """Create non-triangle condition violations (A-B fast, B-C fast, A-C slow)."""
        rng = random.Random(params.topology_seed)

        # Pick a random triplet
        triplet = rng.sample(sorted(self.validators), 3)
        A, B, C = triplet

        log.info("Strategy triangle violation (seed=%d): %s-%s-%s", params.topology_seed, A, B, C)

        # Ensure A-B and B-C are connected and fast
        for (u, v) in [(A, B), (B, C)]:
            if v not in self.state.topology[u]:
                disruptions.unblock_connection(u, v)
                self.state.topology[u].add(v)
                disruptions.unblock_connection(v, u)
                self.state.topology[v].add(u)
            disruptions.add_latency(u, v, params.core_latency_mean, jitter_ms=params.jitter, loss_pct=params.packet_loss)
            disruptions.add_latency(v, u, params.core_latency_mean, jitter_ms=params.jitter, loss_pct=params.packet_loss)

        # Make A-C slow or blocked
        if C not in self.state.topology[A]:
            disruptions.unblock_connection(A, C)
            self.state.topology[A].add(C)
            disruptions.unblock_connection(C, A)
            self.state.topology[C].add(A)

        # Use fringe latency for the "bad" link
        disruptions.add_latency(A, C, params.fringe_latency_mean, jitter_ms=params.jitter, loss_pct=params.packet_loss)
        disruptions.add_latency(C, A, params.fringe_latency_mean, jitter_ms=params.jitter, loss_pct=params.packet_loss)

if __name__ == "__main__":
    configure_logging()
    validators = [f"validator-{i}" for i in range(1, 19)]
    try:
        v_list = docker_env.list_validator_containers()
        validators = sorted([v.name for v in v_list])
    except Exception:
        validators = [f"validator-{i}" for i in range(1, 20)]

    fuzzer = AdaptiveFuzzer(validators)
    try:
        fuzzer.run()
    except KeyboardInterrupt:
        log.info("Stopping...")
        disruptions.reset_network(len(validators))
