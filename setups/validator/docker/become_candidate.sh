#!/bin/bash

FAUCET_URL=${IOTA_FAUCET_ENDPOINT:-"https://faucet.testnet.iota.cafe/v1/gas"}
IOTA_DOCKER_IMAGE=${IOTA_TOOLS_DOCKER_IMAGE:-"iotaledger/iota-tools:testnet"}

./check_iota_services.sh --faucet ${FAUCET_URL} --docker ${IOTA_DOCKER_IMAGE}
if [ $? -ne 0 ]; then
    exit 1
fi

if [ ! -f "./key-pairs/validator.info" ]; then
    echo "Error: ./key-pairs/validator.info file not found"
    exit 1
fi

cat ./key-pairs/validator.info

read -p "Are your hostname, node info, etc. correct? [y/N] " response
if [[ ! $response =~ ^[Yy]$ ]]; then
    echo "Operation cancelled"
    exit 1
fi

echo "Requesting gas fee from faucet..."
if docker run --rm -v ./iota_config:/root/.iota/iota_config $IOTA_DOCKER_IMAGE /bin/sh -c "/usr/local/bin/iota client faucet --url $FAUCET_URL --json"; then
    sleep 2
    
    echo "Sending request to be candidate..."
    docker run --rm -v ./iota_config:/root/.iota/iota_config -v ./key-pairs/validator.info:/iota/validator.info $IOTA_DOCKER_IMAGE /bin/sh -c "/usr/local/bin/iota validator become-candidate /iota/validator.info"
fi