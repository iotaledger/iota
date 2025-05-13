# Copyright (c) Mysten Labs, Inc.
# Modifications Copyright (c) 2024 IOTA Stiftung
# SPDX-License-Identifier: Apache-2.0

# basic test that iota-move new outputs correct files
iota-move new example
echo ==== files in project ====
ls -A example
echo ==== files in sources ====
ls -A example/sources
echo ==== files in tests =====
ls -A example/tests