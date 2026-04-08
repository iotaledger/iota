# OneSig Move Example

The OneSigAccount module defines an account abstraction that allows executing multiple transactions with a single signature using a Merkle tree structure for transaction authorization. It includes functionality for account creation, authentication, and Merkle proof verification.

The account is created with a public key and an authenticator function. To authenticate the account, the authenticator verifies the provided signature against the Merkle root, which represents the set of authorized transactions. It also verifies that the transaction digest is part of the authorized set using the Merkle proof.

The implementation of this module is based on the OneSig protocol (https://github.com/LayerZero-Labs/OneSig) and is designed for demonstration purposes only. It can be extended to support more complex authentication schemes, such as multiple signatures or different types of authenticators.

## How to run (WIP, currently the --auth-call-args is broken for this example)

In a dedicated terminal run a local IOTA network:

```bash
RUST_LOG="info,consensus=warn,iota_core=warn,fastcrypto_tbls=off,starfish_core=warn,iota_indexer=warn,iota_data_ingestion_core=error,iota_graphql_rpc=warn" iota-localnet start --force-regenesis --committee-size 1 --with-faucet --with-indexer --with-graphql
```

In another terminal run the rest of the commands:

```bash
# To re-run the commands below, first switch to a non account address like this:
# iota client switch --address 0x111111111504e9350e635d65cd38ccd2c029434c6a3a480d8947a9ba6a15b215
# Commands assume the active address is from an Ed25519 key

# Useful names for this example
export EXAMPLE_DIR="onesig"
export ACCOUNT_MODULE_NAME="account"
export ACCOUNT_TYPE_NAME="OneSigAccount"
export AUTH_MODULE_NAME="account"
export AUTH_FUNCTION_NAME="onesig_authenticator"
export CREATE_MODULE_NAME="account"
export CREATE_FUNCTION_NAME="create"

# Get the signing address 
export SIGN_ADDRESS=$(iota client active-address)
echo "Signing address: $SIGN_ADDRESS"
# Get the public key of the signing address 
export SIGN_PUB_KEY_HEX=$(iota keytool export $SIGN_ADDRESS --json | jq -r '.key.publicBase64Key' | base64 -d | od -An -tx1 | tr -d ' \n')
export SIGN_PUB_KEY_BYTES=$(python3 -c "import sys; print([int('$SIGN_PUB_KEY_HEX'[i:i+2],16) for i in range(0,len('$SIGN_PUB_KEY_HEX'),2)])")
echo "Signing public key hex: $SIGN_PUB_KEY_HEX"
echo "Signing public key bytes: $SIGN_PUB_KEY_BYTES"

# Switch to localnet and get some funds to publish the necessary packages
iota client switch --env localnet
iota client faucet
# Publish the account package
export JSON=$(iota client publish examples/move/abstract_iota_accounts/$EXAMPLE_DIR --with-unpublished-dependencies --json | awk '/{/ { if (!in_json) { in_json=1; brace_count=1 } else { brace_count++ } } /}/ { brace_count-- } in_json { print } brace_count == 0 && in_json { exit }')
echo $JSON
# Derive the ids needed to build authenticator function refs
export DIGEST=$(echo $JSON | jq -r .digest)
export PACKAGE_ID=$(echo $JSON | jq -r '.objectChanges[] | select(.type == "published") | .packageId')
export METADATA_ID=$(echo $JSON | jq -r '.objectChanges[] | select(.type == "created" and .objectType == "0x2::package_metadata::PackageMetadataV1") | .objectId')
echo "Transaction Digest: $DIGEST"
echo "Package ID: $PACKAGE_ID"
echo "Package Metadata Object ID: $METADATA_ID"

# Create a new account through a PTB which firstly builds an authenticator function ref for the ed25519 authenticator
export PTB_JSON=$(iota client ptb \
--move-call 0x2::authenticator_function::create_auth_function_ref_v1 '<'$PACKAGE_ID'::'$ACCOUNT_MODULE_NAME'::'$ACCOUNT_TYPE_NAME'>' @$METADATA_ID '"'$AUTH_MODULE_NAME'"' '"'$AUTH_FUNCTION_NAME'"' \
--assign ref \
--move-call $PACKAGE_ID::$CREATE_MODULE_NAME::$CREATE_FUNCTION_NAME vector"$SIGN_PUB_KEY_BYTES" ref \
--json)
export ABSTRACTACCOUNT=$(echo $PTB_JSON | jq -r '.objectChanges[] | select(.type == "created" and (.objectType | endswith("::'$ACCOUNT_MODULE_NAME'::'$ACCOUNT_TYPE_NAME'"))) | .objectId')
echo "Account Object ID: $ABSTRACTACCOUNT"

# Check the public key stored in the account
DYNAMIC_FIELD_JSON=$(iota client dynamic-field $ABSTRACTACCOUNT --json)
export PUBLIC_KEY_FIELD_ID=$(echo $DYNAMIC_FIELD_JSON | jq -r '.data[] | select(.name.type | endswith("::public_key_authentication::PublicKeyFieldName")) | .objectId')
echo "Public Key Field ID: $PUBLIC_KEY_FIELD_ID"
OBJECT_JSON=$(iota client object $PUBLIC_KEY_FIELD_ID --json)
HEX=$(echo $OBJECT_JSON | jq -r '.content.fields.value[]' | xargs printf "%02x")
echo "Dynamic field public key: $HEX"

# Add the newly created account to the CLI keystore and set is as active
iota client add-account $ABSTRACTACCOUNT
# Request funds for the account
iota client faucet --address $ABSTRACTACCOUNT
iota client faucet --address $ABSTRACTACCOUNT
iota client faucet --address $ABSTRACTACCOUNT
export GAS_JSON=$(iota client gas $ABSTRACTACCOUNT --json)
export COIN_1=$(echo "$GAS_JSON" | jq -r '.[0].gasCoinId')
export COIN_2=$(echo "$GAS_JSON" | jq -r '.[1].gasCoinId')
export COIN_3=$(echo "$GAS_JSON" | jq -r '.[2].gasCoinId')

# Create a transaction where the sender is the account, but don't issue it; just take the bytes
UNSIGNED_TX_BYTES_1=$(iota client ptb \
--move-call 0x2::clock::timestamp_ms @0x6 \
--sender @$ABSTRACTACCOUNT \
--gas-coins @$COIN_1 \
--serialize-unsigned-transaction)
echo "Unsigned TX1: $UNSIGNED_TX_BYTES_1"
TX_DIGEST_HEX_1=$(iota keytool tx-digest $UNSIGNED_TX_BYTES_1 --json | jq -r '.digestHex[2:]')
echo "TX Digest Hex: $TX_DIGEST_HEX_1"
UNSIGNED_TX_BYTES_2=$(iota client ptb \
--move-call 0x2::clock::timestamp_ms @0x6 \
--sender @$ABSTRACTACCOUNT \
--gas-coins @$COIN_2 \
--serialize-unsigned-transaction)
echo "Unsigned TX1: $UNSIGNED_TX_BYTES_2"
TX_DIGEST_HEX_2=$(iota keytool tx-digest $UNSIGNED_TX_BYTES_2 --json | jq -r '.digestHex[2:]')
echo "TX Digest Hex: $TX_DIGEST_HEX_2"
UNSIGNED_TX_BYTES_3=$(iota client ptb \
--move-call 0x2::clock::timestamp_ms @0x6 \
--sender @$ABSTRACTACCOUNT \
--gas-coins @$COIN_3 \
--serialize-unsigned-transaction)
echo "Unsigned TX1: $UNSIGNED_TX_BYTES_3"
TX_DIGEST_HEX_3=$(iota keytool tx-digest $UNSIGNED_TX_BYTES_3 --json | jq -r '.digestHex[2:]')
echo "TX Digest Hex: $TX_DIGEST_HEX_3"

# obtain the necessary inputs
export TX_DIGEST_BYTES_1=$(python3 -c "import sys; print([int('$TX_DIGEST_HEX_1'[i:i+2],16) for i in range(0,len('$TX_DIGEST_HEX_1'),2)])")
export TX_DIGEST_BYTES_2=$(python3 -c "import sys; print([int('$TX_DIGEST_HEX_2'[i:i+2],16) for i in range(0,len('$TX_DIGEST_HEX_2'),2)])")
export TX_DIGEST_BYTES_3=$(python3 -c "import sys; print([int('$TX_DIGEST_HEX_3'[i:i+2],16) for i in range(0,len('$TX_DIGEST_HEX_3'),2)])")
export VIEW_RESULT=$(curl -s http://127.0.0.1:9000 -X POST -H 'Content-Type: application/json' \
-d "{
  \"jsonrpc\": \"2.0\",
  \"id\": 1,
  \"method\": \"iota_view\",
  \"params\": [
    \"${PACKAGE_ID}::merkle::build_merkle_tree_with_proofs\",
    [],
    [[$TX_DIGEST_BYTES_1,$TX_DIGEST_BYTES_2,$TX_DIGEST_BYTES_3]]
  ]
}" | jq .)

# Merkle root → hex
export MERKLE_ROOT=$(echo "$VIEW_RESULT" | jq -r '.result.functionReturnValues[0][]' | xargs printf "%02x")

# Proof hashes as hex (one hash per line within each proof)
export PROOF_1_0=$(echo "$VIEW_RESULT" | jq -r '.result.functionReturnValues[1][0][0][]' | xargs printf "%02x")
export PROOF_1_1=$(echo "$VIEW_RESULT" | jq -r '.result.functionReturnValues[1][0][1][]' | xargs printf "%02x")
export PROOF_2_0=$(echo "$VIEW_RESULT" | jq -r '.result.functionReturnValues[1][1][0][]' | xargs printf "%02x")
export PROOF_2_1=$(echo "$VIEW_RESULT" | jq -r '.result.functionReturnValues[1][1][1][]' | xargs printf "%02x")
export PROOF_3_0=$(echo "$VIEW_RESULT" | jq -r '.result.functionReturnValues[1][2][0][]' | xargs printf "%02x")

# Obtain the signature where the message is the TX1 digest and the signing key is part of the keypair from which the signing address was derived
export IOTA_SIGNATURE_HEX=$(iota keytool sign-raw --address $SIGN_ADDRESS --data $TX_DIGEST_HEX_1 --json | jq -r '.iotaSignature' | base64 -d | od -An -tx1 | tr -d ' \n')
echo "IOTA signature hex: $IOTA_SIGNATURE_HEX"
# The IOTA signature contains a flag and the public key, so here it strips those information (not necessary for the authenticator)
export SIGNATURE_HEX=$(echo $IOTA_SIGNATURE_HEX | cut -c 3-130)
echo "Signature hex: $SIGNATURE_HEX"

# Finally, execute the TX using the signature just created as auth-call-arg
# TODO fix --auth-call-args in order to support vector<vector<u8>>
export SIGNED_TX_BYTES=$(iota client ptb \
--move-call 0x2::clock::timestamp_ms @0x6 \
--sender @$ABSTRACTACCOUNT \
--gas-coins @$COIN_1 \
--auth-call-args 0x$MERKLE_ROOT '["'0x$PROOF_1_0'","'0x$PROOF_1_1'"]' 0x$SIGNATURE_HEX \
--serialize-signed-transaction)
echo "Signed tx bytes: $SIGNED_TX_BYTES"
iota client execute-combined-signed-tx --signed-tx-bytes $SIGNED_TX_BYTES

# optionally decode the signature:
iota keytool decode-sig --json $SIGNED_TX_BYTES
```
