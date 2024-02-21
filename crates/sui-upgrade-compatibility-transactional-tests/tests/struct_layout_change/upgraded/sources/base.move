// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

module base::base_module {
    struct X {
        field0: u64,
        field1: u64,
        extra_field: u64,
    }

    public fun public_fun(): u64 { 0 }
    fun private_fun(): u64 { 0 }
    entry fun private_entry_fun(_x: u64) { }
}
