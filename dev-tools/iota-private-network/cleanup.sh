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
# Read-only FD + shared (-s) non-blocking flock. A write open (plain
# `flock -n FILE`) can be blocked by fs.protected_regular on a /tmp file owned
# by another (possibly dead) user, giving a false "locked"; a read open always
# succeeds. The shared flock then succeeds iff no one holds the orchestrator's
# exclusive lock.
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
