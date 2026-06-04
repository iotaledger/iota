// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

module view_functions::view_metadata;

use iota::dynamic_field;

public struct Object has key {
    id: UID,
}

public struct DynamicField has copy, drop, store {
    value: u64,
}

public struct DynamicFieldKey has copy, drop, store {}

#[view]
public fun answer(): u64 {
    42
}

#[view]
public fun returns_dynamic_field_reference(object: &Object): &DynamicField {
    dynamic_field::borrow<DynamicFieldKey, DynamicField>(&object.id, DynamicFieldKey {})
}

public entry fun create_object(ctx: &mut TxContext) {
    let mut obj = Object { id: object::new(ctx) };
    let dynamic_field = DynamicField { value: 42 };
    dynamic_field::add(&mut obj.id, DynamicFieldKey {}, dynamic_field);
    transfer::transfer(obj, ctx.sender())
}

public fun assert_returns_dynamic_field_reference(object: &Object) {
    let dynamic_field_ref = returns_dynamic_field_reference(object);
    assert!(dynamic_field_ref.value == 42, 0);
}
