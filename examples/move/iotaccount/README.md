# IOTAccount Move Example

This example shows how to create and use an IOTAccount Move smart contract that uses an Ed25519 public key for authentication.

```bash
# run in one terminal:
RUST_LOG="info,consensus=warn,iota_core=warn,fastcrypto_tbls=off,starfish_core=warn,iota_indexer=warn,iota_data_ingestion_core=error,iota_graphql_rpc=warn" iota start --force-regenesis --committee-size 1 --with-faucet --with-indexer --with-graphql
```

```bash
# in another terminal:
# to re-run the commands below, first switch to a non account address like this:
# iota client switch --address 0x111111111504e9350e635d65cd38ccd2c029434c6a3a480d8947a9ba6a15b215
# Commands assume the active address is from an Ed25519 key
SIGN_ADDRESS=$(iota client active-address)
echo "Sign address: $SIGN_ADDRESS"
export KEY_JSON=$(iota keytool export $SIGN_ADDRESS --json)
export SIGN_PUB_KEY_B64=$(echo $KEY_JSON | jq -r '.key.publicBase64Key')
export SIGN_PUB_KEY_HEX=$(echo $SIGN_PUB_KEY_B64 | base64 -d | od -An -tx1 | tr -d ' \n')
echo "Sign public key hex: $SIGN_PUB_KEY_HEX"

iota client switch --env localnet
iota client faucet
# publish, extract JSON, set env vars, and print info
export JSON=$(iota client publish examples/move/iotaccount --json | awk '/{/ { if (!in_json) { in_json=1; brace_count=1 } else { brace_count++ } } /}/ { brace_count-- } in_json { print } brace_count == 0 && in_json { exit }')
echo $JSON
export DIGEST=$(echo $JSON | jq -r .digest)
export PACKAGE_ID=$(echo $JSON | jq -r '.objectChanges[] | select(.type == "published") | .packageId')
export METADATA_ID=$(echo $JSON | jq -r '.objectChanges[] | select(.type == "created" and .objectType == "0x2::package_metadata::PackageMetadataV1") | .objectId')
echo "Transaction Digest: $DIGEST"
echo "Package ID: $PACKAGE_ID"
echo "Package Metadata Object ID: $METADATA_ID"

export PTB_JSON=$(iota client ptb \
--move-call 0x2::authenticator_function::create_auth_function_ref_v1 '<'$PACKAGE_ID'::iotaccount::IOTAccount>' @$METADATA_ID '"keyed_iotaccount"' '"authenticate_ed25519"' \
--assign ref \
--move-call $PACKAGE_ID::keyed_iotaccount::create vector"$SIGN_PUB_KEY_BYTES" ref \
--json)
export IOTACCOUNT=$(echo $PTB_JSON | jq -r '.objectChanges[] | select(.type == "created" and (.objectType | endswith("::iotaccount::IOTAccount"))) | .objectId')
echo "IOT Account Object ID: $IOTACCOUNT"

DYNAMIC_FIELD_JSON=$(iota client dynamic-field $IOTACCOUNT --json)
export OWNER_PUBLIC_KEY_ID=$(echo $DYNAMIC_FIELD_JSON | jq -r '.data[] | select(.name.type | endswith("::keyed_iotaccount::OwnerPublicKey")) | .objectId')
echo "Owner Public Key ID: $OWNER_PUBLIC_KEY_ID"
OBJECT_JSON=$(iota client object $OWNER_PUBLIC_KEY_ID --json)
HEX=$(echo $OBJECT_JSON | jq -r '.content.fields.value[]' | xargs printf "%02x")
echo "Dynamic field public key: $HEX"

iota client add-account $IOTACCOUNT
iota client switch --address $IOTACCOUNT
iota client faucet

UNSIGNED_TX_BYTES=$(iota client pay-iota --recipients 0x111111111504e9350e635d65cd38ccd2c029434c6a3a480d8947a9ba6a15b215 --amounts 1 --serialize-unsigned-transaction)
echo "Unsigned TX: $UNSIGNED_TX_BYTES"
# iota keytool decode-or-verify-tx --tx-bytes $UNSIGNED_TX_BYTES

# smart contract requires the tx digest, not the tx signing digest
TX_DIGEST_HEX=$(iota keytool tx-digest $UNSIGNED_TX_BYTES --json | jq -r '.digestHex')
echo "TX Digest Hex: $TX_DIGEST_HEX"

IOTA_SIGNATURE_BASE64=$(iota keytool sign-raw --address $SIGN_ADDRESS --data $TX_DIGEST_HEX --json | jq -r '.iotaSignature')
echo "Signature: $IOTA_SIGNATURE_BASE64"
export IOTA_SIGNATURE_HEX=$(echo $IOTA_SIGNATURE_BASE64 | base64 -d | od -An -tx1 | tr -d ' \n')
echo "IOTA signature hex: $IOTA_SIGNATURE_HEX"
export SIGNATURE_HEX=$(echo $IOTA_SIGNATURE_HEX | cut -c 3-130)
echo "Signature hex: $SIGNATURE_HEX"

# Add "0x" before $SIGNATURE_HEX if no hex decoding is used in the smart contract
export SIGNED_TX_BYTES=$(iota client pay-iota --recipients 0x111111111504e9350e635d65cd38ccd2c029434c6a3a480d8947a9ba6a15b215 --amounts 1 --auth-call-args $SIGNATURE_HEX --serialize-signed-transaction)
echo "Signed tx bytes: $SIGNED_TX_BYTES"
iota client execute-combined-signed-tx --signed-tx-bytes $SIGNED_TX_BYTES
echo "Tx digest: $TX_DIGEST_B58"

# optionally decode the signature:
# iota keytool decode-sig --json $SIGNED_TX_BYTES
```
