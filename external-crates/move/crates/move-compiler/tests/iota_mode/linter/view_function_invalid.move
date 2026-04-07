// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

module a::invalid_view {
    use iota::object::{UID, do_something};

    struct Counter has key {
        id: UID,
        value: u64,
    }

    // Error: void return type
    #[view]
    public fun no_return(x: u64) {
        let _ = x;
    }

    // Error: takes object by value (has key ability)
    #[view]
    public fun takes_object(counter: Counter): u64 {
        let Counter { id: _, value } = counter;
        value
    }

    // Error: mutable reference to object
    #[view]
    public fun mutates_object(counter: &mut Counter): u64 {
        counter.value = counter.value + 1;
        counter.value
    }

    // Error: calls external module function
    #[view]
    public fun calls_external(_x: u64): u64 {
        do_something()
    }

    // Error: native function
    #[view]
    public native fun native_view(x: u64): u64;
}

module iota::object {
    struct UID has drop, store {
        id: address,
    }

    #[view]
    public fun do_something(): u64 {
        42
    }
}
