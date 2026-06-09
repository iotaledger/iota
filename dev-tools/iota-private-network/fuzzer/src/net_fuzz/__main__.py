"""Module executed when running ``python -m net_fuzz``."""

from __future__ import annotations

from .cli import main


if __name__ == "__main__":  # pragma: no cover
    raise SystemExit(main())
