# IOTAccount Move Example

This example shows how to create and use an IOTAccount Move smart contract that uses an Ed25519 public key for authentication.

```bash
# run in one terminal:
RUST_LOG="info,consensus=warn,iota_core=warn,fastcrypto_tbls=off,starfish_core=warn,iota_indexer=warn,iota_data_ingestion_core=error,iota_graphql_rpc=warn" iota start --force-regenesis --committee-size 1 --with-faucet --with-indexer --with-graphql
```

```bash
# in another terminal:
# to re-run the commands below, first switch to a non account address
# iota client switch --address 0x111111111504e9350e635d65cd38ccd2c029434c6a3a480d8947a9ba6a15b215
# Assumes active address is an Ed25519 key
SIGN_ADDRESS=$(iota client active-address)
echo "Sign address: $SIGN_ADDRESS"
export KEY_JSON=$(iota keytool export $SIGN_ADDRESS --json)
export SIGN_PUB_KEY_B64=$(echo $KEY_JSON | jq -r '.key.publicBase64Key')
export SIGN_PUB_KEY_HEX=$(echo $SIGN_PUB_KEY_B64 | base64 -d | od -An -tx1 | tr -d ' \n')
echo "Sign public key hex: $SIGN_PUB_KEY_HEX"
export SIGN_PUB_KEY_BYTES=$(python3 -c "import sys; print([int('$SIGN_PUB_KEY_HEX'[i:i+2],16) for i in range(0,len('$SIGN_PUB_KEY_HEX'),2)])")
echo "Sign public key bytes: $SIGN_PUB_KEY_BYTES"

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
# echo $DYNAMIC_FIELD_JSON
export OWNER_PUBLIC_KEY_ID=$(echo $DYNAMIC_FIELD_JSON | jq -r '.data[] | select(.name.type | endswith("::keyed_iotaccount::OwnerPublicKey")) | .objectId')
echo "Owner Public Key ID: $OWNER_PUBLIC_KEY_ID"
OBJECT_JSON=$(iota client object $OWNER_PUBLIC_KEY_ID --json)
# echo $OBJECT_JSON
HEX=$(echo $OBJECT_JSON | jq -r '.content.fields.value[]' | xargs printf "%02x")
echo "Dynamic field public key: $HEX"

iota client add-account $IOTACCOUNT
iota client switch --address $IOTACCOUNT
iota client faucet

UNSIGNED_TX=$(iota client pay-iota --recipients 0x111111111504e9350e635d65cd38ccd2c029434c6a3a480d8947a9ba6a15b215 --amounts 1 --serialize-unsigned-transaction)
echo "Unsigned TX: $UNSIGNED_TX"
# iota keytool decode-or-verify-tx --tx-bytes $UNSIGNED_TX

# Get the transaction digest using dry-run
TX_DIGEST_INFO=$(iota client pay-iota --recipients 0x111111111504e9350e635d65cd38ccd2c029434c6a3a480d8947a9ba6a15b215 --amounts 1 --dry-run --json)
TX_DIGEST_B58=$(echo $TX_DIGEST_INFO | jq -r '.effects.transactionDigest')
TX_DIGEST_HEX=$(python3 -c "
import sys
s = sys.argv[1]
alphabet = '123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz'
base = 58
leading_zeros = len(s) - len(s.lstrip('1'))
s = s[leading_zeros:]
num = 0
for char in s:
    num = num * base + alphabet.index(char)
bytes_data = num.to_bytes((num.bit_length() + 7) // 8, 'big')
result = (b'\x00' * leading_zeros + bytes_data).hex()
print(result)
" "$TX_DIGEST_B58")
echo "TX Digest Hex: $TX_DIGEST_HEX"

IOTA_SIGNATURE_BASE64=$(iota keytool sign-pure --address $SIGN_ADDRESS --data $TX_DIGEST_HEX --json | jq -r '.iotaSignature')
echo "Signature: $IOTA_SIGNATURE_BASE64"
export IOTA_SIGNATURE_HEX=$(echo $IOTA_SIGNATURE_BASE64 | base64 -d | od -An -tx1 | tr -d ' \n')
echo "IOTA signature hex: $IOTA_SIGNATURE_HEX"
export SIGNATURE_HEX=$(echo $IOTA_SIGNATURE_HEX | cut -c 3-130)
echo "Signature hex: $SIGNATURE_HEX"
# export SIGNATURE_BYTES=$(python3 -c "import sys; print([int('$SIGNATURE_HEX'[i:i+2],16) for i in range(0,len('$SIGNATURE_HEX'),2)])")
# echo "Signature bytes: $SIGNATURE_BYTES"

# 0x before $SIGNATURE_HEX if no hex decoding is needed in the smart contract
export SIGNED_TX_BYTES=$(iota client pay-iota --recipients 0x111111111504e9350e635d65cd38ccd2c029434c6a3a480d8947a9ba6a15b215 --amounts 1 --auth-call-args $SIGNATURE_HEX --serialize-signed-transaction)
echo "Signed tx bytes: $SIGNED_TX_BYTES"
iota client execute-combined-signed-tx --signed-tx-bytes $SIGNED_TX_BYTES
echo "Tx digest: $TX_DIGEST_B58"
export URL_ENCODED_TX=$(python3 -c "import urllib.parse; print(urllib.parse.quote('$SIGNED_TX_BYTES'))")
echo "Decoded tx: https://iotatools.dev/#/sign?network=localnet&view=formatted&tx=$URL_ENCODED_TX"
# decode signature to see details
# iota keytool decode-sig --json BwEAggGAATkwZTI2MGJlMWI0MWQzMDkxODgyNzA3ZGY5ZWNlNzBlNDc5ZmRiZThjNTYxNDI0YzZlYTViNmQwY2NmOThmNjBjZThkMmQzNDM3ZTJmMjUwZjYwZTc3ZDE0MDk2MThkYjI1NGNhMjRjYzZmYzZiMTJlNTY0OTA2ZDQ5OTIwMDBkAAEBUbV64+wJ2UXtrrE+kfVlodytKf/D7974d3GJofYp5BMEAAAAAAAAAAA=
```

RUST_LOG="off" cargo test --package iota-e2e-tests --test abstract_account_tests -- test_abstract_account_creation_and_issue_tx --exact --nocapture

```bash
# export SIGNATURE_HEX=5edfa4d0dd94ee2fda5902cfbc833cd082fa5d0e09f518f843662f2e6a638e59dca0352e276fe3f08ad350b83a1c5a1f509bfc827cf1db01dae203718d0a120a
# export SIGN_PUB_KEY_HEX=287bc969b5d88c530de1deb7314097f76d6a7dcc52cfe04ab7ae940e6a6e7673
# export TX_DIGEST_HEX=06ca8a906ee0032261889bc74caa90293110dfd9f65b501dbd5580882b8ae2b99f
echo $SIGNATURE_HEX
echo $SIGN_PUB_KEY_HEX
echo $TX_DIGEST_HEX

hex_to_array() { local hex="$1"; echo "$hex" | sed 's/../0x&,/g; s/,$//'; }
SIGNATURE_ARR=$(hex_to_array "$SIGNATURE_HEX")
SIGN_PUB_KEY_ARR=$(hex_to_array "$SIGN_PUB_KEY_HEX")
TX_DIGEST_ARR=$(hex_to_array "$TX_DIGEST_HEX")

# iota client ptb \
#   --make-move-vec "<u8>" "[$SIGNATURE_ARR]" --assign sig \
#   --make-move-vec "<u8>" "[$SIGN_PUB_KEY_ARR]" --assign pk \
#   --make-move-vec "<u8>" "[$TX_DIGEST_ARR]" --assign msg \
#   --move-call 0x2::ed25519::ed25519_verify sig pk msg \
#   --dry-run

iota client ptb \
  --make-move-vec "<u8>" "[$SIGNATURE_ARR]" --assign sig \
  --make-move-vec "<u8>" "[$SIGN_PUB_KEY_ARR]" --assign pk \
  --make-move-vec "<u8>" "[$TX_DIGEST_ARR]" --assign msg \
  --move-call 0x2::ed25519::ed25519_verify sig pk msg \
  --dev-inspect
```
