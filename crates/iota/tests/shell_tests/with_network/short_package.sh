# Copyright (c) Mysten Labs, Inc.
# Modifications Copyright (c) 2025 IOTA Stiftung
# SPDX-License-Identifier: Apache-2.0

COIN=$(iota client --client.config $CONFIG objects   --json | jq '.[0].data.objectId')
ADDR=$(iota client --client.config $CONFIG addresses --json | jq '.addresses[0][1]')

iota client --client.config $CONFIG \
  call --package 0x2 --module iota --function transfer --args $COIN $ADDR \
  --gas-budget 100000000 \
  --json | jq '.effects.status'
