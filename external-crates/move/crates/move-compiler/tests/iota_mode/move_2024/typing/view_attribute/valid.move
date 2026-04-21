module a::m {
    public struct Obj has key, store {
        id: iota::object::UID,
    }

    #[view]
    public fun value_and_object_ref(a: u64, o: &Obj): u64 {
        let _ = o;
        a
    }

    #[view]
    public fun returns_ref(): &u64 {
        abort 0
    }

    #[view]
    public native fun native_view(v: u64): u64;
}

module iota::object {
    public struct UID has store {
        id: address,
    }
}
