// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//# init --addresses test=0x0 --accounts A

//# publish --sender A
module test::valid_view_functions;

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

//# run test::valid_view_functions::create_object --sender A

//# run test::valid_view_functions::assert_returns_dynamic_field_reference --sender A --args object(2,1)

//# view-object 1,0

//# view-object 2,1
