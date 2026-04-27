// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

module a::view_suggestions {
    struct Obj has key, store {
        id: iota::object::UID,
        value: u64,
    }

    struct Data has store {
        value: u64,
    }

    struct Wrapper {
        obj: Obj,
    }

    public fun maybe_view(a: u64, o: &Obj): u64 {
        let _ = o;
        a
    }

    // Warning: could be marked #[view]
    public fun get_value(data: &Data): u64 {
        data.value
    }

    // Warning: pure computation could be #[view]
    public fun multiply(a: u64, b: u64): u64 {
        a * b
    }

    public fun wrapper_ref(w: &mut Wrapper): u64 {
        let old = w.obj.value;
        w.obj.value = 42;
        old
    }

    public native fun returns_object(): Obj;

    public fun takes_mut_ref(x: &mut u64): u64 {
        *x
    }

    public fun generic_key<T: key>(x: &T): u64 {
        let _ = x;
        0
    }

    public native fun wrapper_value(w: Wrapper): u64;
}

module iota::object {
    struct UID has store {
        id: address,
    }
}
