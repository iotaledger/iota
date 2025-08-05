# Copyright (c) Mysten Labs, Inc.
# Modifications Copyright (c) 2025 IOTA Stiftung
# SPDX-License-Identifier: Apache-2.0

# tests that iota move new followed by iota move publish succeeds on network defined by current branch

iota move new example
echo "module example::example;" >> example/sources/example.move

echo "=== publish package (localnet) ===" | tee /dev/stderr
iota client --client.config $CONFIG publish "example" \
  --json 2> /dev/null > output
cat output | jq '.effects.status'
UPGRADE_CAP=$(cat output | jq -r '.objectChanges[] | select(.objectType == "0x2::package::UpgradeCap") | .objectId')

echo "=== upgrade package (localnet) ===" | tee /dev/stderr
iota client --client.config $CONFIG upgrade --upgrade-capability $UPGRADE_CAP example \
  --json 2> /dev/null | jq '.effects.status'

echo "=== set up networks ===" | tee /dev/stderr
iota client --client.config $CONFIG new-env --alias devnet --rpc https://api.devnet.iota.cafe
iota client --client.config $CONFIG new-env --alias testnet --rpc https://api.testnet.iota.cafe
iota client --client.config $CONFIG new-env --alias mainnet --rpc https://api.mainnet.iota.cafe

for i in devnet testnet mainnet; do
  echo "=== publish package ($i) ===" | tee /dev/stderr
  iota client --client.config $CONFIG switch --env "$i" \
    2> /dev/null
  iota client --client.config $CONFIG publish "example" \
    --dry-run \
    --json 2> /dev/null | jq '.effects.status'
done
