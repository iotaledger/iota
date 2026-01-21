"""Top-level package for modular network fuzzing utilities.

This package gradually replaces the bash-heavy scripts in
``experiments/network-fuzz.sh`` and related tooling.  Modules are
structured so they can be exercised from pytest-based unit tests while
still interoperating with the existing Docker-based network harness.
"""

from __future__ import annotations

import logging

DEFAULT_LOG_LEVEL = logging.INFO


def configure_logging(level: int = DEFAULT_LOG_LEVEL) -> None:
    """Configure a basic logging setup for the package."""

    logging.basicConfig(
        level=level,
        format="%(asctime)s [%(levelname)s] %(name)s: %(message)s",
    )


configure_logging()

__all__ = ["configure_logging"]
