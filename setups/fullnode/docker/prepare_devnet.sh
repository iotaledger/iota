#!/bin/bash

# check if the "data" folder exists
if [ -d ./data ] && [ "$(ls -A ./data)" ]; then
    echo "Data folder found and not empty. Aborting."
    exit 1
fi

# create the "data" folder if it does not exist
mkdir -p ./data/config

# download the genesis file
curl -fLJ https://dbfiles.devnet.iota.cafe/genesis.blob -o ./data/config/genesis.blob

# download the migration file
curl -fLJ https://dbfiles.devnet.iota.cafe/migration.blob -o ./data/config/migration.blob

# check if the "fullnode.yaml" file exists
if [ ! -f ./data/config/fullnode.yaml ]; then
    echo "Error: fullnode.yaml not found, copying from the devnet template."
    cp ../fullnode-template-devnet.yaml ./data/config/fullnode.yaml
fi