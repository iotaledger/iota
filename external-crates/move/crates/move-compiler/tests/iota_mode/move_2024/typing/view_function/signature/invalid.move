module a::m {
    use std::ascii::{String, char};

    public struct Obj has key, store {
        id: iota::object::UID,
        value: u64,
    }

    public struct Wrapper {
        obj: Obj,
    }

    public struct StoreOnly has store {
        value: u64,
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
    public fun wrapper_by_value(_w: Wrapper): u64 {
        abort 0
    }

    #[view]
    public fun returns_wrapper(): Wrapper {
        abort 0
    }

    #[view]
    public fun store_only_by_value(s: StoreOnly): u64 {
        let StoreOnly { value } = s;
        value
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

    #[view]
    public fun update_string_by_value(mut name: String): String {
        name.push_char(char(43));
        name
    }

    #[view]
    public fun generic_reference<T: key>(_o: &mut T): bool {
        true
    }

    #[view]
    public fun unused_unconstrained_type_param<T>(): u64 {
        0
    }

    #[view]
    public native fun store_only_type_param<T: store>(x: T): u64;

    #[view]
    public native fun native_bad_type_param<T: key>(): u64;

    #[view]
    public native fun native_mut_ref(x: &mut u64): u64;
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

// module std::ascii {
//     public struct String has copy, drop, store {
//         bytes: vector<u8>,
//     }
//     /// An ASCII character.
//     public struct Char has copy, drop, store {
//         byte: u8,
//     }

//     /// Push a `Char` to the end of the `string`.
//     public fun push_char(string: &mut String, char: Char) {
//         string.bytes.push_back(char.byte);
//     }

//     /// Convert a `byte` into a `Char` that is checked to make sure it is valid ASCII.
//     public fun char(byte: u8): Char {
//         assert!(is_valid_char(byte), 0);
//         Char { byte }
//     }

//     /// Returns `true` if `b` is a valid ASCII character.
//     /// Returns `false` otherwise.
//     public fun is_valid_char(b: u8): bool {
//         b <= 0x7F
//     }
// }
