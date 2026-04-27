// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

module a::valid_view {
    struct Data has store {
        value: u64,
    }

    // Valid: returns value, immutable ref, no module calls
    #[view]
    public fun get_value(data: &Data): u64 {
        data.value
    }

    // Valid: pure computation
    #[view]
    public fun add(a: u64, b: u64): u64 {
        a + b
    }

    // Valid: returns tuple
    #[view]
    public fun get_pair(a: u64, b: u64): (u64, u64) {
        (a, b)
    }
}