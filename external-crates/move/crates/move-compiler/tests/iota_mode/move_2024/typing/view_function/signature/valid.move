module a::m {
    public struct Obj has key, store {
        id: iota::object::UID,
        value: u64,
    }

    public struct Data has copy, drop {
        value: u64,
    }

    #[view]
    public fun value_and_object_ref(a: u64, o: &Obj): u64 {
        let _ = o;
        a
    }

    #[view]
    public fun object_vector_ref(v: &vector<Obj>): u64 {
        v.length()
    }

    #[view]
    public fun data_by_value(d: Data): u64 {
        d.value
    }

    #[view]
    public fun valid_copy_type_param<T: copy>(x: T): T {
        x
    }

    #[view]
    public fun valid_drop_type_param<T: drop>(x: T): u64 {
        let _ = x;
        0
    }

    #[view]
    public entry fun entry_view(a: u64): u64 {
        a
    }

    #[view]
    public fun returns_ref(): &u64 {
        abort 0
    }

    #[view]
    public fun object_ref_return(o: &Obj): &Obj {
        o
    }

    #[view]
    public native fun native_view(v: u64): u64;
}

module iota::object {
    public struct UID has store {
        id: address,
    }
}
