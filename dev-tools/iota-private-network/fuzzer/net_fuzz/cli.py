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
    run.add_argument("--name", required=True, help="Scenario name, e.g. latency or fuzz")
    run.add_argument("--src", help="Source validator/container name")
    run.add_argument("--dst", help="Destination validator/container name")
    run.add_argument("--delay-ms", type=int, default=100)

    # Long-running fuzz scenario parameters
    run.add_argument("--num-validators", type=int, default=4, help="Number of validators (validator-1..N)")
    run.add_argument("--duration", type=int, default=600, help="Scenario duration in seconds")
    run.add_argument("--seed", type=int, default=42, help="Random seed for deterministic runs")
    run.add_argument(
        "--mean-down",
        type=float,
        default=120.0,
        help="Mean time (seconds) a node stays up before going down",
    )
    run.add_argument(
        "--mean-up",
        type=float,
        default=180.0,
        help="Mean time (seconds) a node stays down before recovering",
    )
    run.add_argument(
        "--max-latency-ms",
        type=int,
        default=200,
        help="Upper bound for per-node latency (ms)",
    )
    run.add_argument(
        "--max-loss-pct",
        type=float,
        default=5.0,
        help="Upper bound for per-node packet loss (percent)",
    )
    run.add_argument(
        "--latency-interval",
        type=float,
        default=30.0,
        help="Seconds between latency/loss reconfigurations",
    )
    run.add_argument(
        "--block-interval",
        type=float,
        default=20.0,
        help="Seconds between block/unblock churn events",
    )
    run.add_argument(
        "--block-fraction",
        type=float,
        default=0.2,
        help="Target long-run fraction of time a pair is blocked (0..1)",
    )
    run.add_argument(
        "--spammer-tps",
        type=int,
        default=0,
        help="If >0, start stress spammer at this TPS during the fuzz scenario",
    )

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
        if args.name == "fuzz":
            result = scenarios.fuzz_scenario(
                num_validators=args.num_validators,
                duration_s=args.duration,
                seed=args.seed,
                mean_time_to_stop_s=args.mean_down,
                mean_time_to_recover_s=args.mean_up,
                max_latency_ms=args.max_latency_ms,
                max_loss_pct=args.max_loss_pct,
                latency_update_interval_s=args.latency_interval,
                block_update_interval_s=args.block_interval,
                block_fraction=args.block_fraction,
                spammer_tps=args.spammer_tps,
            )
            log.info("Scenario completed: %s", result)
            return 0
        parser.error("Unsupported scenario or missing required parameters")
    else:
        parser.error(f"Unknown command {args.command}")

    return 1


if __name__ == "__main__":  # pragma: no cover
    raise SystemExit(main())
