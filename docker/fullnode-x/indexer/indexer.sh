#!/bin/bash
# Copyright (c) Mysten Labs, Inc.
# Modifications Copyright (c) 2024 IOTA Stiftung
# SPDX-License-Identifier: Apache-2.0

/usr/local/bin/iota-indexer --db-url ${DATABASE_URL} --rpc-client-url ${RPC_CLIENT_URL}
