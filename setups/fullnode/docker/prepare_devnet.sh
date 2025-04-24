#!/bin/bash -ex
WORKDIR="$( dirname "${BASH_SOURCE[0]}" )"
DATA_DIR="$WORKDIR/data"
CONFIG_DIR="$DATA_DIR/config"

# check if the "data" folder exists
if [ -d "$CONFIG_DIR" ] && ([ -f "$CONFIG_DIR/genesis.blob" ] || [ -f "$CONFIG_DIR/migration.blob" ]); then
    echo "Config folder found and not empty. Aborting."
    exit 1
fi

# create "data/" and "data/config/"
mkdir -p "$CONFIG_DIR"
# download the genesis file
curl -fLJ https://dbfiles.devnet.iota.cafe/genesis.blob -o "$CONFIG_DIR/genesis.blob"
# download the migration file
curl -fLJ https://dbfiles.devnet.iota.cafe/migration.blob -o "$CONFIG_DIR/migration.blob"

