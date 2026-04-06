# DynamicMultisigAccount Move Example

The DynamicMultisigAccount module defines a generic account struct that can be used to handle a dynamic multisig account. The account data, such as the members information, the threshold and the proposed transactions, are stored as dynamic fields of the account object. The module provides functions to create a new DynamicMultisigAccount, update the account data and propose and approve transactions.

The module also defines an authenticator that checks that the sender of the transaction is the account and that the total weight of the members who approved the transaction is greater than or equal to the threshold.

## How to run

In a dedicated terminal run a local IOTA network:

```bash
RUST_LOG="info,consensus=warn,iota_core=warn,fastcrypto_tbls=off,starfish_core=warn,iota_indexer=warn,iota_data_ingestion_core=error,iota_graphql_rpc=warn" iota-localnet start --force-regenesis --committee-size 1 --with-faucet --with-indexer --with-graphql
```

In another terminal run the rest of the commands:

```bash
# To re-run the commands below, first switch to a non account address like this:
# iota client switch --address 0x111111111504e9350e635d65cd38ccd2c029434c6a3a480d8947a9ba6a15b215
# Commands assume the active address is from an Ed25519 key
# Commands also assume that 2 ed25519 public key addresses are present

# Useful names for this example
export EXAMPLE_DIR="dynamic_multisig_account"
export ACCOUNT_MODULE_NAME="dynamic_multisig_account"
export ACCOUNT_TYPE_NAME="DynamicMultisigAccount"
export AUTH_MODULE_NAME="dynamic_multisig_account"
export AUTH_FUNCTION_NAME="approval_authenticator"
export CREATE_MODULE_NAME="dynamic_multisig_account"
export CREATE_FUNCTION_NAME="create"

# Get the signing addresses
export JSON_KEYS=$(iota keytool list --json)
_key_count=$(echo "$JSON_KEYS" | jq '[.[] | select(.publicBase64Key)] | length')
if [ "$_key_count" -lt 2 ]; then
  echo "Error: expected at least 2 keypairs with publicBase64Key, got $_key_count" >&2
  exit 1
fi
export ALICE_SIGN_PUB_KEY=$(echo "$JSON_KEYS" | jq -r '[.[] | select(.publicBase64Key)][0].publicBase64Key')
export ALICE_SIGN_ADDRESS=$(echo "$JSON_KEYS" | jq -r '[.[] | select(.publicBase64Key)][0].iotaAddress')
export BOB_SIGN_PUB_KEY=$(echo "$JSON_KEYS" | jq -r '[.[] | select(.publicBase64Key)][1].publicBase64Key')
export BOB_SIGN_ADDRESS=$(echo "$JSON_KEYS" | jq -r '[.[] | select(.publicBase64Key)][1].iotaAddress')
export ALICE_SIGN_PUB_KEY_HEX=$(echo "$ALICE_SIGN_PUB_KEY" | base64 -d | od -An -tx1 | tr -d ' \n')
export ALICE_SIGN_PUB_KEY_BYTES=$(python3 -c "import sys; print([int('$ALICE_SIGN_PUB_KEY_HEX'[i:i+2],16) for i in range(0,len('$ALICE_SIGN_PUB_KEY_HEX'),2)])")
echo "Alice address: $ALICE_SIGN_ADDRESS"
echo "Alice public key hex: $ALICE_SIGN_PUB_KEY_HEX"
echo "Alice public key bytes: $ALICE_SIGN_PUB_KEY_BYTES"
export BOB_SIGN_PUB_KEY_HEX=$(echo "$BOB_SIGN_PUB_KEY" | base64 -d | od -An -tx1 | tr -d ' \n')
export BOB_SIGN_PUB_KEY_BYTES=$(python3 -c "import sys; print([int('$BOB_SIGN_PUB_KEY_HEX'[i:i+2],16) for i in range(0,len('$BOB_SIGN_PUB_KEY_HEX'),2)])")
echo "Bob address: $BOB_SIGN_ADDRESS"
echo "Bob public key hex: $BOB_SIGN_PUB_KEY_HEX"
echo "Bob public key bytes: $BOB_SIGN_PUB_KEY_BYTES"

# Switch to localnet and get some funds to publish the necessary packages
iota client switch --env localnet
iota client switch --address $ALICE_SIGN_ADDRESS
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
--move-call $PACKAGE_ID::$CREATE_MODULE_NAME::$CREATE_FUNCTION_NAME vector"[@$ALICE_SIGN_ADDRESS,@$BOB_SIGN_ADDRESS]" "vector[1,1]" 2 ref \
--json)
export ABSTRACTACCOUNT=$(echo $PTB_JSON | jq -r '.objectChanges[] | select(.type == "created" and (.objectType | endswith("::'$ACCOUNT_MODULE_NAME'::'$ACCOUNT_TYPE_NAME'"))) | .objectId')
echo "Account Object ID: $ABSTRACTACCOUNT"

# Add the newly created account to the CLI keystore and set is as active
iota client add-account $ABSTRACTACCOUNT
iota client switch --address $ABSTRACTACCOUNT
# Request funds for the account
iota client faucet

# Create a transaction where the sender is the account, but don't issue it; creates a function call key allowance
UNSIGNED_TX_BYTES=$(iota client ptb \
--move-call 0x2::clock::timestamp_ms @0x6 \
--sender @$ABSTRACTACCOUNT \
--serialize-unsigned-transaction)
echo "Unsigned TX: $UNSIGNED_TX_BYTES"
# Analyze the the TX just created
# iota keytool decode-or-verify-tx --tx-bytes $UNSIGNED_TX_BYTES

# Extract the TX digest that is used by the authenticator to check the signature for the account
TX_DIGEST_HEX=$(iota keytool tx-digest $UNSIGNED_TX_BYTES --json | jq -r '.digestHex[2:]')
echo "TX Digest Hex: $TX_DIGEST_HEX"
TX_DIGEST_BYTES=$(python3 -c "import sys; print([int('$TX_DIGEST_HEX'[i:i+2],16) for i in range(0,len('$TX_DIGEST_HEX'),2)])")

# bob proposes tx
iota client switch --address $BOB_SIGN_ADDRESS
iota client faucet
iota client ptb \
--move-call $PACKAGE_ID::$CREATE_MODULE_NAME::propose_transaction @$ABSTRACTACCOUNT vector"$TX_DIGEST_BYTES" \
--sender @$BOB_SIGN_ADDRESS

# alice approves tx
iota client switch --address $ALICE_SIGN_ADDRESS
iota client faucet
iota client ptb \
--move-call $PACKAGE_ID::$CREATE_MODULE_NAME::approve_transaction @$ABSTRACTACCOUNT vector"$TX_DIGEST_BYTES" \
--sender @$ALICE_SIGN_ADDRESS

# Finally, execute the TX
export SIGNED_TX_BYTES=$(iota client ptb \
--move-call 0x2::clock::timestamp_ms @0x6 \
--sender @$ABSTRACTACCOUNT \
--auth-call-args \
--serialize-signed-transaction)
echo "Signed tx bytes: $SIGNED_TX_BYTES"
iota client execute-combined-signed-tx --signed-tx-bytes $SIGNED_TX_BYTES

# optionally decode the signature:
iota keytool decode-sig --json $SIGNED_TX_BYTES
```
