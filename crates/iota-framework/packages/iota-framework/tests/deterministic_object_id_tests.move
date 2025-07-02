// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

#[test_only]
module iota::deterministic_object_id_tests;

use iota::account_registry::{Self, EIdAlreadyExists, destroy_for_testing};
use iota::deterministic_object_id as deterministic_obj_id;

#[test]
#[expected_failure(abort_code = EIdAlreadyExists)]
fun test_id_generation_already_exists() {
    let mut context = tx_context::dummy();
    let mut account_registry = account_registry::create_for_testing(&mut context);
    let addr = deterministic_obj_id::dummy_address();
    let salt = vector[0x12, 0x34, 0xab, 0xcd];
    let id1 = deterministic_obj_id::new_precomputed(addr, salt, &mut account_registry);
    let id2 = deterministic_obj_id::new_precomputed(addr, salt, &mut account_registry);

    id1.delete();
    id2.delete();
    account_registry.destroy_for_testing();
}

#[test]
fun test_different_salt_id_generation() {
    let mut context = tx_context::dummy();
    let mut account_registry = account_registry::create_for_testing(&mut context);
    let addr = deterministic_obj_id::dummy_address();
    let salt1 = vector[0x12, 0x34, 0xab, 0xcd];
    let salt2 = vector[0x56, 0x78, 0xef, 0x90];
    let id1 = deterministic_obj_id::new_precomputed(addr, salt1, &mut account_registry);
    let id2 = deterministic_obj_id::new_precomputed(addr, salt2, &mut account_registry);

    assert!(&id1 != &id2);
    assert!(account_registry.length() == 2);
    id1.delete();
    id2.delete();
    account_registry.destroy_for_testing();
}

#[test]
fun test_different_address_id_generation() {
    let mut context = tx_context::dummy();
    let mut account_registry = account_registry::create_for_testing(&mut context);
    let addr1 = deterministic_obj_id::dummy_address();
    let addr2 = @0x2;
    let salt = vector[0x12, 0x34, 0xab, 0xcd];
    let id1 = deterministic_obj_id::new_precomputed(addr1, salt, &mut account_registry);
    let id2 = deterministic_obj_id::new_precomputed(addr2, salt, &mut account_registry);

    assert!(&id1 != &id2);
    assert!(account_registry.length() == 2);
    id1.delete();
    id2.delete();
    account_registry.destroy_for_testing();
}

#[test]
fun test_mixed_salt_address_id_generation() {
    let mut context = tx_context::dummy();
    let mut account_registry = account_registry::create_for_testing(&mut context);
    let addr1 = deterministic_obj_id::dummy_address();
    let addr2 = @0x2;
    let salt1 = vector[0x12, 0x34, 0xab, 0xcd];
    let salt2 = vector[0x56, 0x78, 0xef, 0x90];
    let id1 = deterministic_obj_id::new_precomputed(addr1, salt1, &mut account_registry);
    let id2 = deterministic_obj_id::new_precomputed(addr2, salt2, &mut account_registry);

    assert!(&id1 != &id2);
    assert!(account_registry.length() == 2);
    let id3 = deterministic_obj_id::new_precomputed(addr1, salt2, &mut account_registry);

    assert!(&id1 != &id3);
    assert!(account_registry.length() == 3);
    id1.delete();
    id2.delete();
    id3.delete();
    account_registry.destroy_for_testing();
}
