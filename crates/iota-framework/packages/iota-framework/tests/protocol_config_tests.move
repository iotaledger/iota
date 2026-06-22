// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

#[test_only]
module iota::protocol_config_tests;

use iota::protocol_config;
use iota::test_utils::assert_eq;

#[test]
fun test_is_feature_enabled_true() {
    let is_enabled = protocol_config::is_feature_enabled(b"enable_move_authentication");
    assert_eq(is_enabled, true);
}

#[test]
fun test_is_feature_enabled_false() {
    let is_enabled = protocol_config::is_feature_enabled(b"consensus_smart_ancestor_selection");
    assert_eq(is_enabled, false);
}

// --- get_attr tests ---

#[test]
// max_tx_size_bytes is a u64 set to 128 * 1024 = 131072 since protocol v1.
fun test_get_attr_u64() {
    let val: u64 = protocol_config::get_attr(b"max_tx_size_bytes");
    assert_eq(val, 131072u64);
}

#[test]
// max_arguments is a u32 set to 512 since protocol v1.
fun test_get_attr_u32() {
    let val: u32 = protocol_config::get_attr(b"max_arguments");
    assert_eq(val, 512u32);
}

#[test]
// binary_module_handles is a u16 set to 100 since protocol v1.
fun test_get_attr_u16() {
    let val: u16 = protocol_config::get_attr(b"binary_module_handles");
    assert_eq(val, 100u16);
}
