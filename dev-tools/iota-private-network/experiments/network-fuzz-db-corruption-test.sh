#!/usr/bin/env bash
# Copyright (c) 2025 IOTA Stiftung
# SPDX-License-Identifier: Apache-2.0

set -euo pipefail
IFS=$'\n\t'

if [ $# -ne 1 ]; then
  echo "Usage: $0 <validator-name>"
  exit 1
fi

VALIDATOR="$1"
DATA_DIR="./data/${VALIDATOR}"
DB_DIR="${DATA_DIR}/consensus_db"
SNAPSHOT_DIR="${DATA_DIR}/consensus_db_snapshot"

log() {
  echo "[$(date -Iseconds)] $*"
}

# 1. Snapshot live DB
log "Creating snapshot of ${VALIDATOR} consensus_db"
sudo cp -r "${DB_DIR}" "${SNAPSHOT_DIR}"
log "Snapshot saved to ${SNAPSHOT_DIR}"

sleep 1

# 2. Stop the container
log "Stopping container ${VALIDATOR}"
docker stop "${VALIDATOR}"

# 3. Restore snapshot
log "Restoring snapshot to live DB"
sudo rm -rf "${DB_DIR}"
sudo mv "${SNAPSHOT_DIR}" "${DB_DIR}"
log "Restoration complete"

# 4. Restart the container
log "Restarting container ${VALIDATOR}"
docker start "${VALIDATOR}"

# 5. Verify
log "Verifying inside container"
docker exec -it "${VALIDATOR}" ls /opt/iota/db/consensus_db
log "Done."