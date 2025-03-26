#!/bin/bash

IOTA_DOCKER_IMAGE=${IOTA_TOOLS_DOCKER_IMAGE:-"iotaledger/iota-tools:testnet"}

./check_iota_services.sh --docker ${IOTA_DOCKER_IMAGE}
if [ $? -ne 0 ]; then
    exit 1
fi

echo "Joining committee..."
docker run --rm -v ./iota_config:/root/.iota/iota_config ${IOTA_DOCKER_IMAGE} /bin/sh -c "/usr/local/bin/iota validator join-validators"