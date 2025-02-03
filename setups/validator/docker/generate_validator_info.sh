#!/bin/bash

IOTA_DOCKER_IMAGE=${IOTA_TOOLS_DOCKER_IMAGE:-"iotaledger/iota-tools:testnet"}
API_URL=${IOTA_API_ENDPOINT:-"https://api.testnet.iota.cafe"}

echo "Will using ${API_URL} for iota_config API endpopint"

./check_iota_services.sh --api ${API_URL} --docker ${IOTA_DOCKER_IMAGE}
if [ $? -ne 0 ]; then
    exit 1
fi

if ! command -v jq &> /dev/null; then
    echo "jq is not installed. Installing jq..."
    apt update && apt install -y jq
fi

if [ -d "./key-pairs" ]; then
    read -p "Key Pairs Directory already exists. This will overwrite everything? [y/N] " response
    if [[ ! $response =~ ^[Yy]$ ]]; then
        echo "Operation cancelled"
        exit 1
    fi
    
    TIMESTAMP=$(date +"%Y%m%d_%H%M%S")
    BACKUP_DIR="key-pairs_backup_${TIMESTAMP}"
    cp -r ./key-pairs "${BACKUP_DIR}"
    rm -r ./key-pairs
fi

mkdir -p ./key-pairs
mkdir -p ./iota_config
TIMESTAMP=$(date +"%Y%m%d_%H%M%S")
mkdir -p "./tmp/key-pairs-for-making-info_${TIMESTAMP}"

KEYGEN_OUTPUT=$(docker run --rm ${IOTA_DOCKER_IMAGE} /bin/sh -c '/usr/local/bin/iota keytool generate ed25519 --json && cat *.key')

JSON_PART=$(echo "$KEYGEN_OUTPUT" | head -n-1)

IOTA_ADDRESS=$(echo "$JSON_PART" | jq -r '.iotaAddress')
PUBLIC_KEY=$(echo "$JSON_PART" | jq -r '.publicBase64Key')
ACCOUNT_BECH32_PRIVATE_KEY=$(echo "$KEYGEN_OUTPUT" | tail -n1)

cat > ./iota_config/client.yaml << EOF
---
keystore:
  File: /root/.iota/iota_config/iota.keystore
envs:
  - alias: custom
    rpc: "${API_URL}"
    ws: ~
    basic_auth: ~
active_env: custom
active_address: "${IOTA_ADDRESS}"
EOF

cat > ./iota_config/iota.aliases << EOF
[
  {
    "alias": "flamboyant-hematite", 
    "public_key_base64": "${PUBLIC_KEY}"
  }
]
EOF

cat > ./iota_config/iota.keystore << EOF
[ 
    "${ACCOUNT_BECH32_PRIVATE_KEY}"
]
EOF

read -p "Enter validator name: " NAME
read -p "Enter validator description: " DESCRIPTION
read -p "Enter image URL (press enter for default): " IMAGE_URL
read -p "Enter project URL (press enter for default): " PROJECT_URL
read -p "Enter hostname: " HOST_NAME

IMAGE_URL=${IMAGE_URL:-""}
PROJECT_URL=${PROJECT_URL:-""}

docker run --rm -v ./iota_config:/root/.iota/iota_config -v "./tmp/key-pairs-for-making-info_${TIMESTAMP}":/iota ${IOTA_DOCKER_IMAGE} /bin/sh -c "RUST_BACKTRACE=full /usr/local/bin/iota validator make-validator-info \"$NAME\" \"$DESCRIPTION\" \"$IMAGE_URL\" \"$PROJECT_URL\" \"$HOST_NAME\" 1000"

cp "./tmp/key-pairs-for-making-info_${TIMESTAMP}/account.key" ./key-pairs/account.key
cp "./tmp/key-pairs-for-making-info_${TIMESTAMP}/network.key" ./key-pairs/network.key
cp "./tmp/key-pairs-for-making-info_${TIMESTAMP}/protocol.key" ./key-pairs/protocol.key
cp "./tmp/key-pairs-for-making-info_${TIMESTAMP}/authority.key" ./key-pairs/authority.key
cp "./tmp/key-pairs-for-making-info_${TIMESTAMP}/validator.info" .

echo -e "\nPlease share the following information in Slack:"
echo -e "\nValidator Address: ${IOTA_ADDRESS}"
echo "Script Version: $(git rev-parse --short HEAD)"
echo -e "\nMake sure to securely store your key files!\n"