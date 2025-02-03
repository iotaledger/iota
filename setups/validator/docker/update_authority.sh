#!/bin/bash

IOTA_DOCKER_IMAGE=${IOTA_TOOLS_DOCKER_IMAGE:-"iotaledger/iota-tools:testnet"}

./check_iota_services.sh --docker ${IOTA_DOCKER_IMAGE}
if [ $? -ne 0 ]; then
    exit 1
fi

if [ ! -f "./key-pairs/authority.key" ]; then
    echo "Error: authority.key file not found in key-pairs directory"
    exit 1
fi

echo "Updating Authority Key..."
docker run --rm -v ./iota_config:/root/.iota/iota_config -v ./key-pairs:/iota/key-pairs ${IOTA_DOCKER_IMAGE} /bin/sh -c "/usr/local/bin/iota validator update-metadata authority-pub-key /iota/key-pairs/authority.key "
