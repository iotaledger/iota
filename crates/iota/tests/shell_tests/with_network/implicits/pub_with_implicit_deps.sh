# Copyright (c) Mysten Labs, Inc.
# Modifications Copyright (c) 2025 IOTA Stiftung
# SPDX-License-Identifier: Apache-2.0

# tests that publishing a package with an implicit dependency on `Kiosk` succeeds

echo "=== set up networks ===" | tee /dev/stderr
iota client --client.config $CONFIG new-env --alias devnet --rpc https://api.devnet.iota.cafe
iota client --client.config $CONFIG new-env --alias testnet --rpc https://api.testnet.iota.cafe
iota client --client.config $CONFIG new-env --alias mainnet --rpc https://api.mainnet.iota.cafe

for i in localnet devnet testnet mainnet; do
  echo "=== publish package ($i) ===" | tee /dev/stderr
  iota client --client.config $CONFIG switch --env "$i" \
    2> /dev/null
  iota client --client.config $CONFIG publish "example" \
    --dry-run \
    --json 2> /dev/null | jq '.effects.status'
done
