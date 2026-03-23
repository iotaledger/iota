# Spending Limit Account Move Example

The SpendingLimitAccount module defines an account struct that can be used as a programmable account with a spending limit. The account data, stored as dynamic fields, includes a spending limit value and a balance reserve.
The spending limit is a u64 value that represents the maximum amount that can be withdrawn from the account in a single transaction. The balance reserve is a struct that holds the current balance reserved for spending and allows withdrawing and depositing funds to it. The account also has an owner public key.

The module includes functions to create a new `SpendingLimitAccount`, rotate the account's authenticator, rotate the account's owner public key, withdraw from the balance reserve, and deposit to the balance reserve.

The authenticator function for the `SpendingLimitAccount` validates the signature and checks for withdrawal commands in the transaction PTB. It looks into the PTB commands to find calls to the `withdraw_from_balance_reserve` function, calculates the total amount to be withdrawn in the transaction, and checks that the total amount does not exceed the spending limit.

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

# Useful names for this example
export EXAMPLE_DIR="spending_limit"
export ACCOUNT_MODULE_NAME="spending_limit_account"
export ACCOUNT_TYPE_NAME="SpendingLimitAccount"
export AUTH_MODULE_NAME="spending_limit_account"
export AUTH_FUNCTION_NAME="ed25519_authenticator"
export CREATE_MODULE_NAME="spending_limit_account"
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
export LIMIT=999
export PTB_JSON=$(iota client ptb \
--move-call 0x2::authenticator_function::create_auth_function_ref_v1 '<'$PACKAGE_ID'::'$ACCOUNT_MODULE_NAME'::'$ACCOUNT_TYPE_NAME'>' @$METADATA_ID '"'$AUTH_MODULE_NAME'"' '"'$AUTH_FUNCTION_NAME'"' \
--assign ref \
--move-call $PACKAGE_ID::$CREATE_MODULE_NAME::$CREATE_FUNCTION_NAME vector"$SIGN_PUB_KEY_BYTES" $LIMIT ref \
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

# Fund the account balance
iota client ptb \
--split-coins gas "[1000]" \
--assign new_coins \
--move-call $PACKAGE_ID::$CREATE_MODULE_NAME::deposit_to_balance_reserve @$ABSTRACTACCOUNT new_coins

# Add the newly created account to the CLI keystore and set is as active
iota client add-account $ABSTRACTACCOUNT
iota client switch --address $ABSTRACTACCOUNT
# Request funds for the account
iota client faucet

# Create a transaction where the sender is the account, but don't issue it; just take the bytes
UNSIGNED_TX_BYTES=$(iota client ptb \
--move-call $PACKAGE_ID::$CREATE_MODULE_NAME::withdraw_from_balance_reserve @$ABSTRACTACCOUNT 998 \
--assign withdrawn_coin \
--transfer-objects "[withdrawn_coin]" @0x0 \
--serialize-unsigned-transaction)
echo "Unsigned TX: $UNSIGNED_TX_BYTES"
# Analyze the the TX just created
# iota keytool decode-or-verify-tx --tx-bytes $UNSIGNED_TX_BYTES

# Extract the TX digest that is used by the authenticator to check the signature for the account
TX_DIGEST_HEX=$(iota keytool tx-digest $UNSIGNED_TX_BYTES --json | jq -r '.digestHex')
echo "TX Digest Hex: $TX_DIGEST_HEX"

# Obtain the signature where the message is the TX digest and the signing key is part of the keypair from which the signing address was derived
export IOTA_SIGNATURE_HEX=$(iota keytool sign-raw --address $SIGN_ADDRESS --data $TX_DIGEST_HEX --json | jq -r '.iotaSignature' | base64 -d | od -An -tx1 | tr -d ' \n')
echo "IOTA signature hex: $IOTA_SIGNATURE_HEX"
# The IOTA signature contains a flag and the public key, so here it strips those information (not necessary for the authenticator)
export SIGNATURE_HEX=$(echo $IOTA_SIGNATURE_HEX | cut -c 3-130)
echo "Signature hex: $SIGNATURE_HEX"

# Finally, execute the TX using the signature just created as auth-call-arg
export SIGNED_TX_BYTES=$(iota client ptb \
--move-call $PACKAGE_ID::$CREATE_MODULE_NAME::withdraw_from_balance_reserve @$ABSTRACTACCOUNT 998 \
--assign withdrawn_coin \
--transfer-objects "[withdrawn_coin]" @0x0 \
--auth-call-args 0x$SIGNATURE_HEX  \
--serialize-signed-transaction)
echo "Signed tx bytes: $SIGNED_TX_BYTES"
iota client execute-combined-signed-tx --signed-tx-bytes $SIGNED_TX_BYTES

# optionally decode the signature:
iota keytool decode-sig --json $SIGNED_TX_BYTES
```
