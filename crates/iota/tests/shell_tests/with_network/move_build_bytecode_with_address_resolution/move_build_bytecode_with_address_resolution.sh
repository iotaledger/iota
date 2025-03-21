# Copyright (c) Mysten Labs, Inc.
# Modifications Copyright (c) 2025 IOTA Stiftung
# SPDX-License-Identifier: Apache-2.0

iota client --client.config $CONFIG \
  publish simple --verify-deps \
  --json | jq '.effects.status'

iota move --client.config $CONFIG \
  build --path depends_on_simple
