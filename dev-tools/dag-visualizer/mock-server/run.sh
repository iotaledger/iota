#!/bin/bash
#
# Start the mock DAG visualizer backend.
# All arguments are forwarded to mock_server.py.
#
# Examples:
#   ./run.sh
#   ./run.sh --validators 100 --miss-rate 0.1
#   ./run.sh --validators 50 --skip-rate 0.3 --round-interval-ms 300
#   ./run.sh --equivocation-rate 0.02

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
cd "$SCRIPT_DIR"

# Source the project's Python venv wrapper (creates venv, installs deps)
source "$(cd "$SCRIPT_DIR/../../.." && pwd)/scripts/utils/python_venv_wrapper.sh"

exec "$PYTHON_CMD" mock_server.py "$@"
