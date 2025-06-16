// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

#[test_only]
module iota::prover_tests;

public struct Obj has key, store {
    id: UID,
}

// ====================================================================
// Object ownership
// ====================================================================

public fun simple_transfer(o: Obj, recipient: address) {
    iota::transfer::public_transfer(o, recipient);
}

public fun simple_share(o: Obj) {
    iota::transfer::public_share_object(o)
}

public fun simple_freeze(o: Obj) {
    iota::transfer::public_freeze_object(o)
}

public fun simple_delete(o: Obj) {
    let Obj { id } = o;
    id.delete();
}

// ====================================================================
// Dynamic fields
// ====================================================================

public fun simple_field_add(o: &mut Obj, n1: u64, v1: u8, n2: u8, v2: u64) {
    iota::dynamic_field::add(&mut o.id, n1, v1);
    iota::dynamic_field::add(&mut o.id, n2, v2);
}

public fun simple_field_remove(o: &mut Obj, n1: u64, n2: u8) {
    /* ERROR: */iota::dynamic_field::remove<u64,u8>(&mut o.id, n1);
    /* ERROR: */iota::dynamic_field::remove<u8,u64>(&mut o.id, n2);
}
