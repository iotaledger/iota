# FunctionCallKeys IOTAccount Move Example

The IOTAccount with FunctionCallKeys defines an account that can be used to allow function-level delegation through the usage of function call keys. An owner controls the account, while different users can be granted permissions to call specific functions through the usage of function call keys.

This module provides:

- `attach` to initialize the per-account allow-set (a dynamic field).
- `create` to create a new `IOTAccount` with a public key and an authenticator.
- `grant_permission` / `revoke_permission` admin operations over a per-pubkey allow-set.
- `has_permission` read-only query.
- `authenticate` dual-flow implementation:
  1. OWNER FLOW (bypass): if the provided signature verifies against the account owner Ed25519 public key (stored by the underlying account), authentication succeeds **without** enforcing any function call key restrictions or command count checks.
  2. FUNCTION CALL KEY FLOW (delegated): otherwise, we treat `pub_key` as a delegated key:
     - verify signature against `pub_key`
     - enforce exactly one PTB command
     - extract a `FunctionRef` from that sole command and ensure it is allowed for `pub_key`.

This allows the true account owner to perform arbitrary programmable transactions while enabling granular function-level delegation to other keys.

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
export EXAMPLE_DIR="function_call_keys"
export ACCOUNT_MODULE_NAME="iotaccount"
export ACCOUNT_TYPE_NAME="IOTAccount"
export AUTH_MODULE_NAME="function_call_keys"
export AUTH_FUNCTION_NAME="ed25519_authenticator"
export CREATE_MODULE_NAME="function_call_keys"
export CREATE_FUNCTION_NAME="create"

# Get the signing addresses
export JSON_KEYS=$(iota keytool list --json)
_key_count=$(echo "$JSON_KEYS" | jq '[.[] | select(.publicBase64Key)] | length')
if [ "$_key_count" -lt 2 ]; then
  echo "Error: expected at least 2 keypairs with publicBase64Key, got $_key_count" >&2
  exit 1
fi
export OWNER_SIGN_PUB_KEY=$(echo "$JSON_KEYS" | jq -r '[.[] | select(.publicBase64Key)][0].publicBase64Key')
export OWNER_SIGN_ADDRESS=$(echo "$JSON_KEYS" | jq -r '[.[] | select(.publicBase64Key)][0].iotaAddress')
export FUNCALL_SIGN_PUB_KEY=$(echo "$JSON_KEYS" | jq -r '[.[] | select(.publicBase64Key)][1].publicBase64Key')
export FUNCALL_SIGN_ADDRESS=$(echo "$JSON_KEYS" | jq -r '[.[] | select(.publicBase64Key)][1].iotaAddress')
export OWNER_SIGN_PUB_KEY_HEX=$(echo "$OWNER_SIGN_PUB_KEY" | base64 -d | od -An -tx1 | tr -d ' \n')
export OWNER_SIGN_PUB_KEY_BYTES=$(python3 -c "import sys; print([int('$OWNER_SIGN_PUB_KEY_HEX'[i:i+2],16) for i in range(0,len('$OWNER_SIGN_PUB_KEY_HEX'),2)])")
echo "Owner address: $OWNER_SIGN_ADDRESS"
echo "Owner public key hex: $OWNER_SIGN_PUB_KEY_HEX"
echo "Owner public key bytes: $OWNER_SIGN_PUB_KEY_BYTES"
export FUNCALL_SIGN_PUB_KEY_HEX=$(echo "$FUNCALL_SIGN_PUB_KEY" | base64 -d | od -An -tx1 | tr -d ' \n')
export FUNCALL_SIGN_PUB_KEY_BYTES=$(python3 -c "import sys; print([int('$FUNCALL_SIGN_PUB_KEY_HEX'[i:i+2],16) for i in range(0,len('$FUNCALL_SIGN_PUB_KEY_HEX'),2)])")
echo "FunctionCall address: $FUNCALL_SIGN_ADDRESS"
echo "FunctionCall public key hex: $FUNCALL_SIGN_PUB_KEY_HEX"
echo "FunctionCall public key bytes: $FUNCALL_SIGN_PUB_KEY_BYTES"

# Switch to localnet and get some funds to publish the necessary packages
iota client switch --env localnet
iota client switch --address $OWNER_SIGN_ADDRESS
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
--move-call $PACKAGE_ID::$CREATE_MODULE_NAME::$CREATE_FUNCTION_NAME vector"$OWNER_SIGN_PUB_KEY_BYTES" none ref \
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
iota client switch --address $ABSTRACTACCOUNT
# Request funds for the account
iota client faucet

# Create a transaction where the sender is the account, but don't issue it; creates a function call key allowance
UNSIGNED_TX_BYTES=$(iota client ptb \
--move-call $PACKAGE_ID::function_call_keys_store::make_function_ref @0x2 '"'clock'"' '"'timestamp_ms'"' \
--assign function_ref \
--move-call $PACKAGE_ID::$CREATE_MODULE_NAME::grant_permission @$ABSTRACTACCOUNT vector"$FUNCALL_SIGN_PUB_KEY_BYTES" function_ref \
--serialize-unsigned-transaction)
echo "Unsigned TX: $UNSIGNED_TX_BYTES"
# Analyze the the TX just created
# iota keytool decode-or-verify-tx --tx-bytes $UNSIGNED_TX_BYTES

# Extract the TX digest that is used by the authenticator to check the signature for the account
TX_DIGEST_HEX=$(iota keytool tx-digest $UNSIGNED_TX_BYTES --json | jq -r '.digestHex')
echo "TX Digest Hex: $TX_DIGEST_HEX"

# Obtain the signature where the message is the TX digest and the signing key is part of the keypair from which the signing address was derived
export IOTA_SIGNATURE_HEX=$(iota keytool sign-raw --address $OWNER_SIGN_ADDRESS --data $TX_DIGEST_HEX --json | jq -r '.iotaSignature' | base64 -d | od -An -tx1 | tr -d ' \n')
echo "IOTA signature hex: $IOTA_SIGNATURE_HEX"
# The IOTA signature contains a flag and the public key, so here it strips those information (not necessary for the authenticator)
export SIGNATURE_HEX=$(echo $IOTA_SIGNATURE_HEX | cut -c 3-130)
echo "Signature hex: $SIGNATURE_HEX"

# Finally, execute the TX using the signature just created as auth-call-arg
export SIGNED_TX_BYTES=$(iota client ptb \
--move-call $PACKAGE_ID::function_call_keys_store::make_function_ref @0x2 '"'clock'"' '"'timestamp_ms'"' \
--assign function_ref \
--move-call $PACKAGE_ID::$CREATE_MODULE_NAME::grant_permission @$ABSTRACTACCOUNT vector"$FUNCALL_SIGN_PUB_KEY_BYTES" function_ref \
--auth-call-args 0x$OWNER_SIGN_PUB_KEY_HEX 0x$SIGNATURE_HEX  \
--serialize-signed-transaction)
echo "Signed tx bytes: $SIGNED_TX_BYTES"
iota client execute-combined-signed-tx --signed-tx-bytes $SIGNED_TX_BYTES

# Create a transaction where the sender is the account, using a function call key
FUNCALL_UNSIGNED_TX_BYTES=$(iota client ptb \
--move-call 0x2::clock::timestamp_ms @0x6 \
--sender @$ABSTRACTACCOUNT \
--serialize-unsigned-transaction)
echo "Unsigned TX: $FUNCALL_UNSIGNED_TX_BYTES"
# Analyze the the TX just created
# iota keytool decode-or-verify-tx --tx-bytes $FUNCALL_UNSIGNED_TX_BYTES

# Extract the TX digest that is used by the authenticator to check the signature for the account
FUNCALL_TX_DIGEST_HEX=$(iota keytool tx-digest $FUNCALL_UNSIGNED_TX_BYTES --json | jq -r '.digestHex')
echo "TX Digest Hex: $FUNCALL_TX_DIGEST_HEX"

# Obtain the signature where the message is the TX digest and the signing key is part of the keypair from which the signing address was derived
export FUNCALL_IOTA_SIGNATURE_HEX=$(iota keytool sign-raw --address $FUNCALL_SIGN_ADDRESS --data $FUNCALL_TX_DIGEST_HEX --json | jq -r '.iotaSignature' | base64 -d | od -An -tx1 | tr -d ' \n')
echo "IOTA signature hex: $FUNCALL_IOTA_SIGNATURE_HEX"
# The IOTA signature contains a flag and the public key, so here it strips those information (not necessary for the authenticator)
export FUNCALL_SIGNATURE_HEX=$(echo $FUNCALL_IOTA_SIGNATURE_HEX | cut -c 3-130)
echo "Signature hex: $FUNCALL_SIGNATURE_HEX"

# Finally, execute the TX using the signature just created as auth-call-arg
export SIGNED_TX_BYTES=$(iota client ptb \
--move-call 0x2::clock::timestamp_ms @0x6 \
--sender @$ABSTRACTACCOUNT \
--auth-call-args 0x$FUNCALL_SIGN_PUB_KEY_HEX 0x$FUNCALL_SIGNATURE_HEX  \
--serialize-signed-transaction)
echo "Signed tx bytes: $SIGNED_TX_BYTES"
iota client execute-combined-signed-tx --signed-tx-bytes $SIGNED_TX_BYTES

# optionally decode the signature:
iota keytool decode-sig --json $SIGNED_TX_BYTES
```
