#!/bin/bash -ex
WORKDIR="$(dirname "${BASH_SOURCE[0]}")"
DATA_DIR="$WORKDIR/data"
CONFIG_DIR="$DATA_DIR/config"

# check if the "config/" dir exists
if [ -d "$CONFIG_DIR" ] && ([ -f "$CONFIG_DIR/genesis.blob" ] || [ -f "$CONFIG_DIR/migration.blob" ]); then
	echo "Config folder found and snapshot files already exist. Aborting."
	exit 1
fi

# Pull latest images
docker compose pull

# download the genesis file
curl -fLJ https://dbfiles.testnet.iota.cafe/genesis.blob -o $CONFIG_DIR/genesis.blob
