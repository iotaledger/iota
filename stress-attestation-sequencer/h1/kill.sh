#!/usr/bin/env bash
#
# kill.sh — stop a running matrix.sh (or a stray run.sh) and everything it
# spawned, then tear down the docker containers it left behind.
#
# matrix.sh is launched backgrounded, so it's its own process-group leader and all
# its HOST children (run.sh, cargo/docker CLIs, the host `stress` binary, the sudo
# keepalive subshell) share its process group — we signal the whole group at once.
# The docker CONTAINERS are owned by the docker daemon (not children of matrix.sh),
# so they survive the kill and must be torn down separately via cleanup.sh.
#
# Best-effort: every step tolerates "nothing to do". Run from anywhere.
set -uo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)" # .../h1
TOOLS_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"                  # .../stress-attestation-sequencer (cleanup.sh lives here)

# 1. Kill the matrix.sh process group (all host subprocesses) if it's running.
#    Only group-kill when matrix.sh is actually the group leader (PGID == PID), so
#    we never accidentally signal a shared parent group (e.g. an interactive shell).
pid="$(pgrep -f '[m]atrix\.sh' | head -1 || true)"
if [[ -n "$pid" ]]; then
  pgid="$(ps -o pgid= -p "$pid" | tr -d ' ')"
  if [[ -n "$pgid" && "$pgid" == "$pid" ]]; then
    echo "Killing matrix.sh process group (PID=$pid PGID=$pgid)..."
    sudo kill -TERM -- -"$pgid" 2>/dev/null || true
  else
    echo "matrix.sh PID=$pid is not its own group leader (PGID=$pgid); killing PID only..."
    sudo kill -TERM "$pid" 2>/dev/null || true
  fi
  sleep 2
else
  echo "No matrix.sh process found."
fi

# 2. Mop up stragglers by name (anything that escaped the group).
for pat in '[m]atrix\.sh' 'h1/run\.sh' 'target/release/stress'; do
  sudo pkill -9 -f "$pat" 2>/dev/null || true
done

# 3. Tear down the docker containers it left running (network + monitoring). The
#    daemon owns these, so they outlive the killed host processes.
echo "Tearing down docker network + monitoring (cleanup.sh)..."
sudo "$TOOLS_DIR/cleanup.sh" || true

# 4. Verify.
echo "--- remaining host processes (want: none) ---"
pgrep -af 'matrix\.sh|h1/run\.sh|target/release/stress' || echo "  none"
echo "--- remaining node containers (want: none) ---"
docker ps --format '{{.Names}}' | grep -E '^(validator|fullnode)-[0-9]+$' || echo "  none"
