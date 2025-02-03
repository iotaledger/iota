#!/bin/bash

while [[ $# -gt 0 ]]; do
    case $1 in
        --api)
            API_URL="$2"
            shift 2
            ;;
        --faucet)
            FAUCET_URL="$2"
            shift 2
            ;;
        --docker)
            IOTA_DOCKER_IMAGE="$2"
            shift 2
            ;;
        *)
            shift
            ;;
    esac
done

if [ ! -z "${API_URL}" ]; then
    API_RESPONSE=$(curl --silent --location "${API_URL}" \
    --header 'Content-Type: application/json' \
    --data '{
      "jsonrpc": "2.0",
      "id": 1,
      "method": "iota_getLatestCheckpointSequenceNumber",
      "params": []
    }')

    if [ $? -ne 0 ]; then
        echo "Error: Unable to connect to IOTA_API_ENDPOINT: ${API_URL}"
        echo "Please check your API_URL or set a different endpoint using: export IOTA_API_ENDPOINT=<your-api-endpoint>"
        exit 1
    fi
fi

if [ ! -z "${IOTA_DOCKER_IMAGE}" ]; then
    if ! docker manifest inspect ${IOTA_DOCKER_IMAGE} >/dev/null 2>&1; then
        echo "Error: Unable to access Docker image: ${IOTA_DOCKER_IMAGE}"
        echo "Please check if the image exists or set a different image using: export IOTA_TOOLS_DOCKER_IMAGE=<your-image>"
        exit 1
    fi
fi

if [ ! -z "${FAUCET_URL}" ]; then
    FAUCET_RESPONSE=$(curl --silent --location "${FAUCET_URL}")

    if [ $? -ne 0 ]; then
        echo "Error: Unable to connect to IOTA_FAUCET_ENDPOINT: ${FAUCET_URL}"
        echo "Please check the faucet status or set a different endpoint using: export IOTA_FAUCET_ENDPOINT=<your-faucet-endpoint>"
        exit 1
    fi
fi
