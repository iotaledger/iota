#!/bin/bash -ex
WORKDIR="$(dirname "${BASH_SOURCE[0]}")"
DATA_DIR="$WORKDIR/data"
CONFIG_DIR="$DATA_DIR/config"

# check if the "data" folder exists
if [ -d "$CONFIG_DIR" ] && ([ -f "$CONFIG_DIR/genesis.blob" ] || [ -f "$CONFIG_DIR/migration.blob" ]); then
	echo "Config folder found and snapshot files already exist. Aborting."
	exit 1
fi

# Pull latest images
docker compose pull

# download the genesis file
curl -fLJ https://dbfiles.mainnet.iota.cafe/genesis.blob -o "$CONFIG_DIR/genesis.blob"
# download the migration file
curl -fLJ https://dbfiles.mainnet.iota.cafe/migration.blob -o "$CONFIG_DIR/migration.blob"
