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
