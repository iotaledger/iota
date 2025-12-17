"""Sanity checks for low-level disruption primitives.

This script exercises the basic operations provided by :mod:`disruptions`
and verifies them via :mod:`checks` on a running validator cluster.

It is intended to be run manually, for example:

    cd dev-tools/iota-private-network
    source .venv/bin/activate
    PYTHON=$(python -c 'import sys; print(sys.executable)')
    sudo -E "$PYTHON" -m net_fuzz.verify_disruptions --num-validators 4

The script will:

- kill and restart a validator, checking liveness before/after
- apply and verify a bidirectional block between two validators
- apply and verify per-destination latency and loss from one validator
  to another

All checks are best-effort and the script exits with status 0 on
success or 1 if any test fails or raises an error.
"""

from __future__ import annotations

import argparse
import logging
import time
from typing import Callable

from . import checks, configure_logging, disruptions

log = logging.getLogger(__name__)


def _wait_for(func: Callable[[], bool], timeout_s: float, interval_s: float = 0.5) -> bool:
    """Poll ``func`` until it returns True or timeout is reached."""

    deadline = time.time() + timeout_s
    while time.time() < deadline:
        if func():
            return True
        time.sleep(interval_s)
    return False


def test_restart_validator(name: str, timeout_s: float = 60.0) -> bool:
    log.info("TEST restart/kill: target=%s", name)
    ok = True

    if not checks.check_node_up(name):
        log.warning("Validator %s is not reported up before restart test", name)

    disruptions.kill_node(name)
    if not _wait_for(lambda: checks.check_node_down(name), timeout_s):
        log.error("Validator %s did not go down within %.1fs", name, timeout_s)
        ok = False

    disruptions.restart_node(name)
    if not _wait_for(lambda: checks.check_node_up(name), timeout_s):
        log.error("Validator %s did not come back up within %.1fs", name, timeout_s)
        ok = False

    if ok:
        log.info("TEST restart/kill: OK for %s", name)
    return ok


def test_blocking(src: str, dst: str, timeout_s: float = 30.0) -> bool:
    log.info("TEST blocking: src=%s dst=%s", src, dst)
    ok = True

    # Ensure we start from an unblocked state if possible
    if checks.check_blocked(src, dst):
        log.info("Pair %s<->%s already blocked, attempting to unblock first", src, dst)
        disruptions.unblock_connection(src, dst)
        if not _wait_for(lambda: checks.check_unblocked(src, dst), timeout_s):
            log.error("Failed to clear existing block before test for %s<->%s", src, dst)
            return False

    # Apply block and verify
    disruptions.block_connection(src, dst)
    if not _wait_for(lambda: checks.check_blocked(src, dst), timeout_s):
        log.error("Block check failed for %s<->%s", src, dst)
        ok = False

    # Remove block and verify
    disruptions.unblock_connection(src, dst)
    if not _wait_for(lambda: checks.check_unblocked(src, dst), timeout_s):
        log.error("Unblock check failed for %s<->%s", src, dst)
        ok = False

    if ok:
        log.info("TEST blocking: OK for %s<->%s", src, dst)
    return ok


def test_latency_and_loss(
    src: str,
    dst: str,
    delay_ms: int = 100,
    jitter_ms: int = 10,
    loss_pct: float = 2.0,
    timeout_s: float = 30.0,
) -> bool:
    log.info(
        "TEST latency/loss: src=%s dst=%s delay=%sms jitter=%sms loss=%.2f%%",
        src,
        dst,
        delay_ms,
        jitter_ms,
        loss_pct,
    )
    ok = True

    # Apply per-destination latency + loss
    disruptions.add_latency(src, dst, delay_ms=delay_ms, jitter_ms=jitter_ms, loss_pct=loss_pct)

    # Allow a short settling period
    time.sleep(1.0)

    # Expect the configured delay (parsed from tc qdisc), with a small tolerance
    delta_ms = max(2, int(jitter_ms / 2))
    min_delay = max(0, delay_ms - delta_ms)
    max_delay = delay_ms + delta_ms

    if not _wait_for(lambda: checks.check_latency(src, dst, min_delay, max_delay), timeout_s):
        log.error("Latency check failed for %s->%s", src, dst)
        ok = False

    # Expect loss around the configured value, with a small tolerance
    loss_tol = max(0.5, loss_pct * 0.25)
    min_loss = max(0.0, loss_pct - loss_tol)
    max_loss = loss_pct + loss_tol

    if not _wait_for(lambda: checks.check_loss(src, min_loss, max_loss), timeout_s):
        log.error("Loss check failed for %s", src)
        ok = False

    if ok:
        log.info("TEST latency/loss: OK for %s->%s", src, dst)
    return ok


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        prog="net_fuzz.verify_disruptions",
        description="Verify low-level disruption primitives against a running validator cluster.",
    )
    parser.add_argument(
        "--num-validators",
        type=int,
        default=4,
        help="Number of validators (validator-1..N) for reset-network cleanup (default: 4)",
    )
    parser.add_argument(
        "--src",
        default="validator-1",
        help="Source validator/container name for latency/loss and blocking tests (default: validator-1)",
    )
    parser.add_argument(
        "--dst",
        default="validator-2",
        help="Destination validator/container name for latency/loss and blocking tests (default: validator-2)",
    )
    parser.add_argument(
        "--delay-ms",
        type=int,
        default=100,
        help="Latency in milliseconds for latency test (default: 100)",
    )
    parser.add_argument(
        "--jitter-ms",
        type=int,
        default=10,
        help="Jitter in milliseconds for latency test (default: 10)",
    )
    parser.add_argument(
        "--loss-pct",
        type=float,
        default=2.0,
        help="Packet loss percentage for latency/loss test (default: 2.0)",
    )
    parser.add_argument(
        "--skip-reset",
        action="store_true",
        help="Do not call reset_network before/after tests (use with care).",
    )
    return parser


def main(argv: list[str] | None = None) -> int:
    configure_logging()
    parser = build_parser()
    args = parser.parse_args(argv)

    log.info("Starting low-level disruption verification")

    try:
        if not args.skip_reset:
            log.info("Resetting network before tests (num_validators=%d)", args.num_validators)
            disruptions.reset_network(args.num_validators)

        ok_restart = test_restart_validator(args.src)
        ok_block = test_blocking(args.src, args.dst)
        ok_lat = test_latency_and_loss(
            args.src,
            args.dst,
            delay_ms=args.delay_ms,
            jitter_ms=args.jitter_ms,
            loss_pct=args.loss_pct,
        )

        all_ok = ok_restart and ok_block and ok_lat

    except disruptions.DisruptionError as exc:
        log.error("DisruptionError during verification: %s", exc)
        all_ok = False
    except Exception as exc:  # pragma: no cover - defensive
        log.exception("Unexpected error during verification: %s", exc)
        all_ok = False
    finally:
        if not args.skip_reset:
            log.info("Resetting network after tests")
            try:
                disruptions.reset_network(args.num_validators)
            except Exception as exc:  # pragma: no cover - best effort cleanup
                log.warning("Failed to reset network after tests: %s", exc)

    if all_ok:
        log.info("All disruption checks PASSED")
        return 0

    log.error("One or more disruption checks FAILED")
    return 1


if __name__ == "__main__":  # pragma: no cover
    raise SystemExit(main())

