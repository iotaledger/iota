module a::m {
    public struct Object has key {
        id: iota::object::UID,
    }

    public struct Wrapped has copy, drop, store {
        value: u64,
    }

    public struct StoreOnly has store {
        value: u64,
    }

    public struct GenericObject<T: store> has key, store {
        id: iota::object::UID,
        inner: T,
    }

    public struct GenericObject2<T: store, U: store> has key {
        id: iota::object::UID,
        inner: T,
        other: U,
    }

    public struct Wrapper<T> has key {
        id: iota::object::UID,
        wrapped: vector<T>,
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
    fun private_view(): u64 {
        0
    }

    #[view]
    public fun no_return() {
        abort 0
    }

    #[view]
    public fun object_by_value(_object: Object): u64 {
        abort 0
    }

    #[view]
    public fun object_mutable_ref(_object_ref: &mut Object): u64 {
        abort 0
    }

    #[view]
    public fun concrete_multiple_object_by_value(
        _generic_object2: GenericObject2<Wrapped, Wrapped>,
    ): u64 {
        abort 0
    }

    #[view]
    public fun generic_object_by_value(_generic_object: GenericObject<Wrapped>): u64 {
        abort 0
    }

    #[view]
    public fun generic_object_mutable_ref(_object_ref: &mut GenericObject<Wrapped>): u64 {
        abort 0
    }

    #[view]
    public fun template_by_value<T: store>(_generic_object: GenericObject<T>): u64 {
        abort 0
    }

    #[view]
    public fun template_key_store_by_value<T: key + store>(
        _generic_object: GenericObject<T>,
        _wrapper: &Wrapper<T>,
    ): u64 {
        abort 0
    }

    #[view]
    public fun template_copy_drop_store_by_value<T: copy + drop + store>(
        _generic_object: GenericObject<T>,
    ): u64 {
        abort 0
    }

    #[view]
    public fun direct_key_store_type_param_by_value<T: key + store>(_generic_object: T): u64 {
        abort 0
    }

    #[view]
    public fun unconstrained_type_param_by_value<T>(_value: T): u64 {
        abort 0
    }

    #[view]
    public fun store_only_by_value(value: StoreOnly): u64 {
        let StoreOnly { value } = value;
        value
    }

    #[view]
    public fun store_only_type_param_by_value<T: store>(_value: T): u64 {
        abort 0
    }

    #[view]
    public fun option_object_by_value(_value: Option<GenericObject<Wrapped>>): u64 {
        abort 0
    }

    #[view]
    public fun option_template_object_by_value<T: key + store>(_value: Option<T>): u64 {
        abort 0
    }

    #[view]
    public fun option_primitive_mutable_ref(_value: &mut Option<u64>): u64 {
        abort 0
    }

    #[view]
    public fun option_non_object_mutable_ref(_value: &mut Option<NonObject>): u64 {
        abort 0
    }

    #[view]
    public fun option_object_mutable_ref(_value: &mut Option<GenericObject<Wrapped>>): u64 {
        abort 0
    }

    #[view]
    public fun vector_object_by_value(_value: vector<GenericObject<Wrapped>>): u64 {
        abort 0
    }

    #[view]
    public fun vector_template_object_by_value<T: key + store>(_value: vector<T>): u64 {
        abort 0
    }

    #[view]
    public fun vector_primitive_mutable_ref(_value: &mut vector<u64>): u64 {
        abort 0
    }

    #[view]
    public fun vector_non_object_mutable_ref(_value: &mut vector<NonObject>): u64 {
        abort 0
    }

    #[view]
    public fun vector_object_mutable_ref(_value: &mut vector<GenericObject<Wrapped>>): u64 {
        abort 0
    }

    #[view]
    public fun receiving_by_value(_receiving: Receiving<GenericObject<Wrapped>>): u64 {
        abort 0
    }

    #[view]
    public fun receiving_mutable_ref(_receiving: &mut Receiving<GenericObject<Wrapped>>): u64 {
        abort 0
    }

    #[view]
    public fun tx_context_mutable_ref(_ctx: &mut iota::tx_context::TxContext): u64 {
        abort 0
    }

    #[view]
    public fun returns_object(): Object {
        abort 0
    }

    #[view]
    public fun returns_object_vector(): vector<GenericObject<Wrapped>> {
        abort 0
    }

    #[view]
    public fun returns_option_object(): Option<GenericObject<Wrapped>> {
        abort 0
    }

    #[view]
    public native fun store_only_type_param<T: store>(x: T): u64;

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

module iota::transfer {
    public fun public_transfer<T: key + store>(obj: T, recipient: address) {
        transfer_impl(obj, recipient)
    }

    native fun transfer_impl<T: key>(obj: T, recipient: address);
}
