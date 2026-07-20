module a::m {
    use iota::transfer::Receiving;

    public struct Object has key {
        id: iota::object::UID,
    }

    public struct Wrapped has copy, drop, store {
        value: u64,
    }

    public struct Wrapped2 has copy, drop, store {
        value: u64,
    }

    public struct GenericObject2<T: store, U: store> has key {
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

    public struct PhantomWrapper<phantom T: key> has copy, drop, store {
        id: iota::object::ID,
    }

    public struct DynamicField has copy, drop, store {
        value: u64,
    }

    #[view]
    public entry fun entry_view(a: u64): u64 {
        a
    }

    #[view]
    public fun object_immutable_ref(object: &Object): u64 {
        let _ = object;
        0
    }

    #[view]
    public fun primitive_by_value(object: &Object, val: u8): u64 {
        let _ = object;
        val as u64
    }

    #[view]
    public fun multiple_generic_object_immutable_ref(
        generic_object: &GenericObject2<Wrapped, Wrapped2>,
    ): u64 {
        generic_object.inner.value + generic_object.other.value
    }

    #[view]
    public fun wrapped_by_value(wrapped: Wrapped): bool {
        wrapped.value > 44
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
    public fun template_key_store_immutable_ref<T: key + store>(
        generic_object: &GenericObject<T>,
    ): u64 {
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

    // `iota::transfer::Receiving` is rejected by value but allowed behind a
    // reference, like any other type.
    #[view]
    public fun receiving_immutable_ref(receiving: &Receiving<GenericObject<Wrapped>>): u64 {
        let _ = receiving;
        0
    }

    // A user-defined phantom wrapper is safe by value: the phantom object type
    // argument is not contained. Only `iota::transfer::Receiving` is rejected,
    // by name.
    #[view]
    public fun phantom_wrapper_by_value(wrapper: PhantomWrapper<GenericObject<Wrapped>>): u64 {
        let _ = wrapper;
        0
    }

    #[view]
    public fun copy_type_param<T: copy>(value: T): T {
        value
    }

    #[view]
    public fun drop_type_param<T: drop>(value: T): u64 {
        let _ = value;
        0
    }

    #[view]
    public fun copy_store_type_param<T: copy + store>(value: T): T {
        value
    }

    #[view]
    public fun primitive_tuple_return(a: u64, b: bool): (u64, bool) {
        (a, b)
    }

    #[view]
    public fun returns_generic_obj_reference(
        input: &GenericObject<Wrapped>,
    ): &GenericObject<Wrapped> {
        input
    }

    #[view]
    public fun returns_u64_reference(input: &u64): &u64 {
        input
    }

    #[view]
    public fun returns_tuple_with_reference(input: &u64): (&u64, u64) {
        (input, 0)
    }

    #[view]
    public fun returns_dynamic_field_reference(object: &Object, name: u64): &DynamicField {
        iota::dynamic_field::borrow<u64, DynamicField>(&object.id, name)
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
}

module iota::object {
    public struct ID has copy, drop, store {
        bytes: address,
    }

    public struct UID has store {
        id: ID,
    }
}

module iota::dynamic_field {
    public native fun borrow<Name: copy + drop + store, Value: store>(
        object: &iota::object::UID,
        name: Name,
    ): &Value;
}

module iota::transfer {
    public struct Receiving<phantom T: key> has drop {
        id: iota::object::ID,
    }
}
