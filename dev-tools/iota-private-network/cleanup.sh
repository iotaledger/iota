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

# Refuse to tear down state if another experiment runner holds the shared
# lock — `docker compose down --remove-orphans` here would silently nuke its
# validators. Bypass when the caller is itself a lock-holding orchestrator
# (it sets IOTA_EXPERIMENT_LOCK_HELD=1 in the child env before invoking us,
# so its own bootstrap.sh -> cleanup.sh chain still works) or when the user
# explicitly passes -f.
LOCK_PATH="/tmp/iota-experiments.lock"
# Read-only FD + shared (`-s`) non-blocking flock. Read-only opens are always
# allowed for mode-666 files, unlike write opens which fs.protected_regular=2
# blocks on files owned by other users in sticky /tmp — a plain
# `flock -n FILE ...` would then report the lock as held even after a dead
# holder's flock had been released, because it can't get past the open().
# A shared flock succeeds iff no process holds an EXCLUSIVE flock, which is
# exactly what the orchestrator's fcntl.LOCK_EX contends for.
if [ "$FORCE" != "true" ] \
   && [ "${IOTA_EXPERIMENT_LOCK_HELD:-0}" != "1" ] \
   && [ -f "$LOCK_PATH" ] \
   && ! (flock -n -s 9) 9<"$LOCK_PATH" 2>/dev/null; then
  holder=$(cat "$LOCK_PATH" 2>/dev/null || true)
  echo "ERROR: another experiment run is active: ${holder:-(holder unknown)}" >&2
  echo "       Wait for it to finish, or pass -f to override." >&2
  exit 1
fi

docker compose down --remove-orphans
rm -rf data
