"""Entry point for running fuzz scenarios from the command line."""

from __future__ import annotations

import argparse
import logging

from . import configure_logging, scenarios

log = logging.getLogger(__name__)


def build_parser() -> argparse.ArgumentParser:
    """Return the top-level argument parser used by ``python -m net_fuzz``."""

    parser = argparse.ArgumentParser(prog="net_fuzz", description="Network fuzzing orchestrator")
    sub = parser.add_subparsers(dest="command", required=True)

    run = sub.add_parser("run-scenario", help="Run a named fuzz scenario")
    run.add_argument("--name", required=True, help="Scenario name, e.g. random_partition")
    run.add_argument("--src", help="Source validator/container name")
    run.add_argument("--dst", help="Destination validator/container name")
    run.add_argument("--delay-ms", type=int, default=100)

    return parser


def main(argv: list[str] | None = None) -> int:
    """Dispatch CLI requests to scenario helpers."""

    configure_logging()
    parser = build_parser()
    args = parser.parse_args(argv)

    if args.command == "run-scenario":
        if args.name == "latency" and args.src and args.dst:
            result = scenarios.add_latency_between_validators(args.src, args.dst, args.delay_ms)
            log.info("Scenario completed: %s", result)
            return 0
        parser.error("Unsupported scenario or missing --src/--dst")
    else:
        parser.error(f"Unknown command {args.command}")

    return 1


if __name__ == "__main__":  # pragma: no cover
    raise SystemExit(main())
