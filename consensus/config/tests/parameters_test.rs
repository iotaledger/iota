// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use consensus_config::Parameters;
use insta::assert_yaml_snapshot;

#[test]
fn parameters_snapshot_matches() {
    let parameters = Parameters::default();
    assert_yaml_snapshot!("parameters", parameters)
}
