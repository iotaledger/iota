# Account

## Install iota binary with AA feature

(only needed until AA is stable; lternatively, you can build the iota binary and use the one in `./target/debug/iota` instead)

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
--move-call $PACKAGE_ID::account::link_auth @$ACCOUNT_ADDRESS @$METADATA_ID '"account"' '"authenticate"' \
--dry-run # leave out to execute
```

## Use the account

```bash
iota client add-account $ACCOUNT_ADDRESS
iota client switch --address $ACCOUNT_ADDRESS
iota client faucet
sleep 2 # wait for the gas to be available
iota client gas
# client command
iota client pay-iota --recipients 0x111111111504e9350e635d65cd38ccd2c029434c6a3a480d8947a9ba6a15b215 --amounts 1 --auth-call-args "hello"
# to provide shared or immutable objects in the move authenticator, just add their IDs like 0x6 0x950aed433091d07ba74cb93b9cac1cc334c3b7e8eb791524f10aa3da98ca9a8c
# PTB command
ADDRESS=$(iota client active-address)
iota client ptb \
--assign to_address @$ADDRESS \
--split-coins gas "[1]" \
--assign coin \
--transfer-objects "[coin]" to_address \
--auth-call-args "hello"
# --auth-type-args u64

# tx bytes signing
TX_BYTES=$(iota client pay-iota --recipients 0x111111111504e9350e635d65cd38ccd2c029434c6a3a480d8947a9ba6a15b215 --amounts 1 --serialize-unsigned-transaction)
echo $TX_BYTES
iota client sign --address $ACCOUNT_ADDRESS --data $TX_BYTES --auth-call-args "hello"
```
