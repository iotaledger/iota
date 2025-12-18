"""Prometheus and log parsing utilities used by the fuzz scenarios."""

from __future__ import annotations

from dataclasses import dataclass
from pathlib import Path

from . import docker_env


@dataclass
class SpammerStats:
    total_messages: int
    error_count: int
    duration_seconds: float


def _fetch_metrics(container_name: str, port: int = 9184) -> str:
    """Fetch metrics from a container using curl inside the container."""
    return docker_env.run_in_container(container_name, ["curl", "-s", f"http://localhost:{port}/metrics"])


def get_metric_value(metrics_text: str, metric_name: str) -> float | None:
    """Extract a simple metric value from Prometheus text format."""
    # Simplified parser: return the first matching series value.
    for line in metrics_text.splitlines():
        if line.startswith("#"):
            continue
        if metric_name in line:
            # Check if it's the metric name (followed by { or space)
            if line.startswith(metric_name) and (len(line) == len(metric_name) or line[len(metric_name)] in " {["):
                parts = line.split()
                try:
                    return float(parts[-1])
                except ValueError:
                    continue
    return None


def get_consensus_metrics(container_name: str) -> dict[str, float]:
    """Retrieve relevant consensus metrics from a validator."""
    try:
        text = _fetch_metrics(container_name)
    except docker_env.DockerEnvError:
        return {}

    metrics: dict[str, float] = {}

    # Progress
    val = get_metric_value(text, "consensus_last_committed_leader_round")
    if val is not None:
        metrics["last_committed_round"] = val

    val = get_metric_value(text, "last_executed_checkpoint")
    if val is not None:
        metrics["last_executed_checkpoint"] = val

    val = get_metric_value(text, "last_certified_checkpoint")
    if val is not None:
        metrics["last_certified_checkpoint"] = val

    val = get_metric_value(text, "last_constructed_checkpoint")
    if val is not None:
        metrics["last_constructed_checkpoint"] = val

    val = get_metric_value(text, "consensus_highest_accepted_round")
    if val is not None:
        metrics["highest_accepted_round"] = val

    # Liveness / Errors
    # These might be vectors; picking the first series is sufficient for smoke signals.
    val = get_metric_value(text, "consensus_timeout_total")
    if val is not None:
        metrics["timeouts"] = val

    val = get_metric_value(text, "consensus_proposal_interval_sum")
    if val is not None:
        metrics["proposal_interval_sum"] = val

    # Latency (support both legacy and header-level metrics)
    val = get_metric_value(text, "consensus_block_commit_latency_sum")
    if val is None:
        val = get_metric_value(text, "consensus_block_header_commit_latency_sum")
    if val is not None:
        metrics["block_commit_latency_sum"] = val

    val = get_metric_value(text, "consensus_block_commit_latency_count")
    if val is None:
        val = get_metric_value(text, "consensus_block_header_commit_latency_count")
    if val is not None:
        metrics["block_commit_latency_count"] = val

    # Synchronizer Load
    val = get_metric_value(text, "consensus_transaction_synchronizer_concurrent_requests")
    if val is not None:
        metrics["sync_concurrent_requests"] = val

    return metrics


def get_tps(prometheus_url: str, window_seconds: int) -> float:
    """Placeholder for compatibility with existing scripts."""
    return 0.0


def get_validator_availability(prometheus_url: str, window_seconds: int) -> dict[str, float]:
    """Placeholder for compatibility with existing scripts."""
    return {}


def parse_spammer_log(path: Path) -> SpammerStats:
    """Placeholder implementation for spammer log parsing."""
    return SpammerStats(0, 0, 0.0)
