# Copyright (c) Mysten Labs, Inc.
# Modifications Copyright (c) 2025 IOTA Stiftung
# SPDX-License-Identifier: Apache-2.0

# checks that testing a package with `--dev` that implicitly depends on `Kiosk` works
iota move test -p example --dev 2> /dev/null
