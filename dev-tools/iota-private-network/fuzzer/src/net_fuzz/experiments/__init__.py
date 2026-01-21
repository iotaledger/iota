"""Helpers for long-running experiment scripts."""

from __future__ import annotations

from datetime import datetime
from pathlib import Path
import logging
import threading

from .. import configure_logging, docker_env


def _private_network_root() -> Path:
    return Path(__file__).resolve().parents[4]


def configure_experiment_logging(experiment: str) -> Path:
    """Configure logging to stderr and a timestamped file under experiments/logs."""
    configure_logging()
    log_dir = _private_network_root() / "experiments" / "logs"
    log_dir.mkdir(parents=True, exist_ok=True)
    timestamp = datetime.now().strftime("%Y%m%d-%H%M%S")
    log_path = log_dir / f"{experiment}-{timestamp}.log"

    root_logger = logging.getLogger()
    log_format = "%(asctime)s [%(levelname)s] %(name)s: %(message)s"
    handler = logging.FileHandler(log_path, mode="a")
    handler.setFormatter(logging.Formatter(log_format))
    root_logger.addHandler(handler)
    root_logger.info("Logging to %s", log_path)
    return log_path


class ValidatorLogCollector:
    """Periodically capture validator logs into the experiment log directory."""

    def __init__(self, validators: list[str], log_path: Path, *, interval_s: int = 60) -> None:
        self._validators = list(validators)
        self._interval_s = interval_s
        self._log_dir = log_path.parent / f"{log_path.stem}-validators"
        self._log_dir.mkdir(parents=True, exist_ok=True)
        self._stop_event = threading.Event()
        self._thread = threading.Thread(
            target=self._run,
            name=f"{log_path.stem}-validator-logs",
            daemon=True,
        )
        self._log = logging.getLogger(__name__)
        self._log.info("Validator logs will be written to %s", self._log_dir)

    def start(self) -> None:
        self._thread.start()

    def stop(self) -> None:
        self._stop_event.set()
        self._thread.join(timeout=self._interval_s + 5)
        self._write_logs(suffix="final")

    def _run(self) -> None:
        while not self._stop_event.is_set():
            self._write_logs(suffix="latest")
            self._stop_event.wait(self._interval_s)

    def _write_logs(self, *, suffix: str) -> None:
        for name in self._validators:
            try:
                output = docker_env.get_container_logs(name)
            except docker_env.DockerEnvError as exc:
                self._log.debug("Failed to read logs from %s: %s", name, exc)
                continue
            log_path = self._log_dir / f"{name}-{suffix}.log"
            log_path.write_text(output, encoding="utf-8")


def start_validator_log_collection(
    validators: list[str],
    log_path: Path,
    *,
    interval_s: int = 60,
) -> ValidatorLogCollector:
    """Start background collection of validator logs."""
    collector = ValidatorLogCollector(validators, log_path, interval_s=interval_s)
    collector.start()
    return collector
