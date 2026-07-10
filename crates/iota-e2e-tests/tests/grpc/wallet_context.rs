// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_macros::sim_test;
use test_cluster::TestClusterBuilder;

/// `WalletContext::get_grpc_client()` returns a working client for a
/// gRPC-enabled cluster.
#[sim_test]
async fn wallet_context_get_grpc_client() {
    let test_cluster = TestClusterBuilder::new()
        .with_fullnode_enable_grpc_api(true)
        .with_num_validators(1)
        .build()
        .await;
    test_cluster.wait_for_checkpoint(1, None).await;

    let info = test_cluster
        .wallet
        .get_grpc_client()
        .await
        .unwrap()
        .get_service_info(None)
        .await
        .unwrap();

    assert!(
        info.body().chain_id.is_some(),
        "get_service_info returned no chain id: {:?}",
        info.body()
    );
}
