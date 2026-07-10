#!/bin/bash

# Copyright (c) 2024 IOTA Stiftung
# SPDX-License-Identifier: Apache-2.0

usage() {
  echo "Usage: $0 [-f]" >&2
  echo "  -f   force: skip the /tmp/iota-experiments.lock check" >&2
  exit 2
}

FORCE=false
while getopts ":fh" opt; do
  case "$opt" in
    f) FORCE=true ;;
    h|*) usage ;;
  esac
done

if [[ "$OSTYPE" != "darwin"* && "$EUID" -ne 0 ]]; then
  echo "Please run as root or with sudo"
  exit
fi

# Refuse to tear down state while another runner holds the lock —
# `docker compose down --remove-orphans` would nuke its validators. Bypass via
# IOTA_EXPERIMENT_LOCK_HELD (set by a parent orchestrator) or an explicit -f.
LOCK_PATH="/tmp/iota-experiments.lock"

# Lock state of $LOCK_PATH: 0 = free, 3 = held, 4 = cannot tell. Shared flock
# on a read-only FD avoids a false "locked" under fs.protected_regular; the
# python3 fcntl probe covers macOS, which lacks flock(1).
experiment_lock_state() {
  if command -v flock >/dev/null 2>&1; then
    (flock -n -s 9) 9<"$LOCK_PATH" 2>/dev/null && return 0
    return 3
  fi
  if command -v python3 >/dev/null 2>&1; then
    python3 -c 'import fcntl, sys
try:
    fcntl.flock(open(sys.argv[1]), fcntl.LOCK_SH | fcntl.LOCK_NB)
except OSError:
    sys.exit(3)' "$LOCK_PATH" 2>/dev/null
    case "$?" in
      0) return 0 ;;
      3) return 3 ;;
    esac
  fi
  return 4
}

if [ "$FORCE" != "true" ] \
   && [ "${IOTA_EXPERIMENT_LOCK_HELD:-0}" != "1" ] \
   && [ -f "$LOCK_PATH" ]; then
  lock_state=0
  experiment_lock_state || lock_state=$?
  if [ "$lock_state" -eq 3 ]; then
    holder=$(cat "$LOCK_PATH" 2>/dev/null || true)
    echo "ERROR: another experiment run is active: ${holder:-(holder unknown)}" >&2
    echo "       Wait for it to finish, or pass -f to override." >&2
    exit 1
  elif [ "$lock_state" -ne 0 ]; then
    echo "ERROR: $LOCK_PATH exists but cannot be verified (need flock or a working python3)." >&2
    echo "       Install one, or pass -f if no experiment run is active." >&2
    exit 1
  fi
fi

docker compose down --remove-orphans
rm -rf data
