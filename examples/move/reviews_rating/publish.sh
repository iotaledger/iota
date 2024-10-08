#!/bin/bash

# Load environment variables from .env file
if [ -f .env ]; then
  source .env
else
  echo ".env file not found!"
  exit 1
fi

# Check if dependencies are available.
for i in jq curl iota; do
  if ! command -V ${i} 2>/dev/null; then
    echo "${i} is not installed"
    exit 1
  fi
done

# Use Alphanet settings
NETWORK="https://api.iota-rebased-alphanet.iota.cafe:443"
FAUCET="https://faucet.iota-rebased-alphanet.iota.cafe/gas"
MOVE_PACKAGE_PATH="."  # Since the package is in the current directory

echo "- Admin Address is: ${ADMIN_ADDRESS}"

# Import address and switch to admin account
import_address=$(iota keytool import "$ADMIN_PHRASE" ed25519)

switch_res=$(iota client switch --address ${ADMIN_ADDRESS})

# Commented out Faucet request since it's optional
# faucet_res=$(curl --location --request POST "$FAUCET" --header 'Content-Type: application/json' --data-raw '{"FixedAmountRequest": { "recipient": '$ADMIN_ADDRESS'}}')

# Publish the Move package
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

# Generate environment files
cat >.env<<-API_ENV
IOTA_NETWORK=$NETWORK
PACKAGE_ADDRESS=$PACKAGE_ID
ADMIN_ADDRESS=$ADMIN_ADDRESS
API_ENV

echo "Contract Deployment finished!"
