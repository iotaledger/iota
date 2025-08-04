# Copyright (c) Mysten Labs, Inc.
# Modifications Copyright (c) 2025 IOTA Stiftung
# SPDX-License-Identifier: Apache-2.0

# tests that building a package that implicitly depends on `Kiosk` works in dev mode
iota move build --dev -p example 2> /dev/null
