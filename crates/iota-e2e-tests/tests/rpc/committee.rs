// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_macros::sim_test;
use iota_rpc_api::client::sdk::Client;
use test_cluster::TestClusterBuilder;

#[sim_test]
async fn get_committee() {
    let test_cluster = TestClusterBuilder::new().build().await;

    let client = Client::new(test_cluster.rpc_url()).unwrap();

    let _committee = client.get_committee(0).await.unwrap();
    let _committee = client.get_current_committee().await.unwrap();
}
