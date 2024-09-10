// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

module dep_on_upgrading_package::my_module {
    use base_addr::base;

    public fun call_return_0(): u64 { base::return_0() }
}
