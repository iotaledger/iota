"""High-level fuzzing scenarios composed from the low-level primitives."""

from __future__ import annotations

import logging
import math
import time
from dataclasses import dataclass, field
from random import Random

from . import checks, disruptions, spammer

log = logging.getLogger(__name__)


@dataclass
class ScenarioResult:
    name: str
    details: dict[str, object] = field(default_factory=dict)

    def __str__(self) -> str:
        return f"{self.name}: {self.details}"


def add_latency_between_validators(src: str, dst: str, delay_ms: int) -> ScenarioResult:
    log.info("Starting latency scenario: src=%s dst=%s delay=%sms", src, dst, delay_ms)
    disruptions.add_latency(src, dst, delay_ms, jitter_ms=max(1, delay_ms // 10))
    verified = checks.check_latency(src, dst, delay_ms, delay_ms + 5)
    if verified:
        log.info("Latency scenario verified for %s -> %s", src, dst)
    else:
        log.warning("Latency scenario verification failed for %s -> %s", src, dst)
    return ScenarioResult(
        name="add_latency_between_validators",
        details={
            "src": src,
            "dst": dst,
            "delay_ms": delay_ms,
            "verified": verified,
        },
    )


@dataclass
class _NodeState:
    """Internal state for a validator during a fuzz run."""

    name: str
    up: bool = True
    next_change: float = math.inf
    last_change: float = 0.0
    total_down_time: float = 0.0
    latency_next_change: float = math.inf


def fuzz_scenario(
    *,
    num_validators: int,
    duration_s: int,
    seed: int,
    mean_time_to_stop_s: float,
    mean_time_to_recover_s: float,
    max_latency_ms: int,
    max_loss_pct: float,
    latency_update_interval_s: float = 30.0,
    block_update_interval_s: float = 20.0,
    block_fraction: float = 0.2,
    spammer_tps: int | None = None,
) -> ScenarioResult:
    """Fuzz scenario combining churn, partitions and netem.

    Design goals:
    - Deterministic: all randomness is driven by ``seed`` via ``random.Random``.
    - Safety: at all times strictly fewer than 1/3 of validators are down.
    - Concurrency: node failures/recoveries, connection blocks, and
      latency/loss updates all evolve in parallel.
    - Future-proof: structure is amenable to plugging in real metric
      collectors and adaptive search (e.g. hill-climbing on disruption
      parameters) without changing the basic control loop.
    """

    rng = Random(seed)
    block_fraction = max(0.0, min(block_fraction, 1.0))
    start = time.monotonic()
    deadline = start + float(duration_s)

    # Ensure we start from a clean, non-perturbed state and heal on exit.
    disruptions.reset_network(num_validators)

    # Clean up any existing spammer and start a new one if requested.
    spammer.stop_stress_spammer()
    spammer_started = False
    if spammer_tps and spammer_tps > 0:
        try:
            spammer.start_stress_spammer(tps=spammer_tps, duration_s=duration_s)
            spammer_started = True
            log.info("Started stress spammer at %s TPS for ~%ss", spammer_tps, duration_s)
        except Exception as exc:  # pragma: no cover - defensive
            log.warning("Failed to start stress spammer (tps=%s): %s", spammer_tps, exc)

    names = [f"validator-{i}" for i in range(1, num_validators + 1)]
    nodes = {_name: _NodeState(name=_name, up=True, last_change=start) for _name in names}
    max_down = max(0, (num_validators - 1) // 3)  # strictly < 1/3

    def schedule_next_stop(now: float, state: _NodeState) -> None:
        if mean_time_to_stop_s <= 0:
            state.next_change = math.inf
        else:
            state.next_change = now + rng.expovariate(1.0 / mean_time_to_stop_s)

    def schedule_next_recover(now: float, state: _NodeState) -> None:
        if mean_time_to_recover_s <= 0:
            state.next_change = math.inf
        else:
            state.next_change = now + rng.expovariate(1.0 / mean_time_to_recover_s)

    now = start
    for st in nodes.values():
        schedule_next_stop(now, st)

    def schedule_latency_change(now: float, state: _NodeState) -> None:
        if (max_latency_ms <= 0 and max_loss_pct <= 0.0) or latency_update_interval_s <= 0:
            state.latency_next_change = math.inf
            return
        rate = 1.0 / latency_update_interval_s
        state.latency_next_change = now + rng.expovariate(rate)

    for st in nodes.values():
        schedule_latency_change(now, st)

    @dataclass
    class _EdgeState:
        a: str
        b: str
        blocked: bool = False
        next_change: float = math.inf

    def schedule_edge_change(now: float, edge: _EdgeState) -> None:
        if block_update_interval_s <= 0 or block_fraction <= 0.0 or block_fraction >= 1.0:
            edge.next_change = math.inf
            return
        if edge.blocked:
            rate = (1.0 - block_fraction) / block_update_interval_s
        else:
            rate = block_fraction / block_update_interval_s
        if rate <= 0:
            edge.next_change = math.inf
        else:
            edge.next_change = now + rng.expovariate(rate)

    edges: dict[tuple[str, str], _EdgeState] = {}
    for i in range(len(names)):
        for j in range(i + 1, len(names)):
            a, b = names[i], names[j]
            key = (a, b)
            edge = _EdgeState(a=a, b=b, blocked=False)
            schedule_edge_change(start, edge)
            edges[key] = edge

    num_block_events = 0
    num_latency_updates = 0
    num_stop_events = 0
    num_recover_events = 0

    log.info(
        "Starting fuzz scenario: N=%d duration=%ds seed=%d max_down=%d "
        "mean_stop=%.1fs mean_recover=%.1fs max_latency=%dms max_loss=%.2f%%",
        num_validators,
        duration_s,
        seed,
        max_down,
        mean_time_to_stop_s,
        mean_time_to_recover_s,
        max_latency_ms,
        max_loss_pct,
    )

    try:
        while True:
            now = time.monotonic()
            if now >= deadline:
                break

            next_node_change = min(st.next_change for st in nodes.values())
            next_latency_change = min(st.latency_next_change for st in nodes.values())
            next_edge_change = min(edge.next_change for edge in edges.values())
            next_event_time = min(next_node_change, next_latency_change, next_edge_change, deadline)

            sleep_for = max(0.0, next_event_time - time.monotonic())
            if sleep_for > 0:
                time.sleep(sleep_for)
            now = time.monotonic()
            if now >= deadline:
                break

            # Node state changes (stop / recover)
            for st in nodes.values():
                if st.next_change <= now:
                    down_count = sum(1 for s in nodes.values() if not s.up)
                    if st.up:
                        # request to go down, enforce < 1/3 constraint
                        if down_count >= max_down:
                            log.debug("Skipping stop of %s; max_down=%d already reached", st.name, max_down)
                            schedule_next_stop(now, st)
                            continue
                        disruptions.kill_node(st.name)
                        st.up = False
                        st.last_change = now
                        schedule_next_recover(now, st)
                        num_stop_events += 1
                        # Heal any blocks involving this node to keep state consistent.
                        for key, edge in edges.items():
                            if st.name in key and edge.blocked:
                                disruptions.unblock_connection(edge.a, edge.b)
                                edge.blocked = False
                                schedule_edge_change(now, edge)
                    else:
                        disruptions.restart_node(st.name)
                        st.up = True
                        st.total_down_time += now - st.last_change
                        st.last_change = now
                        schedule_next_stop(now, st)
                        num_recover_events += 1

            # Latency & loss updates (global Poisson process)
            if max_latency_ms > 0 or max_loss_pct > 0.0:
                for st in nodes.values():
                    if st.latency_next_change <= now:
                        if st.up:
                            delay = rng.randint(0, max_latency_ms) if max_latency_ms > 0 else 0
                            jitter = max(1, delay // 10) if delay > 0 else 0
                            loss = rng.uniform(0.0, max_loss_pct) if max_loss_pct > 0.0 else 0.0
                            # Apply per-destination latency/loss from this node to all other *up* nodes
                            for dst_name in names:
                                if dst_name == st.name or not nodes[dst_name].up:
                                    continue
                                disruptions.add_latency(
                                    st.name,
                                    dst_name,
                                    delay_ms=delay,
                                    jitter_ms=jitter,
                                    loss_pct=loss,
                                )
                            num_latency_updates += 1
                        schedule_latency_change(now, st)

            # Per-pair exponential block/unblock processes
            for edge in edges.values():
                if edge.next_change <= now:
                    # Only block/unblock when both nodes are up.
                    if not (nodes[edge.a].up and nodes[edge.b].up):
                        schedule_edge_change(now, edge)
                        continue
                    if edge.blocked:
                        disruptions.unblock_connection(edge.a, edge.b)
                        edge.blocked = False
                    else:
                        disruptions.block_connection(edge.a, edge.b)
                        edge.blocked = True
                    num_block_events += 1
                    schedule_edge_change(now, edge)
    finally:
        # Final accounting: close any open down intervals and heal network.
        end_time = time.monotonic()
        for st in nodes.values():
            if not st.up:
                st.total_down_time += end_time - st.last_change

        disruptions.reset_network(num_validators)
        if spammer_started:
            try:
                spammer.stop_stress_spammer()
            except Exception as exc:  # pragma: no cover - defensive
                log.warning("Failed to stop stress spammer during cleanup: %s", exc)

    details: dict[str, object] = {
        "num_validators": num_validators,
        "duration_s": duration_s,
        "seed": seed,
        "max_down": max_down,
        "num_stop_events": num_stop_events,
        "num_recover_events": num_recover_events,
        "num_latency_updates": num_latency_updates,
        "num_block_events": num_block_events,
        "node_down_time": {st.name: st.total_down_time for st in nodes.values()},
    }
    log.info("Completed fuzz scenario: %s", details)

    return ScenarioResult(name="fuzz_scenario", details=details)
