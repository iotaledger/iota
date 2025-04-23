#!/bin/bash
WORKDIR="$( dirname "${BASH_SOURCE[0]}" )"
DOCKER_DIR="$WORKDIR/docker"
DATA_DIR="$DOCKER_DIR/data"
CONFIG_DIR="$DATA_DIR/config"

# check if the "data" folder exists
if [ -d "$DATA_DIR" ] && [ "$(ls -A "$DATA_DIR")" ]; then
    echo "Data folder found and not empty. Aborting."
    exit 1
fi

# create the "data" folder if it does not exist
mkdir -p "$CONFIG_DIR"

# download the genesis file
curl -fLJ https://dbfiles.devnet.iota.cafe/genesis.blob -o "$CONFIG_DIR/genesis.blob"
# download the migration file
curl -fLJ https://dbfiles.devnet.iota.cafe/migration.blob -o "$CONFIG_DIR/migration.blob"

# check if the "fullnode.yaml" file exists
if [ ! -f "$CONFIG_DIR/fullnode.yaml" ]; then
    echo "[INFO] fullnode.yaml not found, will create it from the devnet template."
    cp "$WORKDIR/../fullnode-template-devnet.yaml" ./data/config/fullnode.yaml
fi