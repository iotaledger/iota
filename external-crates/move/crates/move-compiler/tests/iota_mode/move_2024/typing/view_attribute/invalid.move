module a::m {
    public struct Obj has key, store {
        id: iota::object::UID,
        value: u64,
    }

    public struct Wrapper {
        obj: Obj,
    }

    #[view]
    fun private_view(): u64 {
        0
    }

    #[view]
    public fun no_return() {
        abort 0
    }

    #[view]
    public fun vec_of_obj(_v: vector<Obj>): u64 {
        abort 0
    }

    #[view]
    public fun mut_ref_vec_of_obj(_v: &mut vector<Obj>): u64 {
        abort 0
    }

    #[view]
    public fun wrapper_ref(w: &mut Wrapper): u64 {
        let old = w.obj.value;
        w.obj.value = 42;
        old
    }

    #[view]
    public fun returns_object(): Obj {
        abort 0
    }

    #[view]
    public fun object_by_value(_o: Obj): u64 {
        abort 0
    }

    #[view]
    public fun mut_object_ref(o: &mut Obj): u64 {
        let _ = o;
        0
    }

    #[view]
    public fun nested_vec_of_obj(_v: vector<vector<Obj>>): u64 {
        abort 0
    }

    #[view]
    public fun tx_context_arg(ctx: &mut iota::tx_context::TxContext): u64 {
        let _ = ctx;
        0
    }

    #[view]
    public fun transfer_obj_generic<T: key + store>(o: T, to: address) {
        iota::transfer::public_transfer(o, to);
    }

    #[view]
    public native fun native_view(): bool;

    #[view]
    public fun with_mut_non_object_reference(v: &mut u64): u64 {
        let _ = v;
        0
    }
}

module iota::object {
    public struct ID has copy, drop, store {
        bytes: address,
    }

    public struct UID has store {
        id: ID,
    }

    public fun delete(id: UID) {
        let UID { id: ID { bytes: _bytes } } = id;
    }
}

module iota::tx_context {
    public struct TxContext has drop {}
}

module iota::mutate_object {
    public fun mutate(_o: &mut a::m::Obj) {
        abort 0
    }
}

module iota::transfer {
    public fun public_transfer<T: key + store>(obj: T, recipient: address) {
        transfer_impl(obj, recipient)
    }

    native fun transfer_impl<T: key>(obj: T, recipient: address);
}
