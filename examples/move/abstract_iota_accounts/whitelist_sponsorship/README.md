# Whitelist Sponsorship Move Example

The `WhitelistSponsorshipAccount` defines a sponsoring account abstraction that pays gas only for transactions whose sender authenticator function is in an explicit whitelist, and only up to a per-user gas allowance. All policy state lives as inline struct fields on the account itself: the admin address, a `Bag` of accepted `AuthenticatorFunctionRefV1` entries (heterogeneous in the sender account type), and a `Table<address, u64>` of per-user gas budgets. The example splits responsibilities across two modules: the storage / admin surface lives in [`whitelist_sponsorship_account`](sources/whitelist_sponsorship_account.move); the `#[authenticator]` function and its PTB-scan helper live in [`whitelist_sponsorship_authentication`](sources/whitelist_sponsorship_authentication.move).

Administration is split from consumption: a dedicated admin address manages the whitelists, while a sponsored user can only consume their allowance by including a `deduct_user_gas_allowance` command in the PTB they want sponsored. That function takes no arguments beyond the sponsor account — it implicitly targets `ctx.sender()` and always deducts exactly `ctx.gas_budget()`. The sponsor's `authenticator` checks that the transaction is sponsored by this account, validates the sender's authenticator function against the whitelist, confirms the gas budget fits the sender's per-user allowance, and rejects the transaction unless the PTB includes a matching `deduct_user_gas_allowance` call for this sponsor.

This example chains off the [OneSig Move Example](../onesig/README.md): the **sender** is a `OneSigAccount` (set up by following that README first) and the **sponsor** is a `WhitelistSponsorshipAccount`. Both authenticate via `MoveAuthenticator`, exercising the AA-sender / AA-sponsor flow.

## Prerequisites

Run the [`onesig`](../onesig/README.md) walkthrough end-to-end first. The following environment variables defined in that walkthrough must still be in scope:

- `SIGN_ADDRESS` — the underlying ed25519 address; reused here as the OneSig signing key **and** as the sponsor's admin.
- `PACKAGE_ID`, `METADATA_ID` — the published OneSig package.
- `ABSTRACTACCOUNT` — the OneSig account that will be the sender of the sponsored transaction.
- `TX_DIGEST_BYTES_2`, `TX_DIGEST_BYTES_3` — two of the unsigned tx digests OneSig built (reused below as filler leaves so the Merkle tree has the same shape as OneSig's).

## How to run

```bash
# Re-export OneSig outputs under namespaced names so the WLS variables below don't shadow them
export ONESIG_PACKAGE_ID=$PACKAGE_ID
export ONESIG_METADATA_ID=$METADATA_ID
export ONESIG_ACCOUNT_MODULE_NAME="account"
export ONESIG_ACCOUNT_TYPE_NAME="OneSigAccount"
export ONESIG_AUTH_MODULE_NAME="account"
export ONESIG_AUTH_FUNCTION_NAME="onesig_authenticator"
export ONESIG_SENDER=$ABSTRACTACCOUNT

# Useful names for this example
export EXAMPLE_DIR="whitelist_sponsorship"
export ACCOUNT_MODULE_NAME="whitelist_sponsorship_account"
export ACCOUNT_TYPE_NAME="WhitelistSponsorshipAccount"
export AUTH_MODULE_NAME="whitelist_sponsorship_authentication"
export AUTH_FUNCTION_NAME="authenticator"
export CREATE_MODULE_NAME="whitelist_sponsorship_account"
export CREATE_FUNCTION_NAME="create"

# Make sure the active address is the admin / underlying ed25519 key
# (the OneSig walkthrough may have added other accounts to the keystore)
iota client switch --address $SIGN_ADDRESS

# Publish the whitelist sponsorship package
export JSON=$(iota client publish examples/move/abstract_iota_accounts/$EXAMPLE_DIR --with-unpublished-dependencies --json | awk '/{/ { if (!in_json) { in_json=1; brace_count=1 } else { brace_count++ } } /}/ { brace_count-- } in_json { print } brace_count == 0 && in_json { exit }')
export WLS_PACKAGE_ID=$(echo $JSON | jq -r '.objectChanges[] | select(.type == "published") | .packageId')
export WLS_METADATA_ID=$(echo $JSON | jq -r '.objectChanges[] | select(.type == "created" and .objectType == "0x2::package_metadata::PackageMetadataV1") | .objectId')
echo "WLS Package ID: $WLS_PACKAGE_ID"
echo "WLS Metadata Object ID: $WLS_METADATA_ID"

# Create the WhitelistSponsorshipAccount with $SIGN_ADDRESS as admin
export PTB_JSON=$(iota client ptb \
--move-call 0x2::authenticator_function::create_auth_function_ref_v1 '<'$WLS_PACKAGE_ID'::'$ACCOUNT_MODULE_NAME'::'$ACCOUNT_TYPE_NAME'>' @$WLS_METADATA_ID '"'$AUTH_MODULE_NAME'"' '"'$AUTH_FUNCTION_NAME'"' \
--assign ref \
--move-call $WLS_PACKAGE_ID::$CREATE_MODULE_NAME::$CREATE_FUNCTION_NAME @$SIGN_ADDRESS ref \
--json)
export SPONSOR_ACCOUNT=$(echo $PTB_JSON | jq -r '.objectChanges[] | select(.type == "created" and (.objectType | endswith("::'$ACCOUNT_MODULE_NAME'::'$ACCOUNT_TYPE_NAME'"))) | .objectId')
echo "Sponsor Account Object ID: $SPONSOR_ACCOUNT"

# Add the sponsor account to the CLI keystore and fund it
iota client add-account $SPONSOR_ACCOUNT
iota client faucet --address $SPONSOR_ACCOUNT
iota client faucet --address $SPONSOR_ACCOUNT
export SPONSOR_GAS_JSON=$(iota client gas $SPONSOR_ACCOUNT --json)
export SPONSOR_COIN=$(echo "$SPONSOR_GAS_JSON" | jq -r '.[0].gasCoinId')
echo "Sponsor gas coin: $SPONSOR_COIN"

# `add-account` may rotate the active address; switch back to the admin before issuing admin txs
iota client switch --address $SIGN_ADDRESS

# Admin whitelists OneSig's `onesig_authenticator` for senders of type OneSigAccount
iota client ptb \
--move-call 0x2::authenticator_function::create_auth_function_ref_v1 '<'$ONESIG_PACKAGE_ID'::'$ONESIG_ACCOUNT_MODULE_NAME'::'$ONESIG_ACCOUNT_TYPE_NAME'>' @$ONESIG_METADATA_ID '"'$ONESIG_AUTH_MODULE_NAME'"' '"'$ONESIG_AUTH_FUNCTION_NAME'"' \
--assign onesig_ref \
--move-call $WLS_PACKAGE_ID::$ACCOUNT_MODULE_NAME::add_authenticator_function '<'$ONESIG_PACKAGE_ID'::'$ONESIG_ACCOUNT_MODULE_NAME'::'$ONESIG_ACCOUNT_TYPE_NAME'>' @$SPONSOR_ACCOUNT onesig_ref

# Admin grants a gas allowance to the OneSig sender
export ALLOWANCE=1000000000
iota client ptb \
--move-call $WLS_PACKAGE_ID::$ACCOUNT_MODULE_NAME::add_user_gas_allowance @$SPONSOR_ACCOUNT @$ONESIG_SENDER $ALLOWANCE

# Build the sponsored unsigned transaction. The PTB must include a `deduct_user_gas_allowance`
# command for the sponsor's authenticator's PTB scan to accept it. Sender is the OneSig account,
# sponsor is the WhitelistSponsorshipAccount, gas comes from the sponsor's coin.
export GAS_BUDGET=100000000
export UNSIGNED_TX_BYTES=$(iota client ptb \
--move-call 0x2::clock::timestamp_ms @0x6 \
--move-call $WLS_PACKAGE_ID::$ACCOUNT_MODULE_NAME::deduct_user_gas_allowance @$SPONSOR_ACCOUNT \
--sender @$ONESIG_SENDER \
--gas-sponsor @$SPONSOR_ACCOUNT \
--gas-coins @$SPONSOR_COIN \
--gas-budget $GAS_BUDGET \
--serialize-unsigned-transaction)
echo "Unsigned sponsored TX: $UNSIGNED_TX_BYTES"
export TX_DIGEST_HEX=$(iota keytool tx-digest $UNSIGNED_TX_BYTES --json | jq -r '.digestHex[2:]')
export TX_DIGEST_BYTES=$(python3 -c "import sys; print([int('$TX_DIGEST_HEX'[i:i+2],16) for i in range(0,len('$TX_DIGEST_HEX'),2)])")
echo "Sponsored TX digest hex: $TX_DIGEST_HEX"

# Build a Merkle tree authorizing this sponsored tx. The two filler leaves come from OneSig's
# walkthrough so the tree depth (and proof length) matches what OneSig's authenticator produced.
export VIEW_RESULT=$(curl -s http://127.0.0.1:9000 -X POST -H 'Content-Type: application/json' \
-d "{
  \"jsonrpc\": \"2.0\",
  \"id\": 1,
  \"method\": \"iota_view\",
  \"params\": [
    \"${ONESIG_PACKAGE_ID}::merkle::build_merkle_tree_with_proofs\",
    [],
    [[$TX_DIGEST_BYTES,$TX_DIGEST_BYTES_2,$TX_DIGEST_BYTES_3]]
  ]
}" | jq .)
export MERKLE_ROOT=$(echo "$VIEW_RESULT" | jq -r '.result.functionReturnValues[0][]' | xargs printf "%02x")
export PROOF_0=$(echo "$VIEW_RESULT" | jq -r '.result.functionReturnValues[1][0][0][]' | xargs printf "%02x")
export PROOF_1=$(echo "$VIEW_RESULT" | jq -r '.result.functionReturnValues[1][0][1][]' | xargs printf "%02x")
echo "Merkle root: $MERKLE_ROOT"

# Sign the Merkle root with the underlying ed25519 key
# (OneSig's `onesig_authenticator` verifies the signature against the root, not the tx digest)
export IOTA_SIGNATURE_HEX=$(iota keytool sign-raw --address $SIGN_ADDRESS --data $MERKLE_ROOT --json | jq -r '.iotaSignature' | base64 -d | od -An -tx1 | tr -d ' \n')
export SIGNATURE_HEX=$(echo $IOTA_SIGNATURE_HEX | cut -c 3-130)
echo "Signature hex: $SIGNATURE_HEX"

# Re-issue the sponsored PTB, this time signed. `--auth-call-args` supplies OneSig's three sender
# inputs (merkle_root, proof, signature). The sponsor authenticator takes no user-facing inputs,
# so `--sponsor-auth-call-args` is omitted — the CLI auto-detects the sponsor as an abstract
# account from the keystore (`iota client add-account $SPONSOR_ACCOUNT` above) and routes it
# through the `MoveAuthenticator` path with empty args.
export SIGNED_TX_BYTES=$(iota client ptb \
--move-call 0x2::clock::timestamp_ms @0x6 \
--move-call $WLS_PACKAGE_ID::$ACCOUNT_MODULE_NAME::deduct_user_gas_allowance @$SPONSOR_ACCOUNT \
--sender @$ONESIG_SENDER \
--gas-sponsor @$SPONSOR_ACCOUNT \
--gas-coins @$SPONSOR_COIN \
--gas-budget $GAS_BUDGET \
--auth-call-args 0x$MERKLE_ROOT '["'0x$PROOF_0'","'0x$PROOF_1'"]' 0x$SIGNATURE_HEX \
--serialize-signed-transaction)
echo "Signed sponsored TX: $SIGNED_TX_BYTES"
iota client execute-combined-signed-tx --signed-tx-bytes $SIGNED_TX_BYTES

# optionally decode the signature:
iota keytool decode-sig --json $SIGNED_TX_BYTES
```

After successful execution, the sponsor's user-gas-allowances table entry for `$ONESIG_SENDER` will have been reduced by `$GAS_BUDGET`.
