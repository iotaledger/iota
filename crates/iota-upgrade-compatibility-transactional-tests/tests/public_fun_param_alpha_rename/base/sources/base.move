// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung 
// SPDX-License-Identifier: Apache-2.0

module base::base_module {
    struct X {
        field0: u64,
        field1: u64,
    }

    public fun public_fun(x: u64, y: u64): u64 { x + y }
    fun private_fun(): u64 { 0 }
    entry fun private_entry_fun(_x: u64) { }
}
