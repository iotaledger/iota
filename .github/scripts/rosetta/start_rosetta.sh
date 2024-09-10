#!/bin/bash
# Copyright (c) Mysten Labs, Inc.
# Modifications Copyright (c) 2024 IOTA Stiftung
# SPDX-License-Identifier: Apache-2.0

echo "Start Rosetta online server"
iota-rosetta start-online-server --data-path ./data &

echo "Start Rosetta offline server"
iota-rosetta start-offline-server &
