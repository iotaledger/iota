# lean-imt-account

An Abstract IOTA Account backed by a [Lean Incremental Merkle Tree](https://github.com/privacy-scaling-explorations/zk-kit.circom/issues/17) (LeanIMT). A set of IOTA addresses are hashed with [Poseidon](https://docs.rs/fastcrypto-zkp/latest/fastcrypto_zkp/) and inserted into the tree. Any address in the tree can authenticate as the account by submitting a [Groth16](https://docs.iota.org/developer/cryptography/on-chain/groth16) zero-knowledge proof of membership.

This enables shared accounts controlled by a large group of addresses without requiring individual on-chain transactions for each member, useful for airdrops, DAOs, or any scenario where many addresses need to act through a single account.

Two authentication modes are supported:

- **Secret mode** -- the caller proves membership without revealing their public key.
- **Public key mode** -- the caller's public key is disclosed on-chain and the leaf is derived from it.

> [!WARNING]\
> This is a PoC, as a properly secure design would involve at least some salt mechanism for the hash and additional proving mechanism; in here, if a public key being part of the IMT is disclosed, then it would be trivial to obtain an unwanted access to the account.

Check https://github.com/miker83z/iota-lean-imt-account for the instructions and code on how to generate a new tree and proofs.

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
export EXAMPLE_DIR="lean_imt_account"
export ACCOUNT_MODULE_NAME="lean_imt_account"
export ACCOUNT_TYPE_NAME="LeanIMTAccount"
export AUTH_MODULE_NAME="lean_imt_account"
export AUTH_FUNCTION_NAME="secret_ed25519_authenticator"
export CREATE_MODULE_NAME="lean_imt_account"
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
export ROOT_HEX="4b61023a56e6b37edec2ba55c1b3f0cf0f4789431aafd6da10d32de09bb97402"
export ROOT_BYTES=$(python3 -c "import sys; print([int('$ROOT_HEX'[i:i+2],16) for i in range(0,len('$ROOT_HEX'),2)])")
export PTB_JSON=$(iota client ptb \
--move-call 0x2::authenticator_function::create_auth_function_ref_v1 '<'$PACKAGE_ID'::'$ACCOUNT_MODULE_NAME'::'$ACCOUNT_TYPE_NAME'>' @$METADATA_ID '"'$AUTH_MODULE_NAME'"' '"'$AUTH_FUNCTION_NAME'"' \
--assign ref \
--move-call $PACKAGE_ID::$CREATE_MODULE_NAME::$CREATE_FUNCTION_NAME vector"$ROOT_BYTES" ref \
--json)
export ABSTRACTACCOUNT=$(echo $PTB_JSON | jq -r '.objectChanges[] | select(.type == "created" and (.objectType | endswith("::'$ACCOUNT_MODULE_NAME'::'$ACCOUNT_TYPE_NAME'"))) | .objectId')
echo "Account Object ID: $ABSTRACTACCOUNT"

# Add the newly created account to the CLI keystore and set is as active
iota client add-account $ABSTRACTACCOUNT
iota client switch --address $ABSTRACTACCOUNT
# Request funds for the account
iota client faucet

# Create a transaction where the sender is the account, but don't issue it; just take the bytes
UNSIGNED_TX_BYTES=$(iota client pay-iota --recipients 0x111111111504e9350e635d65cd38ccd2c029434c6a3a480d8947a9ba6a15b215 --amounts 1 --serialize-unsigned-transaction)
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
# https://github.com/miker83z/iota-lean-imt-account
# ADDRESS=0x6b72f63997aa75e2aff8e7cb119f5507f8b521dade51003fc07c8a4c70f79a70 cargo run --bin hash_address
export DOUBLE_HASHED_PUB_KEY_LEAF="3bdfd5246d42721d0a65eb7700be407b537b71f491cded4e1743b6253e353322"
#  ADDRESS=0x6b72f63997aa75e2aff8e7cb119f5507f8b521dade51003fc07c8a4c70f79a70 cargo run --bin generate_claim_proof
export VERIFYINGKEY="bbebdc7c4023eeb6a81fcbbc37613366f4dac6687cbe9abc5d09e4cef8899b173a2747c5442ddb898d34de2429a9b43f86b12aeea35d58ec1d97a009eb2a9c2d34d3f96ebdb7416fbedf83ff29abee30941a380166aac2b2557476f50ffe2094777610b217740ac57c573cbf6af8bce106f7772241dce3406f0b1f2b845570074dd670d78f1c9e0d29fa7113753e384f56775627c9c64dd899566f90b7813301b20482628c99f957ff584f940965bc6b711d377c76b12921e0816b421cae5c098432218eb209bb104559ddc0ad78173ebde47c918c540b82e7e6b658b52bb19e0300000000000000926536117de81e192a1c9bec13bbdfb102852c05911d2ebb306e09706547dfac4dfc834de8b175761425b0dd4080c5c3700a63a4da9548d08ab47ba968d8d09969cdb618703b45502a39c93ce22bbd739c946fba6fd4e10285806ed6de1acaa0"
export PROOF_POINTS="641a8593665c9e415c6f7f2c57ad992566ee5af86d86f00812541e97ef1fa4182493694830c22645274d619dbdb95886a8cbf23c5f3683dd3b1a39a7953898243eab26e677c31f863201c3339c427d46ebc1390203917f39c8479135f275bf17456182c1f59a574d3390206bd51f8373384a56c407f94f677089d3d0ce9999a3"
export SIGNED_TX_BYTES=$(iota client pay-iota --recipients 0x111111111504e9350e635d65cd38ccd2c029434c6a3a480d8947a9ba6a15b215 --amounts 1 --auth-call-args 0x$SIGNATURE_HEX 0x$SIGN_PUB_KEY_HEX 0x$DOUBLE_HASHED_PUB_KEY_LEAF 0x$VERIFYINGKEY 0x$PROOF_POINTS --serialize-signed-transaction)
echo "Signed tx bytes: $SIGNED_TX_BYTES"
iota client execute-combined-signed-tx --signed-tx-bytes $SIGNED_TX_BYTES

# optionally decode the signature:
iota keytool decode-sig --json $SIGNED_TX_BYTES
```
