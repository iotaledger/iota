"""High-level fuzzing scenarios composed from the low-level primitives."""

from __future__ import annotations

import logging
from dataclasses import dataclass, field

from . import checks, disruptions

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
