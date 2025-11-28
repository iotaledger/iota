# Account

## Install iota binary with AA feature

(only needed until AA is stable or replace commands to use `./target/debug/iota` instead)

```bash
cargo install --locked --bin iota --features=iota-names,indexer --path crates/iota
```

## Setup localnet and publish account package

Start localnet with faucet

```bash
RUST_LOG="info,consensus=warn,iota_core=warn,fastcrypto_tbls=off,starfish_core=warn" iota start --force-regenesis --with-faucet
```

Get funds, publish move package, and create an account

```bash
iota client switch --env localnet
iota client faucet
# publish, extract JSON, set env vars, and print info
export JSON=$(iota client publish examples/move/account --json | awk '/{/ { if (!in_json) { in_json=1; brace_count=1 } else { brace_count++ } } /}/ { brace_count-- } in_json { print } brace_count == 0 && in_json { exit }')
echo $JSON
export DIGEST=$(echo $JSON | jq -r .digest)
export ACCOUNT_ADDRESS=$(echo $JSON | jq -r '.objectChanges[] | select(.type == "created" and (.objectType | endswith("::account::Account"))) | .objectId')
export INITIAL_VERSION=$(echo $JSON | jq -r '.objectChanges[] | select(.type == "created" and (.objectType | endswith("::account::Account"))) | .owner.Shared.initial_shared_version')
export PACKAGE_ID=$(echo $JSON | jq -r '.objectChanges[] | select(.type == "created" and (.objectType | endswith("::account::Account"))) | .objectType | split("::") | .[0]')
export METADATA_ID=$(echo $JSON | jq -r '.objectChanges[] | select(.type == "created" and .objectType == "0x2::package_metadata::PackageMetadataV1") | .objectId')
echo "Transaction Digest: $DIGEST"
echo "Account Object ID: $ACCOUNT_ADDRESS"
echo "Initial Shared Version: $INITIAL_VERSION"
echo "Account Package ID: $PACKAGE_ID"
echo "Package Metadata Object ID: $METADATA_ID"
```

## Claim the account by attaching the auth info

```bash
iota client ptb \
--move-call iota::account::create_auth_info_v1 "<$PACKAGE_ID::account::Account>" @$METADATA_ID '"account"' '"authenticate"' \
--assign authenticator \
--move-call iota::account::check_auth_info_v1_compatibility "<$PACKAGE_ID::account::Account>" @$ACCOUNT_ADDRESS authenticator \
--assign authenticator_proof \
--move-call $PACKAGE_ID::account::attach_auth_info_v1  "<$PACKAGE_ID::account::Account>" @$ACCOUNT_ADDRESS authenticator_proof \
--dry-run
```

## Use the account

```bash
iota client new-account $ACCOUNT_ADDRESS
iota client switch --address $ACCOUNT_ADDRESS
iota client gas
sleep 2 # wait for the gas to be available
iota client pay-iota --recipients 0x111111111504e9350e635d65cd38ccd2c029434c6a3a480d8947a9ba6a15b215 --amounts 1 --auth-args "hello"
```
