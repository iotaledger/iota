// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

module a::m {
    use std::ascii::{String, char};
    use iota::transfer::Receiving;

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

    const ABORT_CODE: u64 = 0;

    fun private_view(): u64 {
        0
    }

    public fun no_return() {
        abort ABORT_CODE
    }

    public fun object_by_value(_object: Object): u64 {
        abort ABORT_CODE
    }

    public fun object_mutable_ref(_object_ref: &mut Object): u64 {
        abort ABORT_CODE
    }

    public fun concrete_multiple_object_by_value(
        _generic_object2: GenericObject2<Wrapped, Wrapped>,
    ): u64 {
        abort ABORT_CODE
    }

    public fun generic_object_by_value(_generic_object: GenericObject<Wrapped>): u64 {
        abort ABORT_CODE
    }

    public fun generic_object_mutable_ref(_object_ref: &mut GenericObject<Wrapped>): u64 {
        abort ABORT_CODE
    }

    public fun template_by_value<T: store>(_generic_object: GenericObject<T>): u64 {
        abort ABORT_CODE
    }

    public fun template_key_store_by_value<T: key + store>(
        _generic_object: GenericObject<T>,
        _wrapper: &Wrapper<T>,
    ): u64 {
        abort ABORT_CODE
    }

    public fun template_copy_drop_store_by_value<T: copy + drop + store>(
        _generic_object: GenericObject<T>,
    ): u64 {
        abort ABORT_CODE
    }

    public fun mutable_primitive_param(mut value: u64): u64 {
        value = value + 1;
        value
    }

    public fun mutable_non_object_param(mut value: NonObject): u64 {
        value.value = value.value + 1;
        value.value
    }

    public fun update_string_by_value(mut name: String): String {
        name.push_char(char(43));
        name
    }

    public fun direct_key_store_type_param_by_value<T: key + store>(_generic_object: T): u64 {
        abort ABORT_CODE
    }

    public fun unconstrained_type_param_by_value<T>(_value: T): u64 {
        abort ABORT_CODE
    }

    public fun store_only_by_value(value: StoreOnly): u64 {
        let StoreOnly { value } = value;
        value
    }

    public fun store_only_type_param_by_value<T: store>(_value: T): u64 {
        abort ABORT_CODE
    }

    public fun option_object_by_value(_value: Option<GenericObject<Wrapped>>): u64 {
        abort ABORT_CODE
    }

    public fun option_template_object_by_value<T: key + store>(_value: Option<T>): u64 {
        abort ABORT_CODE
    }

    public fun option_vector_object_by_value(_value: Option<vector<GenericObject<Wrapped>>>): u64 {
        abort ABORT_CODE
    }

    public fun vector_option_object_by_value(_value: vector<Option<GenericObject<Wrapped>>>): u64 {
        abort ABORT_CODE
    }

    public fun option_primitive_mutable_ref(_value: &mut Option<u64>): u64 {
        abort ABORT_CODE
    }

    public fun option_non_object_mutable_ref(_value: &mut Option<NonObject>): u64 {
        abort ABORT_CODE
    }

    public fun option_object_mutable_ref(_value: &mut Option<GenericObject<Wrapped>>): u64 {
        abort ABORT_CODE
    }

    public fun vector_object_by_value(_value: vector<GenericObject<Wrapped>>): u64 {
        abort ABORT_CODE
    }

    public fun vector_template_object_by_value<T: key + store>(_value: vector<T>): u64 {
        abort ABORT_CODE
    }

    public fun vector_primitive_mutable_ref(_value: &mut vector<u64>): u64 {
        abort ABORT_CODE
    }

    public fun vector_non_object_mutable_ref(_value: &mut vector<NonObject>): u64 {
        abort ABORT_CODE
    }

    public fun vector_object_mutable_ref(_value: &mut vector<GenericObject<Wrapped>>): u64 {
        abort ABORT_CODE
    }

    public fun receiving_by_value(_receiving: Receiving<GenericObject<Wrapped>>): u64 {
        abort ABORT_CODE
    }

    public fun receiving_mutable_ref(_receiving: &mut Receiving<GenericObject<Wrapped>>): u64 {
        abort ABORT_CODE
    }

    public fun tx_context_mutable_ref(_ctx: &mut iota::tx_context::TxContext): u64 {
        abort ABORT_CODE
    }

    public fun returns_object(): Object {
        abort ABORT_CODE
    }

    public fun returns_object_vector(): vector<GenericObject<Wrapped>> {
        abort ABORT_CODE
    }

    public fun returns_option_object(): Option<GenericObject<Wrapped>> {
        abort ABORT_CODE
    }

    public fun returns_store_only(): StoreOnly {
        abort ABORT_CODE
    }

    public fun returns_option_store_only(): Option<StoreOnly> {
        abort ABORT_CODE
    }

    public fun returns_key_store_type_param<T: key + store>(): T {
        abort ABORT_CODE
    }

    public fun returns_store_only_type_param<T: store>(): T {
        abort ABORT_CODE
    }

    public fun returns_tuple_with_object(): (u64, GenericObject<Wrapped>) {
        abort ABORT_CODE
    }

    // Satisfies the view signature constraints but is a macro, so no `#[view]`
    // suggestion is emitted.
    public macro fun qualifying_macro($x: u64): u64 {
        $x
    }

    public native fun store_only_type_param<T: store>(x: T): u64;

    public native fun native_mut_ref(x: &mut u64): u64;

    public native fun returns_mut_reference(input: &u64): &mut u64;
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
    public struct Receiving<phantom T: key> has drop {
        id: iota::object::ID,
    }

    public fun public_transfer<T: key + store>(obj: T, recipient: address) {
        transfer_impl(obj, recipient)
    }

    native fun transfer_impl<T: key>(obj: T, recipient: address);
}
