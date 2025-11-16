"""Prometheus and log parsing utilities used by the fuzz scenarios."""

from __future__ import annotations

from dataclasses import dataclass
from pathlib import Path


@dataclass
class SpammerStats:
    total_messages: int
    error_count: int
    duration_seconds: float


def get_tps(prometheus_url: str, window_seconds: int) -> float:
    raise NotImplementedError


def get_validator_availability(prometheus_url: str, window_seconds: int) -> dict[str, float]:
    raise NotImplementedError


def parse_spammer_log(path: Path) -> SpammerStats:
    raise NotImplementedError
