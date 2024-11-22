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
  if ! command -V ${i} &> /dev/null; then
    echo "${i} is not installed"
    exit 1
  fi
done

# List all available public keys using keytool list
echo "Listing all available public keys:"
iota keytool list
echo

# Ask how many public keys the user wants to provide, with a minimum of 2 required
read -p "How many public keys do you want to use for multisig? (Minimum 2): " num_keys

if [ "$num_keys" -lt 2 ]; then
  echo "At least 2 public keys are required for multisig."
  exit 1
fi

# Prompt user for public keys, weights, and IOTA addresses
public_keys=()
weights=()
iota_addresses=()

for ((i=1; i<=num_keys; i++)); do
  read -p "Enter public key $i: " pk
  public_keys+=("$pk")
  
  read -p "Enter weight for public key $i: " weight
  weights+=("$weight")
  
  read -p "Enter IOTA address for public key $i: " iota_address
  iota_addresses+=("$iota_address")
done

# Prompt for threshold
read -p "Enter threshold for multisig: " threshold

# Create multisig address
echo "Creating multisig address..."
multisig_res=$(iota keytool multi-sig-address --pks "${public_keys[@]}" --weights "${weights[@]}" --threshold "$threshold")

# Display the multisig address to the user and ask them to copy it
echo "Multisig address created. Please copy the multisig address displayed below and paste it as requested."
echo "$multisig_res"

# Ask the user to manually input the multisig address
read -p "Please paste the multisig address here: " multisig_address

# Set the admin address to the multisig address
MULTISIG_ADMIN_ADDRESS=$multisig_address
echo "- Admin (Multisig) Address is: ${MULTISIG_ADMIN_ADDRESS}"

# Optional: Request tokens from the faucet (for testing purposes)
echo "Requesting tokens from faucet..."
faucet_res=""
attempt=0
max_attempts=5

# Loop to check the faucet response
while [[ "$faucet_res" == "" && $attempt -lt $max_attempts ]]; do
  attempt=$((attempt + 1))
  faucet_res=$(curl --location --request POST "${IOTA_FAUCET}" --header 'Content-Type: application/json' --data-raw "{ \"FixedAmountRequest\": { \"recipient\": \"$MULTISIG_ADMIN_ADDRESS\" } }")
  
  # Check for successful response
  if echo "$faucet_res" | grep -q "error"; then
    echo "Faucet request failed: $faucet_res"
    echo "Retrying... attempt $attempt of $max_attempts"
    faucet_res=""
    sleep 1 # wait before retrying
  fi
done

# Check if faucet request was successful
if [[ "$faucet_res" == "" ]]; then
  echo "Failed to request tokens from faucet after $max_attempts attempts."
else
  echo "Faucet request succeeded."
fi

# Run the transfer command
echo "Running transfer command..."
tx_bytes=$(iota client publish --skip-fetch-latest-git-deps --gas-budget 2000000000 ${MOVE_PACKAGE_PATH} --skip-dependency-verification --serialize-unsigned-transaction)

echo "Raw tx_bytes to execute: $tx_bytes"

# Ask the user to manually input the signatures for each IOTA address
signatures=()
for ((i=0; i<num_keys; i++)); do
  read -p "Enter the signature for address ${iota_addresses[$i]}: " sig
  signatures+=("$sig")
done

# Combine individual signatures into a multisig signature
multisig_serialized=$(iota keytool multi-sig-combine-partial-sig --pks "${public_keys[@]}" --weights "${weights[@]}" --threshold "$threshold" --sigs "${signatures[@]}")

# Ask the user to manually input the obtained multisig serialized
read -p "Please enter all the obtained multisig serialized signatures: " multisig_serialized

# Execute a transaction with multisig
execute_res=$(iota client execute-signed-tx --tx-bytes "$tx_bytes" --signatures "$multisig_serialized")

# Save the publish result
echo "$execute_res" > .publish.res.json

# Check if the publish command succeeded
if [[ "$execute_res" =~ "error" ]]; then
  echo "Error during move contract publishing. Details: $execute_res"
  exit 1
fi

# Extract the Package ID from the publish response
publishedObjs=$(echo "$execute_res" | jq -r '.objectChanges[] | select(.type == "published")')
PACKAGE_ID=$(echo "$publishedObjs" | jq -r '.packageId')

# Update the .env file with the new Package ID
cat > .env <<-API_ENV
IOTA_NETWORK=$IOTA_NETWORK
IOTA_FAUCET=$IOTA_FAUCET
MOVE_PACKAGE_PATH=$MOVE_PACKAGE_PATH
PACKAGE_ADDRESS=$PACKAGE_ID
MULTISIG_ADMIN_ADDRESS=$MULTISIG_ADMIN_ADDRESS
API_ENV

echo "Contract Deployment finished!"
echo "Package ID: $PACKAGE_ID"
