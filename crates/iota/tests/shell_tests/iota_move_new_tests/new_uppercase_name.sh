# Copyright (c) Mysten Labs, Inc.
# Modifications Copyright (c) 2025 IOTA Stiftung
# SPDX-License-Identifier: Apache-2.0

# test that `iota move new` works as expected with `<NAME>` containing uppercase letter(s)
iota move new _Example_A
echo ==== files in project ====
ls -A _Example_A
echo ==== files in sources ====
ls -A _Example_A/sources
echo ==== files in tests =====
ls -A _Example_A/tests
