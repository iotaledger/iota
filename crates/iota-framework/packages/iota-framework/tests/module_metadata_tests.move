// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

#[test_only]
module iota::module_metadata_tests;

use iota::module_metadata::{Self, ModuleMetadata};
use iota::test_utils::{Self, assert_eq};
use std::ascii;

/// An arbitrary key type used to exercise the generic dynamic-field helpers.
public struct TestKey(u64) has copy, drop, store;

// === Test helpers ===

fun new_test_metadata(): ModuleMetadata {
    module_metadata::new(object::id_from_address(@0xA), ascii::string(b"my_module"))
}

// === Generic dynamic-field bookkeeping ===

#[test]
fun new_module_metadata_is_empty() {
    let md = new_test_metadata();
    assert_eq(md.length(), 0);
    assert!(md.is_empty());
    test_utils::destroy(md);
}

#[test]
fun add_updates_size_and_membership() {
    let mut md = new_test_metadata();

    md.add(TestKey(1), 10u64);
    assert_eq(md.length(), 1);
    assert!(!md.is_empty());
    assert!(md.contains(TestKey(1)));
    assert!(module_metadata::contains_with_type<TestKey, u64>(&md, TestKey(1)));
    assert!(!md.contains(TestKey(2)));

    md.add(TestKey(2), 20u64);
    assert_eq(md.length(), 2);

    test_utils::destroy(md);
}

#[test]
fun remove_updates_size_and_returns_value() {
    let mut md = new_test_metadata();
    md.add(TestKey(1), 10u64);
    md.add(TestKey(2), 20u64);

    let removed = module_metadata::remove<TestKey, u64>(&mut md, TestKey(1));
    assert_eq(removed, 10);
    assert_eq(md.length(), 1);
    assert!(!md.contains(TestKey(1)));
    assert!(md.contains(TestKey(2)));

    test_utils::destroy(md);
}

#[test]
fun borrow_and_borrow_mut() {
    let mut md = new_test_metadata();
    md.add(TestKey(1), 10u64);

    assert_eq(*module_metadata::borrow<TestKey, u64>(&md, TestKey(1)), 10);

    let value = module_metadata::borrow_mut<TestKey, u64>(&mut md, TestKey(1));
    *value = 99;
    assert_eq(*module_metadata::borrow<TestKey, u64>(&md, TestKey(1)), 99);

    test_utils::destroy(md);
}

#[test]
fun contains_with_type_respects_value_type() {
    let mut md = new_test_metadata();
    md.add(TestKey(1), 10u64);

    assert!(module_metadata::contains_with_type<TestKey, u64>(&md, TestKey(1)));
    // The key exists, but with a different value type.
    assert!(!module_metadata::contains_with_type<TestKey, bool>(&md, TestKey(1)));

    test_utils::destroy(md);
}

// === View-function metadata ===

#[test]
fun view_functions_round_trip() {
    let mut md = new_test_metadata();
    let view_a = ascii::string(b"view_a");
    let view_b = ascii::string(b"view_b");
    md.add_view_function_metadata_v1(vector[view_a, view_b]);

    assert_eq(md.borrow_view_functions_metadata_v1().length(), 2);
    assert!(md.is_view_function_v1(&view_a));
    assert!(md.is_view_function_v1(&view_b));
    assert!(!md.is_view_function_v1(&ascii::string(b"not_a_view")));

    test_utils::destroy(md);
}

#[test]
fun view_functions_absent_returns_false() {
    let mut md = new_test_metadata();
    // The field is present but empty (the constructor always adds it, even
    // when a module declares no view functions).
    md.add_view_function_metadata_v1(vector[]);

    assert!(md.borrow_view_functions_metadata_v1().is_empty());
    assert!(!md.is_view_function_v1(&ascii::string(b"anything")));

    test_utils::destroy(md);
}

// === Abort behaviour ===

#[test]
#[expected_failure(abort_code = iota::dynamic_field::EFieldAlreadyExists)]
fun add_duplicate_key_aborts() {
    let mut md = new_test_metadata();
    md.add(TestKey(1), 10u64);
    md.add(TestKey(1), 20u64);
    test_utils::destroy(md);
}

#[test]
#[expected_failure(abort_code = iota::dynamic_field::EFieldDoesNotExist)]
fun remove_missing_key_aborts() {
    let mut md = new_test_metadata();
    let _: u64 = module_metadata::remove<TestKey, u64>(&mut md, TestKey(1));
    test_utils::destroy(md);
}

#[test]
#[expected_failure(abort_code = iota::dynamic_field::EFieldDoesNotExist)]
// `is_view_function_v1` aborts (rather than returning `false`) when
// `add_view_function_metadata_v1` was never called, since the backing field is
// missing. This pins down the constructor's contract of always adding it.
fun is_view_function_without_metadata_aborts() {
    let md = new_test_metadata();
    md.is_view_function_v1(&ascii::string(b"anything"));
    test_utils::destroy(md);
}
