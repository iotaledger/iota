module a::m {
    use std::ascii::{String, char};
    
    public struct Account has key {
        id: iota::object::UID,
    }

    public struct Wrapped has copy, drop, store {
        value: u64,
    }

    public struct Wrapped2 has copy, drop, store {
        value: u64,
    }

    public struct GenericAccount<T: store> has key {
        id: iota::object::UID,
        inner: T,
    }

    public struct GenericAccount2<T: store, U: store> has key {
        id: iota::object::UID,
        inner: T,
        other: U,
    }

    public struct GenericObject<T: store> has key, store {
        id: iota::object::UID,
        inner: T,
    }

    public struct NonObject has copy, drop, store {
        value: u64,
    }

    public struct NonObjectTemplated<T: copy + drop + store> has copy, drop, store {
        inner: T,
    }

    public struct Receiving<phantom T: key> has copy, drop, store {
        id: iota::object::ID,
    }

    #[view]
    public entry fun entry_view(a: u64): u64 {
        a
    }

    #[view]
    public fun minimally_viable(account: &Account): u64 {
        let _ = account;
        0
    }

    #[view]
    public fun primitive_by_value(account: &Account, val: u8): u64 {
        let _ = account;
        val as u64
    }

    #[view]
    public fun mutable_binding_for_non_object(account: &Account, mut val: u64): u64 {
        let _ = account;
        val = val + 1;
        val
    }

    #[view]
    public fun concrete_account(account: &GenericAccount<Wrapped>): u64 {
        account.inner.value
    }

    #[view]
    public fun concrete_multiple_account(account: &GenericAccount2<Wrapped, Wrapped2>): u64 {
        account.inner.value + account.other.value
    }

    #[view]
    public fun generic_object_immutable_ref(generic_object: &GenericObject<Wrapped>): u64 {
        generic_object.inner.value
    }

    #[view]
    public fun template_immutable_ref<T: store>(generic_object: &GenericObject<T>): u64 {
        let _ = generic_object;
        0
    }

    #[view]
    public fun template_key_store_immutable_ref<T: key + store>(generic_object: &GenericObject<T>): u64 {
        let _ = generic_object;
        0
    }

    #[view]
    public fun template_copy_drop_store_immutable_ref<T: copy + drop + store>(
        generic_object: &GenericObject<T>,
    ): u64 {
        let _ = generic_object;
        0
    }

    #[view]
    public fun non_object_by_value(value: NonObject): u64 {
        value.value
    }

    #[view]
    public fun templated_non_object_by_value<T: copy + drop + store>(
        value: NonObjectTemplated<T>,
    ): u64 {
        let _ = value;
        0
    }

    #[view]
    public fun option_primitive_by_value(value: Option<u64>): u64 {
        if (value.is_some()) {
            value.destroy_some()
        } else {
            0
        }
    }

    #[view]
    public fun option_non_object_by_value(value: Option<NonObject>): u64 {
        if (value.is_some()) {
            value.destroy_some().value
        } else {
            0
        }
    }

    #[view]
    public fun option_generic_object_immutable_ref(value: &Option<GenericObject<Wrapped>>): u64 {
        let _ = value;
        0
    }

    #[view]
    public fun vector_primitive_by_value(value: vector<u64>): u64 {
        value.length()
    }

    #[view]
    public fun vector_non_object_by_value(value: vector<NonObject>): u64 {
        value.length()
    }

    #[view]
    public fun vector_generic_object_immutable_ref(value: &vector<GenericObject<Wrapped>>): u64 {
        value.length()
    }

    #[view]
    public fun receiving_immutable_ref(receiving: &Receiving<GenericObject<Wrapped>>): u64 {
        let _ = receiving;
        0
    }

    #[view]
    public native fun native_view(v: u64): u64;

    #[view]
    public native fun native_view_no_param(): bool;

    #[view]
    public native fun native_type_param<T: key>(): u64;

    #[view]
    public fun unused_unconstrained_type_param<T>(): u64 {
        0
    }

    #[view]
    public fun unconstrained_type_param_by_ref<T>(x: &T): u64 {
        let _ = x;
        0
    }

    #[view]
    public fun unconstrained_type_param_vector_by_ref<T>(x: &vector<T>): u64 {
        let _ = x;
        0
    }

    #[view]
    public fun unconstrained_type_param_option_by_ref<T>(x: &Option<T>): u64 {
        let _ = x;
        0
    }

    #[view]
    public fun update_string_by_value(mut name: String): String {
        name.push_char(char(43));
        name
    }
}

module iota::object {
    public struct ID has copy, drop, store {
        bytes: address,
    }

    public struct UID has store {
        id: ID,
    }
}
