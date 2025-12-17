"""Helpers to run a background spammer against the private network.

This module wraps the `stress` binary from the `iota-benchmark` crate,
running it via the `iotaledger/iota-tools` Docker image on the
`iota-private-network_iota-network` Docker network.

The main entry points are:

- :func:`start_stress_spammer` to start a background spammer container
  with a given target TPS (QPS) and optional auto-stop duration.
- :func:`is_stress_spammer_running` to check whether the container is up.
- A small CLI:

    PYTHON=$(python -c 'import sys; print(sys.executable)')
    sudo -E "$PYTHON" -m net_fuzz.spammer --tps 500 --duration 600
"""

from __future__ import annotations

import argparse
import logging
import subprocess
import threading
from pathlib import Path

log = logging.getLogger(__name__)

_DEFAULT_CONTAINER_NAME = "stress-benchmark"
_DEFAULT_NETWORK_NAME = "iota-private-network_iota-network"


def _run_host_command(args: list[str], *, check: bool = True) -> subprocess.CompletedProcess[str]:
    log.debug("Host command: %s", " ".join(args))
    result = subprocess.run(args, capture_output=True, text=True)
    if check and result.returncode != 0:
        raise RuntimeError(
            f"Command {' '.join(args)} failed (code={result.returncode}): {result.stderr.strip()}"
        )
    return result


def _private_network_root() -> Path:
    """Return the private-network root directory.

    We resolve relative to this file:
    net_fuzz/spammer.py → fuzzer/ → iota-private-network/
    """

    return Path(__file__).resolve().parents[3]


def _container_running(name: str) -> bool:
    """Return True if a container with the given name is running."""

    res = _run_host_command(["docker", "ps", "--format", "{{.Names}}"], check=False)
    names = {line.strip() for line in res.stdout.splitlines() if line.strip()}
    return name in names


def _ensure_service_running(service: str, privnet_root: Path) -> bool:
    """Ensure a docker-compose service is running.

    Returns True if the service was started by this call, False if it
    was already running or if startup failed.
    """

    if _container_running(service):
        return False

    log.info("Starting %s via docker compose", service)
    result = subprocess.run(
        ["docker", "compose", "up", "-d", service],
        cwd=str(privnet_root),
        capture_output=True,
        text=True,
    )
    if result.returncode != 0:
        log.warning(
            "Failed to start %s via docker compose (code=%s): %s",
            service,
            result.returncode,
            result.stderr.strip(),
        )
        return False
    return True


def start_stress_spammer(
    tps: int,
    duration_s: int | None = None,
    *,
    container_name: str = _DEFAULT_CONTAINER_NAME,
    network_name: str = _DEFAULT_NETWORK_NAME,
) -> None:
    """Start the `stress` spammer in a detached Docker container.

    Parameters
    ----------
    tps:
        Target transactions per second (mapped to `--target-qps`).
    duration_s:
        Optional wall-clock duration in seconds. If provided, a background
        thread will issue `docker stop <container_name>` after this delay.
        The stress binary itself runs in unbounded mode; we control lifetime
        via the container.
    container_name:
        Name for the spammer container (default: `stress-benchmark`).
    network_name:
        Docker network to attach to (default:
        `iota-private-network_iota-network`).
    """

    privnet_root = _private_network_root()
    genesis_blob = privnet_root / "configs" / "genesis" / "genesis.blob"
    keystore = privnet_root / "configs" / "faucet" / "iota.keystore"

    if not genesis_blob.is_file():
        raise RuntimeError(f"Genesis blob not found at {genesis_blob}")
    if not keystore.is_file():
        raise RuntimeError(f"Faucet keystore not found at {keystore}")

    # Ensure fullnode-1 and faucet-1 are running; start them via docker compose if needed.
    started_fullnode = _ensure_service_running("fullnode-1", privnet_root)
    started_faucet = _ensure_service_running("faucet-1", privnet_root)

    # If we just started the faucet, give it a short head start before spamming.
    if started_faucet:
        import time

        log.info("Faucet started; sleeping 20s before starting stress spammer")
        time.sleep(20)

    # Best-effort stop any previous instance with the same name.
    _run_host_command(["docker", "stop", container_name], check=False)

    cmd = [
        "docker",
        "run",
        "-d",
        "--rm",
        "--name",
        container_name,
        "--network",
        network_name,
        "-v",
        f"{genesis_blob}:/opt/iota/config/genesis.blob:ro",
        "-v",
        f"{keystore}:/opt/iota/config/iota.keystore:ro",
        "iotaledger/iota-tools",
        "/usr/local/bin/stress",
        "--local",
        "false",
        "--use-fullnode-for-execution",
        "true",
        "--fullnode-rpc-addresses",
        "http://fullnode-1:9000",
        "--genesis-blob-path",
        "/opt/iota/config/genesis.blob",
        "--keystore-path",
        "/opt/iota/config/iota.keystore",
        "--primary-gas-owner-id",
        "0x7cc6ff19b379d305b8363d9549269e388b8c1515772253ed4c868ee80b149ca0",
        "bench",
        "--target-qps",
        str(tps),
        "--in-flight-ratio",
        "5",
        "--transfer-object",
        "100",
    ]

    log.info(
        "Starting stress spammer: tps=%s container=%s duration_s=%s",
        tps,
        container_name,
        duration_s,
    )
    _run_host_command(cmd, check=True)

    if duration_s is not None and duration_s > 0:
        def _stop_later() -> None:
            import time

            time.sleep(duration_s)
            log.info("Stopping stress spammer container %s after %ss", container_name, duration_s)
            _run_host_command(["docker", "stop", container_name], check=False)

        threading.Thread(target=_stop_later, name="stress-spammer-stop", daemon=True).start()


def is_stress_spammer_running(container_name: str = _DEFAULT_CONTAINER_NAME) -> bool:
    """Return True if the stress spammer container is currently running."""

    res = _run_host_command(["docker", "ps", "--format", "{{.Names}}"], check=False)
    names = {line.strip() for line in res.stdout.splitlines() if line.strip()}
    return container_name in names


def stop_stress_spammer(container_name: str = _DEFAULT_CONTAINER_NAME) -> None:
    """Best-effort stop and cleanup of the stress spammer container.

    Containers started by :func:`start_stress_spammer` use ``--rm``, so
    stopping them is sufficient to remove them.  This helper is still
    careful to also attempt an explicit ``docker rm`` in case a previous
    run created a container without ``--rm``.
    """

    log.info("Stopping stress spammer container %s (if running)", container_name)
    _run_host_command(["docker", "stop", container_name], check=False)
    _run_host_command(["docker", "rm", container_name], check=False)


def test_stress_spammer(
    tps: int = 50,
    duration_s: int = 20,
    *,
    container_name: str = _DEFAULT_CONTAINER_NAME,
) -> bool:
    """Smoke-test that the stress spammer can be started.

    Starts a short-lived spammer run and checks that the container shows
    up in `docker ps` shortly after launch.
    """

    start_stress_spammer(tps=tps, duration_s=duration_s, container_name=container_name)

    # Give Docker a moment to start the container.
    import time

    deadline = time.time() + 10.0
    while time.time() < deadline:
        if is_stress_spammer_running(container_name):
            log.info("Stress spammer container %s is running", container_name)
            return True
        time.sleep(0.5)

    log.warning("Stress spammer container %s did not appear within 10s", container_name)
    return False


def _build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        prog="net_fuzz.spammer",
        description="Start a background stress spammer against the private network.",
    )
    parser.add_argument(
        "--tps",
        type=int,
        required=True,
        help="Target transactions per second (mapped to stress --target-qps).",
    )
    parser.add_argument(
        "--duration",
        type=int,
        default=0,
        help="Optional duration in seconds after which the container will be stopped (0 = no auto-stop).",
    )
    parser.add_argument(
        "--container-name",
        default=_DEFAULT_CONTAINER_NAME,
        help=f"Spammer container name (default: {_DEFAULT_CONTAINER_NAME}).",
    )
    parser.add_argument(
        "--network-name",
        default=_DEFAULT_NETWORK_NAME,
        help=f"Docker network name (default: {_DEFAULT_NETWORK_NAME}).",
    )
    parser.add_argument(
        "--test",
        action="store_true",
        help="Run a simple smoke test (start + check container shows up).",
    )
    return parser


def main(argv: list[str] | None = None) -> int:
    from . import configure_logging

    configure_logging()
    parser = _build_parser()
    args = parser.parse_args(argv)

    if args.test:
        ok = test_stress_spammer(tps=args.tps, duration_s=args.duration, container_name=args.container_name)
        return 0 if ok else 1

    start_stress_spammer(
        tps=args.tps,
        duration_s=args.duration if args.duration > 0 else None,
        container_name=args.container_name,
        network_name=args.network_name,
    )
    return 0


if __name__ == "__main__":  # pragma: no cover
    raise SystemExit(main())
