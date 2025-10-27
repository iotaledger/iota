# Copyright (c) Mysten Labs, Inc.
# Modifications Copyright (c) 2025 IOTA Stiftung
# SPDX-License-Identifier: Apache-2.0

# check that iota move new followed by iota move test succeeds
iota move new example
cd example && iota move test
