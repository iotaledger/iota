#!/bin/bash

IOTA_DOCKER_IMAGE=${IOTA_TOOLS_DOCKER_IMAGE:-"iotaledger/iota-tools:testnet"}
API_URL=${IOTA_API_ENDPOINT:-"https://api.testnet.iota.cafe"}

if ! command -v jq &> /dev/null; then
    echo "jq is not installed. Installing jq..."
    apt update && apt install -y jq
fi

if [ -d "./iota_config" ] || [ -d "./key-pairs" ]; then
    read -p "./iota_config or key-pairs Directory already exists. This will overwrite everything? [y/N] " response
    if [[ ! $response =~ ^[Yy]$ ]]; then
        echo "Operation cancelled"
        exit 1
    fi

    rm -r ./iota_config
    rm -r ./key-pairs
fi

echo "Will using ${API_URL} for iota_config API endpopint"

./check_iota_services.sh --api ${API_URL} --docker ${IOTA_DOCKER_IMAGE}
if [ $? -ne 0 ]; then
    exit 1
fi

mkdir -p ./key-pairs
mkdir -p ./iota_config
TIMESTAMP=$(date +"%Y%m%d_%H%M%S")
mkdir -p "./tmp/backup_${TIMESTAMP}"

NEW_ADDRESS_OUTPUT=$(docker run --rm -v ./iota_config:/root/.iota/iota_config "${IOTA_DOCKER_IMAGE}" /bin/sh -c '/usr/local/bin/iota client -y new-address --json 2>&1')

IOTA_ADDRESS=$(echo "$NEW_ADDRESS_OUTPUT" | sed -n '/{/,/}/p' | jq -r '.address')


if [ -z "$IOTA_ADDRESS" ] || [ "$IOTA_ADDRESS" = "null" ]; then
    echo "Error: Failed to generate IOTA address"
    exit 1
fi

cp -r ./iota_config "./tmp/backup_${TIMESTAMP}/iota_config"

read -p "Enter validator name: " NAME
read -p "Enter validator description: " DESCRIPTION
read -p "Enter image URL (press enter for default): " IMAGE_URL
read -p "Enter project URL (press enter for default): " PROJECT_URL
read -p "Enter hostname: " HOST_NAME

IMAGE_URL=${IMAGE_URL:-""}
PROJECT_URL=${PROJECT_URL:-""}

docker run --rm -v ./iota_config:/root/.iota/iota_config -v "./key-pairs":/iota ${IOTA_DOCKER_IMAGE} /bin/sh -c "RUST_BACKTRACE=full /usr/local/bin/iota validator make-validator-info \"$NAME\" \"$DESCRIPTION\" \"$IMAGE_URL\" \"$PROJECT_URL\" \"$HOST_NAME\" 1000"

if [ ! "$(ls -A ./key-pairs)" ]; then
    echo "Error: Failed to generate validator info"
    exit 1
fi

cp -r ./key-pairs "./tmp/backup_${TIMESTAMP}/key-pairs"

echo -e "\nPlease share the following information in Slack:"
echo -e "\nValidator Address: ${IOTA_ADDRESS}"
echo "Script Version: $(git rev-parse --short HEAD)"
echo -e "\nMake sure to securely store your key files!\n"