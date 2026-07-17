// Copyright (c) 2021, Facebook, Inc. and its affiliates
// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_macros::sim_test;
use iota_sdk_types::ObjectId;
use test_cluster::TestClusterBuilder;

#[sim_test]
async fn test_check_randomness_state_object_exists() {
    let test_cluster = TestClusterBuilder::new()
        .with_protocol_version(1.into())
        .with_epoch_duration_ms(10000)
        .build()
        .await;

    for h in &test_cluster.all_node_handles() {
        h.with(|node| {
            node.state()
                .get_object_cache_reader()
                .get_latest_object_ref_or_tombstone(ObjectId::RANDOMNESS_STATE)
                .expect("randomness state object should exist");
        });
    }
}
