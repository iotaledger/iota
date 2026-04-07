// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

module a::view_suggestions {

    struct Data has store {
        value: u64,
    }

    // Warning: could be marked #[view]
    public fun get_value(data: &Data): u64 {
        data.value
    }

    // Warning: pure computation could be #[view]
    public fun multiply(a: u64, b: u64): u64 {
        a * b
    }
}