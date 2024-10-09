#!/bin/bash

# Load environment variables from .env file
if [ -f .env ]; then
  source .env
else
  echo ".env file not found!"
  exit 1
fi

# Check if dependencies are available
for i in jq curl iota; do
  if ! command -V ${i} 2>/dev/null; then
    echo "${i} is not installed"
    exit 1
  fi
done

echo "- Admin Address is: ${ADMIN_ADDRESS}"

# Import address and switch to admin account using variables from .env
import_address=$(iota keytool import "$ADMIN_PHRASE" ed25519)

switch_res=$(iota client switch --address ${ADMIN_ADDRESS})

# Optional: Request tokens from the faucet
# faucet_res=$(curl --location --request POST "$IOTA_FAUCET" --header 'Content-Type: application/json' --data-raw '{"FixedAmountRequest": { "recipient": '$ADMIN_ADDRESS'}}')

# Publish the Move package using values from .env
publish_res=$(iota client publish --skip-fetch-latest-git-deps --gas-budget 2000000000 --json ${MOVE_PACKAGE_PATH} --skip-dependency-verification)

echo ${publish_res} >.publish.res.json

# Check if the publish command succeeded
if [[ "$publish_res" =~ "error" ]]; then
  echo "Error during move contract publishing.  Details: $publish_res"
  exit 1
fi

# Extract the Package ID from the publish response
publishedObjs=$(echo "$publish_res" | jq -r '.objectChanges[] | select(.type == "published")')
PACKAGE_ID=$(echo "$publishedObjs" | jq -r '.packageId')

# Update the .env file with the new Package ID
cat >.env<<-API_ENV
IOTA_NETWORK=$IOTA_NETWORK
IOTA_FAUCET=$IOTA_FAUCET
MOVE_PACKAGE_PATH=$MOVE_PACKAGE_PATH
PACKAGE_ADDRESS=$PACKAGE_ID
ADMIN_ADDRESS=$ADMIN_ADDRESS
API_ENV

echo "Contract Deployment finished!"
echo "Package ID: $PACKAGE_ID"
